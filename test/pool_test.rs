// Copyright (c) 2025, vistone
// All rights reserved.

#[cfg(test)]
mod tests {
    use netconnpool::*;
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    #[test]
    fn test_pool_creation() {
        let mut config = DefaultConfig();
        config.Dialer = Some(Box::new(|| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        config.MaxConnections = 5;
        config.MinConnections = 0; // 不预热，避免连接失败

        let pool = Pool::NewPool(config);
        assert!(pool.is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = DefaultConfig();
        // 缺少 Dialer
        assert!(config.Validate().is_err());

        config.Dialer = Some(Box::new(|| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        assert!(config.Validate().is_ok());
    }

    #[test]
    fn test_server_config() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut config = DefaultServerConfig();
        config.Listener = Some(listener);
        config.MaxConnections = 10;
        config.MinConnections = 0;

        let pool = Pool::NewPool(config);
        assert!(pool.is_ok());
    }

    #[test]
    fn test_pool_close() {
        let mut config = DefaultConfig();
        config.Dialer = Some(Box::new(|| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        config.MaxConnections = 5;
        config.MinConnections = 0;

        let pool = Pool::NewPool(config).unwrap();
        assert!(pool.Close().is_ok());
    }

    #[test]
    fn test_stats() {
        let mut config = DefaultConfig();
        config.Dialer = Some(Box::new(|| {
            TcpStream::connect("127.0.0.1:8080")
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }));
        config.MaxConnections = 5;
        config.MinConnections = 0;
        config.EnableStats = true;

        let pool = Pool::NewPool(config).unwrap();
        let stats = pool.Stats();
        assert_eq!(stats.TotalConnectionsCreated, 0);
        assert_eq!(stats.CurrentConnections, 0);
    }
}
