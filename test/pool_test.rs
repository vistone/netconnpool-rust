// Copyright (c) 2025, vistone
// All rights reserved.

#[cfg(test)]
mod tests {
    use netconnpool::*;
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    #[test]
    fn test_pool_creation() {
        let mut config = default_config();
        config.dialer = Some(Box::new(|_| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        config.max_connections = 5;
        config.min_connections = 0; // 不预热，避免连接失败

        let pool = Pool::new(config);
        assert!(pool.is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = default_config();
        // 缺少 Dialer
        assert!(config.validate().is_err());

        config.dialer = Some(Box::new(|_| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_server_config() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut config = default_server_config();
        config.listener = Some(listener);
        config.max_connections = 10;
        config.min_connections = 0;

        let pool = Pool::new(config);
        assert!(pool.is_ok());
    }

    #[test]
    fn test_pool_close() {
        let mut config = default_config();
        config.dialer = Some(Box::new(|_| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        config.max_connections = 5;
        config.min_connections = 0;

        let pool = Pool::new(config).unwrap();
        assert!(pool.close().is_ok());
    }

    #[test]
    fn test_stats() {
        let mut config = default_config();
        config.dialer = Some(Box::new(|_| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        config.max_connections = 5;
        config.min_connections = 0;
        config.enable_stats = true;

        let pool = Pool::new(config).unwrap();
        let stats = pool.stats();
        assert_eq!(stats.total_connections_created, 0);
        assert_eq!(stats.current_connections, 0);
    }
}
