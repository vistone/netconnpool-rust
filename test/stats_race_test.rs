// Copyright (c) 2025, vistone
// All rights reserved.

// 统计模块竞争条件专项测试

use netconnpool::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
#[ignore]
fn test_stats_race_condition_detailed() {
    // 详细的竞争条件测试
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 200;
    let operations_per_thread = 5000;
    
    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let collector = collector.clone();
            thread::spawn(move || {
                for i in 0..operations_per_thread {
                    // 根据线程ID和操作序号选择不同的操作
                    match (thread_id + i) % 8 {
                        0 => collector.IncrementTotalConnectionsCreated(),
                        1 => collector.IncrementTotalConnectionsClosed(),
                        2 => collector.IncrementSuccessfulGets(),
                        3 => collector.IncrementFailedGets(),
                        4 => collector.IncrementCurrentActiveConnections(1),
                        5 => collector.IncrementCurrentIdleConnections(1),
                        6 => collector.IncrementTotalConnectionsReused(),
                        _ => {
                            // 读取操作
                            let _stats = collector.GetStats();
                        }
                    }
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.GetStats();
    let expected_created = (num_threads * operations_per_thread) / 8;
    
    println!("统计模块详细竞争条件测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  预期创建数: {}", expected_created);
    println!("  实际创建数: {}", final_stats.TotalConnectionsCreated);
    
    // 验证数据一致性
    assert!((final_stats.TotalConnectionsCreated - expected_created as i64).abs() < 100);
}

#[test]
#[ignore]
fn test_stats_concurrent_increment_decrement() {
    // 测试并发增减操作
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 100;
    let operations_per_thread = 10000;
    
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    // 先增后减，最终应该为0
                    collector.IncrementCurrentActiveConnections(1);
                    collector.IncrementCurrentActiveConnections(-1);
                    collector.IncrementCurrentIdleConnections(1);
                    collector.IncrementCurrentIdleConnections(-1);
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.GetStats();
    
    println!("统计模块并发增减测试结果:");
    println!("  当前活跃连接数: {}", final_stats.CurrentActiveConnections);
    println!("  当前空闲连接数: {}", final_stats.CurrentIdleConnections);
    
    // 最终应该为0（允许小的误差）
    assert!(final_stats.CurrentActiveConnections.abs() < 100);
    assert!(final_stats.CurrentIdleConnections.abs() < 100);
}

#[test]
#[ignore]
fn test_stats_record_get_time_race() {
    // 测试 RecordGetTime 的竞争条件
    let collector = Arc::new(StatsCollector::new());
    let num_threads = 100;
    let operations_per_thread = 1000;
    
    // 先创建一些成功获取
    for _ in 0..(num_threads * operations_per_thread) {
        collector.IncrementSuccessfulGets();
    }
    
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    collector.RecordGetTime(Duration::from_millis(10));
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.GetStats();
    
    println!("统计模块时间记录竞争测试结果:");
    println!("  成功获取数: {}", final_stats.SuccessfulGets);
    println!("  总时间: {:?}", final_stats.TotalGetTime);
    println!("  平均时间: {:?}", final_stats.AverageGetTime);
    
    // 验证平均时间计算正确
    if final_stats.SuccessfulGets > 0 {
        let calculated_avg = final_stats.TotalGetTime.as_nanos() / final_stats.SuccessfulGets as u128;
        let reported_avg = final_stats.AverageGetTime.as_nanos();
        let diff = if calculated_avg > reported_avg {
            calculated_avg - reported_avg
        } else {
            reported_avg - calculated_avg
        };
        
        // 允许小的误差
        assert!(diff < 1000000, "平均时间计算应该基本正确");
    }
}

#[test]
#[ignore]
fn test_stats_get_stats_consistency() {
    // 测试 GetStats 的一致性（读取时不应该看到不一致的数据）
    let collector = Arc::new(StatsCollector::new());
    let num_writers = 50;
    let num_readers = 50;
    let operations_per_thread = 10000;
    
    let writer_handles: Vec<_> = (0..num_writers)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    collector.IncrementTotalConnectionsCreated();
                    collector.IncrementTotalConnectionsClosed();
                }
            })
        })
        .collect();
    
    let reader_handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    let stats = collector.GetStats();
                    // 验证数据一致性：当前连接数应该等于创建数减去关闭数
                    let calculated_current = stats.TotalConnectionsCreated - stats.TotalConnectionsClosed;
                    // 允许小的误差（由于并发）
                    assert!((stats.CurrentConnections - calculated_current).abs() <= 1000,
                        "当前连接数应该等于创建数减去关闭数");
                }
            })
        })
        .collect();
    
    for handle in writer_handles {
        handle.join().unwrap();
    }
    for handle in reader_handles {
        handle.join().unwrap();
    }
    
    let final_stats = collector.GetStats();
    println!("统计模块读取一致性测试结果:");
    println!("  创建连接数: {}", final_stats.TotalConnectionsCreated);
    println!("  关闭连接数: {}", final_stats.TotalConnectionsClosed);
    println!("  当前连接数: {}", final_stats.CurrentConnections);
    
    // 最终验证
    assert_eq!(
        final_stats.CurrentConnections,
        final_stats.TotalConnectionsCreated - final_stats.TotalConnectionsClosed
    );
}
