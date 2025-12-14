// Copyright (c) 2025, vistone
// All rights reserved.

use crate::config::ConnectionType;
use crate::ipversion::{detect_ip_version, IPVersion};
use crate::protocol::Protocol;
use std::net::{TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static CONNECTION_ID_GENERATOR: AtomicU64 = AtomicU64::new(1);

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

    /// LastUsedAt 最后使用时间
    last_used_at: Arc<std::sync::Mutex<Instant>>,

    /// LastHealthCheckAt 最后健康检查时间
    last_health_check_at: Arc<std::sync::Mutex<Instant>>,

    /// IsHealthy 是否健康
    is_healthy: Arc<AtomicBool>,

    /// Closed 是否已关闭（用于 close 幂等）
    closed: Arc<AtomicBool>,

    /// InUse 是否正在使用中
    in_use: Arc<AtomicBool>,

    /// ReuseCount 连接复用次数
    reuse_count: Arc<AtomicI64>,

    /// leak_reported 是否已上报过泄漏（避免重复计数）
    leak_reported: Arc<AtomicBool>,

    /// on_close 关闭回调
    on_close: Option<
        Box<
            dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync,
        >,
    >,
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
    /// NewConnection 创建新连接
    pub fn new(
        conn: ConnectionType,
        on_close: Option<
            Box<
                dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                    + Send
                    + Sync,
            >,
        >,
    ) -> Self {
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

        Self {
            id: CONNECTION_ID_GENERATOR.fetch_add(1, Ordering::Relaxed),
            conn,
            protocol,
            ip_version,
            created_at: now,
            last_used_at: Arc::new(std::sync::Mutex::new(now)),
            last_health_check_at: Arc::new(std::sync::Mutex::new(now)),
            is_healthy: Arc::new(AtomicBool::new(true)),
            closed: Arc::new(AtomicBool::new(false)),
            in_use: Arc::new(AtomicBool::new(false)),
            reuse_count: Arc::new(AtomicI64::new(0)),
            leak_reported: Arc::new(AtomicBool::new(false)),
            on_close,
        }
    }

    /// NewConnectionFromTcp 从TCP流创建连接
    pub fn new_from_tcp(
        stream: TcpStream,
        on_close: Option<
            Box<
                dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                    + Send
                    + Sync,
            >,
        >,
    ) -> Self {
        Self::new(ConnectionType::Tcp(stream), on_close)
    }

    /// NewConnectionFromUdp 从UDP套接字创建连接
    pub fn new_from_udp(
        socket: UdpSocket,
        on_close: Option<
            Box<
                dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                    + Send
                    + Sync,
            >,
        >,
    ) -> Self {
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
        *self.last_used_at.lock().unwrap() = Instant::now();
    }

    /// MarkIdle 标记为空闲
    pub fn mark_idle(&self) {
        self.in_use.store(false, Ordering::Release);
        *self.last_used_at.lock().unwrap() = Instant::now();
    }

    /// UpdateHealth 更新健康状态
    pub fn update_health(&self, healthy: bool) {
        self.is_healthy.store(healthy, Ordering::Release);
        *self.last_health_check_at.lock().unwrap() = Instant::now();
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
        let last = *self.last_health_check_at.lock().unwrap();
        Instant::now().duration_since(last) >= interval
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
        let last_used = *self.last_used_at.lock().unwrap();
        Instant::now().duration_since(last_used) > idle_timeout
    }

    /// IsLeaked 检查连接是否泄漏（超过ConnectionLeakTimeout且仍在使用时）
    pub fn is_leaked(&self, leak_timeout: Duration) -> bool {
        if leak_timeout.is_zero() || !self.in_use.load(Ordering::Acquire) {
            return false;
        }
        let last_used = *self.last_used_at.lock().unwrap();
        Instant::now().duration_since(last_used) > leak_timeout
    }

    /// Close 关闭连接
    pub fn close(&self) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        if let Some(on_close) = &self.on_close {
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
        let last_used = *self.last_used_at.lock().unwrap();
        Instant::now().duration_since(last_used)
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
