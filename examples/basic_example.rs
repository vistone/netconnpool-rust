// Copyright (c) 2025, vistone
// All rights reserved.

// 基本使用示例

use netconnpool::*;
use std::net::TcpStream;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建客户端连接池配置
    let mut config = DefaultConfig();
    config.MaxConnections = 10;
    config.MinConnections = 2; // 预热2个连接
    
    // 设置连接创建函数
    config.Dialer = Some(Box::new(|| {
        TcpStream::connect("127.0.0.1:8080")
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    
    // 创建连接池
    let pool = Pool::NewPool(config)?;
    
    // 获取连接
    let conn = pool.Get()?;
    
    // 使用连接
    if let Some(tcp_stream) = conn.GetTcpConn() {
        println!("获取到TCP连接: {:?}", tcp_stream.peer_addr());
    }
    
    // 归还连接
    pool.Put(conn)?;
    
    // 关闭连接池
    pool.Close()?;
    
    Ok(())
}
