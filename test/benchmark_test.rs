// Copyright (c) 2025, vistone
// All rights reserved.

// 性能基准测试

use netconnpool::config::default_config;
use netconnpool::*;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn create_test_server() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").unwrap()
}

fn get_server_addr(listener: &TcpListener) -> String {
    format!("{}", listener.local_addr().unwrap())
}

#[test]
#[ignore]
fn benchmark_get_put_operations() {
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    let max_conns = 100;
    config.max_connections = max_conns;
    config.min_connections = 10; // 预热连接
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    // 等待预热完成
    thread::sleep(Duration::from_millis(100));

    let iterations = 100000;
    let start = Instant::now();

    for _ in 0..iterations {
        if let Ok(conn) = pool.get() {
            drop(conn);
        }
    }

    let duration = start.elapsed();
    let stats = pool.stats();

    println!("获取/归还操作基准测试:");
    println!("  迭代数: {}", iterations);
    println!("  总耗时: {:?}", duration);
    println!("  平均耗时: {:?} ns/op", duration.as_nanos() / iterations);
    println!(
        "  吞吐量: {:.2} ops/sec",
        iterations as f64 / duration.as_secs_f64()
    );
    // average_reuse_count 是平均每个连接的复用次数，不是复用率
    // 复用率 = total_connections_reused / successful_gets * 100%
    let reuse_rate = if stats.successful_gets > 0 {
        stats.total_connections_reused as f64 / stats.successful_gets as f64 * 100.0
    } else {
        0.0
    };
    println!("  连接复用率: {:.2}%", reuse_rate);
    println!("  平均复用次数: {:.2}", stats.average_reuse_count);

    // 性能要求：每秒至少10万次操作
    let ops_per_sec = iterations as f64 / duration.as_secs_f64();
    assert!(ops_per_sec > 10000.0, "吞吐量应该超过10000 ops/sec");
}

#[test]
#[ignore]
fn benchmark_concurrent_get_put() {
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    let max_conns = 100;
    config.max_connections = max_conns;
    config.min_connections = 20;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    // 等待预热完成
    thread::sleep(Duration::from_millis(100));

    let num_threads = 50;
    let operations_per_thread = 2000;
    let total_operations = num_threads * operations_per_thread;

    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    if let Ok(conn) = pool.get() {
                        drop(conn);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let stats = pool.stats();

    println!("并发获取/归还基准测试:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  总操作数: {}", total_operations);
    println!("  总耗时: {:?}", duration);
    println!(
        "  吞吐量: {:.2} ops/sec",
        total_operations as f64 / duration.as_secs_f64()
    );
    // average_reuse_count 是平均每个连接的复用次数，不是复用率
    // 复用率 = total_connections_reused / successful_gets * 100%
    let reuse_rate = if stats.successful_gets > 0 {
        stats.total_connections_reused as f64 / stats.successful_gets as f64 * 100.0
    } else {
        0.0
    };
    println!("  连接复用率: {:.2}%", reuse_rate);
    println!("  平均复用次数: {:.2}", stats.average_reuse_count);

    let ops_per_sec = total_operations as f64 / duration.as_secs_f64();
    assert!(ops_per_sec > 50000.0, "并发吞吐量应该超过50000 ops/sec");
}

#[test]
#[ignore]
fn benchmark_connection_creation() {
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    let max_conns = 1000;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    let num_connections = 100;
    let start = Instant::now();

    let mut connections = Vec::new();
    for _ in 0..num_connections {
        if let Ok(conn) = pool.get() {
            connections.push(conn);
        }
    }

    let creation_time = start.elapsed();

    // 归还所有连接
    for conn in connections {
        drop(conn);
    }

    let _stats = pool.stats();

    println!("连接创建基准测试:");
    println!("  创建连接数: {}", num_connections);
    println!("  总耗时: {:?}", creation_time);
    println!(
        "  平均耗时: {:?} ms/conn",
        creation_time.as_millis() / num_connections
    );
    println!(
        "  创建速率: {:.2} conn/sec",
        num_connections as f64 / creation_time.as_secs_f64()
    );

    // 创建100个连接应该在合理时间内完成
    assert!(
        creation_time < Duration::from_secs(10),
        "创建连接应该在10秒内完成"
    );
}

#[test]
#[ignore]
fn benchmark_stats_collection() {
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    let max_conns = 100;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    // 执行一些操作
    for _ in 0..1000 {
        if let Ok(conn) = pool.get() {
            drop(conn);
        }
    }

    // 基准测试获取统计信息的速度
    let iterations = 100000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = pool.stats();
    }

    let duration = start.elapsed();

    println!("统计信息收集基准测试:");
    println!("  迭代数: {}", iterations);
    println!("  总耗时: {:?}", duration);
    println!("  平均耗时: {:?} ns/op", duration.as_nanos() / iterations);
    println!(
        "  吞吐量: {:.2} stats/sec",
        iterations as f64 / duration.as_secs_f64()
    );

    // 获取统计信息应该很快
    let avg_ns = duration.as_nanos() / iterations;
    assert!(avg_ns < 10000, "获取统计信息应该在10微秒内完成");
}
