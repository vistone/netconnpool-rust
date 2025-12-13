// Copyright (c) 2025, vistone
// All rights reserved.

use crate::errors::{NetConnPoolError, Result};
use crate::mode::PoolMode;
use std::net::{TcpStream, UdpSocket};
use std::time::Duration;

/// Dialer 连接创建函数类型（客户端模式）
/// 返回网络连接和错误
pub type Dialer = Box<dyn Fn() -> std::result::Result<ConnectionType, Box<dyn std::error::Error + Send + Sync>> + Send + Sync>;

/// Acceptor 连接接受函数类型（服务器端模式）
/// 从Listener接受新连接，返回网络连接和错误
pub type Acceptor = Box<dyn Fn(&std::net::TcpListener) -> std::result::Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> + Send + Sync>;

/// HealthChecker 健康检查函数类型
/// 返回连接是否健康
pub type HealthChecker = Box<dyn Fn(&ConnectionType) -> bool + Send + Sync>;

/// ConnectionType 连接类型（TCP或UDP）
#[derive(Debug)]
pub enum ConnectionType {
    Tcp(TcpStream),
    Udp(UdpSocket),
}

/// Config 连接池配置
pub struct Config {
    /// Mode 连接池模式：客户端或服务器端
    /// 默认值为PoolModeClient（客户端模式）
    pub Mode: PoolMode,

    /// MaxConnections 最大连接数，0表示无限制
    pub MaxConnections: usize,

    /// MinConnections 最小连接数（预热连接数）
    pub MinConnections: usize,

    /// MaxIdleConnections 最大空闲连接数
    pub MaxIdleConnections: usize,

    /// ConnectionTimeout 连接创建超时时间
    pub ConnectionTimeout: Duration,

    /// IdleTimeout 空闲连接超时时间，超过此时间的空闲连接将被关闭
    pub IdleTimeout: Duration,

    /// MaxLifetime 连接最大生命周期，超过此时间的连接将被关闭
    pub MaxLifetime: Duration,

    /// GetConnectionTimeout 获取连接的超时时间
    pub GetConnectionTimeout: Duration,

    /// HealthCheckInterval 健康检查间隔
    pub HealthCheckInterval: Duration,

    /// HealthCheckTimeout 健康检查超时时间
    pub HealthCheckTimeout: Duration,

    /// ConnectionLeakTimeout 连接泄漏检测超时时间
    /// 如果连接在此时间内未归还，将触发泄漏警告
    pub ConnectionLeakTimeout: Duration,

    /// Dialer 连接创建函数（客户端模式必需）
    /// 在客户端模式下，用于主动创建连接到服务器
    pub Dialer: Option<Dialer>,

    /// Listener 网络监听器（服务器端模式必需）
    /// 在服务器端模式下，用于接受客户端连接
    pub Listener: Option<std::net::TcpListener>,

    /// Acceptor 连接接受函数（服务器端模式可选）
    /// 在服务器端模式下，用于从Listener接受连接
    /// 如果为None，将使用默认的Accept方法
    pub Acceptor: Option<Acceptor>,

    /// HealthChecker 健康检查函数（可选）
    /// 如果为None，将使用默认的ping检查
    pub HealthChecker: Option<HealthChecker>,

    /// CloseConn 连接关闭函数（可选）
    /// 如果为None，将尝试关闭连接
    pub CloseConn: Option<Box<dyn Fn(&ConnectionType) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,

    /// OnCreated 连接创建后调用
    pub OnCreated: Option<Box<dyn Fn(&ConnectionType) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync>>,

    /// OnBorrow 连接从池中取出前调用
    pub OnBorrow: Option<Box<dyn Fn(&ConnectionType) + Send + Sync>>,

    /// OnReturn 连接归还池中前调用
    pub OnReturn: Option<Box<dyn Fn(&ConnectionType) + Send + Sync>>,

    /// EnableStats 是否启用统计信息
    pub EnableStats: bool,

    /// EnableHealthCheck 是否启用健康检查
    pub EnableHealthCheck: bool,

    /// ClearUDPBufferOnReturn 是否在归还UDP连接时清空读取缓冲区
    /// 启用此选项可以防止UDP连接复用时的数据混淆
    /// 默认值为true，建议保持启用
    pub ClearUDPBufferOnReturn: bool,

    /// UDPBufferClearTimeout UDP缓冲区清理超时时间
    /// 如果为0，将使用默认值100ms
    pub UDPBufferClearTimeout: Duration,

    /// MaxBufferClearPackets UDP缓冲区清理最大包数
    /// 默认值: 100
    pub MaxBufferClearPackets: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self::default_config()
    }
}

/// DefaultConfig 返回默认配置（客户端模式）
pub fn DefaultConfig() -> Config {
    Config::default_config()
}

/// DefaultServerConfig 返回默认服务器端配置
pub fn DefaultServerConfig() -> Config {
    Config::default_server_config()
}

impl Config {
    /// DefaultConfig 返回默认配置（客户端模式）
    pub fn default_config() -> Self {
        Self {
            Mode: PoolMode::Client,
            MaxConnections: 10,
            MinConnections: 2,
            MaxIdleConnections: 10,
            ConnectionTimeout: Duration::from_secs(10),
            IdleTimeout: Duration::from_secs(5 * 60),
            MaxLifetime: Duration::from_secs(30 * 60),
            GetConnectionTimeout: Duration::from_secs(5),
            HealthCheckInterval: Duration::from_secs(30),
            HealthCheckTimeout: Duration::from_secs(3),
            ConnectionLeakTimeout: Duration::from_secs(5 * 60),
            Dialer: None,
            Listener: None,
            Acceptor: None,
            HealthChecker: None,
            CloseConn: None,
            OnCreated: None,
            OnBorrow: None,
            OnReturn: None,
            EnableStats: true,
            EnableHealthCheck: true,
            ClearUDPBufferOnReturn: true,
            UDPBufferClearTimeout: Duration::from_millis(100),
            MaxBufferClearPackets: 100,
        }
    }

    /// DefaultServerConfig 返回默认服务器端配置
    pub fn default_server_config() -> Self {
        Self {
            Mode: PoolMode::Server,
            MaxConnections: 100, // 服务器端通常需要更多连接
            MinConnections: 0,   // 服务器端通常不需要预热
            MaxIdleConnections: 50,
            ConnectionTimeout: Duration::from_secs(10),
            IdleTimeout: Duration::from_secs(5 * 60),
            MaxLifetime: Duration::from_secs(30 * 60),
            GetConnectionTimeout: Duration::from_secs(5),
            HealthCheckInterval: Duration::from_secs(30),
            HealthCheckTimeout: Duration::from_secs(3),
            ConnectionLeakTimeout: Duration::from_secs(5 * 60),
            Dialer: None,
            Listener: None,
            Acceptor: None,
            HealthChecker: None,
            CloseConn: None,
            OnCreated: None,
            OnBorrow: None,
            OnReturn: None,
            EnableStats: true,
            EnableHealthCheck: true,
            ClearUDPBufferOnReturn: true,
            UDPBufferClearTimeout: Duration::from_millis(100),
            MaxBufferClearPackets: 100,
        }
    }

    /// Validate 验证配置有效性
    pub fn Validate(&mut self) -> Result<()> {
        // 根据模式验证必需的配置
        match self.Mode {
            PoolMode::Client => {
                // 客户端模式需要Dialer
                if self.Dialer.is_none() {
                    return Err(NetConnPoolError::InvalidConfig);
                }
            }
            PoolMode::Server => {
                // 服务器端模式需要Listener
                if self.Listener.is_none() {
                    return Err(NetConnPoolError::InvalidConfig);
                }
                // 如果未提供Acceptor，使用默认的Accept方法
                if self.Acceptor.is_none() {
                    self.Acceptor = Some(Box::new(default_acceptor));
                }
            }
        }

        if self.MinConnections > 0 && self.MaxConnections > 0 && self.MinConnections > self.MaxConnections {
            return Err(NetConnPoolError::InvalidConfig);
        }
        if self.MaxIdleConnections == 0 {
            return Err(NetConnPoolError::InvalidConfig);
        }
        if self.ConnectionTimeout.is_zero() {
            return Err(NetConnPoolError::InvalidConfig);
        }
        if self.MaxIdleConnections > 0 && self.MaxConnections > 0 && self.MaxIdleConnections > self.MaxConnections {
            // 最大空闲连接数不应超过最大连接数
            self.MaxIdleConnections = self.MaxConnections;
        }
        if !self.HealthCheckInterval.is_zero() && self.HealthCheckTimeout > self.HealthCheckInterval {
            // 健康检查超时不应超过检查间隔
            self.HealthCheckTimeout = self.HealthCheckInterval / 2;
        }
        if self.MaxBufferClearPackets == 0 {
            self.MaxBufferClearPackets = 100;
        }
        Ok(())
    }
}

/// default_acceptor 默认的连接接受函数
fn default_acceptor(listener: &std::net::TcpListener) -> std::result::Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    listener.accept().map(|(stream, _)| stream).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}
