// Copyright (c) 2025, vistone
// All rights reserved.

use crate::ipversion::{DetectIPVersion, IPVersion};
use crate::protocol::Protocol;
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static CONNECTION_ID_GENERATOR: AtomicU64 = AtomicU64::new(1);

/// Connection 连接封装
pub struct Connection {
    /// ID 连接唯一标识符
    pub ID: u64,

    /// Conn 底层连接对象（TCP或UDP）
    conn: ConnectionInner,

    /// Protocol 协议类型（TCP或UDP）
    pub Protocol: Protocol,

    /// IPVersion IP版本（IPv4或IPv6）
    pub IPVersion: IPVersion,

    /// CreatedAt 创建时间
    CreatedAt: Instant,

    /// LastUsedAt 最后使用时间
    LastUsedAt: Arc<std::sync::Mutex<Instant>>,

    /// LastHealthCheckAt 最后健康检查时间
    LastHealthCheckAt: Arc<std::sync::Mutex<Instant>>,

    /// IsHealthy 是否健康
    IsHealthy: Arc<AtomicBool>,

    /// InUse 是否正在使用中
    InUse: Arc<AtomicBool>,

    /// LeakDetected 是否检测到泄漏
    LeakDetected: Arc<AtomicBool>,

    /// ReuseCount 连接复用次数（从连接池中获取的次数）
    ReuseCount: Arc<AtomicI64>,

    /// on_close 关闭回调
    on_close: Option<Box<dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,
}

/// ConnectionInner 连接内部类型（TCP或UDP）
pub enum ConnectionInner {
    Tcp(TcpStream),
    Udp(UdpSocket),
}

impl ConnectionInner {
    fn local_addr(&self) -> io::Result<SocketAddr> {
        match self {
            ConnectionInner::Tcp(stream) => stream.local_addr(),
            ConnectionInner::Udp(socket) => socket.local_addr(),
        }
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self {
            ConnectionInner::Tcp(stream) => stream.peer_addr(),
            ConnectionInner::Udp(socket) => socket.peer_addr(),
        }
    }
}

use std::io;

impl Connection {
    /// NewConnection 创建新连接
    pub fn NewConnection(
        conn: ConnectionInner,
        on_close: Option<Box<dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,
    ) -> Self {
        let now = Instant::now();
        let protocol = match &conn {
            ConnectionInner::Tcp(_) => Protocol::TCP,
            ConnectionInner::Udp(_) => Protocol::UDP,
        };

        let ip_version = conn
            .peer_addr()
            .or_else(|_| conn.local_addr())
            .map(|addr| DetectIPVersion(&addr))
            .unwrap_or(IPVersion::Unknown);

        Self {
            ID: CONNECTION_ID_GENERATOR.fetch_add(1, Ordering::Relaxed),
            conn,
            Protocol: protocol,
            IPVersion: ip_version,
            CreatedAt: now,
            LastUsedAt: Arc::new(std::sync::Mutex::new(now)),
            LastHealthCheckAt: Arc::new(std::sync::Mutex::new(now)),
            IsHealthy: Arc::new(AtomicBool::new(true)),
            InUse: Arc::new(AtomicBool::new(false)),
            LeakDetected: Arc::new(AtomicBool::new(false)),
            ReuseCount: Arc::new(AtomicI64::new(0)),
            on_close,
        }
    }

    /// NewConnectionFromTcp 从TCP流创建连接
    pub fn NewConnectionFromTcp(
        stream: TcpStream,
        on_close: Option<Box<dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,
    ) -> Self {
        Self::NewConnection(ConnectionInner::Tcp(stream), on_close)
    }

    /// NewConnectionFromUdp 从UDP套接字创建连接
    pub fn NewConnectionFromUdp(
        socket: UdpSocket,
        on_close: Option<Box<dyn Fn() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,
    ) -> Self {
        Self::NewConnection(ConnectionInner::Udp(socket), on_close)
    }

    /// GetProtocol 获取连接的协议类型（无锁，因为Protocol在创建后不会改变）
    pub fn GetProtocol(&self) -> Protocol {
        self.Protocol
    }

    /// GetIPVersion 获取连接的IP版本（无锁，因为IPVersion在创建后不会改变）
    pub fn GetIPVersion(&self) -> IPVersion {
        self.IPVersion
    }

    /// GetConn 获取底层连接对象（TCP流）
    pub fn GetTcpConn(&self) -> Option<&TcpStream> {
        match &self.conn {
            ConnectionInner::Tcp(stream) => Some(stream),
            _ => None,
        }
    }

    /// GetUdpConn 获取底层连接对象（UDP套接字）
    pub fn GetUdpConn(&self) -> Option<&UdpSocket> {
        match &self.conn {
            ConnectionInner::Udp(socket) => Some(socket),
            _ => None,
        }
    }

    /// MarkInUse 标记为使用中
    pub fn MarkInUse(&self) {
        self.InUse.store(true, Ordering::Release);
        *self.LastUsedAt.lock().unwrap() = Instant::now();
    }

    /// MarkIdle 标记为空闲
    pub fn MarkIdle(&self) {
        self.InUse.store(false, Ordering::Release);
        *self.LastUsedAt.lock().unwrap() = Instant::now();
    }

    /// UpdateHealth 更新健康状态
    pub fn UpdateHealth(&self, healthy: bool) {
        self.IsHealthy.store(healthy, Ordering::Release);
        *self.LastHealthCheckAt.lock().unwrap() = Instant::now();
    }

    /// IsExpired 检查连接是否过期（超过MaxLifetime）
    pub fn IsExpired(&self, max_lifetime: Duration) -> bool {
        if max_lifetime.is_zero() {
            return false;
        }
        Instant::now().duration_since(self.CreatedAt) > max_lifetime
    }

    /// IsIdleTooLong 检查连接是否空闲太久（超过IdleTimeout）
    pub fn IsIdleTooLong(&self, idle_timeout: Duration) -> bool {
        if idle_timeout.is_zero() {
            return false;
        }
        if self.InUse.load(Ordering::Acquire) {
            return false;
        }
        let last_used = *self.LastUsedAt.lock().unwrap();
        Instant::now().duration_since(last_used) > idle_timeout
    }

    /// IsLeaked 检查连接是否泄漏（超过ConnectionLeakTimeout且仍在使用时）
    pub fn IsLeaked(&self, leak_timeout: Duration) -> bool {
        if leak_timeout.is_zero() || !self.InUse.load(Ordering::Acquire) {
            return false;
        }
        let last_used = *self.LastUsedAt.lock().unwrap();
        Instant::now().duration_since(last_used) > leak_timeout
    }

    /// Close 关闭连接
    pub fn Close(&self) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(on_close) = &self.on_close {
            on_close()?;
        }
        Ok(())
    }

    /// GetAge 获取连接年龄
    pub fn GetAge(&self) -> Duration {
        Instant::now().duration_since(self.CreatedAt)
    }

    /// GetIdleTime 获取空闲时间
    pub fn GetIdleTime(&self) -> Duration {
        if self.InUse.load(Ordering::Acquire) {
            return Duration::ZERO;
        }
        let last_used = *self.LastUsedAt.lock().unwrap();
        Instant::now().duration_since(last_used)
    }

    /// IncrementReuseCount 增加复用次数
    pub fn IncrementReuseCount(&self) {
        self.ReuseCount.fetch_add(1, Ordering::Relaxed);
    }

    /// GetReuseCount 获取复用次数
    pub fn GetReuseCount(&self) -> i64 {
        self.ReuseCount.load(Ordering::Relaxed)
    }

    /// IsInUse 检查连接是否正在使用中（线程安全）
    pub fn IsInUse(&self) -> bool {
        self.InUse.load(Ordering::Acquire)
    }

    /// GetHealthStatus 获取连接健康状态（线程安全）
    pub fn GetHealthStatus(&self) -> bool {
        self.IsHealthy.load(Ordering::Acquire)
    }
}
