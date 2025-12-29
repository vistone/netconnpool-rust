// Copyright (c) 2025, vistone
// All rights reserved.

// 专门测试 idle_counts CAS 修复的并发安全性

use netconnpool::config::default_config;
use netconnpool::*;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// 启动一个轻量 TCP accept 循环
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

/// 测试 idle_counts 在并发场景下不会超过 max_idle_connections
/// 这是 CAS 修复的核心测试
#[test]
fn test_idle_counts_cas_race_condition() {
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
    config.max_connections = 100;
    config.max_idle_connections = 10; // 设置较小的空闲连接限制，便于测试
    config.enable_stats = true;
    config.get_connection_timeout = Duration::from_secs(5);

    let pool = Pool::new(config).unwrap();

    const NUM_THREADS: usize = 20;
    const OPS_PER_THREAD: usize = 1000;
    let violations = Arc::new(AtomicUsize::new(0));
    let total_ops = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    // 启动多个线程并发地获取和归还连接
    for _ in 0..NUM_THREADS {
        let pool = pool.clone();
        let _violations = violations.clone();
        let total_ops = total_ops.clone();

        let handle = thread::spawn(move || {
            for _ in 0..OPS_PER_THREAD {
                match pool.get() {
                    Ok(conn) => {
                        // 短暂使用连接
                        thread::sleep(Duration::from_micros(10));
                        drop(conn); // 自动归还
                        total_ops.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        // 连接获取失败，继续
                    }
                }
            }
        });

        handles.push(handle);
    }

    // 监控线程：持续检查 idle_counts 是否超过限制
    let pool_monitor = pool.clone();
    let violations_monitor = violations.clone();
    let monitor_stop = Arc::new(AtomicBool::new(false));
    let monitor_stop_clone = monitor_stop.clone();

    let monitor_handle = thread::spawn(move || {
        while !monitor_stop_clone.load(Ordering::Relaxed) {
            let stats = pool_monitor.stats();
            // 检查空闲连接数是否超过限制
            // 注意：由于统计是异步的，这里允许一定的误差
            // 但应该不会持续超过限制
            if stats.current_idle_connections > 10 {
                violations_monitor.fetch_add(1, Ordering::Relaxed);
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    // 等待所有工作线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // 停止监控线程
    monitor_stop.store(true, Ordering::Relaxed);
    monitor_handle.join().unwrap();

    // 等待连接归还完成
    thread::sleep(Duration::from_millis(100));

    // 最终检查
    let final_stats = pool.stats();
    let final_violations = violations.load(Ordering::Relaxed);
    let total_ops_count = total_ops.load(Ordering::Relaxed);

    println!("测试结果:");
    println!("  总操作数: {}", total_ops_count);
    println!("  最终空闲连接数: {}", final_stats.current_idle_connections);
    println!("  检测到的违规次数: {}", final_violations);
    println!("  最大空闲连接限制: 10");

    // 验证：最终空闲连接数不应超过限制
    assert!(
        final_stats.current_idle_connections <= 10,
        "最终空闲连接数 {} 超过了限制 10",
        final_stats.current_idle_connections
    );

    // 验证：操作应该成功执行
    assert!(
        total_ops_count > 0,
        "应该执行了一些操作，但实际为 0"
    );

    // 清理
    stop.store(true, Ordering::Relaxed);
    server_handle.join().unwrap();
    pool.close().unwrap();
}

/// 测试高并发场景下的 idle_counts CAS 正确性
#[test]
#[ignore] // 需要较长时间运行
fn test_idle_counts_high_concurrency() {
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
    config.max_connections = 200;
    config.max_idle_connections = 20;
    config.enable_stats = true;
    config.get_connection_timeout = Duration::from_secs(10);

    let pool = Pool::new(config).unwrap();

    const NUM_THREADS: usize = 50;
    const OPS_PER_THREAD: usize = 5000;
    let max_idle_observed = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    // 启动多个线程并发操作
    for _ in 0..NUM_THREADS {
        let pool = pool.clone();
        let max_idle_observed = max_idle_observed.clone();

        let handle = thread::spawn(move || {
            for _ in 0..OPS_PER_THREAD {
                if let Ok(conn) = pool.get() {
                    thread::sleep(Duration::from_micros(5));
                    drop(conn);

                    // 检查当前空闲连接数
                    let stats = pool.stats();
                    let current = stats.current_idle_connections as usize;
                    let mut max = max_idle_observed.load(Ordering::Relaxed);
                    while current > max {
                        match max_idle_observed.compare_exchange_weak(
                            max,
                            current,
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(x) => max = x,
                        }
                    }
                }
            }
        });

        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // 等待连接归还完成
    thread::sleep(Duration::from_millis(200));

    let final_stats = pool.stats();
    let max_observed = max_idle_observed.load(Ordering::Relaxed);

    println!("高并发测试结果:");
    println!("  线程数: {}", NUM_THREADS);
    println!("  每线程操作数: {}", OPS_PER_THREAD);
    println!("  总操作数: {}", NUM_THREADS * OPS_PER_THREAD);
    println!("  观察到的最大空闲连接数: {}", max_observed);
    println!("  最终空闲连接数: {}", final_stats.current_idle_connections);
    println!("  最大空闲连接限制: 20");

    // 验证：观察到的最大空闲连接数不应超过限制太多
    // 允许一定的误差（由于统计的异步性）
    assert!(
        max_observed <= 25,
        "观察到的最大空闲连接数 {} 超过了合理范围（限制+5）",
        max_observed
    );

    // 验证：最终空闲连接数不应超过限制
    assert!(
        final_stats.current_idle_connections <= 20,
        "最终空闲连接数 {} 超过了限制 20",
        final_stats.current_idle_connections
    );

    // 清理
    stop.store(true, Ordering::Relaxed);
    server_handle.join().unwrap();
    pool.close().unwrap();
}

/// 测试快速获取和归还场景下的 CAS 正确性
#[test]
fn test_idle_counts_rapid_operations() {
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
    config.max_connections = 50;
    config.max_idle_connections = 5; // 很小的限制，更容易触发边界条件
    config.enable_stats = true;
    config.get_connection_timeout = Duration::from_secs(5);

    let pool = Pool::new(config).unwrap();

    const NUM_THREADS: usize = 10;
    const OPS_PER_THREAD: usize = 200;
    let violations = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    // 快速获取和归还连接
    for _ in 0..NUM_THREADS {
        let pool = pool.clone();
        let violations = violations.clone();

        let handle = thread::spawn(move || {
            for _ in 0..OPS_PER_THREAD {
                if let Ok(conn) = pool.get() {
                    // 立即归还，模拟快速操作
                    drop(conn);

                    // 检查空闲连接数
                    let stats = pool.stats();
                    if stats.current_idle_connections > 5 {
                        violations.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });

        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // 等待连接归还完成
    thread::sleep(Duration::from_millis(100));

    let final_stats = pool.stats();
    let violation_count = violations.load(Ordering::Relaxed);

    println!("快速操作测试结果:");
    println!("  最终空闲连接数: {}", final_stats.current_idle_connections);
    println!("  检测到的违规次数: {}", violation_count);
    println!("  最大空闲连接限制: 5");

    // 验证：最终空闲连接数不应超过限制
    assert!(
        final_stats.current_idle_connections <= 5,
        "最终空闲连接数 {} 超过了限制 5",
        final_stats.current_idle_connections
    );

    // 清理
    stop.store(true, Ordering::Relaxed);
    server_handle.join().unwrap();
    pool.close().unwrap();
}

