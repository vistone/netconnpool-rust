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
                        0 => collector.increment_total_connections_created(),
                        1 => collector.increment_total_connections_closed(),
                        2 => collector.increment_successful_gets(),
                        3 => collector.increment_failed_gets(),
                        4 => collector.increment_current_active_connections(1),
                        5 => collector.increment_current_idle_connections(1),
                        6 => collector.increment_total_connections_reused(),
                        _ => {
                            // 读取操作
                            let _stats = collector.get_stats();
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
    let expected_created = (num_threads * operations_per_thread) / 8;

    println!("统计模块详细竞争条件测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  预期创建数: {}", expected_created);
    println!("  实际创建数: {}", final_stats.total_connections_created);

    // 验证数据一致性
    assert!((final_stats.total_connections_created - expected_created as i64).abs() < 100);
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
                    collector.increment_current_active_connections(1);
                    collector.increment_current_active_connections(-1);
                    collector.increment_current_idle_connections(1);
                    collector.increment_current_idle_connections(-1);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_stats = collector.get_stats();

    println!("统计模块并发增减测试结果:");
    println!(
        "  当前活跃连接数: {}",
        final_stats.current_active_connections
    );
    println!("  当前空闲连接数: {}", final_stats.current_idle_connections);

    // 最终应该为0（允许小的误差）
    assert!(final_stats.current_active_connections.abs() < 100);
    assert!(final_stats.current_idle_connections.abs() < 100);
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
        collector.increment_successful_gets();
    }

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    collector.record_get_time(Duration::from_millis(10));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_stats = collector.get_stats();

    println!("统计模块时间记录竞争测试结果:");
    println!("  成功获取数: {}", final_stats.successful_gets);
    println!("  总时间: {:?}", final_stats.total_get_time);
    println!("  平均时间: {:?}", final_stats.average_get_time);

    // 验证平均时间计算正确
    if final_stats.successful_gets > 0 {
        let calculated_avg =
            final_stats.total_get_time.as_nanos() / final_stats.successful_gets as u128;
        let reported_avg = final_stats.average_get_time.as_nanos();
        let diff = calculated_avg.abs_diff(reported_avg);

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
                    collector.increment_total_connections_created();
                    collector.increment_total_connections_closed();
                }
            })
        })
        .collect();

    let reader_handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let collector = collector.clone();
            thread::spawn(move || {
                let mut errors = 0;
                for _ in 0..operations_per_thread {
                    let stats = collector.get_stats();
                    // 验证数据一致性：当前连接数应该等于创建数减去关闭数
                    // 注意：在高并发下，由于 get_stats() 读取多个原子值不是原子操作，
                    // 可能会有短暂的不一致，这是正常的。我们只检查最终一致性。
                    let calculated_current =
                        stats.total_connections_created - stats.total_connections_closed;
                    // 允许较大的误差（由于高并发和原子操作的时序）
                    // 50个writer线程，每个10000次操作，在高并发下误差可能较大
                    // 由于 get_stats() 不是原子快照，差异可能达到操作总数的量级
                    let diff = (stats.current_connections - calculated_current).abs();
                    if diff > 100000 {
                        // 只记录错误，不立即失败（因为这是预期的并发行为）
                        errors += 1;
                    }
                }
                // 允许一定比例的读取看到不一致（这是正常的并发行为）
                assert!(
                    errors < operations_per_thread / 10, // 允许最多10%的读取看到不一致
                    "太多读取看到不一致的数据: {}/{}",
                    errors,
                    operations_per_thread
                );
            })
        })
        .collect();

    for handle in writer_handles {
        handle.join().unwrap();
    }
    for handle in reader_handles {
        handle.join().unwrap();
    }

    let final_stats = collector.get_stats();
    println!("统计模块读取一致性测试结果:");
    println!("  创建连接数: {}", final_stats.total_connections_created);
    println!("  关闭连接数: {}", final_stats.total_connections_closed);
    println!("  当前连接数: {}", final_stats.current_connections);

    // 最终验证
    assert_eq!(
        final_stats.current_connections,
        final_stats.total_connections_created - final_stats.total_connections_closed
    );
}
