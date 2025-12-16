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
use crossbeam::queue::SegQueue;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock, Weak};
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
    // 空闲连接池，按 (Protocol, IPVersion) 分桶（使用无锁队列）
    // 0: TCP IPv4, 1: TCP IPv6, 2: UDP IPv4, 3: UDP IPv6
    idle_connections: [SegQueue<Arc<Connection>>; 4],
    // 每个桶的当前大小（原子计数器，用于 max_idle_connections 限制）
    idle_counts: [AtomicUsize; 4],
    closed: AtomicBool,
    // 当前借出的连接数（不依赖 enable_stats）
    active_count: AtomicUsize,
    // 用于在连接归还/池状态变化时唤醒 get() 等待者
    wait_lock: Mutex<()>,
    wait_cv: Condvar,
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
                SegQueue::new(),
                SegQueue::new(),
                SegQueue::new(),
                SegQueue::new(),
            ],
            idle_counts: [
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
            ],
            closed: AtomicBool::new(false),
            active_count: AtomicUsize::new(0),
            wait_lock: Mutex::new(()),
            wait_cv: Condvar::new(),
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

        // 唤醒所有等待 get() 的线程
        self.wait_cv.notify_all();

        // 1) 先关闭所有 idle 连接（不影响正在使用的连接）
        // 为了保持 idle 统计一致性，这里显式扣减 idle 统计（因为我们会直接 drain bucket）
        let mut idle_conns: Vec<Arc<Connection>> = Vec::new();
        for (idx, idle) in self.idle_connections.iter().enumerate() {
            // 无锁队列：持续 pop 直到为空
            while let Some(conn) = idle.pop() {
                idle_conns.push(conn);
            }
            // 重置计数器
            self.idle_counts[idx].store(0, Ordering::Relaxed);
        }

        for conn in &idle_conns {
            if let Some(stats) = &self.stats_collector {
                self.update_stats_on_idle_pop(stats, conn);
            }
            let _ = self.remove_connection(conn);
        }

        // 2) 等待活跃连接归还（优雅关闭）
        // 为避免 close 永久阻塞，最多等待 connection_leak_timeout（为 0 则不等待）
        let wait_budget = self.config.connection_leak_timeout;
        if !wait_budget.is_zero() {
            let deadline = Instant::now() + wait_budget;
            let mut guard = self.wait_lock.lock().unwrap_or_else(|e| e.into_inner());
            while self.active_count.load(Ordering::Acquire) > 0 && Instant::now() < deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let (g, _timeout) = match self.wait_cv.wait_timeout(guard, remaining) {
                    Ok(res) => res,
                    Err(e) => e.into_inner(),
                };
                guard = g;
            }
        }

        // 3) 最后兜底：关闭所有仍存活的连接（可能包含泄漏/长期占用）
        let conns: Vec<Arc<Connection>> = {
            let connections = self.all_connections.read().map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::other(format!(
                    "获取连接列表失败: {}",
                    e
                )))
            })?;
            connections.values().cloned().collect()
        };

        for conn in conns {
            let _ = self.remove_connection(&conn);
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

            // 1. 尝试从空闲池获取（无锁操作）
            for &idx in &bucket_indices {
                let conn = self.idle_connections[idx].pop();

                if let Some(conn) = conn {
                    // 更新计数器
                    self.idle_counts[idx].fetch_sub(1, Ordering::Relaxed);
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
                    self.active_count.fetch_add(1, Ordering::Relaxed);

                    if let Some(on_borrow) = &self.config.on_borrow {
                        on_borrow(conn.connection_type());
                    }

                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_get_success(stats, true, start_time.elapsed());
                    }

                    return Ok(PooledConnection::new(conn, Arc::downgrade(self)));
                }
            }

            // 2. 创建新连接（若并发下已满，会返回 MaxConnectionsReached）
            match self.create_connection(protocol, ip_version) {
                Ok(conn) => {
                    conn.mark_in_use();
                    self.active_count.fetch_add(1, Ordering::Relaxed);

                    if let Some(on_borrow) = &self.config.on_borrow {
                        on_borrow(conn.connection_type());
                    }

                    if let Some(stats) = &self.stats_collector {
                        self.update_stats_on_get_success(stats, false, start_time.elapsed());
                    }

                    return Ok(PooledConnection::new(conn, Arc::downgrade(self)));
                }
                Err(NetConnPoolError::MaxConnectionsReached { .. }) => {
                    // 池已满：在 timeout 内等待连接归还（避免自旋 & 过早失败）
                    if timeout.is_zero() {
                        // 明确的快速失败语义
                        let current = self
                            .all_connections
                            .read()
                            .map_err(|e| {
                                NetConnPoolError::IoError(std::io::Error::other(format!(
                                    "读取连接数失败: {}",
                                    e
                                )))
                            })?
                            .len();
                        return Err(NetConnPoolError::PoolExhausted {
                            current,
                            max: self.config.max_connections,
                        });
                    }

                    let remaining = timeout.saturating_sub(start_time.elapsed());
                    let guard = self.wait_lock.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = match self.wait_cv.wait_timeout(guard, remaining) {
                        Ok(res) => res,
                        Err(e) => e.into_inner(),
                    };
                    // 被唤醒/超时后继续循环：重试 idle 或创建
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
                    NetConnPoolError::IoError(std::io::Error::other(format!(
                        "读取连接数失败: {}",
                        e
                    )))
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
                        NetConnPoolError::IoError(std::io::Error::other(e.to_string()))
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
                        NetConnPoolError::IoError(std::io::Error::other(e.to_string()))
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
                NetConnPoolError::IoError(std::io::Error::other(e.to_string()))
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
                NetConnPoolError::IoError(std::io::Error::other(format!(
                    "获取连接映射写锁失败: {}",
                    e
                )))
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
        // 归还：从 active -> idle（避免重复扣减 active 统计）
        let was_in_use = conn.is_in_use();
        conn.mark_idle();
        if was_in_use {
            self.active_count.fetch_sub(1, Ordering::Relaxed);
            if let Some(stats) = &self.stats_collector {
                stats.increment_current_active_connections(-1);
            }
            self.wait_cv.notify_all();
        }

        if self.is_closed() {
            let _ = self.remove_connection(&conn);
            return;
        }

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

        // Put back to idle list (无锁操作)
        if let Some(idx) = Self::get_bucket_index(conn.get_protocol(), conn.get_ip_version()) {
            // 使用原子计数器检查 max_idle_connections（近似值，性能优先）
            let current_count = self.idle_counts[idx].load(Ordering::Relaxed);
            if current_count < self.config.max_idle_connections {
                // 先增加计数器（乐观锁）
                self.idle_counts[idx].fetch_add(1, Ordering::Relaxed);
                // 推入队列（无锁操作）
                self.idle_connections[idx].push(conn.clone());

                if let Some(stats) = &self.stats_collector {
                    self.update_stats_on_idle_push(stats, &conn);
                }
            } else {
                // 超过最大空闲连接数，直接移除
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
            conn.mark_idle();
            self.active_count.fetch_sub(1, Ordering::Relaxed);
            if let Some(stats) = &self.stats_collector {
                stats.increment_current_active_connections(-1);
            }
            self.wait_cv.notify_all();
        }

        // 如果它仍在 idle 列表里，先移除（并同步 idle 统计）
        self.remove_from_idle_if_present(conn);

        self.close_connection(conn);

        {
            let mut connections = self.all_connections.write().map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::other(format!(
                    "获取连接映射写锁失败: {}",
                    e
                )))
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
            // 使用原子计数器检查 max_idle_connections（近似值，性能优先）
            let current_count = self.idle_counts[idx].load(Ordering::Relaxed);
            if current_count < self.config.max_idle_connections {
                // 先增加计数器（乐观锁）
                self.idle_counts[idx].fetch_add(1, Ordering::Relaxed);
                // 推入队列（无锁操作）
                self.idle_connections[idx].push(conn.clone());

                if let Some(stats) = &self.stats_collector {
                    self.update_stats_on_idle_push(stats, &conn);
                }
            } else {
                // 超过最大空闲连接数，直接移除
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
            // 无锁队列没有 retain 方法
            // 为了性能，我们采用"标记移除"策略：
            // 1. 连接在队列中时，会在 return_connection 时通过 is_connection_valid_for_borrow 检查被过滤
            // 2. 这里我们尝试从队列中移除（如果存在），但为了性能，我们限制最大检查次数
            // 3. 由于无锁队列的特性，这个操作是 best-effort 的

            const MAX_CHECK: usize = 100; // 最多检查 100 个连接，避免性能问题
            let mut checked = 0;
            let mut found = false;
            let mut temp_vec = Vec::new();

            // 尝试从队列中查找并移除目标连接（限制检查次数）
            while checked < MAX_CHECK {
                if let Some(c) = self.idle_connections[idx].pop() {
                    checked += 1;
                    if c.id == conn.id {
                        found = true;
                        self.idle_counts[idx].fetch_sub(1, Ordering::Relaxed);
                        // 不将连接放回队列
                        break;
                    } else {
                        temp_vec.push(c);
                    }
                } else {
                    // 队列为空，停止查找
                    break;
                }
            }

            // 将其他连接放回队列
            for c in temp_vec {
                self.idle_connections[idx].push(c);
            }

            if found {
                if let Some(stats) = &self.stats_collector {
                    // 从 idle 移除一次
                    self.update_stats_on_idle_pop(stats, conn);
                }
            }
        }
    }
}
