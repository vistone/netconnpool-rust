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
use std::time::{Duration, Instant};

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
    /// 创建新的连接池
    ///
    /// # 参数
    /// - `config`: 连接池配置，必须包含有效的 Dialer（客户端模式）或 Listener（服务器端模式）
    ///
    /// # 返回值
    /// - `Ok(Pool)`: 成功创建连接池
    /// - `Err(NetConnPoolError)`: 配置无效或创建失败
    ///
    /// # 示例
    /// ```rust,no_run
    /// use netconnpool::*;
    /// use std::net::TcpStream;
    ///
    /// let mut config = default_config();
    /// config.dialer = Some(Box::new(|_| {
    ///     TcpStream::connect("127.0.0.1:8080")
    ///         .map(|s| ConnectionType::Tcp(s))
    ///         .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    /// }));
    /// let pool = Pool::new(config).unwrap();
    /// ```
    pub fn new(mut config: Config) -> Result<Self> {
        config.apply_defaults();
        config.validate()?;

        let stats_collector = if config.enable_stats {
            Some(Arc::new(StatsCollector::new()))
        } else {
            None
        };

        let inner = Arc::new(PoolInner {
            config,
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
            .map_err(NetConnPoolError::IoError)?;

        // 启动预热线程（min_connections）
        // 仅客户端模式预热；服务器模式预热可能会阻塞在 accept 上。
        if inner.config.mode == PoolMode::Client && inner.config.min_connections > 0 {
            let weak_inner = Arc::downgrade(&inner);
            let _ = thread::Builder::new()
                .name("connection-pool-prewarmer".to_string())
                .spawn(move || {
                    Self::prewarm(weak_inner);
                });
        }

        Ok(Self { inner })
    }

    fn prewarm(inner: Weak<PoolInner>) {
        let pool = match inner.upgrade() {
            Some(p) => p,
            None => return,
        };

        let target = pool.config.min_connections;
        drop(pool);

        for _ in 0..target {
            let pool = match inner.upgrade() {
                Some(p) => p,
                None => return,
            };
            if pool.is_closed() {
                return;
            }

            // 预热只做 best-effort：创建失败不影响 Pool::new
            if let Ok(conn) = pool.create_connection(None, None) {
                pool.add_idle_connection(conn);
            } else {
                // dialer 可能暂时不可用（例如测试场景未启动服务），直接停止预热
                return;
            }
        }
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

            let interval = if pool.config.health_check_interval.is_zero() {
                Duration::from_secs(1)
            } else {
                pool.config.health_check_interval
            };
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

    /// 获取一个连接（自动选择IP版本和协议）
    ///
    /// # 返回值
    /// - `Ok(PooledConnection)`: 成功获取连接
    /// - `Err(NetConnPoolError)`: 获取失败（超时、池已关闭、连接池耗尽等）
    ///
    /// # 示例
    /// ```rust,no_run
    /// use netconnpool::*;
    /// use std::net::TcpStream;
    ///
    /// let mut config = default_config();
    /// config.dialer = Some(Box::new(|_| {
    ///     TcpStream::connect("127.0.0.1:8080")
    ///         .map(|s| ConnectionType::Tcp(s))
    ///         .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    /// }));
    ///
    /// let pool = Pool::new(config).unwrap();
    /// let conn = pool.get().unwrap();
    /// // 使用连接...
    /// drop(conn); // 自动归还
    /// ```
    pub fn get(&self) -> Result<PooledConnection> {
        self.get_with_timeout(self.inner.config.get_connection_timeout)
    }

    /// GetIPv4 获取一个IPv4连接
    pub fn get_ipv4(&self) -> Result<PooledConnection> {
        self.get_with_ip_version(IPVersion::IPv4, self.inner.config.get_connection_timeout)
    }

    /// 获取一个IPv6连接
    ///
    /// # 返回值
    /// - `Ok(PooledConnection)`: 成功获取IPv6连接
    /// - `Err(NetConnPoolError)`: 获取失败（超时、没有可用IPv6连接等）
    pub fn get_ipv6(&self) -> Result<PooledConnection> {
        self.get_with_ip_version(IPVersion::IPv6, self.inner.config.get_connection_timeout)
    }

    /// 获取一个TCP连接
    ///
    /// # 返回值
    /// - `Ok(PooledConnection)`: 成功获取TCP连接
    /// - `Err(NetConnPoolError)`: 获取失败（超时、没有可用TCP连接等）
    pub fn get_tcp(&self) -> Result<PooledConnection> {
        self.get_with_protocol(Protocol::TCP, self.inner.config.get_connection_timeout)
    }

    /// 获取一个UDP连接
    ///
    /// # 返回值
    /// - `Ok(PooledConnection)`: 成功获取UDP连接
    /// - `Err(NetConnPoolError)`: 获取失败（超时、没有可用UDP连接等）
    pub fn get_udp(&self) -> Result<PooledConnection> {
        self.get_with_protocol(Protocol::UDP, self.inner.config.get_connection_timeout)
    }

    /// 获取指定协议的连接
    ///
    /// # 参数
    /// - `protocol`: 协议类型（TCP 或 UDP）
    /// - `timeout`: 获取连接的超时时间
    ///
    /// # 返回值
    /// - `Ok(PooledConnection)`: 成功获取指定协议的连接
    /// - `Err(NetConnPoolError)`: 获取失败（超时、没有可用连接等）
    pub fn get_with_protocol(
        &self,
        protocol: Protocol,
        timeout: Duration,
    ) -> Result<PooledConnection> {
        self.inner.get_connection(Some(protocol), None, timeout)
    }

    /// 获取指定IP版本的连接
    ///
    /// # 参数
    /// - `ip_version`: IP版本（IPv4 或 IPv6）
    /// - `timeout`: 获取连接的超时时间
    ///
    /// # 返回值
    /// - `Ok(PooledConnection)`: 成功获取指定IP版本的连接
    /// - `Err(NetConnPoolError)`: 获取失败（超时、没有可用连接等）
    pub fn get_with_ip_version(
        &self,
        ip_version: IPVersion,
        timeout: Duration,
    ) -> Result<PooledConnection> {
        self.inner.get_connection(None, Some(ip_version), timeout)
    }

    /// 获取一个连接（带超时，自动选择IP版本和协议）
    ///
    /// # 参数
    /// - `timeout`: 获取连接的超时时间
    ///
    /// # 返回值
    /// - `Ok(PooledConnection)`: 成功获取连接
    /// - `Err(NetConnPoolError)`: 获取失败（超时、池已关闭等）
    pub fn get_with_timeout(&self, timeout: Duration) -> Result<PooledConnection> {
        self.inner.get_connection(None, None, timeout)
    }

    /// 关闭连接池
    ///
    /// 关闭连接池会：
    /// - 停止接受新的连接请求
    /// - 关闭所有空闲连接
    /// - 等待所有使用中的连接归还后关闭
    /// - 停止后台清理线程
    ///
    /// # 返回值
    /// - `Ok(())`: 成功关闭连接池
    /// - `Err(NetConnPoolError)`: 关闭失败
    ///
    /// # 注意
    /// 关闭后的连接池不能再次使用。多次调用 `close()` 是安全的（幂等操作）。
    pub fn close(&self) -> Result<()> {
        self.inner.close()
    }

    /// 获取连接池统计信息
    ///
    /// 返回连接池的统计信息，包括：
    /// - 性能指标（平均复用次数、平均获取时间）
    /// - 健康检查统计
    ///
    /// # 注意
    /// 如果创建连接池时未启用统计功能（`enable_stats = false`），将返回默认值（全为0）。
    ///
    /// # 示例
    /// ```rust,no_run
    /// use netconnpool::*;
    /// use std::net::TcpStream;
    ///
    /// let mut config = default_config();
    /// config.dialer = Some(Box::new(|_| {
    ///     TcpStream::connect("127.0.0.1:8080")
    ///         .map(|s| ConnectionType::Tcp(s))
    ///         .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    /// }));
    ///
    /// let pool = Pool::new(config).unwrap();
    /// let stats = pool.stats();
    /// println!("当前连接数: {}", stats.current_connections);
    /// println!("连接复用率: {:.2}%", stats.average_reuse_count * 100.0);
    /// ```
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
            let connections = self.all_connections.read().map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("获取连接列表失败: {}", e),
                ))
            })?;
            connections.values().cloned().collect()
        };

        // 逐个 remove，确保统计与 idle 清理一致
        for conn in conns {
            let _ = self.remove_connection(&conn);
        }

        // 最后兜底清空 idle（理论上 remove_connection 已做）
        for idle in &self.idle_connections {
            let _ = idle.lock().map(|mut guard| guard.clear());
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
    fn get_target_buckets(
        &self,
        protocol: Option<Protocol>,
        ip_version: Option<IPVersion>,
    ) -> Vec<usize> {
        let mut indices = Vec::new();
        let protocols = if let Some(p) = protocol {
            if p == Protocol::Unknown {
                vec![Protocol::TCP, Protocol::UDP]
            } else {
                vec![p]
            }
        } else {
            vec![Protocol::TCP, Protocol::UDP]
        };

        let ip_versions = if let Some(ip) = ip_version {
            if ip == IPVersion::Unknown {
                vec![IPVersion::IPv4, IPVersion::IPv6]
            } else {
                vec![ip]
            }
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

        let start_time = Instant::now();
        let bucket_indices = self.get_target_buckets(protocol, ip_version);

        loop {
            if self.is_closed() {
                return Err(NetConnPoolError::PoolClosed);
            }

            let elapsed = start_time.elapsed();
            if elapsed > timeout {
                if let Some(stats) = &self.stats_collector {
                    stats.increment_failed_gets();
                    stats.increment_timeout_gets();
                }
                return Err(NetConnPoolError::GetConnectionTimeout {
                    timeout,
                    waited: elapsed,
                });
            }

            // 1. 尝试从空闲池获取
            for &idx in &bucket_indices {
                let conn = {
                    let mut idle = self.idle_connections[idx].lock().map_err(|e| {
                        NetConnPoolError::IoError(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("获取空闲连接池锁失败: {}", e),
                        ))
                    })?;
                    idle.pop()
                };

                if let Some(conn) = conn {
                    // 从 idle 移除即应更新 idle 统计（无论最终是否可用）
                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_idle_pop(stats, &conn);
                    }

                    if !self.is_connection_valid_for_borrow(&conn) {
                        let _ = self.remove_connection(&conn);
                        continue;
                    }

                    conn.mark_in_use();
                    conn.increment_reuse_count();

                    if let Some(on_borrow) = &self.config.on_borrow {
                        on_borrow(conn.connection_type());
                    }

                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_get_success(stats, true, start_time.elapsed());
                    }

                    return Ok(PooledConnection::new(conn, Arc::downgrade(self)));
                }
            }

            // 2. 检查最大连接数
            if self.config.max_connections > 0 {
                let current = self
                    .all_connections
                    .read()
                    .map_err(|e| {
                        NetConnPoolError::IoError(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("读取连接数失败: {}", e),
                        ))
                    })?
                    .len();
                if current >= self.config.max_connections {
                    if let Some(stats) = &self.stats_collector {
                        stats.increment_failed_gets();
                    }
                    // 连接池已耗尽：快速失败（由上层决定是否重试）
                    return Err(NetConnPoolError::PoolExhausted {
                        current,
                        max: self.config.max_connections,
                    });
                }
            }

            // 3. 创建新连接
            match self.create_connection(protocol, ip_version) {
                Ok(conn) => {
                    conn.mark_in_use();

                    if let Some(on_borrow) = &self.config.on_borrow {
                        on_borrow(conn.connection_type());
                    }

                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_get_success(stats, false, start_time.elapsed());
                    }

                    return Ok(PooledConnection::new(conn, Arc::downgrade(self)));
                }
                Err(NetConnPoolError::MaxConnectionsReached { .. }) => {
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
        required_ip_version: Option<IPVersion>,
    ) -> Result<Arc<Connection>> {
        // Double check max connections under write lock to ensure consistency
        // But creating connection takes time, we shouldn't hold lock.
        // We use double check: check count, connect, check count again before insert.

        // Check count first
        if self.config.max_connections > 0 {
            let current = self
                .all_connections
                .read()
                .map_err(|e| {
                    NetConnPoolError::IoError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("读取连接数失败: {}", e),
                    ))
                })?
                .len();
            if current >= self.config.max_connections {
                return Err(NetConnPoolError::MaxConnectionsReached {
                    current,
                    max: self.config.max_connections,
                });
            }
        }

        let conn_type = match self.config.mode {
            PoolMode::Client => {
                if let Some(dialer) = &self.config.dialer {
                    dialer(required_protocol).map_err(|e| {
                        NetConnPoolError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?
                } else {
                    return Err(NetConnPoolError::InvalidConfig {
                        reason: "客户端模式需要 Dialer".to_string(),
                    });
                }
            }
            PoolMode::Server => {
                if let Some(listener) = &self.config.listener {
                    let acceptor = self.config.acceptor.as_ref().ok_or_else(|| {
                        NetConnPoolError::InvalidConfig {
                            reason: "服务器模式需要 Acceptor".to_string(),
                        }
                    })?;
                    ConnectionType::Tcp(acceptor(listener).map_err(|e| {
                        NetConnPoolError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?)
                } else {
                    return Err(NetConnPoolError::InvalidConfig {
                        reason: "服务器模式需要 Listener".to_string(),
                    });
                }
            }
        };

        if let Some(on_created) = &self.config.on_created {
            on_created(&conn_type).map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;
        }

        // 连接池内部统一使用阻塞模式（与 UDP 清缓冲逻辑保持一致）
        let conn = match conn_type {
            ConnectionType::Tcp(stream) => {
                stream
                    .set_nonblocking(false)
                    .map_err(NetConnPoolError::IoError)?;
                Arc::new(Connection::new_from_tcp(stream, None))
            }
            ConnectionType::Udp(socket) => {
                socket
                    .set_nonblocking(false)
                    .map_err(NetConnPoolError::IoError)?;
                Arc::new(Connection::new_from_udp(socket, None))
            }
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
                self.close_connection(&conn);
                return Err(NetConnPoolError::NoConnectionForProtocol {
                    required: format!("{:?}", p),
                });
            }
        }
        if let Some(ip) = required_ip_version {
            if ip != IPVersion::Unknown && conn.get_ip_version() != ip {
                self.close_connection(&conn);
                return Err(NetConnPoolError::NoConnectionForIPVersion {
                    required: format!("{:?}", ip),
                });
            }
        }

        // Insert into map
        {
            let mut connections = self.all_connections.write().map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("获取连接映射写锁失败: {}", e),
                ))
            })?;
            let current = connections.len();
            if self.config.max_connections > 0 && current >= self.config.max_connections {
                self.close_connection(&conn);
                return Err(NetConnPoolError::MaxConnectionsReached {
                    current,
                    max: self.config.max_connections,
                });
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
        // 连接归还意味着不再 active
        if let Some(stats) = &self.stats_collector {
            stats.increment_current_active_connections(-1);
        }

        if self.is_closed() {
            let _ = self.remove_connection(&conn);
            return;
        }

        // 标记为空闲，方便后续 idle/泄漏/超时判断
        conn.mark_idle();

        if !self.is_connection_valid_for_borrow(&conn) {
            let _ = self.remove_connection(&conn);
            return;
        }

        if let Some(on_return) = &self.config.on_return {
            on_return(conn.connection_type());
        }

        // UDP Buffer cleanup
        if self.config.clear_udp_buffer_on_return && conn.get_protocol() == Protocol::UDP {
            if let Some(udp_socket) = conn.udp_conn() {
                let timeout = self.config.udp_buffer_clear_timeout;
                let _ = clear_udp_read_buffer(
                    udp_socket,
                    timeout,
                    self.config.max_buffer_clear_packets,
                );
            }
        }

        // Put back to idle list
        if let Some(idx) = Self::get_bucket_index(conn.get_protocol(), conn.get_ip_version()) {
            // 如果获取锁失败，忽略错误（不影响主流程）
            if let Ok(mut idle) = self.idle_connections[idx].lock() {
                // Check max idle
                if idle.len() < self.config.max_idle_connections {
                    idle.push(conn.clone());
                    drop(idle); // Release lock early

                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_idle_push(stats, &conn);
                    }
                } else {
                    drop(idle);
                    let _ = self.remove_connection(&conn);
                }
            } else {
                // 锁获取失败，直接移除连接
                let _ = self.remove_connection(&conn);
            }
        } else {
            // Unknown protocol/ip, cannot pool efficiently. Close it.
            let _ = self.remove_connection(&conn);
        }
    }

    fn remove_connection(&self, conn: &Arc<Connection>) -> Result<()> {
        // 如果在关闭/清理过程中强制移除使用中的连接，修正 active 统计
        if conn.is_in_use() {
            if let Some(stats) = &self.stats_collector {
                stats.increment_current_active_connections(-1);
            }
            conn.mark_idle();
        }

        // 如果它仍在 idle 列表里，先移除（并同步 idle 统计）
        self.remove_from_idle_if_present(conn);

        self.close_connection(conn);

        {
            let mut connections = self.all_connections.write().map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("获取连接映射写锁失败: {}", e),
                ))
            })?;
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
        let conns: Vec<Arc<Connection>> = {
            // 如果获取锁失败，返回空列表（清理失败不影响主流程）
            if let Ok(connections) = self.all_connections.read() {
                connections.values().cloned().collect()
            } else {
                return; // 锁获取失败，跳过本次清理
            }
        };

        let mut to_remove = Vec::new();

        for conn in conns {
            if self.is_closed() {
                return;
            }

            // 连接使用中：不强制关闭，只做泄漏/超龄标记，等待归还时清理
            if conn.is_in_use() {
                if conn.is_leaked(self.config.connection_leak_timeout) {
                    if conn.report_leak_once() {
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_leaked_connections();
                        }
                    }
                    conn.mark_unhealthy();
                }
                if conn.is_expired(self.config.max_lifetime) {
                    conn.mark_unhealthy();
                }
                continue;
            }

            // 健康检查（仅对 idle 连接）
            if self.config.enable_health_check {
                if let Some(checker) = &self.config.health_checker {
                    if conn.should_health_check(self.config.health_check_interval) {
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_health_check_attempts();
                        }
                        let ok = checker(conn.connection_type());
                        if !ok {
                            if let Some(stats) = &self.stats_collector {
                                stats.increment_health_check_failures();
                                stats.increment_unhealthy_connections();
                            }
                            conn.update_health(false);
                            to_remove.push(conn.clone());
                            continue;
                        }
                        conn.update_health(true);
                    }
                }
            }

            if !self.is_connection_valid_for_borrow(&conn) {
                to_remove.push(conn.clone());
            }
        }

        for conn in to_remove {
            let _ = self.remove_connection(&conn);
        }
    }

    fn is_connection_valid_for_borrow(&self, conn: &Connection) -> bool {
        if conn.is_closed() {
            return false;
        }
        if !conn.health_status() {
            return false;
        }
        if conn.is_expired(self.config.max_lifetime) {
            return false;
        }
        if conn.is_idle_expired(self.config.idle_timeout) {
            return false;
        }
        true
    }

    fn update_stats_on_idle_pop(&self, stats: &StatsCollector, conn: &Connection) {
        stats.increment_current_idle_connections(-1);
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
    }

    fn update_stats_on_idle_push(&self, stats: &StatsCollector, conn: &Connection) {
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

    fn update_stats_on_get_success(
        &self,
        stats: &StatsCollector,
        is_reused: bool,
        get_duration: Duration,
    ) {
        stats.increment_successful_gets();
        stats.increment_current_active_connections(1);
        if is_reused {
            stats.increment_total_connections_reused();
        }
        stats.record_get_time(get_duration);
    }

    fn add_idle_connection(&self, conn: Arc<Connection>) {
        if self.is_closed() {
            let _ = self.remove_connection(&conn);
            return;
        }

        conn.mark_idle();

        if let Some(idx) = Self::get_bucket_index(conn.get_protocol(), conn.get_ip_version()) {
            if let Ok(mut idle) = self.idle_connections[idx].lock() {
                if idle.len() < self.config.max_idle_connections {
                    idle.push(conn.clone());
                    drop(idle);
                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_idle_push(stats, &conn);
                    }
                } else {
                    drop(idle);
                    let _ = self.remove_connection(&conn);
                }
            } else {
                // 锁获取失败，直接移除连接
                let _ = self.remove_connection(&conn);
            }
        } else {
            let _ = self.remove_connection(&conn);
        }
    }

    fn close_connection(&self, conn: &Arc<Connection>) {
        if let Some(closer) = &self.config.close_conn {
            let _ = closer(conn.connection_type());
        }
        let _ = conn.close();
    }

    fn remove_from_idle_if_present(&self, conn: &Arc<Connection>) {
        if let Some(idx) = Self::get_bucket_index(conn.get_protocol(), conn.get_ip_version()) {
            if let Ok(mut idle) = self.idle_connections[idx].lock() {
                let before = idle.len();
                if before == 0 {
                    return;
                }
                idle.retain(|c| c.id != conn.id);
                let after = idle.len();
                drop(idle);

                if before != after {
                    if let Some(stats) = &self.stats_collector {
                        // 从 idle 移除一次
                        self.update_stats_on_idle_pop(stats, conn);
                    }
                }
            }
        }
    }
}
