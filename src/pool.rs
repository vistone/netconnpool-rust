// Copyright (c) 2025, vistone
// All rights reserved.

use crate::config::{Config, ConnectionType};
use crate::connection::Connection;
use crate::errors::{NetConnPoolError, Result};
use crate::ipversion::IPVersion;
use crate::mode::PoolMode;
use crate::protocol::Protocol;
use crate::stats::{Stats, StatsCollector};
use crate::udp_utils::clear_udp_read_buffer;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
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
    pub fn new(mut config: Config) -> Result<Self> {
        config.validate()?;

        let stats_collector = if config.enable_stats {
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
        if pool.config.min_connections > 0 {
            // 预热逻辑将在后续实现
        }

        Ok(pool)
    }

    /// Get 获取一个连接（自动选择IP版本）
    pub fn get(&self) -> Result<Arc<Connection>> {
        self.get_with_timeout(self.config.get_connection_timeout)
    }

    /// GetIPv4 获取一个IPv4连接
    pub fn get_ipv4(&self) -> Result<Arc<Connection>> {
        self.get_with_ip_version(IPVersion::IPv4, self.config.get_connection_timeout)
    }

    /// GetIPv6 获取一个IPv6连接
    pub fn get_ipv6(&self) -> Result<Arc<Connection>> {
        self.get_with_ip_version(IPVersion::IPv6, self.config.get_connection_timeout)
    }

    /// GetTCP 获取一个TCP连接
    pub fn get_tcp(&self) -> Result<Arc<Connection>> {
        self.get_with_protocol(Protocol::TCP, self.config.get_connection_timeout)
    }

    /// GetUDP 获取一个UDP连接
    pub fn get_udp(&self) -> Result<Arc<Connection>> {
        self.get_with_protocol(Protocol::UDP, self.config.get_connection_timeout)
    }

    /// GetWithProtocol 获取指定协议的连接
    pub fn get_with_protocol(&self, protocol: Protocol, timeout: Duration) -> Result<Arc<Connection>> {
        if protocol == Protocol::Unknown {
            return self.get_with_timeout(timeout);
        }

        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_get_requests();
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
                    let _ = self.close_connection(&conn);
                    continue;
                }

                // 检查协议是否匹配
                if conn.get_protocol() != protocol {
                    // 协议不匹配，归还到正确的池并继续
                    self.put(conn.clone())?;
                    protocol_mismatch_count += 1;
                    if protocol_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_failed_gets();
                        }
                        return Err(NetConnPoolError::NoConnectionForProtocol);
                    }
                    continue;
                }

                // 标记为使用中，并记录连接复用
                conn.mark_in_use();
                conn.increment_reuse_count();

                // 调用借出钩子
                if let Some(_on_borrow) = &self.config.on_borrow {
                    // 需要根据连接类型调用
                    // 这里简化处理
                }

                if let Some(stats) = &self.stats_collector {
                    stats.increment_successful_gets();
                    stats.increment_current_active_connections(1);
                    stats.increment_current_idle_connections(-1);
                    stats.increment_total_connections_reused();
                    match conn.get_protocol() {
                        Protocol::TCP => {
                            stats.increment_current_tcp_idle_connections(-1);
                        }
                        Protocol::UDP => {
                            stats.increment_current_udp_idle_connections(-1);
                        }
                        _ => {}
                    }
                }

                return Ok(conn);
            }

            // 检查是否已达到最大连接数
            if self.config.max_connections > 0 {
                let current = self.get_current_connections_count();
                if current >= self.config.max_connections {
                    // 等待可用连接或超时
                    return Err(NetConnPoolError::GetConnectionTimeout);
                }
            }

            // 创建新连接
            match self.create_connection() {
                Ok(conn) => {
                    if conn.get_protocol() == protocol {
                        conn.mark_in_use();

                        if let Some(_on_borrow) = &self.config.on_borrow {
                            // 调用借出钩子
                        }

                        if let Some(stats) = &self.stats_collector {
                            stats.increment_successful_gets();
                            stats.increment_current_active_connections(1);
                        }

                        return Ok(conn);
                    }

                    // 协议不匹配，归还连接
                    protocol_mismatch_count += 1;
                    self.put(conn)?;

                    if protocol_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_failed_gets();
                        }
                        return Err(NetConnPoolError::NoConnectionForProtocol);
                    }
                }
                Err(e) => {
                    if let Some(stats) = &self.stats_collector {
                        stats.increment_failed_gets();
                        stats.increment_connection_errors();
                    }
                    return Err(e);
                }
            }
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_failed_gets();
        }
        Err(NetConnPoolError::NoConnectionForProtocol)
    }

    /// GetWithIPVersion 获取指定IP版本的连接
    pub fn get_with_ip_version(&self, ip_version: IPVersion, timeout: Duration) -> Result<Arc<Connection>> {
        if ip_version == IPVersion::Unknown {
            return self.get_with_timeout(timeout);
        }

        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_get_requests();
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
                if conn.get_ip_version() != ip_version {
                    self.put(conn.clone())?;
                    ip_version_mismatch_count += 1;
                    if ip_version_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_failed_gets();
                        }
                        return Err(NetConnPoolError::NoConnectionForIPVersion);
                    }
                    continue;
                }

                if !self.is_connection_valid(&conn) {
                    let _ = self.close_connection(&conn);
                    continue;
                }

                conn.mark_in_use();
                conn.increment_reuse_count();

                if let Some(_on_borrow) = &self.config.on_borrow {
                    // 调用借出钩子
                }

                if let Some(stats) = &self.stats_collector {
                    stats.increment_successful_gets();
                    stats.increment_current_active_connections(1);
                    stats.increment_current_idle_connections(-1);
                    stats.increment_total_connections_reused();
                    match conn.get_ip_version() {
                        IPVersion::IPv4 => {
                            stats.increment_current_ipv4_idle_connections(-1);
                        }
                        IPVersion::IPv6 => {
                            stats.increment_current_ipv6_idle_connections(-1);
                        }
                        _ => {}
                    }
                    match conn.get_protocol() {
                        Protocol::TCP => {
                            stats.increment_current_tcp_idle_connections(-1);
                        }
                        Protocol::UDP => {
                            stats.increment_current_udp_idle_connections(-1);
                        }
                        _ => {}
                    }
                }

                return Ok(conn);
            }

            // 尝试创建
            if self.config.max_connections > 0 {
                let current = self.get_current_connections_count();
                if current >= self.config.max_connections {
                    return Err(NetConnPoolError::GetConnectionTimeout);
                }
            }

            match self.create_connection() {
                Ok(conn) => {
                    if conn.get_ip_version() == ip_version {
                        conn.mark_in_use();
                        if let Some(_on_borrow) = &self.config.on_borrow {
                            // 调用借出钩子
                        }
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_successful_gets();
                            stats.increment_current_active_connections(1);
                        }
                        return Ok(conn);
                    }

                    // IP版本不匹配，归还连接
                    ip_version_mismatch_count += 1;
                    self.put(conn)?;

                    if ip_version_mismatch_count >= 3 {
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_failed_gets();
                        }
                        return Err(NetConnPoolError::NoConnectionForIPVersion);
                    }
                }
                Err(e) => {
                    if let Some(stats) = &self.stats_collector {
                        stats.increment_failed_gets();
                        stats.increment_connection_errors();
                    }
                    return Err(e);
                }
            }
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_failed_gets();
        }
        Err(NetConnPoolError::NoConnectionForIPVersion)
    }

    /// GetWithTimeout 获取一个连接（带超时，自动选择IP版本）
    pub fn get_with_timeout(&self, _timeout: Duration) -> Result<Arc<Connection>> {
        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_get_requests();
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
                    let _ = self.close_connection(&conn);
                    continue;
                }

                conn.mark_in_use();
                conn.increment_reuse_count();

                if let Some(_on_borrow) = &self.config.on_borrow {
                    // 调用借出钩子
                }

                if let Some(stats) = &self.stats_collector {
                    stats.increment_successful_gets();
                    stats.increment_current_active_connections(1);
                    stats.increment_current_idle_connections(-1);
                    stats.increment_total_connections_reused();
                    match conn.get_ip_version() {
                        IPVersion::IPv4 => {
                            stats.increment_current_ipv4_idle_connections(-1);
                        }
                        IPVersion::IPv6 => {
                            stats.increment_current_ipv6_idle_connections(-1);
                        }
                        _ => {}
                    }
                    match conn.get_protocol() {
                        Protocol::TCP => {
                            stats.increment_current_tcp_idle_connections(-1);
                        }
                        Protocol::UDP => {
                            stats.increment_current_udp_idle_connections(-1);
                        }
                        _ => {}
                    }
                }

                return Ok(conn);
            }

            if self.config.max_connections > 0 {
                let current = self.get_current_connections_count();
                if current >= self.config.max_connections {
                    return Err(NetConnPoolError::GetConnectionTimeout);
                }
            }

            match self.create_connection() {
                Ok(conn) => {
                    conn.mark_in_use();
                    if let Some(_on_borrow) = &self.config.on_borrow {
                        // 调用借出钩子
                    }
                    if let Some(stats) = &self.stats_collector {
                        stats.increment_successful_gets();
                        stats.increment_current_active_connections(1);
                    }
                    return Ok(conn);
                }
                Err(e) => {
                    if let Some(stats) = &self.stats_collector {
                        stats.increment_failed_gets();
                        stats.increment_connection_errors();
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
    pub fn put(&self, conn: Arc<Connection>) -> Result<()> {
        if self.is_closed() {
            return self.close_connection(&conn);
        }

        if !self.is_connection_valid(&conn) {
            return self.close_connection(&conn);
        }

        // 调用归还钩子
        if let Some(_on_return) = &self.config.on_return {
            // 调用归还钩子
        }

        // UDP缓冲区清理
        if self.config.clear_udp_buffer_on_return && conn.get_protocol() == Protocol::UDP {
            if let Some(udp_socket) = conn.udp_conn() {
                let timeout = if self.config.udp_buffer_clear_timeout.is_zero() {
                    Duration::from_millis(100)
                } else {
                    self.config.udp_buffer_clear_timeout
                };
                let _ = clear_udp_read_buffer(udp_socket, timeout, self.config.max_buffer_clear_packets);
            }
        }

        conn.mark_idle();

        if let Some(stats) = &self.stats_collector {
            stats.increment_current_active_connections(-1);
        }

        // 确定归还到哪个通道
        let idle_chan = if conn.get_protocol() == Protocol::TCP {
            &self.idle_tcp_connections
        } else {
            &self.idle_udp_connections
        };

        let mut idle = idle_chan.lock().unwrap();
        if idle.len() < self.config.max_idle_connections {
            idle.push(conn.clone());
            drop(idle);

            if let Some(stats) = &self.stats_collector {
                stats.increment_current_idle_connections(1);
                match conn.get_ip_version() {
                    IPVersion::IPv4 => {
                        stats.increment_current_ipv4_idle_connections(1);
                    }
                    IPVersion::IPv6 => {
                        stats.increment_current_ipv6_idle_connections(1);
                    }
                    _ => {}
                }
                match conn.get_protocol() {
                    Protocol::TCP => {
                        stats.increment_current_tcp_idle_connections(1);
                    }
                    Protocol::UDP => {
                        stats.increment_current_udp_idle_connections(1);
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
    pub fn close(&self) -> Result<()> {
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
            let _ = conn.close();
            if let Some(stats) = &self.stats_collector {
                stats.increment_total_connections_closed();
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
    pub fn stats(&self) -> Stats {
        if let Some(stats) = &self.stats_collector {
            stats.get_stats()
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
        if !conn.health_status() {
            return false;
        }
        // 检查连接是否过期
        if conn.is_expired(self.config.max_lifetime) {
            return false;
        }
        true
    }

    fn close_connection(&self, conn: &Arc<Connection>) -> Result<()> {
        let id = conn.id;
        let _ = conn.close();

        // 从连接映射中移除
        {
            let mut connections = self.connections.write().unwrap();
            connections.remove(&id);
        }

        // 从空闲池中移除
        {
            let mut tcp_idle = self.idle_tcp_connections.lock().unwrap();
            tcp_idle.retain(|c| c.id != id);
        }
        {
            let mut udp_idle = self.idle_udp_connections.lock().unwrap();
            udp_idle.retain(|c| c.id != id);
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_connections_closed();
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

        if self.config.max_connections > 0 {
            let current = self.get_current_connections_count();
            if current >= self.config.max_connections {
                return Err(NetConnPoolError::MaxConnectionsReached);
            }
        }

        let conn_type = match self.config.mode {
            PoolMode::Client => {
                if let Some(dialer) = &self.config.dialer {
                    dialer().map_err(|e| NetConnPoolError::IoError(e.to_string()))?
                } else {
                    return Err(NetConnPoolError::InvalidConfig);
                }
            }
            PoolMode::Server => {
                if let Some(listener) = &self.config.listener {
                    let acceptor = self.config.acceptor.as_ref().unwrap();
                    ConnectionType::Tcp(acceptor(listener).map_err(|e| NetConnPoolError::IoError(e.to_string()))?)
                } else {
                    return Err(NetConnPoolError::InvalidConfig);
                }
            }
        };

        // 调用创建钩子
        if let Some(on_created) = &self.config.on_created {
            on_created(&conn_type).map_err(|e| NetConnPoolError::IoError(e.to_string()))?;
        }

        let conn = match conn_type {
            ConnectionType::Tcp(stream) => {
                Arc::new(Connection::new_from_tcp(stream, None))
            }
            ConnectionType::Udp(socket) => {
                Arc::new(Connection::new_from_udp(socket, None))
            }
        };

        // 添加到连接映射
        {
            let mut connections = self.connections.write().unwrap();
            connections.insert(conn.id, conn.clone());
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_connections_created();
            match conn.get_ip_version() {
                IPVersion::IPv4 => {
                    stats.increment_current_ipv4_connections(1);
                }
                IPVersion::IPv6 => {
                    stats.increment_current_ipv6_connections(1);
                }
                _ => {}
            }
            match conn.get_protocol() {
                Protocol::TCP => {
                    stats.increment_current_tcp_connections(1);
                }
                Protocol::UDP => {
                    stats.increment_current_udp_connections(1);
                }
                _ => {}
            }
        }

        Ok(conn)
    }
}
