// Copyright (c) 2025, vistone
// All rights reserved.

use std::io;
use thiserror::Error;

/// 连接池相关错误定义
#[derive(Error, Debug)]
pub enum NetConnPoolError {
    #[error("连接池已关闭")]
    PoolClosed,

    #[error("连接已关闭 (connection_id: {connection_id})")]
    ConnectionClosed { connection_id: u64 },

    #[error("获取连接超时 (timeout: {timeout:?}, waited: {waited:?})")]
    GetConnectionTimeout {
        timeout: std::time::Duration,
        waited: std::time::Duration,
    },

    #[error("已达到最大连接数限制 (current: {current}, max: {max})")]
    MaxConnectionsReached { current: usize, max: usize },

    #[error("无效连接 (connection_id: {connection_id}, reason: {reason})")]
    InvalidConnection { connection_id: u64, reason: String },

    #[error("连接不健康 (connection_id: {connection_id})")]
    ConnectionUnhealthy { connection_id: u64 },

    #[error("配置参数无效: {reason}")]
    InvalidConfig { reason: String },

    #[error("连接泄漏检测：连接未在超时时间内归还 (connection_id: {connection_id}, timeout: {timeout:?})")]
    ConnectionLeaked {
        connection_id: u64,
        timeout: std::time::Duration,
    },

    #[error("连接池已耗尽，无法创建新连接 (current: {current}, max: {max})")]
    PoolExhausted { current: usize, max: usize },

    #[error("不支持的IP版本: {version:?}")]
    UnsupportedIPVersion { version: String },

    #[error("指定IP版本没有可用连接 (required: {required:?})")]
    NoConnectionForIPVersion { required: String },

    #[error("不支持的协议类型: {protocol:?}")]
    UnsupportedProtocol { protocol: String },

    #[error("指定协议没有可用连接 (required: {required:?})")]
    NoConnectionForProtocol { required: String },

    #[error("IO错误: {0}")]
    IoError(#[from] io::Error),
}

impl PartialEq for NetConnPoolError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PoolClosed, Self::PoolClosed) => true,
            (
                Self::ConnectionClosed { connection_id: id1 },
                Self::ConnectionClosed { connection_id: id2 },
            ) => id1 == id2,
            (Self::GetConnectionTimeout { .. }, Self::GetConnectionTimeout { .. }) => true,
            (
                Self::MaxConnectionsReached {
                    current: c1,
                    max: m1,
                },
                Self::MaxConnectionsReached {
                    current: c2,
                    max: m2,
                },
            ) => c1 == c2 && m1 == m2,
            (
                Self::InvalidConnection {
                    connection_id: id1, ..
                },
                Self::InvalidConnection {
                    connection_id: id2, ..
                },
            ) => id1 == id2,
            (
                Self::ConnectionUnhealthy { connection_id: id1 },
                Self::ConnectionUnhealthy { connection_id: id2 },
            ) => id1 == id2,
            (Self::InvalidConfig { reason: r1 }, Self::InvalidConfig { reason: r2 }) => r1 == r2,
            (
                Self::ConnectionLeaked {
                    connection_id: id1, ..
                },
                Self::ConnectionLeaked {
                    connection_id: id2, ..
                },
            ) => id1 == id2,
            (
                Self::PoolExhausted {
                    current: c1,
                    max: m1,
                },
                Self::PoolExhausted {
                    current: c2,
                    max: m2,
                },
            ) => c1 == c2 && m1 == m2,
            (
                Self::UnsupportedIPVersion { version: v1 },
                Self::UnsupportedIPVersion { version: v2 },
            ) => v1 == v2,
            (
                Self::NoConnectionForIPVersion { required: r1 },
                Self::NoConnectionForIPVersion { required: r2 },
            ) => r1 == r2,
            (
                Self::UnsupportedProtocol { protocol: p1 },
                Self::UnsupportedProtocol { protocol: p2 },
            ) => p1 == p2,
            (
                Self::NoConnectionForProtocol { required: r1 },
                Self::NoConnectionForProtocol { required: r2 },
            ) => r1 == r2,
            (Self::IoError(e1), Self::IoError(e2)) => e1.kind() == e2.kind(),
            _ => false,
        }
    }
}

/// 连接池相关错误类型别名
pub type Result<T> = std::result::Result<T, NetConnPoolError>;
