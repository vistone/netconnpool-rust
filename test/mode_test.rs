// Copyright (c) 2025, vistone
// All rights reserved.

#[cfg(test)]
mod tests {
    use netconnpool::*;

    #[test]
    fn test_pool_mode() {
        assert_eq!(PoolMode::Client.to_string(), "client");
        assert_eq!(PoolMode::Server.to_string(), "server");
    }

    #[test]
    fn test_parse_pool_mode() {
        assert_eq!(ParsePoolMode("client"), PoolMode::Client);
        assert_eq!(ParsePoolMode("server"), PoolMode::Server);
        assert_eq!(ParsePoolMode("CLIENT"), PoolMode::Client);
        assert_eq!(ParsePoolMode("unknown"), PoolMode::Client); // 默认
    }
}
