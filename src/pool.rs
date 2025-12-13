// Copyright (c) 2025, vistone
// All rights reserved.

use crate::config::{Config, ConnectionType};
use crate::connection::Connection;
use crate::errors::{NetConnPoolError, Result};
use crate::ipversion::IPVersion;
use crate::mode::PoolMode;
use crate::protocol::Protocol;
use crate::stats::{Stats, StatsCollector};
use crate::udp_utils::ClearUDPReadBuffer;
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

/// Pool 连接池
pub struct Pool {
    config: Config,
    connections: Arc<RwLock<HashMap<u64, Arc<Connection>>>>,
    idle_tcp_connections: Arc<Mutex<Vec<Arc<Connection>>>>,
    idle_udp_connections: Arc<Mutex<Vec<Arc<Connection>>>>,
    closed: Arc<AtomicBool>,
    stats_collector: Option<Arc<StatsCollector>>,
    create_semaphore: Arc<std::sync::Mutex<()>>, // 简化实现，使用 Mutex 代替 Semaphore
}

impl Pool {
    /// NewPool 创建新的连接池
    pub fn NewPool(mut config: Config) -> Result<Self> {
        config.Validate()?;

        let stats_collector = if config.EnableStats {
            Some(Arc::new(StatsCollector::new()))
        } else {
            None
        };

        let pool = Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            idle_tcp_connections: Arc::new(Mutex::new(Vec::new())),
            idle_udp_connections: Arc::new(Mutex::new(Vec::new())),
            closed: Arc::new(AtomicBool::new(false)),
            stats_collector: stats_collector.clone(),
            create_semaphore: Arc::new(std::sync::Mutex::new(())),
        };

        // 预热连接池
        if pool.config.MinConnections > 0 {
            // 预热逻辑将在后续实现
        }

        Ok(pool)
    }

    /// Get 获取一个连接（自动选择IP版本）
    pub fn Get(&self) -> Result<Arc<Connection>> {
        self.GetWithTimeout(self.config.GetConnectionTimeout)
    }

    /// GetIPv4 获取一个IPv4连接
    pub fn GetIPv4(&self) -> Result<Arc<Connection>> {
        self.GetWithIPVersion(IPVersion::IPv4, self.config.GetConnectionTimeout)
    }

    /// GetIPv6 获取一个IPv6连接
    pub fn GetIPv6(&self) -> Result<Arc<Connection>> {
        self.GetWithIPVersion(IPVersion::IPv6, self.config.GetConnectionTimeout)
    }

    /// GetTCP 获取一个TCP连接
    pub fn GetTCP(&self) -> Result<Arc<Connection>> {
        self.GetWithProtocol(Protocol::TCP, self.config.GetConnectionTimeout)
    }

    /// GetUDP 获取一个UDP连接
    pub fn GetUDP(&self) -> Result<Arc<Connection>> {
        self.GetWithProtocol(Protocol::UDP, self.config.GetConnectionTimeout)
    }

    /// GetWithProtocol 获取指定协议的连接
    pub fn GetWithProtocol(&self, protocol: Protocol, timeout: Duration) -> Result<Arc<Connection>> {
        if protocol == Protocol::Unknown {
            return self.GetWithTimeout(timeout);
        }

        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if let Some(stats) = &self.stats_collector {
            stats.IncrementTotalGetRequests();
        }

        // 尝试从空闲连接池获取指定协议的连接
        let idle_chan = if protocol == Protocol::TCP {
            &self.idle_tcp_connections
        } else {
            &self.idle_udp_connections
        };

        let mut max_attempts = 10;
        let mut protocol_mismatch_count = 0;

        while max_attempts > 0 {
            max_attempts -= 1;

            if self.is_closed() {
                return Err(NetConnPoolError::PoolClosed);
            }

            // 尝试从空闲连接池获取
            let conn = {
                let mut idle = idle_chan.lock().unwrap();
                idle.pop()
            };

            if let Some(conn) = conn {
                // 检查连接是否有效
                if !self.is_connection_valid(&conn) {
                    self.close_connection(&conn);
                    continue;
                }

                // 检查协议是否匹配
                if conn.GetProtocol() != protocol {
                    // 协议不匹配，归还到正确的池并继续
                    self.Put(conn.clone())?;
                    protocol_mismatch_count += 1;
                    if protocol_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.IncrementFailedGets();
                        }
                        return Err(NetConnPoolError::NoConnectionForProtocol);
                    }
                    continue;
                }

                // 标记为使用中，并记录连接复用
                conn.MarkInUse();
                conn.IncrementReuseCount();

                // 调用借出钩子
                if let Some(on_borrow) = &self.config.OnBorrow {
                    // 需要根据连接类型调用
                    // 这里简化处理
                }

                if let Some(stats) = &self.stats_collector {
                    stats.IncrementSuccessfulGets();
                    stats.IncrementCurrentActiveConnections(1);
                    stats.IncrementCurrentIdleConnections(-1);
                    stats.IncrementTotalConnectionsReused();
                    match conn.GetProtocol() {
                        Protocol::TCP => {
                            stats.IncrementCurrentTCPIdleConnections(-1);
                        }
                        Protocol::UDP => {
                            stats.IncrementCurrentUDPIdleConnections(-1);
                        }
                        _ => {}
                    }
                }

                return Ok(conn);
            }

            // 检查是否已达到最大连接数
            if self.config.MaxConnections > 0 {
                let current = self.get_current_connections_count();
                if current >= self.config.MaxConnections {
                    // 等待可用连接或超时
                    return Err(NetConnPoolError::GetConnectionTimeout);
                }
            }

            // 创建新连接
            match self.create_connection() {
                Ok(conn) => {
                    if conn.GetProtocol() == protocol {
                        conn.MarkInUse();

                        if let Some(on_borrow) = &self.config.OnBorrow {
                            // 调用借出钩子
                        }

                        if let Some(stats) = &self.stats_collector {
                            stats.IncrementSuccessfulGets();
                            stats.IncrementCurrentActiveConnections(1);
                        }

                        return Ok(conn);
                    }

                    // 协议不匹配，归还连接
                    protocol_mismatch_count += 1;
                    self.Put(conn)?;

                    if protocol_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.IncrementFailedGets();
                        }
                        return Err(NetConnPoolError::NoConnectionForProtocol);
                    }
                }
                Err(e) => {
                    if let Some(stats) = &self.stats_collector {
                        stats.IncrementFailedGets();
                        stats.IncrementConnectionErrors();
                    }
                    return Err(e);
                }
            }
        }

        if let Some(stats) = &self.stats_collector {
            stats.IncrementFailedGets();
        }
        Err(NetConnPoolError::NoConnectionForProtocol)
    }

    /// GetWithIPVersion 获取指定IP版本的连接
    pub fn GetWithIPVersion(&self, ip_version: IPVersion, timeout: Duration) -> Result<Arc<Connection>> {
        if ip_version == IPVersion::Unknown {
            return self.GetWithTimeout(timeout);
        }

        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if let Some(stats) = &self.stats_collector {
            stats.IncrementTotalGetRequests();
        }

        let mut max_attempts = 10;
        let mut ip_version_mismatch_count = 0;

        while max_attempts > 0 {
            max_attempts -= 1;

            if self.is_closed() {
                return Err(NetConnPoolError::PoolClosed);
            }

            // 尝试从两个空闲池中获取
            let conn = {
                let mut tcp_idle = self.idle_tcp_connections.lock().unwrap();
                let mut udp_idle = self.idle_udp_connections.lock().unwrap();
                tcp_idle.pop().or_else(|| udp_idle.pop())
            };

            if let Some(conn) = conn {
                // 检查IP版本是否匹配
                if conn.GetIPVersion() != ip_version {
                    self.Put(conn.clone())?;
                    ip_version_mismatch_count += 1;
                    if ip_version_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.IncrementFailedGets();
                        }
                        return Err(NetConnPoolError::NoConnectionForIPVersion);
                    }
                    continue;
                }

                if !self.is_connection_valid(&conn) {
                    self.close_connection(&conn);
                    continue;
                }

                conn.MarkInUse();
                conn.IncrementReuseCount();

                if let Some(on_borrow) = &self.config.OnBorrow {
                    // 调用借出钩子
                }

                if let Some(stats) = &self.stats_collector {
                    stats.IncrementSuccessfulGets();
                    stats.IncrementCurrentActiveConnections(1);
                    stats.IncrementCurrentIdleConnections(-1);
                    stats.IncrementTotalConnectionsReused();
                    match conn.GetIPVersion() {
                        IPVersion::IPv4 => {
                            stats.IncrementCurrentIPv4IdleConnections(-1);
                        }
                        IPVersion::IPv6 => {
                            stats.IncrementCurrentIPv6IdleConnections(-1);
                        }
                        _ => {}
                    }
                    match conn.GetProtocol() {
                        Protocol::TCP => {
                            stats.IncrementCurrentTCPIdleConnections(-1);
                        }
                        Protocol::UDP => {
                            stats.IncrementCurrentUDPIdleConnections(-1);
                        }
                        _ => {}
                    }
                }

                return Ok(conn);
            }

            // 尝试创建
            if self.config.MaxConnections > 0 {
                let current = self.get_current_connections_count();
                if current >= self.config.MaxConnections {
                    return Err(NetConnPoolError::GetConnectionTimeout);
                }
            }

            match self.create_connection() {
                Ok(conn) => {
                    if conn.GetIPVersion() == ip_version {
                        conn.MarkInUse();
                        if let Some(on_borrow) = &self.config.OnBorrow {
                            // 调用借出钩子
                        }
                        if let Some(stats) = &self.stats_collector {
                            stats.IncrementSuccessfulGets();
                            stats.IncrementCurrentActiveConnections(1);
                        }
                        return Ok(conn);
                    }

                    // IP版本不匹配，归还连接
                    ip_version_mismatch_count += 1;
                    self.Put(conn)?;

                    if ip_version_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.IncrementFailedGets();
                        }
                        return Err(NetConnPoolError::NoConnectionForIPVersion);
                    }
                }
                Err(e) => {
                    if let Some(stats) = &self.stats_collector {
                        stats.IncrementFailedGets();
                        stats.IncrementConnectionErrors();
                    }
                    return Err(e);
                }
            }
        }

        if let Some(stats) = &self.stats_collector {
            stats.IncrementFailedGets();
        }
        Err(NetConnPoolError::NoConnectionForIPVersion)
    }

    /// GetWithTimeout 获取一个连接（带超时，自动选择IP版本）
    pub fn GetWithTimeout(&self, timeout: Duration) -> Result<Arc<Connection>> {
        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if let Some(stats) = &self.stats_collector {
            stats.IncrementTotalGetRequests();
        }

        loop {
            if self.is_closed() {
                return Err(NetConnPoolError::PoolClosed);
            }

            // 尝试从任意空闲池获取
            let conn = {
                let mut tcp_idle = self.idle_tcp_connections.lock().unwrap();
                let mut udp_idle = self.idle_udp_connections.lock().unwrap();
                tcp_idle.pop().or_else(|| udp_idle.pop())
            };

            if let Some(conn) = conn {
                if !self.is_connection_valid(&conn) {
                    self.close_connection(&conn);
                    continue;
                }

                conn.MarkInUse();
                conn.IncrementReuseCount();

                if let Some(on_borrow) = &self.config.OnBorrow {
                    // 调用借出钩子
                }

                if let Some(stats) = &self.stats_collector {
                    stats.IncrementSuccessfulGets();
                    stats.IncrementCurrentActiveConnections(1);
                    stats.IncrementCurrentIdleConnections(-1);
                    stats.IncrementTotalConnectionsReused();
                    match conn.GetIPVersion() {
                        IPVersion::IPv4 => {
                            stats.IncrementCurrentIPv4IdleConnections(-1);
                        }
                        IPVersion::IPv6 => {
                            stats.IncrementCurrentIPv6IdleConnections(-1);
                        }
                        _ => {}
                    }
                    match conn.GetProtocol() {
                        Protocol::TCP => {
                            stats.IncrementCurrentTCPIdleConnections(-1);
                        }
                        Protocol::UDP => {
                            stats.IncrementCurrentUDPIdleConnections(-1);
                        }
                        _ => {}
                    }
                }

                return Ok(conn);
            }

            if self.config.MaxConnections > 0 {
                let current = self.get_current_connections_count();
                if current >= self.config.MaxConnections {
                    return Err(NetConnPoolError::GetConnectionTimeout);
                }
            }

            match self.create_connection() {
                Ok(conn) => {
                    conn.MarkInUse();
                    if let Some(on_borrow) = &self.config.OnBorrow {
                        // 调用借出钩子
                    }
                    if let Some(stats) = &self.stats_collector {
                        stats.IncrementSuccessfulGets();
                        stats.IncrementCurrentActiveConnections(1);
                    }
                    return Ok(conn);
                }
                Err(e) => {
                    if let Some(stats) = &self.stats_collector {
                        stats.IncrementFailedGets();
                        stats.IncrementConnectionErrors();
                    }
                    if e == NetConnPoolError::MaxConnectionsReached {
                        return Err(NetConnPoolError::GetConnectionTimeout);
                    }
                    return Err(e);
                }
            }
        }
    }

    /// Put 归还连接
    pub fn Put(&self, conn: Arc<Connection>) -> Result<()> {
        if self.is_closed() {
            return self.close_connection(&conn);
        }

        if !self.is_connection_valid(&conn) {
            return self.close_connection(&conn);
        }

        // 调用归还钩子
        if let Some(on_return) = &self.config.OnReturn {
            // 调用归还钩子
        }

        // UDP缓冲区清理
        if self.config.ClearUDPBufferOnReturn && conn.GetProtocol() == Protocol::UDP {
            if let Some(udp_socket) = conn.GetUdpConn() {
                let timeout = if self.config.UDPBufferClearTimeout.is_zero() {
                    Duration::from_millis(100)
                } else {
                    self.config.UDPBufferClearTimeout
                };
                let _ = ClearUDPReadBuffer(udp_socket, timeout, self.config.MaxBufferClearPackets);
            }
        }

        conn.MarkIdle();

        if let Some(stats) = &self.stats_collector {
            stats.IncrementCurrentActiveConnections(-1);
        }

        // 确定归还到哪个通道
        let idle_chan = if conn.GetProtocol() == Protocol::TCP {
            &self.idle_tcp_connections
        } else {
            &self.idle_udp_connections
        };

        let mut idle = idle_chan.lock().unwrap();
        if idle.len() < self.config.MaxIdleConnections {
            idle.push(conn.clone());
            drop(idle);

            if let Some(stats) = &self.stats_collector {
                stats.IncrementCurrentIdleConnections(1);
                match conn.GetIPVersion() {
                    IPVersion::IPv4 => {
                        stats.IncrementCurrentIPv4IdleConnections(1);
                    }
                    IPVersion::IPv6 => {
                        stats.IncrementCurrentIPv6IdleConnections(1);
                    }
                    _ => {}
                }
                match conn.GetProtocol() {
                    Protocol::TCP => {
                        stats.IncrementCurrentTCPIdleConnections(1);
                    }
                    Protocol::UDP => {
                        stats.IncrementCurrentUDPIdleConnections(1);
                    }
                    _ => {}
                }
            }

            Ok(())
        } else {
            drop(idle);
            self.close_connection(&conn)
        }
    }

    /// Close 关闭连接池
    pub fn Close(&self) -> Result<()> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(()); // 已经关闭
        }

        // 获取所有连接
        let conns: Vec<Arc<Connection>> = {
            let connections = self.connections.read().unwrap();
            connections.values().cloned().collect()
        };

        // 关闭所有连接
        for conn in conns {
            let _ = conn.Close();
            if let Some(stats) = &self.stats_collector {
                stats.IncrementTotalConnectionsClosed();
            }
        }

        // 清空连接映射
        {
            let mut connections = self.connections.write().unwrap();
            connections.clear();
        }

        // 清空空闲连接池
        {
            let mut tcp_idle = self.idle_tcp_connections.lock().unwrap();
            tcp_idle.clear();
        }
        {
            let mut udp_idle = self.idle_udp_connections.lock().unwrap();
            udp_idle.clear();
        }

        Ok(())
    }

    /// Stats 获取统计信息
    pub fn Stats(&self) -> Stats {
        if let Some(stats) = &self.stats_collector {
            stats.GetStats()
        } else {
            Stats::default()
        }
    }

    // 私有辅助方法

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn is_connection_valid(&self, conn: &Connection) -> bool {
        // 检查连接是否健康
        if !conn.GetHealthStatus() {
            return false;
        }
        // 检查连接是否过期
        if conn.IsExpired(self.config.MaxLifetime) {
            return false;
        }
        true
    }

    fn close_connection(&self, conn: &Arc<Connection>) -> Result<()> {
        let id = conn.ID;
        let _ = conn.Close();

        // 从连接映射中移除
        {
            let mut connections = self.connections.write().unwrap();
            connections.remove(&id);
        }

        // 从空闲池中移除
        {
            let mut tcp_idle = self.idle_tcp_connections.lock().unwrap();
            tcp_idle.retain(|c| c.ID != id);
        }
        {
            let mut udp_idle = self.idle_udp_connections.lock().unwrap();
            udp_idle.retain(|c| c.ID != id);
        }

        if let Some(stats) = &self.stats_collector {
            stats.IncrementTotalConnectionsClosed();
        }

        Ok(())
    }

    fn get_current_connections_count(&self) -> usize {
        let connections = self.connections.read().unwrap();
        connections.len()
    }

    fn create_connection(&self) -> Result<Arc<Connection>> {
        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if self.config.MaxConnections > 0 {
            let current = self.get_current_connections_count();
            if current >= self.config.MaxConnections {
                return Err(NetConnPoolError::MaxConnectionsReached);
            }
        }

        let conn_type = match self.config.Mode {
            PoolMode::Client => {
                if let Some(dialer) = &self.config.Dialer {
                    dialer().map_err(|e| NetConnPoolError::IoError(e.to_string()))?
                } else {
                    return Err(NetConnPoolError::InvalidConfig);
                }
            }
            PoolMode::Server => {
                if let Some(listener) = &self.config.Listener {
                    let acceptor = self.config.Acceptor.as_ref().unwrap();
                    ConnectionType::Tcp(acceptor(listener).map_err(|e| NetConnPoolError::IoError(e.to_string()))?)
                } else {
                    return Err(NetConnPoolError::InvalidConfig);
                }
            }
        };

        // 调用创建钩子
        if let Some(on_created) = &self.config.OnCreated {
            on_created(&conn_type).map_err(|e| NetConnPoolError::IoError(e.to_string()))?;
        }

        let conn = match conn_type {
            ConnectionType::Tcp(stream) => {
                Arc::new(Connection::NewConnectionFromTcp(stream, None))
            }
            ConnectionType::Udp(socket) => {
                Arc::new(Connection::NewConnectionFromUdp(socket, None))
            }
        };

        // 添加到连接映射
        {
            let mut connections = self.connections.write().unwrap();
            connections.insert(conn.ID, conn.clone());
        }

        if let Some(stats) = &self.stats_collector {
            stats.IncrementTotalConnectionsCreated();
            match conn.GetIPVersion() {
                IPVersion::IPv4 => {
                    stats.IncrementCurrentIPv4Connections(1);
                }
                IPVersion::IPv6 => {
                    stats.IncrementCurrentIPv6Connections(1);
                }
                _ => {}
            }
            match conn.GetProtocol() {
                Protocol::TCP => {
                    stats.IncrementCurrentTCPConnections(1);
                }
                Protocol::UDP => {
                    stats.IncrementCurrentUDPConnections(1);
                }
                _ => {}
            }
        }

        Ok(conn)
    }
}
