// Copyright (c) 2025, vistone
// All rights reserved.

// 全面暴力测试 - 测试各种极端场景，带内存监控和限制

use netconnpool::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// 响应时间统计
struct LatencyStats {
    times: Arc<Mutex<Vec<u64>>>, // 微秒
    total_count: AtomicUsize,
    success_count: AtomicUsize,
    timeout_count: AtomicUsize,
    error_count: AtomicUsize,
}

impl LatencyStats {
    fn new() -> Self {
        Self {
            times: Arc::new(Mutex::new(Vec::new())),
            total_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            timeout_count: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
        }
    }

    fn record(&self, latency: Duration, success: bool, is_timeout: bool) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        if success {
            self.success_count.fetch_add(1, Ordering::Relaxed);
            let mut times = self.times.lock().unwrap();
            times.push(latency.as_micros() as u64);
            // 保持最近10000个样本，避免内存爆炸
            if times.len() > 10000 {
                times.remove(0);
            }
        } else if is_timeout {
            self.timeout_count.fetch_add(1, Ordering::Relaxed);
        } else {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn get_stats(&self) -> (f64, u64, u64, f64, f64, f64, f64) {
        let times = self.times.lock().unwrap();
        let total = self.total_count.load(Ordering::Relaxed);
        let _success = self.success_count.load(Ordering::Relaxed);
        let timeout = self.timeout_count.load(Ordering::Relaxed);
        let error = self.error_count.load(Ordering::Relaxed);

        if times.is_empty() {
            return (0.0, 0, 0, 0.0, 0.0, 0.0, 0.0);
        }

        let mut sorted = times.clone();
        sorted.sort();

        let count = sorted.len();
        let sum: u64 = sorted.iter().sum();
        let avg = sum as f64 / count as f64 / 1000.0; // 转换为毫秒
        let min = sorted[0] as f64 / 1000.0;
        let max = sorted[count - 1] as f64 / 1000.0;

        let p50 = sorted[count / 2] as f64 / 1000.0;
        let p95 = sorted[(count as f64 * 0.95) as usize] as f64 / 1000.0;
        let p99 = sorted[(count as f64 * 0.99) as usize] as f64 / 1000.0;

        let packet_loss_rate = if total > 0 {
            (timeout + error) as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        (avg, min as u64, max as u64, p50, p95, p99, packet_loss_rate)
    }
}

// 内存监控
struct MemoryMonitor {
    peak_memory_mb: Arc<Mutex<f64>>,
    current_memory_mb: Arc<AtomicUsize>,
    memory_limit_mb: usize,
}

impl MemoryMonitor {
    fn new(limit_mb: usize) -> Self {
        Self {
            peak_memory_mb: Arc::new(Mutex::new(0.0)),
            current_memory_mb: Arc::new(AtomicUsize::new(0)),
            memory_limit_mb: limit_mb,
        }
    }

    fn update(&self) {
        let rss = Self::get_rss_mb();
        self.current_memory_mb.store(rss, Ordering::Relaxed);
        let mut peak = self.peak_memory_mb.lock().unwrap();
        if rss as f64 > *peak {
            *peak = rss as f64;
        }
    }

    fn get_rss_mb() -> usize {
        let pid = std::process::id();
        if let Ok(status) = std::fs::read_to_string(format!("/proc/{}/status", pid)) {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<usize>() {
                            return kb / 1024; // 转换为MB
                        }
                    }
                }
            }
        }
        0
    }

    fn check_limit(&self) -> bool {
        let current = self.current_memory_mb.load(Ordering::Relaxed);
        current < self.memory_limit_mb
    }

    fn get_stats(&self) -> (usize, f64) {
        let current = self.current_memory_mb.load(Ordering::Relaxed);
        let peak = *self.peak_memory_mb.lock().unwrap();
        (current, peak)
    }
}

// 服务器统计
struct ServerStats {
    total_connections: AtomicUsize,
    active_connections: AtomicUsize,
    total_requests: AtomicUsize,
    total_bytes_received: AtomicUsize,
    total_bytes_sent: AtomicUsize,
    errors: AtomicUsize,
}

// 客户端统计
struct ClientStats {
    total_requests: AtomicUsize,
    success_requests: AtomicUsize,
    failed_requests: AtomicUsize,
    total_bytes_sent: AtomicUsize,
    total_bytes_received: AtomicUsize,
    connection_errors: AtomicUsize,
    timeout_errors: AtomicUsize,
    latency_stats: Arc<LatencyStats>,
}

// 启动真实TCP服务器
fn start_tcp_server(port: u16, stats: Arc<ServerStats>) -> TcpListener {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
    let listener_clone = listener.try_clone().unwrap();

    thread::spawn(move || {
        for stream in listener_clone.incoming() {
            match stream {
                Ok(mut stream) => {
                    stats.total_connections.fetch_add(1, Ordering::Relaxed);
                    stats.active_connections.fetch_add(1, Ordering::Relaxed);

                    let stats_clone = stats.clone();
                    thread::spawn(move || {
                        let mut buffer = [0u8; 8192];
                        loop {
                            match stream.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(n) => {
                                    stats_clone.total_requests.fetch_add(1, Ordering::Relaxed);
                                    stats_clone
                                        .total_bytes_received
                                        .fetch_add(n, Ordering::Relaxed);

                                    if stream.write_all(&buffer[..n]).is_err() {
                                        stats_clone.errors.fetch_add(1, Ordering::Relaxed);
                                        break;
                                    }
                                    stats_clone.total_bytes_sent.fetch_add(n, Ordering::Relaxed);
                                }
                                Err(_) => {
                                    stats_clone.errors.fetch_add(1, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                        stats_clone
                            .active_connections
                            .fetch_sub(1, Ordering::Relaxed);
                    });
                }
                Err(e) => {
                    eprintln!("Server accept error: {}", e);
                    stats.errors.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    });

    listener
}

// 启动真实UDP服务器
fn start_udp_server(port: u16, stats: Arc<ServerStats>) {
    let socket = UdpSocket::bind(format!("127.0.0.1:{}", port)).unwrap();
    let socket_clone = socket.try_clone().unwrap();

    thread::spawn(move || {
        let mut buf = [0u8; 65535];
        loop {
            match socket_clone.recv_from(&mut buf) {
                Ok((n, src)) => {
                    stats.total_requests.fetch_add(1, Ordering::Relaxed);
                    stats.total_bytes_received.fetch_add(n, Ordering::Relaxed);

                    if socket_clone.send_to(&buf[..n], src).is_ok() {
                        stats.total_bytes_sent.fetch_add(n, Ordering::Relaxed);
                    } else {
                        stats.errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    eprintln!("UDP server error: {}", e);
                    stats.errors.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    });
}

#[test]
#[ignore]
fn test_extreme_stress_all_scenarios() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║        全面暴力测试 - 极端场景 + 内存监控                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // 获取系统内存信息
    let total_memory_mb = {
        let mut result = 163840; // 默认164GB
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<usize>() {
                            result = kb / 1024; // 转换为MB
                            break;
                        }
                    }
                }
            }
        }
        result
    };

    // 设置内存限制（使用系统内存的80%）
    let memory_limit_mb = (total_memory_mb as f64 * 0.8) as usize;
    let memory_monitor = Arc::new(MemoryMonitor::new(memory_limit_mb));

    println!("💾 内存监控配置:");
    println!(
        "   系统总内存: {} MB ({:.1} GB)",
        total_memory_mb,
        total_memory_mb as f64 / 1024.0
    );
    println!(
        "   内存限制: {} MB ({:.1} GB)",
        memory_limit_mb,
        memory_limit_mb as f64 / 1024.0
    );
    println!("   安全阈值: 80%\n");

    // 运行多个极端测试场景
    // 调整场景：降低负载，保护CPU温度，但仍保持全面测试
    let scenarios = vec![
        ("高频小数据", 400, 10000, 512, 28000, 28001),
        ("中频中等数据", 300, 20000, 4096, 28002, 28003),
        ("低频大数据", 200, 50000, 16384, 28004, 28005),
        ("混合负载", 350, 15000, 8192, 28006, 28007),
    ];

    let mut all_results = Vec::new();

    for (scenario_name, threads, requests_per_thread, data_size, tcp_port, udp_port) in scenarios {
        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!(
            "║  场景: {}                                              ║",
            scenario_name
        );
        println!("╚════════════════════════════════════════════════════════════════╝");
        let data_size_kb = if data_size < 1024 {
            format!("{:.1}KB", data_size as f64 / 1024.0)
        } else {
            format!("{}KB", data_size / 1024)
        };
        println!(
            "配置: {}线程 × {}请求/线程 × {}/请求",
            threads, requests_per_thread, data_size_kb
        );

        // 检查内存
        memory_monitor.update();
        if !memory_monitor.check_limit() {
            let (current, peak) = memory_monitor.get_stats();
            println!(
                "⚠️  内存使用已达限制: {} MB / {} MB (峰值: {} MB)",
                current, memory_limit_mb, peak as usize
            );
            println!("跳过此场景以避免内存溢出\n");
            continue;
        }

        let result = run_extreme_scenario(
            scenario_name,
            threads,
            requests_per_thread,
            data_size,
            tcp_port,
            udp_port,
            memory_monitor.clone(),
        );

        all_results.push((scenario_name, result));

        // 场景间充分休息，让CPU降温
        println!("\n⏸️  场景完成，休息10秒让CPU降温...");
        thread::sleep(Duration::from_secs(10));
    }

    // 最终报告
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║                   全面暴力测试总结                              ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let (final_current, final_peak) = memory_monitor.get_stats();
    println!("💾 最终内存使用:");
    println!("   当前: {} MB", final_current);
    println!("   峰值: {} MB", final_peak as usize);
    println!("   限制: {} MB", memory_limit_mb);
    println!(
        "   使用率: {:.1}%",
        final_peak as f64 / memory_limit_mb as f64 * 100.0
    );

    println!("\n📊 各场景测试结果:");
    for (name, result) in &all_results {
        if result.success {
            println!(
                "  ✅ {}: 成功率 {:.2}%, QPS {:.0}, 复用率 {:.2}%",
                name, result.success_rate, result.qps, result.reuse_rate
            );
        } else {
            println!("  ❌ {}: 失败 - {}", name, result.error_msg);
        }
    }

    // 验证所有场景都通过
    let all_passed = all_results.iter().all(|(_, r)| r.success);
    assert!(all_passed, "部分场景测试失败");

    println!("\n🎉 全面暴力测试完成！所有场景通过，内存使用安全！");
}

struct ScenarioResult {
    success: bool,
    success_rate: f64,
    qps: f64,
    reuse_rate: f64,
    error_msg: String,
}

fn run_extreme_scenario(
    _name: &str,
    num_threads: usize,
    requests_per_thread: usize,
    data_size: usize,
    tcp_port: u16,
    udp_port: u16,
    memory_monitor: Arc<MemoryMonitor>,
) -> ScenarioResult {
    // 服务器统计
    let server_stats = Arc::new(ServerStats {
        total_connections: AtomicUsize::new(0),
        active_connections: AtomicUsize::new(0),
        total_requests: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
        errors: AtomicUsize::new(0),
    });

    // 启动服务器
    println!("  启动服务器...");
    let _tcp_listener = start_tcp_server(tcp_port, server_stats.clone());
    thread::sleep(Duration::from_millis(200));
    start_udp_server(udp_port, server_stats.clone());
    thread::sleep(Duration::from_millis(200));
    println!("  服务器已启动 (TCP:{}, UDP:{})", tcp_port, udp_port);

    // 连接池配置（根据场景调整，降低连接数以减少资源占用）
    let max_conns = (num_threads * 2).min(2000); // 降低最大连接数
    let min_conns = (num_threads / 4).max(50); // 降低最小连接数

    println!("  配置连接池: max={}, min={}", max_conns, min_conns);

    let mut tcp_config = default_config();
    tcp_config.dialer = Some(Box::new({
        let addr = format!("127.0.0.1:{}", tcp_port);
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    tcp_config.max_connections = max_conns;
    tcp_config.min_connections = min_conns;
    tcp_config.max_idle_connections = max_conns / 2;
    tcp_config.enable_stats = true;
    tcp_config.get_connection_timeout = Duration::from_secs(60); // 增加超时时间
    tcp_config.connection_timeout = Duration::from_secs(10);

    let mut udp_config = default_config();
    udp_config.dialer = Some(Box::new({
        let addr = format!("127.0.0.1:{}", udp_port);
        move |_| {
            UdpSocket::bind("0.0.0.0:0")
                .and_then(|s| {
                    s.connect(&addr)?;
                    Ok(ConnectionType::Udp(s))
                })
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    udp_config.max_connections = max_conns;
    udp_config.min_connections = min_conns;
    udp_config.max_idle_connections = max_conns / 2;
    udp_config.enable_stats = true;
    udp_config.get_connection_timeout = Duration::from_secs(60); // 增加超时时间
    udp_config.connection_timeout = Duration::from_secs(10);

    println!("  创建TCP连接池...");
    let tcp_pool = match Pool::new(tcp_config) {
        Ok(p) => {
            println!("  TCP连接池创建成功");
            Arc::new(p)
        }
        Err(e) => {
            println!("  ❌ TCP连接池创建失败: {}", e);
            return ScenarioResult {
                success: false,
                success_rate: 0.0,
                qps: 0.0,
                reuse_rate: 0.0,
                error_msg: format!("TCP连接池创建失败: {}", e),
            };
        }
    };

    println!("  创建UDP连接池...");
    let udp_pool = match Pool::new(udp_config) {
        Ok(p) => {
            println!("  UDP连接池创建成功");
            Arc::new(p)
        }
        Err(e) => {
            println!("  ❌ UDP连接池创建失败: {}", e);
            return ScenarioResult {
                success: false,
                success_rate: 0.0,
                qps: 0.0,
                reuse_rate: 0.0,
                error_msg: format!("UDP连接池创建失败: {}", e),
            };
        }
    };

    println!("  预热连接池...");
    thread::sleep(Duration::from_secs(1));
    println!("  开始测试...");

    // 客户端统计（包含响应时间统计）
    let latency_stats = Arc::new(LatencyStats::new());
    let client_stats = Arc::new(ClientStats {
        total_requests: AtomicUsize::new(0),
        success_requests: AtomicUsize::new(0),
        failed_requests: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
        connection_errors: AtomicUsize::new(0),
        timeout_errors: AtomicUsize::new(0),
        latency_stats: latency_stats.clone(),
    });

    let test_data = vec![b'X'; data_size];
    let start_time = Instant::now();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let total_requests = num_threads * requests_per_thread;

    // 内存监控线程
    let memory_monitor_clone = memory_monitor.clone();
    let stop_flag_clone = stop_flag.clone();
    thread::spawn(move || {
        while !stop_flag_clone.load(Ordering::Relaxed) {
            memory_monitor_clone.update();
            if !memory_monitor_clone.check_limit() {
                println!("⚠️  内存使用超限，停止测试");
                stop_flag_clone.store(true, Ordering::Relaxed);
                break;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    // 进度监控线程（降低频率，减少CPU占用）
    let stats_clone = client_stats.clone();
    let stop_flag_progress = stop_flag.clone();
    let start_time_progress = start_time;
    thread::spawn(move || {
        let mut last_count = 0;
        while !stop_flag_progress.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(10)); // 从5秒改为10秒，减少监控频率
            let current = stats_clone.total_requests.load(Ordering::Relaxed);
            let elapsed = start_time_progress.elapsed().as_secs_f64();
            let progress = if total_requests > 0 {
                current as f64 / total_requests as f64 * 100.0
            } else {
                0.0
            };
            let qps = if elapsed > 0.0 {
                (current - last_count) as f64 / 10.0 // 对应10秒间隔
            } else {
                0.0
            };

            if current > 0 {
                let (avg_lat, _, _, _, _, _, loss_rate) = stats_clone.latency_stats.get_stats();
                println!("  进度: {:.1}% ({}/{}), 平均QPS: {:.0}, 平均延迟: {:.2}ms, 丢包率: {:.4}%, 已用时间: {:.1}s", 
                    progress, current, total_requests, qps, avg_lat, loss_rate, elapsed);
            }
            last_count = current;
        }
    });

    // 启动客户端线程
    println!("  启动 {} 个客户端线程...", num_threads);
    let mut handles = Vec::new();

    for i in 0..num_threads {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        // 每200个线程输出一次进度（减少输出频率）
        if i % 200 == 0 && i > 0 {
            println!("    已启动 {}/{} 线程", i, num_threads);
        }

        let tcp_pool = tcp_pool.clone();
        let udp_pool = udp_pool.clone();
        let stats = client_stats.clone();
        let data = test_data.clone();
        let stop = stop_flag.clone();
        let use_tcp = i % 2 == 0;

        let handle = thread::spawn(move || {
            for _ in 0..requests_per_thread {
                if stop.load(Ordering::Relaxed) {
                    break;
                }

                stats.total_requests.fetch_add(1, Ordering::Relaxed);

                if use_tcp {
                    let req_start = Instant::now();

                    match tcp_pool.get_tcp() {
                        Ok(conn) => {
                            if let Some(stream_ref) = conn.tcp_conn() {
                                match stream_ref.try_clone() {
                                    Ok(mut stream) => {
                                        if stream.write_all(&data).is_err() {
                                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                            stats.latency_stats.record(
                                                req_start.elapsed(),
                                                false,
                                                false,
                                            );
                                            continue;
                                        }
                                        stats
                                            .total_bytes_sent
                                            .fetch_add(data_size, Ordering::Relaxed);

                                        let mut buffer = vec![0u8; data_size];
                                        if stream.read_exact(&mut buffer).is_err() {
                                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                            stats.latency_stats.record(
                                                req_start.elapsed(),
                                                false,
                                                false,
                                            );
                                            continue;
                                        }
                                        stats
                                            .total_bytes_received
                                            .fetch_add(data_size, Ordering::Relaxed);
                                        stats.success_requests.fetch_add(1, Ordering::Relaxed);
                                        stats.latency_stats.record(
                                            req_start.elapsed(),
                                            true,
                                            false,
                                        );
                                    }
                                    Err(_) => {
                                        stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                        stats.latency_stats.record(
                                            req_start.elapsed(),
                                            false,
                                            false,
                                        );
                                    }
                                }
                            } else {
                                stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                stats
                                    .latency_stats
                                    .record(req_start.elapsed(), false, false);
                            }
                        }
                        Err(NetConnPoolError::GetConnectionTimeout { .. }) => {
                            stats.timeout_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                            stats.latency_stats.record(req_start.elapsed(), false, true);
                        }
                        Err(_) => {
                            stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                            stats
                                .latency_stats
                                .record(req_start.elapsed(), false, false);
                        }
                    }
                } else {
                    let req_start = Instant::now();

                    match udp_pool.get_udp() {
                        Ok(conn) => {
                            if let Some(socket) = conn.udp_conn() {
                                let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));

                                if socket.send(&data).is_err() {
                                    stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                    stats
                                        .latency_stats
                                        .record(req_start.elapsed(), false, false);
                                    continue;
                                }
                                stats
                                    .total_bytes_sent
                                    .fetch_add(data_size, Ordering::Relaxed);

                                let mut buffer = vec![0u8; data_size + 100];
                                match socket.recv(&mut buffer) {
                                    Ok(n) if n >= data_size => {
                                        stats
                                            .total_bytes_received
                                            .fetch_add(data_size, Ordering::Relaxed);
                                        stats.success_requests.fetch_add(1, Ordering::Relaxed);
                                        stats.latency_stats.record(
                                            req_start.elapsed(),
                                            true,
                                            false,
                                        );
                                    }
                                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                                        stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                        stats.latency_stats.record(
                                            req_start.elapsed(),
                                            false,
                                            true,
                                        );
                                    }
                                    _ => {
                                        stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                        stats.latency_stats.record(
                                            req_start.elapsed(),
                                            false,
                                            false,
                                        );
                                    }
                                }
                            } else {
                                stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                stats
                                    .latency_stats
                                    .record(req_start.elapsed(), false, false);
                            }
                        }
                        Err(NetConnPoolError::GetConnectionTimeout { .. }) => {
                            stats.timeout_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                            stats.latency_stats.record(req_start.elapsed(), false, true);
                        }
                        Err(_) => {
                            stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                            stats
                                .latency_stats
                                .record(req_start.elapsed(), false, false);
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    println!("  所有线程已启动，等待完成...");

    // 等待完成（带超时保护，增加超时时间以降低CPU压力）
    let max_wait_time = Duration::from_secs(600); // 最多等待10分钟
    let wait_start = Instant::now();

    for (idx, handle) in handles.into_iter().enumerate() {
        if wait_start.elapsed() > max_wait_time {
            println!("  ⚠️  等待超时，强制停止");
            stop_flag.store(true, Ordering::Relaxed);
            break;
        }

        // 每200个线程输出一次进度（减少输出频率）
        if idx % 200 == 0 && idx > 0 {
            let completed = client_stats.total_requests.load(Ordering::Relaxed);
            let progress = if total_requests > 0 {
                completed as f64 / total_requests as f64 * 100.0
            } else {
                0.0
            };
            println!(
                "    线程完成进度: {}/{} ({:.1}%)",
                idx, num_threads, progress
            );
        }

        if let Err(e) = handle.join() {
            eprintln!("  线程 {} 异常: {:?}", idx, e);
        }
    }

    println!("  所有线程已完成");

    stop_flag.store(true, Ordering::Relaxed);
    let total_time = start_time.elapsed();

    // 计算统计
    let total = client_stats.total_requests.load(Ordering::Relaxed);
    let success_count = client_stats.success_requests.load(Ordering::Relaxed);
    let success_rate = if total > 0 {
        success_count as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let qps = total as f64 / total_time.as_secs_f64();

    let tcp_stats = tcp_pool.stats();
    let udp_stats = udp_pool.stats();
    let tcp_reuse_rate = if tcp_stats.successful_gets > 0 {
        tcp_stats.total_connections_reused as f64 / tcp_stats.successful_gets as f64 * 100.0
    } else {
        0.0
    };
    let udp_reuse_rate = if udp_stats.successful_gets > 0 {
        udp_stats.total_connections_reused as f64 / udp_stats.successful_gets as f64 * 100.0
    } else {
        0.0
    };
    let reuse_rate = (tcp_reuse_rate + udp_reuse_rate) / 2.0;

    // 获取响应时间和丢包率统计
    let (avg_latency, min_latency, max_latency, p50, p95, p99, packet_loss_rate) =
        latency_stats.get_stats();

    // 清理
    let _ = tcp_pool.close();
    let _ = udp_pool.close();

    // 验证结果（成功率、复用率、丢包率都要满足要求）
    let test_passed = success_rate > 99.0
        && reuse_rate > 95.0
        && packet_loss_rate < 1.0
        && !stop_flag.load(Ordering::Relaxed);

    println!("\n📊 响应时间统计:");
    println!("  平均响应时间: {:.2} ms", avg_latency);
    println!("  最小响应时间: {} ms", min_latency);
    println!("  最大响应时间: {} ms", max_latency);
    println!("  P50 (中位数): {:.2} ms", p50);
    println!("  P95: {:.2} ms", p95);
    println!("  P99: {:.2} ms", p99);
    println!("\n📊 丢包率统计:");
    println!("  总请求数: {}", total);
    println!("  成功请求: {} ({:.2}%)", success_count, success_rate);
    println!(
        "  失败请求: {}",
        client_stats.failed_requests.load(Ordering::Relaxed)
    );
    println!(
        "  超时请求: {}",
        client_stats.timeout_errors.load(Ordering::Relaxed)
    );
    println!(
        "  连接错误: {}",
        client_stats.connection_errors.load(Ordering::Relaxed)
    );
    println!("  丢包率: {:.4}% (超时+错误)", packet_loss_rate);

    if test_passed {
        println!("\n✅ 场景完成: 成功率 {:.2}%, QPS {:.0}, 复用率 {:.2}%, 平均延迟 {:.2}ms, 丢包率 {:.4}%", 
            success_rate, qps, reuse_rate, avg_latency, packet_loss_rate);
    } else {
        println!("\n❌ 场景失败: 成功率 {:.2}%, QPS {:.0}, 复用率 {:.2}%, 平均延迟 {:.2}ms, 丢包率 {:.4}%", 
            success_rate, qps, reuse_rate, avg_latency, packet_loss_rate);
    }

    ScenarioResult {
        success: test_passed,
        success_rate,
        qps,
        reuse_rate,
        error_msg: if stop_flag.load(Ordering::Relaxed) {
            "内存超限".to_string()
        } else {
            String::new()
        },
    }
}
