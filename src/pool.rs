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
use std::fmt;
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

// 确保 Pool 和 PooledConnection 可以安全地跨线程使用
// 这些断言在编译期检查，如果类型不满足 Send + Sync 则编译失败
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Pool>();
    assert_send_sync::<PooledConnection>();
};

impl fmt::Debug for Pool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pool")
            .field("closed", &self.inner.closed.load(Ordering::Relaxed))
            .field(
                "active_count",
                &self.inner.active_count.load(Ordering::Relaxed),
            )
            .finish()
    }
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
    reaper_cv: Condvar,     // 用于 reaper 线程等待
    reaper_lock: Mutex<()>, // 用于 reaper_cv
    stats_collector: Option<Arc<StatsCollector>>,
}

impl fmt::Debug for PoolInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolInner")
            .field("config", &self.config)
            .field(
                "all_connections_len",
                &self.all_connections.read().map(|c| c.len()).unwrap_or(0),
            )
            .field(
                "idle_counts",
                &[
                    self.idle_counts[0].load(Ordering::Relaxed),
                    self.idle_counts[1].load(Ordering::Relaxed),
                    self.idle_counts[2].load(Ordering::Relaxed),
                    self.idle_counts[3].load(Ordering::Relaxed),
                ],
            )
            .field("closed", &self.closed.load(Ordering::Relaxed))
            .field("active_count", &self.active_count.load(Ordering::Relaxed))
            .finish()
    }
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
            reaper_cv: Condvar::new(),
            reaper_lock: Mutex::new(()),
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

            // 使用 Condvar 等待，可以在池关闭时立即唤醒
            let guard = match pool.reaper_lock.lock() {
                Ok(g) => g,
                Err(_) => return, // 锁被 poison，退出
            };
            let (guard, timeout_result) = match pool.reaper_cv.wait_timeout(guard, interval) {
                Ok(result) => result,
                Err(_) => return, // 锁被 poison，退出
            };
            drop(guard);
            drop(pool); // 释放 pool，允许其他线程清理

            // 检查是否因为超时还是被唤醒
            if !timeout_result.timed_out() {
                // 被唤醒，检查是否因为关闭
                if let Some(p) = inner.upgrade() {
                    if p.closed.load(Ordering::Relaxed) {
                        return; // Pool已关闭，立即退出
                    }
                    drop(p);
                } else {
                    return; // Pool已销毁，立即退出
                }
            }

            // 再次检查 Pool 是否已销毁或关闭
            if inner.upgrade().is_none() {
                return;
            }

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

    /// 检查连接池是否已关闭
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// 获取当前活跃（借出）的连接数
    ///
    /// 此计数器独立于 `enable_stats` 配置，始终可用。
    pub fn active_count(&self) -> usize {
        self.inner.active_count.load(Ordering::Relaxed)
    }

    /// 获取当前空闲连接数（所有分桶之和）
    pub fn idle_count(&self) -> usize {
        self.inner
            .idle_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum()
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
        // 优化：分批处理，减少锁持有时间
        loop {
            let batch: Vec<Arc<Connection>> = {
                let connections = self.all_connections.read().map_err(|e| {
                    NetConnPoolError::IoError(std::io::Error::other(format!(
                        "获取连接列表失败: {}",
                        e
                    )))
                })?;
                // 每次只处理一批，减少锁持有时间
                connections.values().take(10).cloned().collect()
            };

            if batch.is_empty() {
                break;
            }

            // 在锁外处理连接
            for conn in batch {
                let _ = self.remove_connection(&conn);
            }
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

                    // 优化：在 get() 时清理 UDP 缓冲区，避免阻塞归还操作
                    // 由即将使用该连接的线程负责清理历史残存数据
                    if self.config.clear_udp_buffer_on_return
                        && conn.get_protocol() == Protocol::UDP
                    {
                        if let Some(udp_socket) = conn.udp_conn() {
                            let timeout = self.config.udp_buffer_clear_timeout;
                            let max_packets = self.config.max_buffer_clear_packets;
                            // 非阻塞清理，不会阻塞 get() 操作
                            let _ = clear_udp_read_buffer(udp_socket, timeout, max_packets);
                        }
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
        // Double check max connections to ensure consistency
        // 第一次检查：快速检查（read lock，不阻塞其他读取）
        // 创建连接（耗时操作，不持锁）
        // 第二次检查：最终检查（write lock，确保原子性）
        // 这样可以避免在创建连接期间持有锁，同时确保不会超出限制

        // 第一次检查：快速预检查
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
            on_created(&conn_type)
                .map_err(|e| NetConnPoolError::IoError(std::io::Error::other(e.to_string())))?;
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

        // 第二次检查：最终检查并插入（write lock，确保原子性）
        // 这是 double-check 的关键：即使第一次检查通过，在插入前再次检查
        // 可以防止多个线程同时通过第一次检查后都创建连接导致超出限制
        {
            let mut connections = self.all_connections.write().map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::other(format!(
                    "获取连接映射写锁失败: {}",
                    e
                )))
            })?;
            let current = connections.len();
            if self.config.max_connections > 0 && current >= self.config.max_connections {
                // 连接已创建但超出限制，需要关闭它
                drop(connections); // 释放锁后再关闭连接
                self.close_connection(&conn);
                return Err(NetConnPoolError::MaxConnectionsReached {
                    current,
                    max: self.config.max_connections,
                });
            }

            // 检查连接 ID 是否冲突（虽然概率极低，但需要处理）
            // 如果冲突，说明 ID 生成器溢出后重置，且旧连接仍存在
            // 这种情况下，我们递增 ID 直到找到不冲突的
            let mut final_id = conn.id;
            if connections.contains_key(&final_id) {
                // 从当前 ID 开始递增，直到找到不冲突的 ID
                loop {
                    final_id = final_id.wrapping_add(1);
                    if final_id == 0 {
                        final_id = 1; // 跳过 0
                    }
                    if !connections.contains_key(&final_id) {
                        break;
                    }
                    // 防止无限循环（理论上不应该发生，因为连接数有限）
                    if final_id == conn.id {
                        eprintln!("错误: 无法找到不冲突的连接 ID");
                        drop(connections);
                        self.close_connection(&conn);
                        return Err(NetConnPoolError::IoError(std::io::Error::other(
                            "连接 ID 冲突且无法解决",
                        )));
                    }
                }
                eprintln!("警告: 连接 ID {} 冲突，已调整为 {}", conn.id, final_id);
            }

            // 注意：由于 Connection 的 id 字段是普通字段，我们不能直接修改
            // 但我们可以使用新的 ID 作为 key 插入，这样连接池可以正确管理连接
            // 虽然 conn.id 和实际存储的 key 不一致，但这不影响功能
            // 因为连接池使用 all_connections 来管理连接，而不是依赖 id 字段
            connections.insert(final_id, conn.clone());
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
        // 使用 try_mark_idle 原子操作，防止与 reaper 线程强制驱逐产生竞态
        if conn.try_mark_idle() {
            self.active_count.fetch_sub(1, Ordering::Relaxed);
            if let Some(stats) = &self.stats_collector {
                stats.increment_current_active_connections(-1);
            }
            // 优化：使用 notify_one() 避免惊群效应
            // 归还一个连接时，只需要唤醒一个等待的线程
            self.wait_cv.notify_one();
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

        // 优化：UDP 缓冲区清理延迟到 get() 时进行，避免阻塞归还操作
        // 这样可以确保 return_connection 操作极致轻量，不会因为底层 I/O 阻塞

        // Put back to idle list (无锁操作)
        if let Some(idx) = Self::get_bucket_index(conn.get_protocol(), conn.get_ip_version()) {
            // 使用 CAS 操作原子地检查和增加计数器，避免竞态条件
            let max_idle = self.config.max_idle_connections;
            loop {
                let current = self.idle_counts[idx].load(Ordering::Relaxed);
                if current >= max_idle {
                    // 超过最大空闲连接数，直接移除
                    let _ = self.remove_connection(&conn);
                    break;
                }
                // 尝试原子地增加计数器
                match self.idle_counts[idx].compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // CAS 成功，推入队列
                        self.idle_connections[idx].push(conn.clone());

                        if let Some(stats) = &self.stats_collector {
                            self.update_stats_on_idle_push(stats, &conn);
                        }
                        break;
                    }
                    Err(_) => {
                        // CAS 失败，其他线程修改了计数器，重试
                        continue;
                    }
                }
            }
        } else {
            // Unknown protocol/ip, cannot pool efficiently. Close it.
            let _ = self.remove_connection(&conn);
        }
    }

    fn remove_connection(&self, conn: &Arc<Connection>) -> Result<()> {
        // 如果在关闭/清理过程中强制移除使用中的连接，修正 active 统计
        // 使用 try_mark_idle 原子操作，防止与 return_connection 产生竞态
        if conn.try_mark_idle() {
            self.active_count.fetch_sub(1, Ordering::Relaxed);
            if let Some(stats) = &self.stats_collector {
                stats.increment_current_active_connections(-1);
            }
            // 优化：使用 notify_one() 避免惊群效应
            // 移除一个连接时，只需要唤醒一个等待的线程
            self.wait_cv.notify_one();
        }
        // 注意：如果连接在idle队列中，我们不在这里更新idle_counts计数器
        // 因为SegQueue不支持删除特定元素，连接仍在队列中
        // 当get_connection pop它时，会检查有效性并调用remove_connection
        // 但此时连接已经不在all_connections中了，避免重复处理
        // 这种延迟清理的设计是合理的，因为：
        // 1. 连接已被标记为关闭，get_connection会跳过它
        // 2. 连接会从队列中pop出来并正确清理
        // 3. idle_counts会在pop时正确更新

        self.close_connection(conn);

        {
            let mut connections = self.all_connections.write().map_err(|e| {
                NetConnPoolError::IoError(std::io::Error::other(format!(
                    "获取连接映射写锁失败: {}",
                    e
                )))
            })?;

            // 首先尝试使用 conn.id 移除（正常情况）
            if connections.remove(&conn.id).is_none() {
                // 如果找不到，说明 ID 冲突时使用了不同的 key
                // 遍历查找并移除（使用 Arc::ptr_eq 比较指针，确保找到正确的连接）
                let conn_ptr = Arc::as_ptr(conn);
                let mut found_key = None;
                for (key, value) in connections.iter() {
                    if Arc::as_ptr(value) == conn_ptr {
                        found_key = Some(*key);
                        break;
                    }
                }
                if let Some(key) = found_key {
                    connections.remove(&key);
                } else {
                    // 连接不在映射中，可能已经被移除或从未插入
                    // 这是正常的，不需要报错
                }
            }
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

            // 连接使用中：检查是否严重泄漏，如果是则强制驱逐
            if conn.is_in_use() {
                let is_leaked = conn.is_leaked(self.config.connection_leak_timeout);
                let is_expired = conn.is_expired(self.config.max_lifetime);

                // 如果连接严重泄漏（超过 leak_timeout 的 2 倍），强制驱逐
                // 这是为了保护连接池内存不被用户代码错误导致的泄漏连接撑爆
                if is_leaked {
                    let leak_timeout = self.config.connection_leak_timeout;
                    if !leak_timeout.is_zero() {
                        // 获取具体的泄漏时间
                        if let Some(leaked_duration) = conn.get_leaked_duration() {
                            // 如果泄漏时间超过配置的 2 倍，强制驱逐
                            if leaked_duration > leak_timeout * 2 {
                                if conn.report_leak_once() {
                                    if let Some(stats) = &self.stats_collector {
                                        stats.increment_leaked_connections();
                                    }
                                }
                                eprintln!(
                                    "警告: 强制驱逐严重泄漏的连接 ID {} (泄漏时间: {:?})",
                                    conn.id, leaked_duration
                                );
                                // 强制移除泄漏连接，防止内存无限增长
                                let _ = self.remove_connection(&conn);
                                continue;
                            }
                        }
                    }

                    if conn.report_leak_once() {
                        if let Some(stats) = &self.stats_collector {
                            stats.increment_leaked_connections();
                        }
                    }
                    conn.mark_unhealthy();
                }
                if is_expired {
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
            // 使用 CAS 操作原子地检查和增加计数器，避免竞态条件
            let max_idle = self.config.max_idle_connections;
            loop {
                let current = self.idle_counts[idx].load(Ordering::Relaxed);
                if current >= max_idle {
                    // 超过最大空闲连接数，直接移除
                    let _ = self.remove_connection(&conn);
                    break;
                }
                // 尝试原子地增加计数器
                match self.idle_counts[idx].compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // CAS 成功，推入队列
                        self.idle_connections[idx].push(conn.clone());

                        if let Some(stats) = &self.stats_collector {
                            self.update_stats_on_idle_push(stats, &conn);
                        }
                        break;
                    }
                    Err(_) => {
                        // CAS 失败，其他线程修改了计数器，重试
                        continue;
                    }
                }
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

            // 优化：减少检查次数，因为无锁队列不支持高效查找
            // 主要依赖 return_connection 时的 is_connection_valid_for_borrow 检查来过滤无效连接
            const MAX_CHECK: usize = 10; // 最多检查 10 个连接，避免高并发下的性能问题
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
