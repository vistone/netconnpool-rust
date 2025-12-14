// Copyright (c) 2025, vistone
// All rights reserved.

/// PoolMode 连接池模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolMode {
    /// PoolModeClient 客户端模式：主动连接到服务器
    Client = 0,
    /// PoolModeServer 服务器端模式：接受客户端连接
    Server = 1,
}

impl Default for PoolMode {
    fn default() -> Self {
        PoolMode::Client
    }
}

impl std::fmt::Display for PoolMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolMode::Client => write!(f, "client"),
            PoolMode::Server => write!(f, "server"),
        }
    }
}

/// parse_pool_mode 从字符串解析连接池模式
pub fn parse_pool_mode(s: &str) -> PoolMode {
    match s.to_lowercase().as_str() {
        "client" => PoolMode::Client,
        "server" => PoolMode::Server,
        _ => PoolMode::Client, // 默认客户端模式
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_mode_display() {
        assert_eq!(PoolMode::Client.to_string(), "client");
        assert_eq!(PoolMode::Server.to_string(), "server");
    }

    #[test]
    fn test_parse_pool_mode() {
        assert_eq!(parse_pool_mode("client"), PoolMode::Client);
        assert_eq!(parse_pool_mode("server"), PoolMode::Server);
        assert_eq!(parse_pool_mode("CLIENT"), PoolMode::Client);
        assert_eq!(parse_pool_mode("SERVER"), PoolMode::Server);
        assert_eq!(parse_pool_mode("unknown"), PoolMode::Client); // 默认
    }
}
