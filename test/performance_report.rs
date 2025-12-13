// Copyright (c) 2025, vistone
// All rights reserved.

// 性能测试报告生成器

use netconnpool::*;
use netconnpool::config::{DefaultConfig, ConnectionType};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::io::{Read, Write};

fn create_test_server() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").unwrap()
}

fn get_server_addr(listener: &TcpListener) -> String {
    format!("{}", listener.local_addr().unwrap())
}

#[test]
#[ignore]
fn generate_performance_report() {
    // 生成完整的性能测试报告
    println!("\n");
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║         NetConnPool Rust 全面性能测试报告                      ║");
    println!("║         Comprehensive Performance Test Report                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    use std::time::SystemTime;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    println!("\n测试时间: {}\n", now.as_secs());
    
    let listener = create_test_server();
    let addr = get_server_addr(&listener);
    
    // 启动测试服务器
    let server_addr = addr.clone();
    let server_handle = thread::spawn(move || {
        let server = TcpListener::bind(&server_addr).unwrap();
        for stream in server.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = vec![0u8; 8192];
                if stream.read(&mut buf).is_ok() {
                    let _ = stream.write_all(&buf);
                }
            }
        }
    });
    
    thread::sleep(Duration::from_millis(100));
    
    // 测试1: 单线程吞吐量
    println!("【测试1】单线程获取/归还吞吐量测试");
    println!("────────────────────────────────────────────────────────────");
    let mut config = DefaultConfig();
    config.Dialer = Some(Box::new({
        let addr = addr.clone();
        move || {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.MaxConnections = 100;
    config.MinConnections = 20;
    config.EnableStats = true;
    
    let pool = Arc::new(Pool::NewPool(config).unwrap());
    thread::sleep(Duration::from_millis(200));
    
    let iterations = 200000;
    let mut latencies = Vec::with_capacity(iterations);
    
    let start = Instant::now();
    for _ in 0..iterations {
        let op_start = Instant::now();
        if let Ok(conn) = pool.Get() {
            let get_time = op_start.elapsed();
            let put_start = Instant::now();
            let _ = pool.Put(conn);
            let put_time = put_start.elapsed();
            latencies.push((get_time.as_nanos() as u64, put_time.as_nanos() as u64));
        }
    }
    let duration = start.elapsed();
    
    let throughput = iterations as f64 / duration.as_secs_f64();
    let mut get_latencies: Vec<u64> = latencies.iter().map(|(g, _)| *g).collect();
    let mut put_latencies: Vec<u64> = latencies.iter().map(|(_, p)| *p).collect();
    get_latencies.sort();
    put_latencies.sort();
    
    println!("  操作数: {}", iterations);
    println!("  总耗时: {:?}", duration);
    println!("  吞吐量: {:.2} ops/sec", throughput);
    println!("  获取延迟 - P50: {:.2}μs, P95: {:.2}μs, P99: {:.2}μs",
        get_latencies[get_latencies.len()/2] as f64 / 1000.0,
        get_latencies[get_latencies.len()*95/100] as f64 / 1000.0,
        get_latencies[get_latencies.len()*99/100] as f64 / 1000.0);
    println!("  归还延迟 - P50: {:.2}μs, P95: {:.2}μs, P99: {:.2}μs",
        put_latencies[put_latencies.len()/2] as f64 / 1000.0,
        put_latencies[put_latencies.len()*95/100] as f64 / 1000.0,
        put_latencies[put_latencies.len()*99/100] as f64 / 1000.0);
    println!();
    
    // 测试2: 并发吞吐量
    println!("【测试2】高并发吞吐量测试");
    println!("────────────────────────────────────────────────────────────");
    let num_threads = 200;
    let ops_per_thread = 10000;
    let total_ops = num_threads * ops_per_thread;
    
    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                for _ in 0..ops_per_thread {
                    if let Ok(conn) = pool.Get() {
                        let _ = pool.Put(conn);
                    }
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    let duration = start.elapsed();
    let concurrent_throughput = total_ops as f64 / duration.as_secs_f64();
    
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", ops_per_thread);
    println!("  总操作数: {}", total_ops);
    println!("  总耗时: {:?}", duration);
    println!("  并发吞吐量: {:.2} ops/sec", concurrent_throughput);
    println!("  单线程平均吞吐量: {:.2} ops/sec", concurrent_throughput / num_threads as f64);
    println!();
    
    // 测试3: IO吞吐量
    println!("【测试3】IO吞吐量测试");
    println!("────────────────────────────────────────────────────────────");
    let io_threads = 50;
    let io_ops_per_thread = 2000;
    let io_data_size = 4096; // 4KB
    let mut total_io_bytes = Arc::new(std::sync::Mutex::new(0u64));
    
    let start = Instant::now();
    let io_handles: Vec<_> = (0..io_threads)
        .map(|_| {
            let pool = pool.clone();
            let total_io_bytes = total_io_bytes.clone();
            thread::spawn(move || {
                for _ in 0..io_ops_per_thread {
                    if let Ok(conn) = pool.Get() {
                        if let Some(mut stream) = conn.GetTcpConn().map(|s| s.try_clone().ok()).flatten() {
                            let data = vec![0u8; io_data_size];
                            if stream.write_all(&data).is_ok() {
                                let mut buf = vec![0u8; io_data_size];
                                if stream.read_exact(&mut buf).is_ok() {
                                    *total_io_bytes.lock().unwrap() += io_data_size as u64 * 2;
                                }
                            }
                        }
                        let _ = pool.Put(conn);
                    }
                }
            })
        })
        .collect();
    
    for handle in io_handles {
        handle.join().unwrap();
    }
    let io_duration = start.elapsed();
    let total_io = *total_io_bytes.lock().unwrap();
    let io_throughput = total_io as f64 / io_duration.as_secs_f64();
    
    println!("  IO线程数: {}", io_threads);
    println!("  每线程IO操作数: {}", io_ops_per_thread);
    println!("  数据大小: {} bytes/op", io_data_size);
    println!("  总数据传输: {:.2} MB", total_io as f64 / 1_000_000.0);
    println!("  IO耗时: {:?}", io_duration);
    println!("  IO吞吐量: {:.2} MB/s", io_throughput / 1_000_000.0);
    println!("  IO操作吞吐量: {:.2} ops/sec", 
        (io_threads * io_ops_per_thread) as f64 / io_duration.as_secs_f64());
    println!();
    
    // 测试4: 连接创建速度
    println!("【测试4】连接创建速度测试");
    println!("────────────────────────────────────────────────────────────");
    let create_count = 1000;
    let mut creation_times = Vec::with_capacity(create_count);
    
    let start = Instant::now();
    for _ in 0..create_count {
        let create_start = Instant::now();
        if let Ok(conn) = pool.Get() {
            creation_times.push(create_start.elapsed().as_nanos() as u64);
            let _ = pool.Put(conn);
        }
    }
    let create_duration = start.elapsed();
    
    creation_times.sort();
    let avg_create_time = creation_times.iter().sum::<u64>() / creation_times.len() as u64;
    let p95_create_time = creation_times[(creation_times.len() * 95) / 100];
    
    println!("  创建连接数: {}", create_count);
    println!("  总耗时: {:?}", create_duration);
    println!("  平均创建时间: {:.2} ms", avg_create_time as f64 / 1_000_000.0);
    println!("  P95创建时间: {:.2} ms", p95_create_time as f64 / 1_000_000.0);
    println!("  创建速率: {:.2} conn/sec", create_count as f64 / create_duration.as_secs_f64());
    println!();
    
    // 测试5: 统计信息收集性能
    println!("【测试5】统计信息收集性能测试");
    println!("────────────────────────────────────────────────────────────");
    let stats_iterations = 1000000;
    let mut stats_latencies = Vec::with_capacity(stats_iterations);
    
    let start = Instant::now();
    for _ in 0..stats_iterations {
        let stats_start = Instant::now();
        let _stats = pool.Stats();
        stats_latencies.push(stats_start.elapsed().as_nanos() as u64);
    }
    let stats_duration = start.elapsed();
    
    stats_latencies.sort();
    let avg_stats_latency = stats_latencies.iter().sum::<u64>() / stats_latencies.len() as u64;
    let p99_stats_latency = stats_latencies[(stats_latencies.len() * 99) / 100];
    
    println!("  操作数: {}", stats_iterations);
    println!("  总耗时: {:?}", stats_duration);
    println!("  吞吐量: {:.2} stats/sec", stats_iterations as f64 / stats_duration.as_secs_f64());
    println!("  平均延迟: {:.2} ns ({:.2} μs)", avg_stats_latency, avg_stats_latency as f64 / 1000.0);
    println!("  P99延迟: {:.2} ns ({:.2} μs)", p99_stats_latency, p99_stats_latency as f64 / 1000.0);
    println!();
    
    // 最终统计
    let final_stats = pool.Stats();
    println!("【最终统计信息】");
    println!("────────────────────────────────────────────────────────────");
    println!("  总创建连接数: {}", final_stats.TotalConnectionsCreated);
    println!("  总关闭连接数: {}", final_stats.TotalConnectionsClosed);
    println!("  当前连接数: {}", final_stats.CurrentConnections);
    println!("  成功获取数: {}", final_stats.SuccessfulGets);
    println!("  失败获取数: {}", final_stats.FailedGets);
    println!("  连接复用数: {}", final_stats.TotalConnectionsReused);
    println!("  连接复用率: {:.2}%", final_stats.AverageReuseCount * 100.0);
    println!("  平均获取时间: {:?}", final_stats.AverageGetTime);
    println!();
    
    // 性能总结
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                        性能测试总结                            ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();
    println!("单线程吞吐量:     {:.2} ops/sec", throughput);
    println!("并发吞吐量:       {:.2} ops/sec", concurrent_throughput);
    println!("IO吞吐量:         {:.2} MB/s", io_throughput / 1_000_000.0);
    println!("连接创建速率:     {:.2} conn/sec", create_count as f64 / create_duration.as_secs_f64());
    println!("统计收集吞吐量:   {:.2} stats/sec", stats_iterations as f64 / stats_duration.as_secs_f64());
    println!("连接复用率:       {:.2}%", final_stats.AverageReuseCount * 100.0);
    println!();
    
    // 性能评估
    println!("【性能评估】");
    println!("────────────────────────────────────────────────────────────");
    let mut passed = 0;
    let mut total = 0;
    
    total += 1;
    if throughput > 100000.0 {
        println!("✅ 单线程吞吐量: 优秀 ({:.2} ops/sec)", throughput);
        passed += 1;
    } else {
        println!("❌ 单线程吞吐量: 需要优化 ({:.2} ops/sec)", throughput);
    }
    
    total += 1;
    if concurrent_throughput > 200000.0 {
        println!("✅ 并发吞吐量: 优秀 ({:.2} ops/sec)", concurrent_throughput);
        passed += 1;
    } else {
        println!("❌ 并发吞吐量: 需要优化 ({:.2} ops/sec)", concurrent_throughput);
    }
    
    total += 1;
    if io_throughput > 10_000_000.0 {
        println!("✅ IO吞吐量: 优秀 ({:.2} MB/s)", io_throughput / 1_000_000.0);
        passed += 1;
    } else {
        println!("❌ IO吞吐量: 需要优化 ({:.2} MB/s)", io_throughput / 1_000_000.0);
    }
    
    total += 1;
    if final_stats.AverageReuseCount > 5.0 {
        println!("✅ 连接复用率: 优秀 ({:.2}%)", final_stats.AverageReuseCount * 100.0);
        passed += 1;
    } else {
        println!("❌ 连接复用率: 需要优化 ({:.2}%)", final_stats.AverageReuseCount * 100.0);
    }
    
    println!();
    println!("性能测试通过率: {}/{} ({:.1}%)", passed, total, passed as f64 / total as f64 * 100.0);
    println!();
    
    drop(server_handle);
}
