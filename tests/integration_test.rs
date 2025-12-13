// Copyright (c) 2025, vistone
// All rights reserved.

// 集成测试

use netconnpool::*;
use netconnpool::config::DefaultConfig;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn create_test_server() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").unwrap()
}

fn get_server_addr(listener: &TcpListener) -> String {
    format!("{}", listener.local_addr().unwrap())
}

#[test]
#[ignore]
fn test_full_lifecycle() {
    let listener = create_test_server();
    let addr = get_server_addr(&listener);
    
    let mut config = DefaultConfig();
    config.Dialer = Some(Box::new(move || {
        TcpStream::connect(&addr)
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    let max_conns = 50;
    config.MaxConnections = max_conns;
    config.MinConnections = 5;
    config.MaxIdleConnections = 20;
    config.IdleTimeout = Duration::from_secs(10);
    config.MaxLifetime = Duration::from_secs(60);
    config.EnableStats = true;
    config.EnableHealthCheck = true;
    
    // 创建连接池
    let pool = Arc::new(Pool::NewPool(config).unwrap());
    
    // 阶段1: 预热阶段
    println!("阶段1: 预热阶段");
    thread::sleep(Duration::from_millis(100));
    let stats1 = pool.Stats();
    println!("  当前连接数: {}", stats1.CurrentConnections);
    
    // 阶段2: 正常使用阶段
    println!("阶段2: 正常使用阶段");
    let num_threads = 10;
    let operations_per_thread = 100;
    
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    if let Ok(conn) = pool.Get() {
                        thread::sleep(Duration::from_millis(5));
                        let _ = pool.Put(conn);
                    }
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let stats2 = pool.Stats();
    println!("  总操作数: {}", stats2.SuccessfulGets);
    println!("  连接复用率: {:.2}%", stats2.AverageReuseCount * 100.0);
    
    // 阶段3: 高负载阶段
    println!("阶段3: 高负载阶段");
    let high_load_threads = 50;
    let high_load_ops = 200;
    
    let handles: Vec<_> = (0..high_load_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                for _ in 0..high_load_ops {
                    if let Ok(conn) = pool.Get() {
                        thread::sleep(Duration::from_millis(1));
                        let _ = pool.Put(conn);
                    }
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let stats3 = pool.Stats();
    println!("  高负载操作数: {}", stats3.SuccessfulGets - stats2.SuccessfulGets);
    
    // 阶段4: 清理和关闭
    println!("阶段4: 清理和关闭");
    assert!(pool.Close().is_ok(), "应该能成功关闭连接池");
    
    let final_stats = pool.Stats();
    println!("最终统计:");
    println!("  总创建连接数: {}", final_stats.TotalConnectionsCreated);
    println!("  总关闭连接数: {}", final_stats.TotalConnectionsClosed);
    println!("  总成功获取: {}", final_stats.SuccessfulGets);
    println!("  总失败获取: {}", final_stats.FailedGets);
    
    // 验证统计数据的合理性
    assert!(final_stats.TotalConnectionsCreated <= max_conns as i64);
    assert!(final_stats.SuccessfulGets > 0);
}

#[test]
#[ignore]
fn test_error_recovery() {
    let listener = create_test_server();
    let addr = get_server_addr(&listener);
    
    let mut config = DefaultConfig();
    let addr_clone = addr.clone();
    config.Dialer = Some(Box::new(move || {
        // 模拟偶尔的连接失败
        static mut COUNTER: u32 = 0;
        unsafe {
            COUNTER += 1;
            if COUNTER % 10 == 0 {
                // 每10次失败一次
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "模拟连接失败",
                )) as Box<dyn std::error::Error + Send + Sync>);
            }
        }
        TcpStream::connect(&addr_clone)
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    let max_conns = 20;
    config.MaxConnections = max_conns;
    config.MinConnections = 0;
    config.EnableStats = true;
    
    let pool = Arc::new(Pool::NewPool(config).unwrap());
    
    let mut success_count = 0;
    let mut error_count = 0;
    
    for _ in 0..100 {
        match pool.Get() {
            Ok(conn) => {
                success_count += 1;
                thread::sleep(Duration::from_millis(1));
                let _ = pool.Put(conn);
            }
            Err(_) => {
                error_count += 1;
            }
        }
    }
    
    let stats = pool.Stats();
    println!("错误恢复测试结果:");
    println!("  成功操作: {}", success_count);
    println!("  错误操作: {}", error_count);
    println!("  统计中的失败数: {}", stats.FailedGets);
    println!("  统计中的错误数: {}", stats.ConnectionErrors);
    
    // 即使有错误，也应该能继续工作
    assert!(success_count > 0, "应该有成功的操作");
}

#[test]
#[ignore]
fn test_concurrent_pool_operations() {
    let listener = create_test_server();
    let addr = get_server_addr(&listener);
    
    let mut config = DefaultConfig();
    config.Dialer = Some(Box::new(move || {
        TcpStream::connect(&addr)
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    let max_conns = 100;
    config.MaxConnections = max_conns;
    config.MinConnections = 0;
    config.EnableStats = true;
    
    let pool = Arc::new(Pool::NewPool(config).unwrap());
    
    // 创建多个线程同时进行不同的操作
    let num_threads = 20;
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let pool = pool.clone();
            thread::spawn(move || {
                match i % 4 {
                    0 => {
                        // 线程组1: 获取和归还
                        for _ in 0..50 {
                            if let Ok(conn) = pool.Get() {
                                let _ = pool.Put(conn);
                            }
                        }
                    }
                    1 => {
                        // 线程组2: 只获取TCP连接
                        for _ in 0..50 {
                            if let Ok(conn) = pool.GetTCP() {
                                let _ = pool.Put(conn);
                            }
                        }
                    }
                    2 => {
                        // 线程组3: 获取IPv4连接
                        for _ in 0..50 {
                            if let Ok(conn) = pool.GetIPv4() {
                                let _ = pool.Put(conn);
                            }
                        }
                    }
                    _ => {
                        // 线程组4: 获取统计信息
                        for _ in 0..100 {
                            let _ = pool.Stats();
                        }
                    }
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let stats = pool.Stats();
    println!("并发操作测试结果:");
    println!("  总成功获取: {}", stats.SuccessfulGets);
    println!("  当前连接数: {}", stats.CurrentConnections);
    println!("  连接复用: {}", stats.TotalConnectionsReused);
    
    assert!(stats.SuccessfulGets > 0, "应该有成功的操作");
}
