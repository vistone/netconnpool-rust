// Copyright (c) 2025, vistone
// All rights reserved.

use thiserror::Error;
use std::io;

/// 连接池相关错误定义
#[derive(Error, Debug)]
pub enum NetConnPoolError {
    #[error("连接池已关闭")]
    PoolClosed,

    #[error("连接已关闭")]
    ConnectionClosed,

    #[error("获取连接超时")]
    GetConnectionTimeout,

    #[error("已达到最大连接数限制")]
    MaxConnectionsReached,

    #[error("无效连接")]
    InvalidConnection,

    #[error("连接不健康")]
    ConnectionUnhealthy,

    #[error("配置参数无效")]
    InvalidConfig,

    #[error("连接泄漏检测：连接未在超时时间内归还")]
    ConnectionLeaked,

    #[error("连接池已耗尽，无法创建新连接")]
    PoolExhausted,

    #[error("不支持的IP版本")]
    UnsupportedIPVersion,

    #[error("指定IP版本没有可用连接")]
    NoConnectionForIPVersion,

    #[error("不支持的协议类型")]
    UnsupportedProtocol,

    #[error("指定协议没有可用连接")]
    NoConnectionForProtocol,

    #[error("IO错误: {0}")]
    IoError(#[from] io::Error),
}

impl PartialEq for NetConnPoolError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PoolClosed, Self::PoolClosed) => true,
            (Self::ConnectionClosed, Self::ConnectionClosed) => true,
            (Self::GetConnectionTimeout, Self::GetConnectionTimeout) => true,
            (Self::MaxConnectionsReached, Self::MaxConnectionsReached) => true,
            (Self::InvalidConnection, Self::InvalidConnection) => true,
            (Self::ConnectionUnhealthy, Self::ConnectionUnhealthy) => true,
            (Self::InvalidConfig, Self::InvalidConfig) => true,
            (Self::ConnectionLeaked, Self::ConnectionLeaked) => true,
            (Self::PoolExhausted, Self::PoolExhausted) => true,
            (Self::UnsupportedIPVersion, Self::UnsupportedIPVersion) => true,
            (Self::NoConnectionForIPVersion, Self::NoConnectionForIPVersion) => true,
            (Self::UnsupportedProtocol, Self::UnsupportedProtocol) => true,
            (Self::NoConnectionForProtocol, Self::NoConnectionForProtocol) => true,
            (Self::IoError(e1), Self::IoError(e2)) => e1.kind() == e2.kind(),
            _ => false,
        }
    }
}

/// 连接池相关错误类型别名
pub type Result<T> = std::result::Result<T, NetConnPoolError>;

// 为了保持与原项目相同的错误名称，提供静态错误
pub use NetConnPoolError::*;
