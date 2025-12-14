// Copyright (c) 2025, vistone
// All rights reserved.

// 统计模块压力测试和竞争条件测试

use netconnpool::*;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
#[ignore]
fn test_stats_concurrent_updates() {
    // 测试并发更新统计信息时的竞争条件
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 100;
    let operations_per_thread = 1000;
    
    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    collector.increment_total_connections_created();
                    collector.increment_successful_gets();
                    collector.increment_current_active_connections(1);
                    collector.increment_current_idle_connections(-1);
                    collector.increment_total_connections_reused();
                    collector.record_get_time(Duration::from_millis(10));
                    
                    // 同时读取统计信息
                    let stats = collector.get_stats();
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let duration = start.elapsed();
    let final_stats = collector.get_stats();
    
    println!("统计模块并发更新测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  总操作数: {}", num_threads * operations_per_thread);
    println!("  耗时: {:?}", duration);
    println!("  最终统计:");
    println!("    创建连接数: {}", final_stats.total_connections_created);
    println!("    成功获取数: {}", final_stats.successful_gets);
    println!("    连接复用数: {}", final_stats.total_connections_reused);
    
    // 验证统计数据的正确性
    let expected_created = num_threads * operations_per_thread;
    assert_eq!(final_stats.total_connections_created, expected_created as i64);
    assert_eq!(final_stats.successful_gets, expected_created as i64);
    assert_eq!(final_stats.total_connections_reused, expected_created as i64);
}

#[test]
#[ignore]
fn test_stats_memory_leak() {
    // 测试统计模块是否存在内存泄漏
    let collector = Arc::new(StatsCollector::new());
    let iterations = 100000;
    
    // 记录初始内存使用（如果可能）
    for i in 0..iterations {
        collector.increment_total_connections_created();
        collector.increment_successful_gets();
        collector.record_get_time(Duration::from_millis(1));
        
        // 频繁获取统计信息
        if i % 1000 == 0 {
            let stats = collector.get_stats();
        }
    }
    
    let final_stats = collector.get_stats();
    println!("统计模块内存泄漏测试结果:");
    println!("  迭代数: {}", iterations);
    println!("  最终统计:");
    println!("    创建连接数: {}", final_stats.total_connections_created);
    println!("    成功获取数: {}", final_stats.successful_gets);
    
    assert_eq!(final_stats.total_connections_created, iterations as i64);
    assert_eq!(final_stats.successful_gets, iterations as i64);
}

#[test]
#[ignore]
fn test_stats_race_condition() {
    // 测试统计模块的竞争条件
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 50;
    let operations_per_thread = 10000;
    
    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let collector = collector.clone();
            thread::spawn(move || {
                for i in 0..operations_per_thread {
                    // 混合读写操作
                    match i % 4 {
                        0 => {
                            collector.increment_total_connections_created();
                            // IncrementTotalConnectionsCreated 会自动增加 CurrentConnections
                        }
                        1 => {
                            collector.increment_successful_gets();
                            collector.increment_current_active_connections(1);
                        }
                        2 => {
                            collector.increment_total_connections_reused();
                            collector.record_get_time(Duration::from_micros(100));
                        }
                        _ => {
                            // 读取操作
                            let stats = collector.get_stats();
                        }
                    }
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.get_stats();
    let expected_created = (num_threads * operations_per_thread) / 4;
    let expected_gets = (num_threads * operations_per_thread) / 4;
    
    println!("统计模块竞争条件测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  预期创建数: {}", expected_created);
    println!("  实际创建数: {}", final_stats.total_connections_created);
    println!("  预期获取数: {}", expected_gets);
    println!("  实际获取数: {}", final_stats.successful_gets);
    
    // 允许小的误差（由于并发）
    assert!((final_stats.total_connections_created - expected_created as i64).abs() < 1000);
    assert!((final_stats.successful_gets - expected_gets as i64).abs() < 1000);
}

#[test]
#[ignore]
fn test_stats_infinite_loop_prevention() {
    // 测试防止死循环
    let collector = Arc::new(StatsCollector::new());
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    
    // 快速连续操作，测试是否会死循环
    for _ in 0..1000000 {
        collector.increment_total_connections_created();
        collector.record_get_time(Duration::from_nanos(1));
        
        if start.elapsed() > timeout {
            panic!("操作超时，可能存在死循环");
        }
    }
    
    let duration = start.elapsed();
    println!("统计模块死循环防护测试结果:");
    println!("  操作数: 1000000");
    println!("  耗时: {:?}", duration);
    println!("  平均耗时: {:?} ns/op", duration.as_nanos() / 1000000);
    
    // 应该很快完成
    assert!(duration < timeout, "操作应该在5秒内完成");
}

#[test]
#[ignore]
fn test_stats_concurrent_read_write() {
    // 测试并发读写
    let collector = Arc::new(StatsCollector::new());
    let num_writers = 20;
    let num_readers = 20;
    let operations_per_writer = 10000;
    
    // 写入线程
    let writer_handles: Vec<_> = (0..num_writers)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_writer {
                    collector.increment_total_connections_created();
                    collector.increment_successful_gets();
                    collector.increment_current_active_connections(1);
                    collector.increment_current_idle_connections(-1);
                    collector.record_get_time(Duration::from_micros(100));
                }
            })
        })
        .collect();
    
    // 读取线程
    let reader_handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_writer {
                    let stats = collector.get_stats();
                    // 短暂休眠，模拟实际使用
                    thread::sleep(Duration::from_nanos(1));
                }
            })
        })
        .collect();
    
    // 等待所有线程完成
    for handle in writer_handles {
        handle.join().unwrap();
    }
    for handle in reader_handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.get_stats();
    let expected_created = num_writers * operations_per_writer;
    
    println!("统计模块并发读写测试结果:");
    println!("  写入线程数: {}", num_writers);
    println!("  读取线程数: {}", num_readers);
    println!("  每线程操作数: {}", operations_per_writer);
    println!("  预期创建数: {}", expected_created);
    println!("  实际创建数: {}", final_stats.total_connections_created);
    
    assert_eq!(final_stats.total_connections_created, expected_created as i64);
}

#[test]
#[ignore]
fn test_stats_atomic_operations() {
    // 测试原子操作的正确性
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 100;
    let operations_per_thread = 1000;
    
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    // 执行各种原子操作
                    collector.increment_total_connections_created();
                    collector.increment_total_connections_closed();
                    collector.increment_successful_gets();
                    collector.increment_failed_gets();
                    collector.increment_current_active_connections(1);
                    collector.increment_current_active_connections(-1);
                    collector.increment_current_idle_connections(1);
                    collector.increment_current_idle_connections(-1);
                    collector.increment_total_connections_reused();
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.get_stats();
    let expected_created = num_threads * operations_per_thread;
    let expected_closed = num_threads * operations_per_thread;
    
    println!("统计模块原子操作测试结果:");
    println!("  预期创建数: {}", expected_created);
    println!("  实际创建数: {}", final_stats.total_connections_created);
    println!("  预期关闭数: {}", expected_closed);
    println!("  实际关闭数: {}", final_stats.total_connections_closed);
    println!("  当前连接数: {}", final_stats.current_connections);
    println!("  当前活跃连接数: {}", final_stats.current_active_connections);
    println!("  当前空闲连接数: {}", final_stats.current_idle_connections);
    
    assert_eq!(final_stats.total_connections_created, expected_created as i64);
    assert_eq!(final_stats.total_connections_closed, expected_closed as i64);
    // 当前连接数应该是创建数减去关闭数
    assert_eq!(
        final_stats.current_connections,
        final_stats.total_connections_created - final_stats.total_connections_closed
    );
    // 活跃和空闲连接数的变化应该相互抵消
    assert_eq!(final_stats.current_active_connections, 0);
    assert_eq!(final_stats.current_idle_connections, 0);
}

#[test]
#[ignore]
fn test_stats_record_get_time_consistency() {
    // 测试 RecordGetTime 的一致性
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 50;
    let operations_per_thread = 1000;
    let test_duration = Duration::from_millis(10);
    
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    collector.increment_successful_gets();
                    collector.record_get_time(test_duration);
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.get_stats();
    let expected_total_time = test_duration * (num_threads * operations_per_thread) as u32;
    
    println!("统计模块时间记录一致性测试结果:");
    println!("  成功获取数: {}", final_stats.successful_gets);
    println!("  总时间: {:?}", final_stats.total_get_time);
    println!("  平均时间: {:?}", final_stats.average_get_time);
    println!("  预期总时间: {:?}", expected_total_time);
    
    // 允许小的误差
    let time_diff = if final_stats.total_get_time > expected_total_time {
        final_stats.total_get_time - expected_total_time
    } else {
        expected_total_time - final_stats.total_get_time
    };
    
    assert!(time_diff < Duration::from_secs(1), "时间记录应该基本一致");
    assert_eq!(final_stats.successful_gets, (num_threads * operations_per_thread) as i64);
}

#[test]
#[ignore]
fn test_stats_long_running() {
    // 长时间运行测试，检查是否有内存泄漏或性能退化
    let collector = Arc::new(StatsCollector::new());
    let test_duration = Duration::from_secs(60);
    let start = Instant::now();
    let mut iteration = 0;
    
    while start.elapsed() < test_duration {
        iteration += 1;
        
        // 执行各种操作
        collector.increment_total_connections_created();
        collector.increment_successful_gets();
        collector.record_get_time(Duration::from_millis(1));
        
        // 每1000次迭代检查一次
        if iteration % 1000 == 0 {
            let stats = collector.get_stats();
            println!("迭代 {}: 创建数={}, 获取数={}", 
                iteration, 
                stats.total_connections_created, 
                stats.successful_gets
            );
        }
    }
    
    let final_stats = collector.get_stats();
    println!("统计模块长时间运行测试结果:");
    println!("  运行时间: {:?}", test_duration);
    println!("  总迭代数: {}", iteration);
    println!("  最终统计:");
    println!("    创建连接数: {}", final_stats.total_connections_created);
    println!("    成功获取数: {}", final_stats.successful_gets);
    
    assert_eq!(final_stats.total_connections_created, iteration as i64);
    assert_eq!(final_stats.successful_gets, iteration as i64);
}

#[test]
#[ignore]
fn test_stats_calculate_average_reuse_count() {
    // 测试平均复用次数的计算
    let collector = Arc::new(StatsCollector::new());
    
    // 创建10个连接
    for _ in 0..10 {
        collector.increment_total_connections_created();
    }
    
    // 每个连接复用5次
    for _ in 0..50 {
        collector.increment_total_connections_reused();
    }
    
    let stats = collector.get_stats();
    println!("统计模块平均复用次数测试结果:");
    println!("  创建连接数: {}", stats.total_connections_created);
    println!("  复用次数: {}", stats.total_connections_reused);
    println!("  平均复用次数: {:.2}", stats.average_reuse_count);
    
    assert_eq!(stats.total_connections_created, 10);
    assert_eq!(stats.total_connections_reused, 50);
    assert_eq!(stats.average_reuse_count, 5.0);
}

#[test]
#[ignore]
fn test_stats_update_time_frequency() {
    // 测试时间更新频率控制
    let collector = Arc::new(StatsCollector::new());
    let start = Instant::now();
    
    // 快速连续操作
    for _ in 0..100000 {
        collector.increment_total_connections_created();
    }
    
    let duration = start.elapsed();
    let stats = collector.get_stats();
    
    println!("统计模块时间更新频率测试结果:");
    println!("  操作数: 100000");
    println!("  耗时: {:?}", duration);
    println!("  最后更新时间: {:?}", stats.last_update_time);
    
    // 应该很快完成，不应该因为频繁更新时间而变慢
    assert!(duration < Duration::from_secs(1), "操作应该很快完成");
}

#[test]
#[ignore]
fn test_stats_lock_contention() {
    // 测试锁竞争情况
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 200;
    let operations_per_thread = 5000;
    
    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    // 频繁调用 update_time，测试锁竞争
                    collector.increment_total_connections_created();
                    collector.increment_successful_gets();
                    collector.record_get_time(Duration::from_micros(1));
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let duration = start.elapsed();
    let final_stats = collector.get_stats();
    
    println!("统计模块锁竞争测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  总耗时: {:?}", duration);
    println!("  吞吐量: {:.2} ops/sec", 
        (num_threads * operations_per_thread) as f64 / duration.as_secs_f64());
    println!("  最终统计:");
    println!("    创建连接数: {}", final_stats.total_connections_created);
    
    // 验证数据正确性
    assert_eq!(final_stats.total_connections_created, (num_threads * operations_per_thread) as i64);
    // 性能应该合理，不应该因为锁竞争而太慢
    assert!(duration < Duration::from_secs(10), "操作应该在10秒内完成");
}
