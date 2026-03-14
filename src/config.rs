// Copyright (c) 2025, vistone
// All rights reserved.

use crate::errors::{NetConnPoolError, Result};
use crate::mode::PoolMode;
use crate::protocol::Protocol;
use std::net::{TcpStream, UdpSocket};
use std::time::Duration;

/// CloseConn 连接关闭回调类型
pub type CloseConnCallback = dyn Fn(&ConnectionType) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
    + Send
    + Sync;

/// OnCreated 连接创建后回调类型
pub type OnCreatedCallback = dyn Fn(&ConnectionType) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
    + Send
    + Sync;

/// OnBorrow/OnReturn 借出/归还回调类型
pub type BorrowReturnCallback = dyn Fn(&ConnectionType) + Send + Sync;

/// Dialer 连接创建函数类型（客户端模式）
/// 返回网络连接和错误
/// 参数 `Option<Protocol>` 表示调用方请求的协议，Dialer 应尽量满足
pub type Dialer = Box<
    dyn Fn(
            Option<Protocol>,
        ) -> std::result::Result<ConnectionType, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync,
>;

/// Acceptor 连接接受函数类型（服务器端模式）
/// 从Listener接受新连接，返回网络连接和错误
pub type Acceptor = Box<
    dyn Fn(
            &std::net::TcpListener,
        ) -> std::result::Result<TcpStream, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync,
>;

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
    pub mode: PoolMode,

    /// MaxConnections 最大连接数，0表示无限制
    pub max_connections: usize,

    /// MinConnections 最小连接数（预热连接数）
    pub min_connections: usize,

    /// MaxIdleConnections 最大空闲连接数
    pub max_idle_connections: usize,

    /// ConnectionTimeout 连接创建超时时间
    pub connection_timeout: Duration,

    /// IdleTimeout 空闲连接超时时间，超过此时间的空闲连接将被关闭
    pub idle_timeout: Duration,

    /// MaxLifetime 连接最大生命周期，超过此时间的连接将被关闭
    pub max_lifetime: Duration,

    /// GetConnectionTimeout 获取连接的超时时间
    pub get_connection_timeout: Duration,

    /// HealthCheckInterval 健康检查间隔
    pub health_check_interval: Duration,

    /// HealthCheckTimeout 健康检查超时时间
    pub health_check_timeout: Duration,

    /// ConnectionLeakTimeout 连接泄漏检测超时时间
    /// 如果连接在此时间内未归还，将触发泄漏警告
    pub connection_leak_timeout: Duration,

    /// Dialer 连接创建函数（客户端模式必需）
    /// 在客户端模式下，用于主动创建连接到服务器
    pub dialer: Option<Dialer>,

    /// Listener 网络监听器（服务器端模式必需）
    /// 在服务器端模式下，用于接受客户端连接
    pub listener: Option<std::net::TcpListener>,

    /// Acceptor 连接接受函数（服务器端模式可选）
    /// 在服务器端模式下，用于从Listener接受连接
    /// 如果为None，将使用默认的Accept方法
    pub acceptor: Option<Acceptor>,

    /// HealthChecker 健康检查函数（可选）
    /// 如果为None，将使用默认的ping检查
    pub health_checker: Option<HealthChecker>,

    /// CloseConn 连接关闭函数（可选）
    /// 如果为None，将尝试关闭连接
    pub close_conn: Option<Box<CloseConnCallback>>,

    /// OnCreated 连接创建后调用
    pub on_created: Option<Box<OnCreatedCallback>>,

    /// OnBorrow 连接从池中取出前调用
    pub on_borrow: Option<Box<BorrowReturnCallback>>,

    /// OnReturn 连接归还池中前调用
    pub on_return: Option<Box<BorrowReturnCallback>>,

    /// EnableStats 是否启用统计信息
    pub enable_stats: bool,

    /// EnableHealthCheck 是否启用健康检查
    pub enable_health_check: bool,

    /// ClearUDPBufferOnReturn 是否在归还UDP连接时清空读取缓冲区
    /// 启用此选项可以防止UDP连接复用时的数据混淆
    /// 默认值为true，建议保持启用
    pub clear_udp_buffer_on_return: bool,

    /// UDPBufferClearTimeout UDP缓冲区清理超时时间
    /// 如果为0，将使用默认值100ms
    pub udp_buffer_clear_timeout: Duration,

    /// MaxBufferClearPackets UDP缓冲区清理最大包数
    /// 默认值: 100
    pub max_buffer_clear_packets: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self::default_config()
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("mode", &self.mode)
            .field("max_connections", &self.max_connections)
            .field("min_connections", &self.min_connections)
            .field("max_idle_connections", &self.max_idle_connections)
            .field("connection_timeout", &self.connection_timeout)
            .field("idle_timeout", &self.idle_timeout)
            .field("max_lifetime", &self.max_lifetime)
            .field("get_connection_timeout", &self.get_connection_timeout)
            .field("health_check_interval", &self.health_check_interval)
            .field("health_check_timeout", &self.health_check_timeout)
            .field("connection_leak_timeout", &self.connection_leak_timeout)
            .field("dialer", &self.dialer.as_ref().map(|_| "..."))
            .field("listener", &self.listener)
            .field("acceptor", &self.acceptor.as_ref().map(|_| "..."))
            .field(
                "health_checker",
                &self.health_checker.as_ref().map(|_| "..."),
            )
            .field("close_conn", &self.close_conn.as_ref().map(|_| "..."))
            .field("on_created", &self.on_created.as_ref().map(|_| "..."))
            .field("on_borrow", &self.on_borrow.as_ref().map(|_| "..."))
            .field("on_return", &self.on_return.as_ref().map(|_| "..."))
            .field("enable_stats", &self.enable_stats)
            .field("enable_health_check", &self.enable_health_check)
            .field(
                "clear_udp_buffer_on_return",
                &self.clear_udp_buffer_on_return,
            )
            .field("udp_buffer_clear_timeout", &self.udp_buffer_clear_timeout)
            .field("max_buffer_clear_packets", &self.max_buffer_clear_packets)
            .finish()
    }
}

/// default_config 返回默认配置（客户端模式）
pub fn default_config() -> Config {
    Config::default_config()
}

/// default_server_config 返回默认服务器端配置
pub fn default_server_config() -> Config {
    Config::default_server_config()
}

impl Config {
    /// default_config 返回默认配置（客户端模式）
    pub fn default_config() -> Self {
        Self {
            mode: PoolMode::Client,
            max_connections: 10,
            min_connections: 2,
            max_idle_connections: 10,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(5 * 60),
            max_lifetime: Duration::from_secs(30 * 60),
            get_connection_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(3),
            connection_leak_timeout: Duration::from_secs(5 * 60),
            dialer: None,
            listener: None,
            acceptor: None,
            health_checker: None,
            close_conn: None,
            on_created: None,
            on_borrow: None,
            on_return: None,
            enable_stats: true,
            enable_health_check: true,
            clear_udp_buffer_on_return: true,
            udp_buffer_clear_timeout: Duration::from_millis(100),
            max_buffer_clear_packets: 100,
        }
    }

    /// default_server_config 返回默认服务器端配置
    pub fn default_server_config() -> Self {
        Self {
            mode: PoolMode::Server,
            max_connections: 100, // 服务器端通常需要更多连接
            min_connections: 0,   // 服务器端通常不需要预热
            max_idle_connections: 50,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(5 * 60),
            max_lifetime: Duration::from_secs(30 * 60),
            get_connection_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(3),
            connection_leak_timeout: Duration::from_secs(5 * 60),
            dialer: None,
            listener: None,
            acceptor: None,
            health_checker: None,
            close_conn: None,
            on_created: None,
            on_borrow: None,
            on_return: None,
            enable_stats: true,
            enable_health_check: true,
            clear_udp_buffer_on_return: true,
            udp_buffer_clear_timeout: Duration::from_millis(100),
            max_buffer_clear_packets: 100,
        }
    }

    /// Validate 验证配置有效性
    pub fn validate(&self) -> Result<()> {
        // 根据模式验证必需的配置
        match self.mode {
            PoolMode::Client => {
                // 客户端模式需要Dialer
                if self.dialer.is_none() {
                    return Err(NetConnPoolError::InvalidConfig {
                        reason: "客户端模式需要 Dialer".to_string(),
                    });
                }
            }
            PoolMode::Server => {
                // 服务器端模式需要Listener
                if self.listener.is_none() {
                    return Err(NetConnPoolError::InvalidConfig {
                        reason: "服务器端模式需要 Listener".to_string(),
                    });
                }
            }
        }

        if self.min_connections > 0
            && self.max_connections > 0
            && self.min_connections > self.max_connections
        {
            return Err(NetConnPoolError::InvalidConfig {
                reason: format!(
                    "min_connections ({}) 不能大于 max_connections ({})",
                    self.min_connections, self.max_connections
                ),
            });
        }
        if self.max_idle_connections == 0 {
            return Err(NetConnPoolError::InvalidConfig {
                reason: "max_idle_connections 必须大于 0".to_string(),
            });
        }
        if self.connection_timeout.is_zero() {
            return Err(NetConnPoolError::InvalidConfig {
                reason: "connection_timeout 必须大于 0".to_string(),
            });
        }

        // 添加更多验证
        if self.max_idle_connections > 0
            && self.max_connections > 0
            && self.max_idle_connections > self.max_connections
        {
            return Err(NetConnPoolError::InvalidConfig {
                reason: format!(
                    "max_idle_connections ({}) 不能大于 max_connections ({})",
                    self.max_idle_connections, self.max_connections
                ),
            });
        }

        if self.idle_timeout > self.max_lifetime {
            return Err(NetConnPoolError::InvalidConfig {
                reason: format!(
                    "idle_timeout ({:?}) 不能大于 max_lifetime ({:?})",
                    self.idle_timeout, self.max_lifetime
                ),
            });
        }

        if self.health_check_timeout > self.health_check_interval {
            return Err(NetConnPoolError::InvalidConfig {
                reason: format!(
                    "health_check_timeout ({:?}) 不能大于 health_check_interval ({:?})",
                    self.health_check_timeout, self.health_check_interval
                ),
            });
        }
        Ok(())
    }

    /// apply_defaults 应用默认值并修正不合理的配置
    pub fn apply_defaults(&mut self) {
        if self.mode == PoolMode::Server && self.acceptor.is_none() {
            self.acceptor = Some(Box::new(default_acceptor));
        }
        if self.max_idle_connections > 0
            && self.max_connections > 0
            && self.max_idle_connections > self.max_connections
        {
            self.max_idle_connections = self.max_connections;
        }
        if !self.health_check_interval.is_zero()
            && self.health_check_timeout > self.health_check_interval
        {
            self.health_check_timeout = self.health_check_interval / 2;
        }
        if self.max_buffer_clear_packets == 0 {
            self.max_buffer_clear_packets = 100;
        }
    }
}

/// default_acceptor 默认的连接接受函数
fn default_acceptor(
    listener: &std::net::TcpListener,
) -> std::result::Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    listener
        .accept()
        .map(|(stream, _)| stream)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}

/// ConfigBuilder 用于构建 Config 的配置构建器
///
/// 提供流畅的 API 来创建连接池配置。
///
/// # 示例
/// ```rust,no_run
/// use netconnpool::{ConfigBuilder, PoolMode};
/// use std::net::TcpStream;
///
/// let config = ConfigBuilder::new()
///     .mode(PoolMode::Client)
///     .max_connections(100)
///     .min_connections(10)
///     .dialer(Box::new(|_| {
///         TcpStream::connect("127.0.0.1:8080")
///             .map(|s| netconnpool::ConnectionType::Tcp(s))
///             .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
///     }))
///     .build()
///     .unwrap();
/// ```
pub struct ConfigBuilder {
    config: Config,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigBuilder {
    /// 创建新的 ConfigBuilder，使用客户端模式的默认值
    pub fn new() -> Self {
        Self {
            config: Config::default_config(),
        }
    }

    /// 创建新的 ConfigBuilder，使用服务器端模式的默认值
    pub fn new_server() -> Self {
        Self {
            config: Config::default_server_config(),
        }
    }

    /// 设置连接池模式
    pub fn mode(mut self, mode: PoolMode) -> Self {
        self.config.mode = mode;
        self
    }

    /// 设置最大连接数
    pub fn max_connections(mut self, max_connections: usize) -> Self {
        self.config.max_connections = max_connections;
        self
    }

    /// 设置最小连接数（预热连接数）
    pub fn min_connections(mut self, min_connections: usize) -> Self {
        self.config.min_connections = min_connections;
        self
    }

    /// 设置最大空闲连接数
    pub fn max_idle_connections(mut self, max_idle_connections: usize) -> Self {
        self.config.max_idle_connections = max_idle_connections;
        self
    }

    /// 设置连接创建超时时间
    pub fn connection_timeout(mut self, connection_timeout: Duration) -> Self {
        self.config.connection_timeout = connection_timeout;
        self
    }

    /// 设置空闲连接超时时间
    pub fn idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.config.idle_timeout = idle_timeout;
        self
    }

    /// 设置连接最大生命周期
    pub fn max_lifetime(mut self, max_lifetime: Duration) -> Self {
        self.config.max_lifetime = max_lifetime;
        self
    }

    /// 设置获取连接的超时时间
    pub fn get_connection_timeout(mut self, get_connection_timeout: Duration) -> Self {
        self.config.get_connection_timeout = get_connection_timeout;
        self
    }

    /// 设置健康检查间隔
    pub fn health_check_interval(mut self, health_check_interval: Duration) -> Self {
        self.config.health_check_interval = health_check_interval;
        self
    }

    /// 设置健康检查超时时间
    pub fn health_check_timeout(mut self, health_check_timeout: Duration) -> Self {
        self.config.health_check_timeout = health_check_timeout;
        self
    }

    /// 设置连接泄漏检测超时时间
    pub fn connection_leak_timeout(mut self, connection_leak_timeout: Duration) -> Self {
        self.config.connection_leak_timeout = connection_leak_timeout;
        self
    }

    /// 设置连接创建函数（客户端模式）
    pub fn dialer(mut self, dialer: Dialer) -> Self {
        self.config.dialer = Some(dialer);
        self
    }

    /// 设置网络监听器（服务器端模式）
    pub fn listener(mut self, listener: std::net::TcpListener) -> Self {
        self.config.listener = Some(listener);
        self
    }

    /// 设置连接接受函数（服务器端模式）
    pub fn acceptor(mut self, acceptor: Acceptor) -> Self {
        self.config.acceptor = Some(acceptor);
        self
    }

    /// 设置健康检查函数
    pub fn health_checker(mut self, health_checker: HealthChecker) -> Self {
        self.config.health_checker = Some(health_checker);
        self
    }

    /// 设置连接关闭函数
    pub fn close_conn(mut self, close_conn: Box<CloseConnCallback>) -> Self {
        self.config.close_conn = Some(close_conn);
        self
    }

    /// 设置连接创建后回调
    pub fn on_created(mut self, on_created: Box<OnCreatedCallback>) -> Self {
        self.config.on_created = Some(on_created);
        self
    }

    /// 设置连接借出前回调
    pub fn on_borrow(mut self, on_borrow: Box<BorrowReturnCallback>) -> Self {
        self.config.on_borrow = Some(on_borrow);
        self
    }

    /// 设置连接归还前回调
    pub fn on_return(mut self, on_return: Box<BorrowReturnCallback>) -> Self {
        self.config.on_return = Some(on_return);
        self
    }

    /// 设置是否启用统计信息
    pub fn enable_stats(mut self, enable_stats: bool) -> Self {
        self.config.enable_stats = enable_stats;
        self
    }

    /// 设置是否启用健康检查
    pub fn enable_health_check(mut self, enable_health_check: bool) -> Self {
        self.config.enable_health_check = enable_health_check;
        self
    }

    /// 设置是否在归还 UDP 连接时清空读取缓冲区
    pub fn clear_udp_buffer_on_return(mut self, clear_udp_buffer_on_return: bool) -> Self {
        self.config.clear_udp_buffer_on_return = clear_udp_buffer_on_return;
        self
    }

    /// 设置 UDP 缓冲区清理超时时间
    pub fn udp_buffer_clear_timeout(mut self, udp_buffer_clear_timeout: Duration) -> Self {
        self.config.udp_buffer_clear_timeout = udp_buffer_clear_timeout;
        self
    }

    /// 设置 UDP 缓冲区清理最大包数
    pub fn max_buffer_clear_packets(mut self, max_buffer_clear_packets: usize) -> Self {
        self.config.max_buffer_clear_packets = max_buffer_clear_packets;
        self
    }

    /// 构建并验证配置
    ///
    /// # 返回值
    /// - `Ok(Config)`: 配置有效，返回构建好的配置
    /// - `Err(NetConnPoolError)`: 配置无效，返回错误
    pub fn build(mut self) -> Result<Config> {
        self.config.apply_defaults();
        self.config.validate()?;
        Ok(self.config)
    }

    /// 构建配置但不验证（用于测试场景）
    pub fn build_unchecked(mut self) -> Config {
        self.config.apply_defaults();
        self.config
    }
}
