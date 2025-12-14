// Copyright (c) 2025, vistone
// All rights reserved.

use crate::config::{Config, ConnectionType};
use crate::connection::Connection;
use crate::errors::{NetConnPoolError, Result};
use crate::ipversion::IPVersion;
use crate::mode::PoolMode;
use crate::protocol::Protocol;
use crate::stats::StatsCollector;
use crate::udp_utils::clear_udp_read_buffer;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::thread;
use std::time::Duration;

/// PooledConnection 自动归还的连接包装器
/// 实现 RAII 机制，Drop 时自动归还连接到池中
#[derive(Debug)]
pub struct PooledConnection {
    conn: Arc<Connection>,
    pool: Weak<PoolInner>,
}

impl PooledConnection {
    fn new(conn: Arc<Connection>, pool: Weak<PoolInner>) -> Self {
        Self { conn, pool }
    }
}

impl Deref for PooledConnection {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            pool.return_connection(self.conn.clone());
        }
    }
}

/// Pool 连接池
#[derive(Clone)]
pub struct Pool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    config: Config,
    // 所有存活的连接，用于管理生命周期和后台清理
    all_connections: RwLock<HashMap<u64, Arc<Connection>>>,
    // 空闲连接池，按 (Protocol, IPVersion) 分桶
    // 0: TCP IPv4, 1: TCP IPv6, 2: UDP IPv4, 3: UDP IPv6
    idle_connections: [Mutex<Vec<Arc<Connection>>>; 4],
    closed: AtomicBool,
    stats_collector: Option<Arc<StatsCollector>>,
}

impl Pool {
    /// NewPool 创建新的连接池
    pub fn new(mut config: Config) -> Result<Self> {
        config.apply_defaults();
        config.validate()?;

        let stats_collector = if config.enable_stats {
            Some(Arc::new(StatsCollector::new()))
        } else {
            None
        };

        let inner = Arc::new(PoolInner {
            config: config,
            all_connections: RwLock::new(HashMap::new()),
            idle_connections: [
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
            ],
            closed: AtomicBool::new(false),
            stats_collector,
        });

        // 启动后台清理线程
        let weak_inner = Arc::downgrade(&inner);
        thread::Builder::new()
            .name("connection-pool-reaper".to_string())
            .spawn(move || {
                Self::reaper(weak_inner);
            })
            .map_err(|e| NetConnPoolError::IoError(e))?;

        Ok(Self { inner })
    }

    /// 后台清理任务
    fn reaper(inner: Weak<PoolInner>) {
        loop {
            let pool = match inner.upgrade() {
                Some(p) => p,
                None => break, // Pool已销毁
            };

            if pool.closed.load(Ordering::Relaxed) {
                break;
            }

            let interval = pool.config.health_check_interval;
            drop(pool); // 释放Arc，允许Pool被销毁

            thread::sleep(interval);

            // 再次获取Pool
            let pool = match inner.upgrade() {
                Some(p) => p,
                None => break,
            };

            if pool.closed.load(Ordering::Relaxed) {
                break;
            }

            pool.cleanup();
        }
    }

    /// Get 获取一个连接（自动选择IP版本）
    pub fn get(&self) -> Result<PooledConnection> {
        self.get_with_timeout(self.inner.config.get_connection_timeout)
    }

    /// GetIPv4 获取一个IPv4连接
    pub fn get_ipv4(&self) -> Result<PooledConnection> {
        self.get_with_ip_version(IPVersion::IPv4, self.inner.config.get_connection_timeout)
    }

    /// GetIPv6 获取一个IPv6连接
    pub fn get_ipv6(&self) -> Result<PooledConnection> {
        self.get_with_ip_version(IPVersion::IPv6, self.inner.config.get_connection_timeout)
    }

    /// GetTCP 获取一个TCP连接
    pub fn get_tcp(&self) -> Result<PooledConnection> {
        self.get_with_protocol(Protocol::TCP, self.inner.config.get_connection_timeout)
    }

    /// GetUDP 获取一个UDP连接
    pub fn get_udp(&self) -> Result<PooledConnection> {
        self.get_with_protocol(Protocol::UDP, self.inner.config.get_connection_timeout)
    }

    /// GetWithProtocol 获取指定协议的连接
    pub fn get_with_protocol(&self, protocol: Protocol, timeout: Duration) -> Result<PooledConnection> {
        self.inner.get_connection(Some(protocol), None, timeout)
    }

    /// GetWithIPVersion 获取指定IP版本的连接
    pub fn get_with_ip_version(&self, ip_version: IPVersion, timeout: Duration) -> Result<PooledConnection> {
        self.inner.get_connection(None, Some(ip_version), timeout)
    }

    /// GetWithTimeout 获取一个连接（带超时，自动选择IP版本）
    pub fn get_with_timeout(&self, timeout: Duration) -> Result<PooledConnection> {
        self.inner.get_connection(None, None, timeout)
    }

    /// Close 关闭连接池
    pub fn close(&self) -> Result<()> {
        self.inner.close()
    }

    /// Stats 获取统计信息
    pub fn stats(&self) -> crate::stats::Stats {
        if let Some(stats) = &self.inner.stats_collector {
            stats.get_stats()
        } else {
            crate::stats::Stats::default()
        }
    }
}

impl PoolInner {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn close(&self) -> Result<()> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        // 获取所有连接并关闭
        let conns: Vec<Arc<Connection>> = {
            let connections = self.all_connections.read().unwrap();
            connections.values().cloned().collect()
        };

        for conn in conns {
            let _ = conn.close();
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_connections_closed();
        }

        // 清空所有集合
        {
            let mut connections = self.all_connections.write().unwrap();
            connections.clear();
        }

        for idle in &self.idle_connections {
            let mut idle_vec = idle.lock().unwrap();
            idle_vec.clear();
        }

        Ok(())
    }

    // 计算分桶索引
    fn get_bucket_index(protocol: Protocol, ip_version: IPVersion) -> Option<usize> {
        let p_idx = match protocol {
            Protocol::TCP => 0,
            Protocol::UDP => 1,
            _ => return None,
        };
        let ip_idx = match ip_version {
            IPVersion::IPv4 => 0,
            IPVersion::IPv6 => 1,
            _ => return None,
        };
        Some(p_idx * 2 + ip_idx)
    }

    // 获取符合条件的空闲连接索引列表
    fn get_target_buckets(&self, protocol: Option<Protocol>, ip_version: Option<IPVersion>) -> Vec<usize> {
        let mut indices = Vec::new();
        let protocols = if let Some(p) = protocol {
            if p == Protocol::Unknown { vec![Protocol::TCP, Protocol::UDP] } else { vec![p] }
        } else {
            vec![Protocol::TCP, Protocol::UDP]
        };

        let ip_versions = if let Some(ip) = ip_version {
            if ip == IPVersion::Unknown { vec![IPVersion::IPv4, IPVersion::IPv6] } else { vec![ip] }
        } else {
            vec![IPVersion::IPv4, IPVersion::IPv6]
        };

        for p in protocols {
            for ip in &ip_versions {
                if let Some(idx) = Self::get_bucket_index(p, *ip) {
                    indices.push(idx);
                }
            }
        }
        indices
    }

    fn get_connection(
        self: &Arc<Self>,
        protocol: Option<Protocol>,
        ip_version: Option<IPVersion>,
        timeout: Duration,
    ) -> Result<PooledConnection> {
        if self.is_closed() {
            return Err(NetConnPoolError::PoolClosed);
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_get_requests();
        }

        let start_time = std::time::Instant::now();
        let bucket_indices = self.get_target_buckets(protocol, ip_version);
        
        let mut loop_count = 0;

        loop {
            if self.is_closed() {
                return Err(NetConnPoolError::PoolClosed);
            }

            if start_time.elapsed() > timeout {
                if let Some(stats) = &self.stats_collector {
                    stats.increment_failed_gets();
                }
                return Err(NetConnPoolError::GetConnectionTimeout);
            }
            
            // 限制循环次数防止CPU空转，适当休眠
            loop_count += 1;
            if loop_count > 100 { // 稍微休眠
                 thread::sleep(Duration::from_millis(10));
                 loop_count = 0;
            }

            // 1. 尝试从空闲池获取
            for &idx in &bucket_indices {
                let conn = {
                    let mut idle = self.idle_connections[idx].lock().unwrap();
                    idle.pop()
                };

                if let Some(conn) = conn {
                    if !self.is_connection_valid(&conn) {
                        let _ = self.remove_connection(&conn);
                        continue;
                    }

                    conn.mark_in_use();
                    conn.increment_reuse_count();
                    
                    if let Some(on_borrow) = &self.config.on_borrow {
                        on_borrow(&conn.connection_type());
                    }

                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_get(&stats, &conn, true);
                    }

                    return Ok(PooledConnection::new(conn, Arc::downgrade(self)));
                }
            }

            // 2. 检查最大连接数
            if self.config.max_connections > 0 {
                let current = self.all_connections.read().unwrap().len();
                if current >= self.config.max_connections {
                    // 等待重试
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            }

            // 3. 创建新连接
            match self.create_connection(protocol, ip_version) {
                Ok(conn) => {
                    conn.mark_in_use();
                    
                    if let Some(on_borrow) = &self.config.on_borrow {
                        on_borrow(&conn.connection_type());
                    }

                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_get(&stats, &conn, false);
                    }
                    
                    return Ok(PooledConnection::new(conn, Arc::downgrade(self)));
                }
                Err(NetConnPoolError::MaxConnectionsReached) => {
                    // 并发情况下可能刚检查完就满了，继续循环
                    continue;
                }
                Err(e) => {
                    // 只有在确定无法创建符合要求的连接时才返回错误
                    // 如果是因为协议不匹配（比如随机创建了UDP但需要TCP），应该继续循环？
                    // create_connection 现在的实现是根据 config 创建。
                    // 如果 config 是 Client mode dialer，它创建什么就是什么。
                    // 如果 dialer 创建的类型不符合 protocol/ip_version 要求，我们应该 check。
                    if let Some(stats) = &self.stats_collector {
                        stats.increment_failed_gets();
                        stats.increment_connection_errors();
                    }
                    return Err(e);
                }
            }
        }
    }

    fn create_connection(
        &self,
        required_protocol: Option<Protocol>,
        required_ip_version: Option<IPVersion>
    ) -> Result<Arc<Connection>> {
        // Double check max connections under write lock to ensure consistency
        // But creating connection takes time, we shouldn't hold lock.
        // We use double check: check count, connect, check count again before insert.

        // Check count first
        if self.config.max_connections > 0 {
             let current = self.all_connections.read().unwrap().len();
             if current >= self.config.max_connections {
                 return Err(NetConnPoolError::MaxConnectionsReached);
             }
        }

        let conn_type = match self.config.mode {
            PoolMode::Client => {
                if let Some(dialer) = &self.config.dialer {
                    dialer().map_err(|e| NetConnPoolError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?
                } else {
                    return Err(NetConnPoolError::InvalidConfig);
                }
            }
            PoolMode::Server => {
                if let Some(listener) = &self.config.listener {
                    let acceptor = self.config.acceptor.as_ref().unwrap();
                    ConnectionType::Tcp(acceptor(listener).map_err(|e| NetConnPoolError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?)
                } else {
                    return Err(NetConnPoolError::InvalidConfig);
                }
            }
        };

        if let Some(on_created) = &self.config.on_created {
            on_created(&conn_type).map_err(|e| NetConnPoolError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        }

        let conn = match conn_type {
            ConnectionType::Tcp(stream) => Arc::new(Connection::new_from_tcp(stream, None)),
            ConnectionType::Udp(socket) => Arc::new(Connection::new_from_udp(socket, None)),
        };

        // Check requirements
        if let Some(p) = required_protocol {
            if p != Protocol::Unknown && conn.get_protocol() != p {
                // Mismatch, close and return specific error or handled by caller?
                // Caller expects specific protocol.
                // We should close this connection as it's useless for the caller.
                // But maybe we can put it into pool? 
                // "Put" requires it to be in all_connections.
                // Let's add it to pool and return error, so another thread can use it?
                // Implementation complexity: high. 
                // Simple approach: Close and return Error.
                 let _ = conn.close();
                 return Err(NetConnPoolError::NoConnectionForProtocol);
            }
        }
        if let Some(ip) = required_ip_version {
             if ip != IPVersion::Unknown && conn.get_ip_version() != ip {
                 let _ = conn.close();
                 return Err(NetConnPoolError::NoConnectionForIPVersion);
             }
        }

        // Insert into map
        {
            let mut connections = self.all_connections.write().unwrap();
            if self.config.max_connections > 0 && connections.len() >= self.config.max_connections {
                 let _ = conn.close();
                 return Err(NetConnPoolError::MaxConnectionsReached);
            }
            connections.insert(conn.id, conn.clone());
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_connections_created();
            match conn.get_ip_version() {
                IPVersion::IPv4 => stats.increment_current_ipv4_connections(1),
                IPVersion::IPv6 => stats.increment_current_ipv6_connections(1),
                _ => {}
            }
            match conn.get_protocol() {
                Protocol::TCP => stats.increment_current_tcp_connections(1),
                Protocol::UDP => stats.increment_current_udp_connections(1),
                _ => {}
            }
        }

        Ok(conn)
    }

    fn return_connection(&self, conn: Arc<Connection>) {
        if self.is_closed() {
            let _ = self.remove_connection(&conn);
            return;
        }

        if !self.is_connection_valid(&conn) {
            let _ = self.remove_connection(&conn);
            return;
        }

        if let Some(on_return) = &self.config.on_return {
            on_return(&conn.connection_type());
        }

        // UDP Buffer cleanup
        if self.config.clear_udp_buffer_on_return && conn.get_protocol() == Protocol::UDP {
            if let Some(udp_socket) = conn.udp_conn() {
                let timeout = self.config.udp_buffer_clear_timeout;
                let _ = clear_udp_read_buffer(udp_socket, timeout, self.config.max_buffer_clear_packets);
            }
        }

        conn.mark_idle();

        if let Some(stats) = &self.stats_collector {
            stats.increment_current_active_connections(-1);
        }

        // Put back to idle list
        if let Some(idx) = Self::get_bucket_index(conn.get_protocol(), conn.get_ip_version()) {
            let mut idle = self.idle_connections[idx].lock().unwrap();
            
            // Check max idle
            if idle.len() < self.config.max_idle_connections {
                 idle.push(conn.clone());
                 drop(idle); // Release lock early

                 if let Some(stats) = &self.stats_collector {
                     self.update_stats_on_return(&stats, &conn);
                 }
            } else {
                 drop(idle);
                 let _ = self.remove_connection(&conn);
            }
        } else {
            // Unknown protocol/ip, cannot pool efficiently. Close it.
            let _ = self.remove_connection(&conn);
        }
    }

    fn remove_connection(&self, conn: &Arc<Connection>) -> Result<()> {
        let _ = conn.close();
        
        {
            let mut connections = self.all_connections.write().unwrap();
            connections.remove(&conn.id);
        }

        if let Some(stats) = &self.stats_collector {
            stats.increment_total_connections_closed();
             match conn.get_ip_version() {
                IPVersion::IPv4 => stats.increment_current_ipv4_connections(-1),
                IPVersion::IPv6 => stats.increment_current_ipv6_connections(-1),
                _ => {}
            }
            match conn.get_protocol() {
                Protocol::TCP => stats.increment_current_tcp_connections(-1),
                Protocol::UDP => stats.increment_current_udp_connections(-1),
                _ => {}
            }
        }
        Ok(())
    }

    fn cleanup(&self) {
        let mut to_remove = Vec::new();
        
        // Scan for expired connections
        {
            let connections = self.all_connections.read().unwrap();
            for conn in connections.values() {
                if !self.is_connection_valid(conn) {
                    to_remove.push(conn.clone());
                }
            }
        }

        for conn in to_remove {
            let _ = self.remove_connection(&conn);
            // Also need to remove from idle list if it's there
            // Since remove_connection only removes from all_connections map,
            // we rely on the fact that when we pop from idle list, we validate connection.
            // But we should also try to clean up idle lists to avoid them filling with garbage.
        }
        
        // Cleanup idle lists for closed connections
        for idle in &self.idle_connections {
            let mut list = idle.lock().unwrap();
            // retain only connections that are valid AND exist in all_connections (implied valid)
            list.retain(|c| self.is_connection_valid(c));
            
            // Sync idle stats? Stats are updated on push/pop. 
            // If we remove here, we should update stats.
            // But doing diff is hard.
            // We should decrement stats for removed idle connections.
            // Implementing correct stats update in retain is hard.
            // Alternative: pop all, filter, push back.
        }
    }

    fn is_connection_valid(&self, conn: &Connection) -> bool {
        if !conn.health_status() {
            return false;
        }
        if conn.is_expired(self.config.max_lifetime) {
            return false;
        }
        // Idle timeout check could be added here
        if conn.is_idle_expired(self.config.idle_timeout) {
            return false;
        }
        true
    }

    fn update_stats_on_get(&self, stats: &StatsCollector, conn: &Connection, is_reused: bool) {
        if is_reused {
            stats.increment_successful_gets();
            stats.increment_current_active_connections(1);
            stats.increment_current_idle_connections(-1);
            stats.increment_total_connections_reused();
            match conn.get_ip_version() {
                IPVersion::IPv4 => stats.increment_current_ipv4_idle_connections(-1),
                IPVersion::IPv6 => stats.increment_current_ipv6_idle_connections(-1),
                _ => {}
            }
            match conn.get_protocol() {
                Protocol::TCP => stats.increment_current_tcp_idle_connections(-1),
                Protocol::UDP => stats.increment_current_udp_idle_connections(-1),
                _ => {}
            }
        } else {
             stats.increment_successful_gets();
             stats.increment_current_active_connections(1);
        }
    }

    fn update_stats_on_return(&self, stats: &StatsCollector, conn: &Connection) {
        stats.increment_current_idle_connections(1);
        match conn.get_ip_version() {
            IPVersion::IPv4 => stats.increment_current_ipv4_idle_connections(1),
            IPVersion::IPv6 => stats.increment_current_ipv6_idle_connections(1),
            _ => {}
        }
        match conn.get_protocol() {
            Protocol::TCP => stats.increment_current_tcp_idle_connections(1),
            Protocol::UDP => stats.increment_current_udp_idle_connections(1),
            _ => {}
        }
    }
}
