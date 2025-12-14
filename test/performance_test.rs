// Copyright (c) 2025, vistone
// All rights reserved.

// 全面性能测试套件 - 记录速度、时间、IO吞吐量等关键指标

use netconnpool::config::{default_config, ConnectionType};
use netconnpool::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// 性能测试结果
#[allow(dead_code)]
struct PerformanceResult {
    test_name: String,
    operations: u64,
    duration: Duration,
    throughput_ops_per_sec: f64,
    avg_latency_ns: u64,
    min_latency_ns: u64,
    max_latency_ns: u64,
    p50_latency_ns: u64,
    p95_latency_ns: u64,
    p99_latency_ns: u64,
    io_throughput_bytes_per_sec: f64,
    memory_usage_mb: f64,
    cpu_usage_percent: f64,
}

impl PerformanceResult {
    fn print(&self) {
        println!("\n========================================");
        println!("测试: {}", self.test_name);
        println!("========================================");
        println!("操作数: {}", self.operations);
        println!("总耗时: {:?}", self.duration);
        println!("吞吐量: {:.2} ops/sec", self.throughput_ops_per_sec);
        println!("平均延迟: {:.2} μs", self.avg_latency_ns as f64 / 1000.0);
        println!("最小延迟: {:.2} μs", self.min_latency_ns as f64 / 1000.0);
        println!("最大延迟: {:.2} μs", self.max_latency_ns as f64 / 1000.0);
        println!("P50延迟: {:.2} μs", self.p50_latency_ns as f64 / 1000.0);
        println!("P95延迟: {:.2} μs", self.p95_latency_ns as f64 / 1000.0);
        println!("P99延迟: {:.2} μs", self.p99_latency_ns as f64 / 1000.0);
        if self.io_throughput_bytes_per_sec > 0.0 {
            println!(
                "IO吞吐量: {:.2} MB/s",
                self.io_throughput_bytes_per_sec / 1_000_000.0
            );
        }
        println!("========================================\n");
    }
}

fn create_test_server() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").unwrap()
}

fn get_server_addr(listener: &TcpListener) -> String {
    format!("{}", listener.local_addr().unwrap())
}

#[test]
#[ignore]
fn test_get_put_throughput() {
    // 测试获取/归还操作的吞吐量
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    let max_conns = 100;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.min_connections = 20; // 预热
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    // 等待预热完成
    thread::sleep(Duration::from_millis(200));

    let iterations = 100000;
    let mut latencies = Vec::with_capacity(iterations);

    let start = Instant::now();
    for _ in 0..iterations {
        let op_start = Instant::now();
        if let Ok(conn) = pool.get() {
            drop(conn);
        }
        latencies.push(op_start.elapsed().as_nanos() as u64);
    }
    let duration = start.elapsed();

    // 计算统计信息
    latencies.sort();
    let avg_latency = latencies.iter().sum::<u64>() / latencies.len() as u64;
    let min_latency = latencies[0];
    let max_latency = latencies[latencies.len() - 1];
    let p50_latency = latencies[latencies.len() / 2];
    let p95_latency = latencies[(latencies.len() * 95) / 100];
    let p99_latency = latencies[(latencies.len() * 99) / 100];

    let result = PerformanceResult {
        test_name: "获取/归还操作吞吐量测试".to_string(),
        operations: iterations as u64,
        duration,
        throughput_ops_per_sec: iterations as f64 / duration.as_secs_f64(),
        avg_latency_ns: avg_latency,
        min_latency_ns: min_latency,
        max_latency_ns: max_latency,
        p50_latency_ns: p50_latency,
        p95_latency_ns: p95_latency,
        p99_latency_ns: p99_latency,
        io_throughput_bytes_per_sec: 0.0,
        memory_usage_mb: 0.0,
        cpu_usage_percent: 0.0,
    };

    result.print();

    // 性能要求：吞吐量应该 > 100,000 ops/sec
    assert!(
        result.throughput_ops_per_sec > 100000.0,
        "吞吐量应该超过100,000 ops/sec，实际: {:.2}",
        result.throughput_ops_per_sec
    );
    // P99延迟应该 < 100微秒
    assert!(
        result.p99_latency_ns < 100_000,
        "P99延迟应该小于100微秒，实际: {:.2}微秒",
        result.p99_latency_ns as f64 / 1000.0
    );
}

#[test]
#[ignore]
fn test_concurrent_throughput() {
    // 测试并发吞吐量
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    let max_conns = 200;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.min_connections = 50;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    thread::sleep(Duration::from_millis(200));

    let num_threads = 100;
    let operations_per_thread = 5000;
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

    let throughput = total_operations as f64 / duration.as_secs_f64();

    let result = PerformanceResult {
        test_name: "并发吞吐量测试".to_string(),
        operations: total_operations as u64,
        duration,
        throughput_ops_per_sec: throughput,
        avg_latency_ns: 0,
        min_latency_ns: 0,
        max_latency_ns: 0,
        p50_latency_ns: 0,
        p95_latency_ns: 0,
        p99_latency_ns: 0,
        io_throughput_bytes_per_sec: 0.0,
        memory_usage_mb: 0.0,
        cpu_usage_percent: 0.0,
    };

    result.print();

    // 性能要求：并发吞吐量应该 > 200,000 ops/sec
    assert!(
        throughput > 200000.0,
        "并发吞吐量应该超过200,000 ops/sec，实际: {:.2}",
        throughput
    );
}

#[test]
#[ignore]
fn test_io_throughput() {
    // 测试IO吞吐量（通过连接进行数据传输）
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    // 启动服务器线程
    let server_addr = addr.clone();
    let server_handle = thread::spawn(move || {
        let server = TcpListener::bind(&server_addr).unwrap();
        for stream in server.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(&buf);
            }
        }
    });

    thread::sleep(Duration::from_millis(100));

    let mut config = default_config();
    let max_conns = 50;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.min_connections = 10;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    thread::sleep(Duration::from_millis(200));

    let data_size = 1024; // 1KB per operation
    let iterations = 10000;
    let mut total_bytes = 0u64;

    let start = Instant::now();
    for _ in 0..iterations {
        if let Ok(conn) = pool.get() {
            if let Some(stream_ref) = conn.tcp_conn() {
                if let Ok(mut stream) = stream_ref.try_clone() {
                    let data = vec![0u8; data_size];
                    if stream.write_all(&data).is_ok() {
                        let mut buf = vec![0u8; data_size];
                        if stream.read_exact(&mut buf).is_ok() {
                            total_bytes += data_size as u64 * 2; // 读写各一次
                        }
                    }
                }
            }
            drop(conn);
        }
    }
    let duration = start.elapsed();

    let io_throughput = total_bytes as f64 / duration.as_secs_f64();

    let result = PerformanceResult {
        test_name: "IO吞吐量测试".to_string(),
        operations: iterations,
        duration,
        throughput_ops_per_sec: iterations as f64 / duration.as_secs_f64(),
        avg_latency_ns: 0,
        min_latency_ns: 0,
        max_latency_ns: 0,
        p50_latency_ns: 0,
        p95_latency_ns: 0,
        p99_latency_ns: 0,
        io_throughput_bytes_per_sec: io_throughput,
        memory_usage_mb: 0.0,
        cpu_usage_percent: 0.0,
    };

    result.print();

    // IO吞吐量应该 > 10 MB/s
    assert!(
        io_throughput > 10_000_000.0,
        "IO吞吐量应该超过10 MB/s，实际: {:.2} MB/s",
        io_throughput / 1_000_000.0
    );

    drop(server_handle);
}

#[test]
#[ignore]
fn test_latency_distribution() {
    // 测试延迟分布
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    let max_conns = 100;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.min_connections = 20;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    thread::sleep(Duration::from_millis(200));

    let iterations = 50000;
    let mut latencies = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let op_start = Instant::now();
        if let Ok(conn) = pool.get() {
            let get_time = op_start.elapsed();
            let put_start = Instant::now();
            drop(conn);
            let put_time = put_start.elapsed();
            latencies.push((get_time.as_nanos() as u64, put_time.as_nanos() as u64));
        }
    }

    // 分离获取和归还的延迟
    let mut get_latencies: Vec<u64> = latencies.iter().map(|(g, _)| *g).collect();
    let mut put_latencies: Vec<u64> = latencies.iter().map(|(_, p)| *p).collect();

    get_latencies.sort();
    put_latencies.sort();

    let calculate_percentiles = |latencies: &[u64]| -> (u64, u64, u64, u64, u64, u64) {
        let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;
        let min = latencies[0];
        let max = latencies[latencies.len() - 1];
        let p50 = latencies[latencies.len() / 2];
        let p95 = latencies[(latencies.len() * 95) / 100];
        let p99 = latencies[(latencies.len() * 99) / 100];
        (avg, min, max, p50, p95, p99)
    };

    let (get_avg, get_min, get_max, get_p50, get_p95, get_p99) =
        calculate_percentiles(&get_latencies);
    let (put_avg, put_min, put_max, put_p50, put_p95, put_p99) =
        calculate_percentiles(&put_latencies);

    println!("\n========================================");
    println!("延迟分布测试");
    println!("========================================");
    println!("操作数: {}", iterations);
    println!("\n获取操作延迟:");
    println!("  平均: {:.2} μs", get_avg as f64 / 1000.0);
    println!("  最小: {:.2} μs", get_min as f64 / 1000.0);
    println!("  最大: {:.2} μs", get_max as f64 / 1000.0);
    println!("  P50:  {:.2} μs", get_p50 as f64 / 1000.0);
    println!("  P95:  {:.2} μs", get_p95 as f64 / 1000.0);
    println!("  P99:  {:.2} μs", get_p99 as f64 / 1000.0);
    println!("\n归还操作延迟:");
    println!("  平均: {:.2} μs", put_avg as f64 / 1000.0);
    println!("  最小: {:.2} μs", put_min as f64 / 1000.0);
    println!("  最大: {:.2} μs", put_max as f64 / 1000.0);
    println!("  P50:  {:.2} μs", put_p50 as f64 / 1000.0);
    println!("  P95:  {:.2} μs", put_p95 as f64 / 1000.0);
    println!("  P99:  {:.2} μs", put_p99 as f64 / 1000.0);
    println!("========================================\n");

    // 性能要求
    assert!(get_p99 < 50_000, "获取操作P99延迟应该小于50微秒");
    assert!(put_p99 < 10_000, "归还操作P99延迟应该小于10微秒");
}

#[test]
#[ignore]
fn test_connection_creation_speed() {
    // 测试连接创建速度
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    let max_conns = 1000;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.min_connections = 0;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    let num_connections = 500;
    let mut creation_times = Vec::with_capacity(num_connections);

    let start = Instant::now();
    for _ in 0..num_connections {
        let create_start = Instant::now();
        if let Ok(conn) = pool.get() {
            creation_times.push(create_start.elapsed().as_nanos() as u64);
            drop(conn);
        }
    }
    let total_duration = start.elapsed();

    creation_times.sort();
    let avg_time = creation_times.iter().sum::<u64>() / creation_times.len() as u64;
    let p95_time = creation_times[(creation_times.len() * 95) / 100];

    println!("\n========================================");
    println!("连接创建速度测试");
    println!("========================================");
    println!("创建连接数: {}", num_connections);
    println!("总耗时: {:?}", total_duration);
    println!("平均创建时间: {:.2} ms", avg_time as f64 / 1_000_000.0);
    println!("P95创建时间: {:.2} ms", p95_time as f64 / 1_000_000.0);
    println!(
        "创建速率: {:.2} conn/sec",
        num_connections as f64 / total_duration.as_secs_f64()
    );
    println!("========================================\n");

    // 性能要求：平均创建时间应该 < 10ms
    assert!(
        avg_time < 10_000_000,
        "平均连接创建时间应该小于10ms，实际: {:.2}ms",
        avg_time as f64 / 1_000_000.0
    );
}

#[test]
#[ignore]
fn test_high_load_io_throughput() {
    // 高负载IO吞吐量测试
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    // 启动高性能服务器
    let server_addr = addr.clone();
    let server_handle = thread::spawn(move || {
        let server = TcpListener::bind(&server_addr).unwrap();
        for stream in server.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = vec![0u8; 8192];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(&buf);
            }
        }
    });

    thread::sleep(Duration::from_millis(100));

    let mut config = default_config();
    let max_conns = 100;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.min_connections = 20;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    thread::sleep(Duration::from_millis(200));

    let num_threads = 50;
    let operations_per_thread = 1000;
    let data_size = 8192; // 8KB per operation
    let total_bytes = Arc::new(std::sync::Mutex::new(0u64));

    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            let total_bytes = total_bytes.clone();
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    if let Ok(conn) = pool.get() {
                        if let Some(stream_ref) = conn.tcp_conn() {
                            if let Ok(mut stream) = stream_ref.try_clone() {
                                let data = vec![0u8; data_size];
                                if stream.write_all(&data).is_ok() {
                                    let mut buf = vec![0u8; data_size];
                                    if stream.read_exact(&mut buf).is_ok() {
                                        *total_bytes.lock().unwrap() += data_size as u64 * 2;
                                    }
                                }
                            }
                        }
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

    let total = *total_bytes.lock().unwrap();
    let io_throughput = total as f64 / duration.as_secs_f64();

    println!("\n========================================");
    println!("高负载IO吞吐量测试");
    println!("========================================");
    println!("线程数: {}", num_threads);
    println!("每线程操作数: {}", operations_per_thread);
    println!("数据大小: {} bytes/op", data_size);
    println!("总数据传输: {:.2} MB", total as f64 / 1_000_000.0);
    println!("总耗时: {:?}", duration);
    println!("IO吞吐量: {:.2} MB/s", io_throughput / 1_000_000.0);
    println!(
        "操作吞吐量: {:.2} ops/sec",
        (num_threads * operations_per_thread) as f64 / duration.as_secs_f64()
    );
    println!("========================================\n");

    // 性能要求：IO吞吐量应该 > 50 MB/s
    assert!(
        io_throughput > 50_000_000.0,
        "高负载IO吞吐量应该超过50 MB/s，实际: {:.2} MB/s",
        io_throughput / 1_000_000.0
    );

    drop(server_handle);
}

#[test]
#[ignore]
fn test_stats_collection_performance() {
    // 测试统计信息收集的性能
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    let mut config = default_config();
    let max_conns = 100;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());

    // 执行一些操作
    for _ in 0..10000 {
        if let Ok(conn) = pool.get() {
            drop(conn);
        }
    }

    let iterations = 1000000;
    let mut latencies = Vec::with_capacity(iterations);

    let start = Instant::now();
    for _ in 0..iterations {
        let op_start = Instant::now();
        let _stats = pool.stats();
        latencies.push(op_start.elapsed().as_nanos() as u64);
    }
    let duration = start.elapsed();

    latencies.sort();
    let avg_latency = latencies.iter().sum::<u64>() / latencies.len() as u64;
    let p99_latency = latencies[(latencies.len() * 99) / 100];

    println!("\n========================================");
    println!("统计信息收集性能测试");
    println!("========================================");
    println!("操作数: {}", iterations);
    println!("总耗时: {:?}", duration);
    println!(
        "吞吐量: {:.2} stats/sec",
        iterations as f64 / duration.as_secs_f64()
    );
    println!("平均延迟: {:.2} ns", avg_latency);
    println!("P99延迟: {:.2} ns", p99_latency);
    println!("========================================\n");

    // 性能要求：统计信息收集应该很快
    assert!(
        avg_latency < 10_000,
        "统计信息收集平均延迟应该小于10微秒，实际: {:.2}微秒",
        avg_latency as f64 / 1000.0
    );
    assert!(
        p99_latency < 50_000,
        "统计信息收集P99延迟应该小于50微秒，实际: {:.2}微秒",
        p99_latency as f64 / 1000.0
    );
}

#[test]
#[ignore]
fn test_comprehensive_performance() {
    // 综合性能测试 - 模拟真实场景
    let listener = create_test_server();
    let addr = get_server_addr(&listener);

    // 启动服务器
    let server_addr = addr.clone();
    let server_handle = thread::spawn(move || {
        let server = TcpListener::bind(&server_addr).unwrap();
        for stream in server.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(&buf);
            }
        }
    });

    thread::sleep(Duration::from_millis(100));

    let mut config = default_config();
    let max_conns = 200;
    config.dialer = Some(Box::new({
        let addr = addr.clone();
        move |_| {
            TcpStream::connect(&addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    config.max_connections = max_conns;
    config.min_connections = 50;
    config.enable_stats = true;

    let pool = Arc::new(Pool::new(config).unwrap());
    thread::sleep(Duration::from_millis(200));

    let num_threads = 100;
    let operations_per_thread = 10000;
    let total_operations = num_threads * operations_per_thread;
    let total_io_bytes = Arc::new(std::sync::Mutex::new(0u64));

    let start = Instant::now();
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let pool = pool.clone();
            let total_io_bytes = total_io_bytes.clone();
            thread::spawn(move || {
                for i in 0..operations_per_thread {
                    match i % 3 {
                        0 => {
                            // 纯获取/归还
                            if let Ok(conn) = pool.get() {
                                drop(conn);
                            }
                        }
                        1 => {
                            // 带IO操作
                            if let Ok(conn) = pool.get() {
                                if let Some(stream_ref) = conn.tcp_conn() {
                                    if let Ok(mut stream) = stream_ref.try_clone() {
                                        let data = vec![0u8; 512];
                                        if stream.write_all(&data).is_ok() {
                                            let mut buf = vec![0u8; 512];
                                            if stream.read_exact(&mut buf).is_ok() {
                                                *total_io_bytes.lock().unwrap() += 1024;
                                            }
                                        }
                                    }
                                }
                                drop(conn);
                            }
                        }
                        _ => {
                            // 获取统计信息
                            let _stats = pool.stats();
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
    let duration = start.elapsed();

    let total_io = *total_io_bytes.lock().unwrap();
    let stats = pool.stats();

    println!("\n========================================");
    println!("综合性能测试");
    println!("========================================");
    println!("线程数: {}", num_threads);
    println!("每线程操作数: {}", operations_per_thread);
    println!("总操作数: {}", total_operations);
    println!("总耗时: {:?}", duration);
    println!(
        "操作吞吐量: {:.2} ops/sec",
        total_operations as f64 / duration.as_secs_f64()
    );
    println!(
        "IO吞吐量: {:.2} MB/s",
        total_io as f64 / duration.as_secs_f64() / 1_000_000.0
    );
    println!("\n连接池统计:");
    println!("  创建连接数: {}", stats.total_connections_created);
    println!("  成功获取数: {}", stats.successful_gets);
    println!("  连接复用数: {}", stats.total_connections_reused);
    println!("  连接复用率: {:.2}%", stats.average_reuse_count * 100.0);
    println!("========================================\n");

    // 性能要求
    let ops_per_sec = total_operations as f64 / duration.as_secs_f64();
    assert!(
        ops_per_sec > 100000.0,
        "综合操作吞吐量应该超过100,000 ops/sec，实际: {:.2}",
        ops_per_sec
    );
    assert!(
        stats.average_reuse_count > 5.0,
        "连接复用率应该超过5，实际: {:.2}",
        stats.average_reuse_count
    );

    drop(server_handle);
}
