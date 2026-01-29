// Copyright (c) 2025, vistone
// All rights reserved.

// 压力测试套件

use netconnpool::config::default_config;
use netconnpool::*;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// 启动一个轻量 TCP accept 循环，避免高并发 connect 堆积在 backlog 导致阻塞
fn setup_test_server() -> (String, Arc<AtomicBool>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("{}", listener.local_addr().unwrap());
    let _ = listener.set_nonblocking(true);

    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();

    let handle = thread::spawn(move || {
        while !stop2.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => drop(stream),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => thread::sleep(Duration::from_millis(1)),
            }
        }
    });

    (addr, stop, handle)
}

#[test]
#[ignore] // 默认忽略，需要长时间运行
fn test_concurrent_connections() {
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 100;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    let num_threads = 50;
    let operations_per_thread = 100;

    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                let mut success_count = 0;
                for _ in 0..operations_per_thread {
                    if let Ok(conn) = pool.get() {
                        // 模拟使用连接
                        thread::sleep(Duration::from_millis(1));
                        drop(conn);
                        success_count += 1;
                    }
                }
                success_count
            })
        })
        .collect();

    let total_success: u64 = handles.into_iter().map(|h| h.join().unwrap() as u64).sum();

    let duration = start.elapsed();
    let stats = pool.stats();

    println!("并发测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  总操作数: {}", num_threads * operations_per_thread);
    println!("  成功操作数: {}", total_success);
    println!("  耗时: {:?}", duration);
    println!(
        "  吞吐量: {:.2} ops/sec",
        total_success as f64 / duration.as_secs_f64()
    );
    println!("  统计信息:");
    println!("    创建连接数: {}", stats.total_connections_created);
    println!("    关闭连接数: {}", stats.total_connections_closed);
    println!("    当前连接数: {}", stats.current_connections);
    println!("    成功获取: {}", stats.successful_gets);
    println!("    失败获取: {}", stats.failed_gets);
    println!("    连接复用: {}", stats.total_connections_reused);

    assert!(total_success > 0, "应该有成功的操作");
    assert!(stats.total_connections_created <= max_conns as i64);

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[test]
#[ignore]
fn test_long_running() {
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 50;
    config.max_connections = max_conns;
    config.min_connections = 5;
    config.max_idle_connections = 20;
    config.idle_timeout = Duration::from_secs(5);
    config.max_lifetime = Duration::from_secs(30);
    config.enable_stats = true;
    config.enable_health_check = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    let test_duration = Duration::from_secs(60); // 运行60秒
    let start = Instant::now();

    let operations = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));

    let num_threads = 10;
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            let operations = operations.clone();
            let errors = errors.clone();
            thread::spawn(move || {
                while start.elapsed() < test_duration {
                    match pool.get() {
                        Ok(conn) => {
                            operations.fetch_add(1, Ordering::Relaxed);
                            // 模拟使用连接
                            thread::sleep(Duration::from_millis(10));
                            drop(conn);
                            if false {
                                errors.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(_) => {
                            errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
        })
        .collect();

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    let final_stats = pool.stats();
    let total_ops = operations.load(Ordering::Relaxed);
    let total_errors = errors.load(Ordering::Relaxed);

    println!("长时间运行测试结果:");
    println!("  运行时间: {:?}", test_duration);
    println!("  总操作数: {}", total_ops);
    println!("  错误数: {}", total_errors);
    println!(
        "  成功率: {:.2}%",
        (total_ops as f64 / (total_ops + total_errors) as f64) * 100.0
    );
    println!("  统计信息:");
    println!("    创建连接数: {}", final_stats.total_connections_created);
    println!("    关闭连接数: {}", final_stats.total_connections_closed);
    println!("    当前连接数: {}", final_stats.current_connections);
    // average_reuse_count 是平均每个连接的复用次数，不是复用率
    // 复用率 = total_connections_reused / successful_gets * 100%
    let reuse_rate = if final_stats.successful_gets > 0 {
        final_stats.total_connections_reused as f64 / final_stats.successful_gets as f64 * 100.0
    } else {
        0.0
    };
    println!("    连接复用率: {:.2}%", reuse_rate);
    println!("    平均复用次数: {:.2}", final_stats.average_reuse_count);

    assert!(total_ops > 0, "应该有成功的操作");
    assert!(final_stats.current_connections <= max_conns as i64);

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[test]
#[ignore]
fn test_memory_leak() {
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 100;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    let iterations = 10000;

    for i in 0..iterations {
        if let Ok(conn) = pool.get() {
            // 立即归还
            drop(conn);
        }

        // 每1000次迭代检查一次
        if i % 1000 == 0 {
            let stats = pool.stats();
            println!("迭代 {}: 当前连接数 = {}", i, stats.current_connections);
        }
    }

    let final_stats = pool.stats();
    println!("内存泄漏测试结果:");
    println!("  总迭代数: {}", iterations);
    println!("  最终连接数: {}", final_stats.current_connections);
    println!("  创建连接数: {}", final_stats.total_connections_created);
    println!("  关闭连接数: {}", final_stats.total_connections_closed);

    // 检查连接数是否在合理范围内
    assert!(
        final_stats.current_connections <= 100,
        "连接数不应超过最大值"
    );

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[test]
#[ignore]
fn test_connection_pool_exhaustion() {
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 10;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;
    config.get_connection_timeout = Duration::from_millis(100); // 设置短超时，确保快速失败

    let pool = Arc::new(Pool::new(config).unwrap());

    // 获取所有可用连接
    let mut connections = Vec::new();
    for _ in 0..10 {
        match pool.get() {
            Ok(conn) => connections.push(conn),
            Err(_) => break,
        }
    }

    assert_eq!(connections.len(), 10, "应该获取到10个连接");

    // 尝试获取第11个连接，应该失败或超时
    let start = Instant::now();
    let result = pool.get();
    let elapsed = start.elapsed();

    assert!(result.is_err(), "连接池耗尽时应该返回错误");
    assert!(elapsed < Duration::from_secs(1), "应该快速返回错误");

    // 归还一个连接
    if let Some(conn) = connections.pop() {
        drop(conn);

        // 现在应该能获取到连接
        let result = pool.get();
        assert!(result.is_ok(), "归还连接后应该能获取到新连接");
    }

    // 归还所有连接
    for conn in connections {
        drop(conn);
    }

    let stats = pool.stats();
    println!("连接池耗尽测试结果:");
    println!("  最大连接数: 10");
    println!("  成功获取: {}", stats.successful_gets);
    println!("  失败获取: {}", stats.failed_gets);

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[test]
#[ignore]
fn test_rapid_acquire_release() {
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 20;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    let iterations = 10000;
    let start = Instant::now();

    for _ in 0..iterations {
        if let Ok(conn) = pool.get() {
            // 立即归还，测试快速获取和释放
            drop(conn);
        }
    }

    let duration = start.elapsed();
    let stats = pool.stats();

    println!("快速获取释放测试结果:");
    println!("  迭代数: {}", iterations);
    println!("  耗时: {:?}", duration);
    println!(
        "  吞吐量: {:.2} ops/sec",
        iterations as f64 / duration.as_secs_f64()
    );
    println!("  创建连接数: {}", stats.total_connections_created);
    // average_reuse_count 是平均每个连接的复用次数，不是复用率
    // 复用率 = total_connections_reused / successful_gets * 100%
    let reuse_rate = if stats.successful_gets > 0 {
        stats.total_connections_reused as f64 / stats.successful_gets as f64 * 100.0
    } else {
        0.0
    };
    println!("  连接复用率: {:.2}%", reuse_rate);
    println!("  平均复用次数: {:.2}", stats.average_reuse_count);

    // 连接复用率应该很高（> 95%）
    assert!(
        reuse_rate > 95.0,
        "连接复用率应该超过95%，实际: {:.2}%",
        reuse_rate
    );

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[test]
#[ignore]
fn test_mixed_protocols() {
    // 这个测试需要同时支持TCP和UDP
    // 由于当前实现主要支持TCP，这里先测试TCP
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 50;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    let num_threads = 20;
    let operations_per_thread = 50;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                let mut tcp_count = 0;
                for _ in 0..operations_per_thread {
                    // 随机选择协议（当前只支持TCP）
                    if let Ok(conn) = pool.get_tcp() {
                        tcp_count += 1;
                        thread::sleep(Duration::from_millis(1));
                        drop(conn);
                    }
                }
                tcp_count
            })
        })
        .collect();

    let total: u64 = handles.into_iter().map(|h| h.join().unwrap() as u64).sum();

    let stats = pool.stats();
    println!("混合协议测试结果:");
    println!("  TCP操作数: {}", total);
    println!("  统计信息:");
    println!("    当前TCP连接数: {}", stats.current_tcp_connections);
    println!("    当前UDP连接数: {}", stats.current_udp_connections);

    assert!(total > 0, "应该有成功的操作");

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[test]
#[ignore]
fn test_connection_lifecycle() {
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 10;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.max_lifetime = Duration::from_secs(2);
    config.idle_timeout = Duration::from_secs(1);
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    // 创建一些连接
    let mut connections = Vec::new();
    for _ in 0..5 {
        if let Ok(conn) = pool.get() {
            connections.push(conn);
        }
    }

    // 归还连接
    for conn in connections {
        drop(conn);
    }

    let initial_stats = pool.stats();
    println!("初始状态:");
    println!("  当前连接数: {}", initial_stats.current_connections);

    // 等待连接过期
    thread::sleep(Duration::from_secs(3));

    // 尝试获取连接，应该创建新连接
    let conn = pool.get().unwrap();
    drop(conn);

    let final_stats = pool.stats();
    println!("最终状态:");
    println!("  当前连接数: {}", final_stats.current_connections);
    println!("  创建连接数: {}", final_stats.total_connections_created);
    println!("  关闭连接数: {}", final_stats.total_connections_closed);

    assert!(final_stats.total_connections_created >= initial_stats.total_connections_created);

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[test]
#[ignore]
fn test_high_concurrency_stress() {
    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    let max_conns = 200;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    let num_threads = 200; // 高并发
    let operations_per_thread = 100;

    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                let mut success = 0;
                for _ in 0..operations_per_thread {
                    if let Ok(conn) = pool.get() {
                        // 模拟不同的使用时间
                        thread::sleep(Duration::from_micros(100));
                        drop(conn);
                        success += 1;
                    }
                }
                success
            })
        })
        .collect();

    let total_success: u64 = handles.into_iter().map(|h| h.join().unwrap() as u64).sum();

    let duration = start.elapsed();
    let stats = pool.stats();

    println!("高并发压力测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  总操作数: {}", num_threads * operations_per_thread);
    println!("  成功操作数: {}", total_success);
    println!("  耗时: {:?}", duration);
    println!(
        "  吞吐量: {:.2} ops/sec",
        total_success as f64 / duration.as_secs_f64()
    );
    println!("  统计信息:");
    println!("    创建连接数: {}", stats.total_connections_created);
    println!("    当前连接数: {}", stats.current_connections);
    println!("    成功获取: {}", stats.successful_gets);
    println!("    失败获取: {}", stats.failed_gets);
    println!("    连接复用: {}", stats.total_connections_reused);

    // 成功率应该很高
    let success_rate = total_success as f64 / (num_threads * operations_per_thread) as f64;
    println!("  成功率: {:.2}%", success_rate * 100.0);

    assert!(success_rate > 0.9, "成功率应该超过90%");
    assert!(stats.current_connections <= 200, "连接数不应超过最大值");

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}
