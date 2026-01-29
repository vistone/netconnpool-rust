// Copyright (c) 2025, vistone
// All rights reserved.

use crate::config::ConnectionType;
use crate::ipversion::{detect_ip_version, IPVersion};
use crate::protocol::Protocol;
use std::net::{TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static CONNECTION_ID_GENERATOR: AtomicU64 = AtomicU64::new(1);

/// on_close 关闭回调类型
pub type OnCloseCallback =
    dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync;

/// Connection 连接封装
pub struct Connection {
    /// ID 连接唯一标识符
    pub id: u64,

    /// Conn 底层连接对象（TCP或UDP）
    conn: ConnectionType,

    /// Protocol 协议类型（TCP或UDP）
    pub protocol: Protocol,

    /// IPVersion IP版本（IPv4或IPv6）
    pub ip_version: IPVersion,

    /// CreatedAt 创建时间
    created_at: Instant,

    /// LastUsedAt 最后使用时间（使用 AtomicU64 存储 UNIX 时间戳纳秒）
    last_used_at: AtomicU64,

    /// LastHealthCheckAt 最后健康检查时间（使用 AtomicU64 存储 UNIX 时间戳纳秒）
    last_health_check_at: AtomicU64,

    /// IsHealthy 是否健康
    is_healthy: AtomicBool,

    /// Closed 是否已关闭（用于 close 幂等）
    closed: AtomicBool,

    /// InUse 是否正在使用中
    in_use: AtomicBool,

    /// ReuseCount 连接复用次数
    reuse_count: AtomicI64,

    /// leak_reported 是否已上报过泄漏（避免重复计数）
    leak_reported: AtomicBool,

    /// on_close 关闭回调
    ///
    /// 如果提供了此回调，连接池在关闭连接时将调用此函数，并**跳过默认的关闭逻辑**。
    /// 用户需要负责在回调中正确关闭底层网络连接（例如对于 TCP 调用 shutdown）。
    on_close: Option<Box<OnCloseCallback>>,
}

use std::fmt;

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("id", &self.id)
            .field("conn", &self.conn)
            .field("protocol", &self.protocol)
            .field("ip_version", &self.ip_version)
            .field("created_at", &self.created_at)
            .field("last_used_at", &self.last_used_at)
            .field("last_health_check_at", &self.last_health_check_at)
            .field("is_healthy", &self.is_healthy)
            .field("in_use", &self.in_use)
            .field("reuse_count", &self.reuse_count)
            .finish()
    }
}

impl Connection {
    /// 获取当前时间的 UNIX 时间戳（纳秒）
    /// 注意：在2262年之后，时间戳会超过u64的最大值，这里使用截断处理
    #[inline]
    fn now_nanos() -> u64 {
        // 使用 try_into 安全转换，如果溢出则截断到 u64::MAX
        // 这对于当前日期（2025年）是安全的，时间戳纳秒值约为 1.7e18，远小于 u64::MAX (1.8e19)
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .min(u64::MAX as u128) as u64
    }

    /// NewConnection 创建新连接
    pub fn new(conn: ConnectionType, on_close: Option<Box<OnCloseCallback>>) -> Self {
        let now = Instant::now();
        let protocol = match &conn {
            ConnectionType::Tcp(_) => Protocol::TCP,
            ConnectionType::Udp(_) => Protocol::UDP,
        };

        let ip_version = match &conn {
            ConnectionType::Tcp(s) => s.peer_addr().or_else(|_| s.local_addr()),
            ConnectionType::Udp(s) => s.peer_addr().or_else(|_| s.local_addr()),
        }
        .map(|addr| detect_ip_version(&addr))
        .unwrap_or(IPVersion::Unknown);

        // 安全地生成连接 ID，检测溢出
        let id = loop {
            let old = CONNECTION_ID_GENERATOR.load(Ordering::Relaxed);
            if let Some(new) = old.checked_add(1) {
                if CONNECTION_ID_GENERATOR
                    .compare_exchange_weak(old, new, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    break new;
                }
                // CAS 失败，重试
                continue;
            } else {
                // 溢出：使用基于时间戳的 ID，避免与已有连接冲突
                // 使用纳秒时间戳的低 48 位 + 高 16 位的哈希值，确保唯一性
                let timestamp_nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
                    .min(u64::MAX as u128) as u64;
                // 使用时间戳作为基础 ID，并加上一个小的随机偏移
                let timestamp_based_id =
                    (timestamp_nanos & 0x0000_FFFF_FFFF_FFFF) | 0x0001_0000_0000_0000;
                eprintln!(
                    "警告: 连接 ID 生成器溢出，使用基于时间戳的 ID: {}",
                    timestamp_based_id
                );
                // 更新生成器为时间戳 ID，这样后续的 ID 会从时间戳开始递增
                if CONNECTION_ID_GENERATOR
                    .compare_exchange_weak(
                        old,
                        timestamp_based_id,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    break timestamp_based_id;
                }
                // CAS 失败，重试
                continue;
            }
        };

        // 将当前时间转换为 UNIX 时间戳（纳秒）
        // 使用安全转换，避免溢出（对于当前日期是安全的）
        let system_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .min(u64::MAX as u128) as u64;

        Self {
            id,
            conn,
            protocol,
            ip_version,
            created_at: now,
            last_used_at: AtomicU64::new(system_now),
            last_health_check_at: AtomicU64::new(system_now),
            is_healthy: AtomicBool::new(true),
            closed: AtomicBool::new(false),
            in_use: AtomicBool::new(false),
            reuse_count: AtomicI64::new(0),
            leak_reported: AtomicBool::new(false),
            on_close,
        }
    }

    /// NewConnectionFromTcp 从TCP流创建连接
    pub fn new_from_tcp(stream: TcpStream, on_close: Option<Box<OnCloseCallback>>) -> Self {
        Self::new(ConnectionType::Tcp(stream), on_close)
    }

    /// NewConnectionFromUdp 从UDP套接字创建连接
    pub fn new_from_udp(socket: UdpSocket, on_close: Option<Box<OnCloseCallback>>) -> Self {
        Self::new(ConnectionType::Udp(socket), on_close)
    }

    /// connection_type 获取连接类型引用
    pub fn connection_type(&self) -> &ConnectionType {
        &self.conn
    }

    /// GetProtocol 获取连接的协议类型
    pub fn get_protocol(&self) -> Protocol {
        self.protocol
    }

    /// GetIPVersion 获取连接的IP版本
    pub fn get_ip_version(&self) -> IPVersion {
        self.ip_version
    }

    /// GetConn 获取底层连接对象（TCP流）
    pub fn tcp_conn(&self) -> Option<&TcpStream> {
        match &self.conn {
            ConnectionType::Tcp(stream) => Some(stream),
            _ => None,
        }
    }

    /// GetUdpConn 获取底层连接对象（UDP套接字）
    pub fn udp_conn(&self) -> Option<&UdpSocket> {
        match &self.conn {
            ConnectionType::Udp(socket) => Some(socket),
            _ => None,
        }
    }

    /// MarkInUse 标记为使用中
    pub fn mark_in_use(&self) {
        self.in_use.store(true, Ordering::Release);
        self.last_used_at
            .store(Self::now_nanos(), Ordering::Release);
    }

    /// MarkIdle 标记为空闲
    pub fn mark_idle(&self) {
        self.in_use.store(false, Ordering::Release);
        self.last_used_at
            .store(Self::now_nanos(), Ordering::Release);
    }

    /// TryMarkIdle 尝试标记为空闲，并返回之前是否为使用中状态（原子操作）
    pub fn try_mark_idle(&self) -> bool {
        let was_in_use = self.in_use.swap(false, Ordering::Acquire);
        if was_in_use {
            self.last_used_at
                .store(Self::now_nanos(), Ordering::Release);
        }
        was_in_use
    }

    /// UpdateHealth 更新健康状态
    pub fn update_health(&self, healthy: bool) {
        self.is_healthy.store(healthy, Ordering::Release);
        if healthy {
            self.last_health_check_at
                .store(Self::now_nanos(), Ordering::Release);
        }
    }

    /// mark_unhealthy 仅标记为不健康（不主动关闭）
    pub fn mark_unhealthy(&self) {
        self.is_healthy.store(false, Ordering::Release);
    }

    /// should_health_check 判断是否需要执行健康检查
    pub fn should_health_check(&self, interval: Duration) -> bool {
        if interval.is_zero() {
            return false;
        }
        let last_nanos = self.last_health_check_at.load(Ordering::Acquire);
        let now_nanos = Self::now_nanos();
        if now_nanos >= last_nanos {
            Duration::from_nanos(now_nanos - last_nanos) >= interval
        } else {
            true // 时间戳异常，触发健康检查
        }
    }

    /// report_leak_once 返回是否是首次上报泄漏
    pub fn report_leak_once(&self) -> bool {
        !self.leak_reported.swap(true, Ordering::AcqRel)
    }

    /// IsExpired 检查连接是否过期（超过MaxLifetime）
    pub fn is_expired(&self, max_lifetime: Duration) -> bool {
        if max_lifetime.is_zero() {
            return false;
        }
        Instant::now().duration_since(self.created_at) > max_lifetime
    }

    /// IsIdleExpired 检查连接是否空闲太久（超过IdleTimeout）
    pub fn is_idle_expired(&self, idle_timeout: Duration) -> bool {
        if idle_timeout.is_zero() {
            return false;
        }
        if self.in_use.load(Ordering::Acquire) {
            return false;
        }
        let last_nanos = self.last_used_at.load(Ordering::Acquire);
        let now_nanos = Self::now_nanos();
        if now_nanos >= last_nanos {
            Duration::from_nanos(now_nanos - last_nanos) > idle_timeout
        } else {
            false // 时间戳异常，不认为过期
        }
    }

    /// IsLeaked 检查连接是否泄漏（超过ConnectionLeakTimeout且仍在使用时）
    pub fn is_leaked(&self, leak_timeout: Duration) -> bool {
        if leak_timeout.is_zero() || !self.in_use.load(Ordering::Acquire) {
            return false;
        }
        let last_nanos = self.last_used_at.load(Ordering::Acquire);
        let now_nanos = Self::now_nanos();
        if now_nanos >= last_nanos {
            Duration::from_nanos(now_nanos - last_nanos) > leak_timeout
        } else {
            false // 时间戳异常，不认为泄漏
        }
    }

    /// GetLeakedDuration 获取连接的泄漏时间（如果泄漏）
    /// 返回 None 表示未泄漏，Some(Duration) 表示泄漏的时间
    pub fn get_leaked_duration(&self) -> Option<Duration> {
        if !self.in_use.load(Ordering::Acquire) {
            return None;
        }
        let last_nanos = self.last_used_at.load(Ordering::Acquire);
        let now_nanos = Self::now_nanos();
        if now_nanos >= last_nanos {
            Some(Duration::from_nanos(now_nanos - last_nanos))
        } else {
            None // 时间戳异常
        }
    }

    /// Close 关闭连接
    ///
    /// 如果配置了 `on_close` 回调，将执行回调并直接返回。
    /// 否则，将执行默认关闭策略：TCP 执行 shutdown，UDP 依赖 Drop 物理关闭。
    pub fn close(&self) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        if let Some(on_close) = &self.on_close {
            // 执行用户自定义关闭逻辑。注意：此时默认关闭逻辑（如 TCP shutdown）将被跳过，
            // 用户需确保在回调内部处理了连接实体的关闭。
            on_close()?;
            self.is_healthy.store(false, Ordering::Release);
            return Ok(());
        }

        // 默认关闭策略：TCP 做 shutdown；UDP 无显式 close（drop 时关闭）
        match &self.conn {
            ConnectionType::Tcp(stream) => {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
            ConnectionType::Udp(_) => {}
        }

        self.is_healthy.store(false, Ordering::Release);
        Ok(())
    }

    /// GetAge 获取连接年龄
    pub fn age(&self) -> Duration {
        Instant::now().duration_since(self.created_at)
    }

    /// GetIdleTime 获取空闲时间
    pub fn idle_time(&self) -> Duration {
        if self.in_use.load(Ordering::Acquire) {
            return Duration::ZERO;
        }
        let last_nanos = self.last_used_at.load(Ordering::Acquire);
        let now_nanos = Self::now_nanos();
        if now_nanos >= last_nanos {
            Duration::from_nanos(now_nanos - last_nanos)
        } else {
            Duration::ZERO // 时间戳异常，返回零时长
        }
    }

    /// IncrementReuseCount 增加复用次数
    pub fn increment_reuse_count(&self) {
        self.reuse_count.fetch_add(1, Ordering::Relaxed);
    }

    /// GetReuseCount 获取复用次数
    pub fn reuse_count(&self) -> i64 {
        self.reuse_count.load(Ordering::Relaxed)
    }

    /// IsInUse 检查连接是否正在使用中（线程安全）
    pub fn is_in_use(&self) -> bool {
        self.in_use.load(Ordering::Acquire)
    }

    /// GetHealthStatus 获取连接健康状态（线程安全）
    pub fn health_status(&self) -> bool {
        self.is_healthy.load(Ordering::Acquire)
    }

    /// is_closed 查询是否已关闭
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
