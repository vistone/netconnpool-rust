// Copyright (c) 2025, vistone
// All rights reserved.

// 全面压力测试 - 长时间运行、内存溢出、资源管理测试

use netconnpool::config::default_config;
use netconnpool::*;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

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

/// 长时间运行压力测试 - 测试内存泄漏和资源管理
#[test]
#[ignore] // 默认忽略，需要长时间运行
fn test_long_running_comprehensive() {
    println!("==========================================");
    println!("全面长时间运行压力测试");
    println!("==========================================");

    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));

    // 配置较大的连接池以测试长时间运行
    let max_conns = 500;
    config.max_connections = max_conns;
    config.min_connections = 10;
    config.max_idle_connections = 200;
    config.idle_timeout = Duration::from_secs(30);
    config.max_lifetime = Duration::from_secs(300); // 5分钟
    config.enable_stats = true;
    config.enable_health_check = true;
    config.health_check_interval = Duration::from_secs(10);
    config.connection_leak_timeout = Duration::from_secs(60);

    let pool = Arc::new(Pool::new(config).unwrap());

    // 测试持续时间：2小时
    let test_duration = Duration::from_secs(2 * 60 * 60);
    let start = Instant::now();

    let operations = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let memory_samples = Arc::new(std::sync::Mutex::new(Vec::new()));

    println!("测试配置:");
    println!("  最大连接数: {}", max_conns);
    println!("  测试持续时间: {:?}", test_duration);
    println!("  开始时间: {:?}", start.elapsed());
    println!();

    // 启动多个工作线程
    let num_threads = 100;
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let pool = pool.clone();
            let operations = operations.clone();
            let errors = errors.clone();
            let memory_samples = memory_samples.clone();
            let start = start;

            thread::spawn(move || {
                let mut last_memory_check = Instant::now();
                let mut iteration = 0;

                while start.elapsed() < test_duration {
                    iteration += 1;

                    // 每10秒记录一次内存使用情况
                    if last_memory_check.elapsed() >= Duration::from_secs(10) {
                        let stats = pool.stats();
                        let sample = MemorySample {
                            elapsed: start.elapsed(),
                            current_connections: stats.current_connections,
                            total_created: stats.total_connections_created,
                            total_closed: stats.total_connections_closed,
                            active_connections: stats.current_active_connections,
                            idle_connections: stats.current_idle_connections,
                            successful_gets: stats.successful_gets,
                            failed_gets: stats.failed_gets,
                        };

                        if let Ok(mut samples) = memory_samples.lock() {
                            samples.push(sample);
                        }
                        last_memory_check = Instant::now();
                    }

                    match pool.get() {
                        Ok(conn) => {
                            operations.fetch_add(1, Ordering::Relaxed);

                            // 模拟不同的使用时间（1-100ms）
                            let use_time = if i % 10 == 0 {
                                Duration::from_millis(100) // 10%的连接使用较长时间
                            } else {
                                Duration::from_millis(1 + (i % 10))
                            };
                            thread::sleep(use_time);

                            drop(conn);
                        }
                        Err(_) => {
                            errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    // 每1000次迭代短暂休息，避免CPU占用过高
                    if iteration % 1000 == 0 {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            })
        })
        .collect();

    // 启动监控线程
    let monitor_pool = pool.clone();
    let monitor_operations = operations.clone();
    let monitor_errors = errors.clone();
    let monitor_start = start;
    let monitor_handle = thread::spawn(move || {
        while monitor_start.elapsed() < test_duration {
            thread::sleep(Duration::from_secs(30)); // 每30秒报告一次

            let elapsed = monitor_start.elapsed();
            let ops = monitor_operations.load(Ordering::Relaxed);
            let errs = monitor_errors.load(Ordering::Relaxed);
            let stats = monitor_pool.stats();

            println!(
                "[{:?}] 运行中... 操作数: {}, 错误: {}, 当前连接: {}, 创建: {}, 关闭: {}, 复用: {}",
                elapsed,
                ops,
                errs,
                stats.current_connections,
                stats.total_connections_created,
                stats.total_connections_closed,
                stats.total_connections_reused
            );

            // 检查整数溢出风险
            if stats.total_connections_created > 1_000_000_000 {
                println!("警告: total_connections_created 接近溢出风险");
            }
            if stats.successful_gets > 1_000_000_000 {
                println!("警告: successful_gets 接近溢出风险");
            }
        }
    });

    // 等待所有工作线程完成
    println!("等待所有工作线程完成...");
    for handle in handles {
        handle.join().unwrap();
    }

    monitor_handle.join().unwrap();

    let final_stats = pool.stats();
    let total_ops = operations.load(Ordering::Relaxed);
    let total_errors = errors.load(Ordering::Relaxed);
    let total_time = start.elapsed();

    println!();
    println!("==========================================");
    println!("长时间运行测试结果");
    println!("==========================================");
    println!("  运行时间: {:?}", total_time);
    println!("  总操作数: {}", total_ops);
    println!("  错误数: {}", total_errors);
    println!(
        "  成功率: {:.2}%",
        (total_ops as f64 / (total_ops + total_errors) as f64) * 100.0
    );
    println!(
        "  平均吞吐量: {:.2} ops/sec",
        total_ops as f64 / total_time.as_secs_f64()
    );
    println!();
    println!("  统计信息:");
    println!("    创建连接数: {}", final_stats.total_connections_created);
    println!("    关闭连接数: {}", final_stats.total_connections_closed);
    println!("    当前连接数: {}", final_stats.current_connections);
    println!("    活跃连接数: {}", final_stats.current_active_connections);
    println!("    空闲连接数: {}", final_stats.current_idle_connections);
    println!("    成功获取: {}", final_stats.successful_gets);
    println!("    失败获取: {}", final_stats.failed_gets);
    println!("    连接复用: {}", final_stats.total_connections_reused);
    println!(
        "    连接复用率: {:.2}%",
        final_stats.average_reuse_count * 100.0
    );
    println!();

    // 分析内存样本
    if let Ok(samples) = memory_samples.lock() {
        println!("  内存使用趋势分析:");
        if let (Some(first), Some(last)) = (samples.first(), samples.last()) {
            println!("    初始连接数: {}", first.current_connections);
            println!("    最终连接数: {}", last.current_connections);
            println!(
                "    连接数变化: {}",
                last.current_connections - first.current_connections
            );

            // 检查是否有内存泄漏迹象
            if last.current_connections > first.current_connections + 50 {
                println!("    ⚠️  警告: 连接数增长超过50，可能存在内存泄漏");
            } else {
                println!("    ✅ 连接数稳定，无内存泄漏迹象");
            }
        }
    }

    // 验证统计计数器是否溢出
    println!();
    println!("  整数溢出检测:");
    let mut overflow_detected = false;
    if final_stats.total_connections_created < 0 {
        println!("    ❌ total_connections_created 溢出！");
        overflow_detected = true;
    } else {
        println!(
            "    ✅ total_connections_created 正常: {}",
            final_stats.total_connections_created
        );
    }
    if final_stats.successful_gets < 0 {
        println!("    ❌ successful_gets 溢出！");
        overflow_detected = true;
    } else {
        println!(
            "    ✅ successful_gets 正常: {}",
            final_stats.successful_gets
        );
    }

    if overflow_detected {
        panic!("检测到整数溢出！");
    }

    // 验证连接数限制
    assert!(
        final_stats.current_connections <= max_conns as i64,
        "连接数不应超过最大值: {} > {}",
        final_stats.current_connections,
        max_conns
    );

    assert!(total_ops > 0, "应该有成功的操作");

    println!();
    println!("✅ 长时间运行测试通过！");

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // 这些字段用于内存趋势分析
struct MemorySample {
    elapsed: Duration,
    current_connections: i64,
    total_created: i64,
    total_closed: i64,
    active_connections: i64,
    idle_connections: i64,
    successful_gets: i64,
    failed_gets: i64,
}

/// 测试整数溢出边界情况
#[test]
#[ignore]
fn test_integer_overflow_boundary() {
    println!("==========================================");
    println!("整数溢出边界测试");
    println!("==========================================");

    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));

    config.max_connections = 10;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    // 快速执行大量操作，测试统计计数器
    let num_operations = 1_000_000; // 100万次操作
    let num_threads = 50;
    let operations_per_thread = num_operations / num_threads;

    println!("测试配置:");
    println!("  总操作数: {}", num_operations);
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!();

    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    if let Ok(conn) = pool.get() {
                        // 立即归还
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

    println!("测试结果:");
    println!("  耗时: {:?}", duration);
    println!(
        "  吞吐量: {:.2} ops/sec",
        num_operations as f64 / duration.as_secs_f64()
    );
    println!("  统计信息:");
    println!("    创建连接数: {}", stats.total_connections_created);
    println!("    成功获取: {}", stats.successful_gets);
    println!("    连接复用: {}", stats.total_connections_reused);
    println!();

    // 检查溢出
    println!("溢出检测:");
    if stats.total_connections_created < 0 {
        println!("  ❌ total_connections_created 溢出！");
        panic!("检测到整数溢出！");
    } else {
        println!(
            "  ✅ total_connections_created 正常: {}",
            stats.total_connections_created
        );
    }

    if stats.successful_gets < 0 {
        println!("  ❌ successful_gets 溢出！");
        panic!("检测到整数溢出！");
    } else {
        println!("  ✅ successful_gets 正常: {}", stats.successful_gets);
    }

    // 检查是否接近溢出边界
    if stats.total_connections_created > i64::MAX / 2 {
        println!("  ⚠️  警告: total_connections_created 接近溢出边界");
    }
    if stats.successful_gets > i64::MAX / 2 {
        println!("  ⚠️  警告: successful_gets 接近溢出边界");
    }

    println!();
    println!("✅ 整数溢出边界测试通过！");

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}

/// 测试资源耗尽场景
#[test]
#[ignore]
fn test_resource_exhaustion() {
    println!("==========================================");
    println!("资源耗尽测试");
    println!("==========================================");

    let (addr, stop, server_handle) = setup_test_server();

    let mut config = default_config();
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));

    let max_conns = 50;
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;
    config.get_connection_timeout = Duration::from_secs(1);

    let pool = Arc::new(Pool::new(config).unwrap());

    // 持续运行30分钟，不断获取和释放连接
    let test_duration = Duration::from_secs(30 * 60);
    let start = Instant::now();

    let operations = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let exhausted_count = Arc::new(AtomicU64::new(0));

    let num_threads = 200; // 大量线程竞争连接
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            let operations = operations.clone();
            let errors = errors.clone();
            let exhausted_count = exhausted_count.clone();

            thread::spawn(move || {
                while start.elapsed() < test_duration {
                    match pool.get() {
                        Ok(conn) => {
                            operations.fetch_add(1, Ordering::Relaxed);
                            thread::sleep(Duration::from_millis(10));
                            drop(conn);
                        }
                        Err(NetConnPoolError::PoolExhausted { .. }) => {
                            exhausted_count.fetch_add(1, Ordering::Relaxed);
                            // 连接池耗尽时短暂等待
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => {
                            errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
        })
        .collect();

    // 监控线程
    let monitor_pool = pool.clone();
    let monitor_start = start;
    let monitor_handle = thread::spawn(move || {
        while monitor_start.elapsed() < test_duration {
            thread::sleep(Duration::from_secs(60)); // 每分钟报告一次

            let elapsed = monitor_start.elapsed();
            let stats = monitor_pool.stats();

            println!(
                "[{:?}] 资源耗尽测试 - 当前连接: {}/{}, 成功: {}, 失败: {}",
                elapsed,
                stats.current_connections,
                max_conns,
                stats.successful_gets,
                stats.failed_gets
            );
        }
    });

    for handle in handles {
        handle.join().unwrap();
    }
    monitor_handle.join().unwrap();

    let final_stats = pool.stats();
    let total_ops = operations.load(Ordering::Relaxed);
    let total_errors = errors.load(Ordering::Relaxed);
    let total_exhausted = exhausted_count.load(Ordering::Relaxed);

    println!();
    println!("资源耗尽测试结果:");
    println!("  运行时间: {:?}", test_duration);
    println!("  总操作数: {}", total_ops);
    println!("  错误数: {}", total_errors);
    println!("  连接池耗尽次数: {}", total_exhausted);
    println!("  最终连接数: {}", final_stats.current_connections);
    println!("  最大连接数限制: {}", max_conns);
    println!();

    // 验证连接数不超过限制
    assert!(
        final_stats.current_connections <= max_conns as i64,
        "连接数不应超过最大值"
    );

    println!("✅ 资源耗尽测试通过！");

    stop.store(true, Ordering::Relaxed);
    let _ = server_handle.join();
}
