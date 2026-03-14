// Copyright (c) 2025, vistone
// All rights reserved.

//! PooledConnection 模块
//!
//! 提供自动归还的连接包装器，实现 RAII 机制。

use crate::connection::Connection;
use super::PoolInner;
use std::ops::Deref;
use std::sync::{Arc, Weak};

/// PooledConnection 自动归还的连接包装器
/// 实现 RAII 机制，Drop 时自动归还连接到池中
#[derive(Debug)]
pub struct PooledConnection {
    pub(super) conn: Arc<Connection>,
    pub(super) pool: Weak<PoolInner>,
}

impl PooledConnection {
    /// 创建新的 PooledConnection
    pub(crate) fn new(conn: Arc<Connection>, pool: Weak<PoolInner>) -> Self {
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
