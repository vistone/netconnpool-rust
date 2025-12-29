// Copyright (c) 2025, vistone
// All rights reserved.

// 真实环境端到端压力测试
// 启动真实服务器，使用连接池客户端发起最大压力请求

use netconnpool::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
                                Ok(0) => break, // 连接关闭
                                Ok(n) => {
                                    stats_clone.total_requests.fetch_add(1, Ordering::Relaxed);
                                    stats_clone
                                        .total_bytes_received
                                        .fetch_add(n, Ordering::Relaxed);

                                    // Echo back
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

                    // Echo back
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
fn test_real_world_max_stress() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║           真实环境最大压力测试 - 端到端测试                    ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // 服务器统计
    let server_stats = Arc::new(ServerStats {
        total_connections: AtomicUsize::new(0),
        active_connections: AtomicUsize::new(0),
        total_requests: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
        errors: AtomicUsize::new(0),
    });

    // 启动TCP服务器
    let tcp_port = 18080;
    let _tcp_listener = start_tcp_server(tcp_port, server_stats.clone());
    thread::sleep(Duration::from_millis(100));

    // 启动UDP服务器
    let udp_port = 18081;
    start_udp_server(udp_port, server_stats.clone());
    thread::sleep(Duration::from_millis(100));

    println!("✅ 服务器已启动:");
    println!("   TCP: 127.0.0.1:{}", tcp_port);
    println!("   UDP: 127.0.0.1:{}", udp_port);

    // 客户端连接池配置（利用强大硬件 - 长时间高负载）
    let mut tcp_config = default_config();
    tcp_config.dialer = Some(Box::new({
        let addr = format!("127.0.0.1:{}", tcp_port);
        move |_| {
            TcpStream::connect(&addr)
                .map(ConnectionType::Tcp)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }));
    tcp_config.max_connections = 2000; // 增加连接数以支持800线程
    tcp_config.min_connections = 500;
    tcp_config.max_idle_connections = 1000;
    tcp_config.enable_stats = true;
    tcp_config.get_connection_timeout = Duration::from_secs(30); // 增加超时时间
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
    udp_config.max_connections = 2000;
    udp_config.min_connections = 500;
    udp_config.max_idle_connections = 1000;
    udp_config.enable_stats = true;
    udp_config.get_connection_timeout = Duration::from_secs(30);
    udp_config.connection_timeout = Duration::from_secs(10);

    let tcp_pool = Arc::new(Pool::new(tcp_config).unwrap());
    let udp_pool = Arc::new(Pool::new(udp_config).unwrap());

    // 预热连接池
    thread::sleep(Duration::from_millis(500));

    // 客户端统计
    let client_stats = Arc::new(ClientStats {
        total_requests: AtomicUsize::new(0),
        success_requests: AtomicUsize::new(0),
        failed_requests: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
        connection_errors: AtomicUsize::new(0),
        timeout_errors: AtomicUsize::new(0),
    });

    // 测试配置（利用强大硬件 - 长时间高负载测试）
    let num_threads = 800; // 800个并发线程
    let requests_per_thread = 100000; // 每个线程100,000个请求
    let data_size = 4096; // 4KB per request
    let total_requests = num_threads * requests_per_thread;

    println!("\n📊 测试配置:");
    println!("   并发线程数: {}", num_threads);
    println!("   每线程请求数: {}", requests_per_thread);
    println!("   总请求数: {}", total_requests);
    println!("   每请求数据大小: {} KB", data_size / 1024);
    println!(
        "   总数据量: {:.2} GB",
        (total_requests * data_size * 2) as f64 / 1024.0 / 1024.0 / 1024.0
    );

    let test_data = vec![b'X'; data_size];
    let start_time = Instant::now();

    // 启动统计监控（长时间运行监控）
    let stats_monitor = client_stats.clone();
    let server_monitor = server_stats.clone();
    let tcp_pool_monitor = tcp_pool.clone();
    let udp_pool_monitor = udp_pool.clone();
    let monitor_stop = Arc::new(AtomicBool::new(false));
    let monitor_stop_clone = monitor_stop.clone();

    // 性能稳定性追踪（使用Arc<Mutex<>>共享）
    let qps_history = Arc::new(Mutex::new(Vec::<f64>::new()));
    let success_rate_history = Arc::new(Mutex::new(Vec::<f64>::new()));
    let reuse_rate_history = Arc::new(Mutex::new(Vec::<f64>::new()));

    let qps_history_clone = qps_history.clone();
    let success_rate_history_clone = success_rate_history.clone();
    let reuse_rate_history_clone = reuse_rate_history.clone();

    thread::spawn(move || {
        let mut last_total = 0;
        let mut last_success = 0;
        let mut interval_count = 0;

        while !monitor_stop_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(5)); // 每5秒输出一次
            interval_count += 1;

            let elapsed = start_time.elapsed().as_secs_f64();
            let total = stats_monitor.total_requests.load(Ordering::Relaxed);
            let success = stats_monitor.success_requests.load(Ordering::Relaxed);
            let failed = stats_monitor.failed_requests.load(Ordering::Relaxed);

            // 计算瞬时QPS（最近5秒的QPS）
            let interval_total = total - last_total;
            let interval_success = success - last_success;
            let interval_qps = interval_total as f64 / 5.0;
            let interval_success_rate = if interval_total > 0 {
                interval_success as f64 / interval_total as f64 * 100.0
            } else {
                100.0
            };

            // 记录历史数据
            {
                let mut qps_hist = qps_history_clone.lock().unwrap();
                let mut success_hist = success_rate_history_clone.lock().unwrap();
                qps_hist.push(interval_qps);
                success_hist.push(interval_success_rate);

                // 保持最近100个数据点
                if qps_hist.len() > 100 {
                    qps_hist.remove(0);
                    success_hist.remove(0);
                }
            }

            // 计算平均QPS和成功率
            let (avg_qps, avg_success_rate, hist_len) = {
                let qps_hist = qps_history_clone.lock().unwrap();
                let success_hist = success_rate_history_clone.lock().unwrap();
                let len = qps_hist.len();
                let avg_qps = if !qps_hist.is_empty() {
                    qps_hist.iter().sum::<f64>() / len as f64
                } else {
                    0.0
                };
                let avg_success_rate = if !success_hist.is_empty() {
                    success_hist.iter().sum::<f64>() / success_hist.len() as f64
                } else {
                    100.0
                };
                (avg_qps, avg_success_rate, len)
            };

            // 计算总体QPS和成功率
            let overall_qps = total as f64 / elapsed;
            let overall_success_rate = if total > 0 {
                success as f64 / total as f64 * 100.0
            } else {
                100.0
            };

            println!("\n╔════════════════════════════════════════════════════════════════╗");
            println!(
                "║           实时统计 - 第{}个5秒间隔 ({:.1}s)                    ║",
                interval_count, elapsed
            );
            println!("╚════════════════════════════════════════════════════════════════╝");
            println!("客户端:");
            println!("  瞬时QPS: {:.0} req/s (最近5秒)", interval_qps);
            println!(
                "  平均QPS: {:.0} req/s (最近{}个间隔)",
                avg_qps,
                hist_len.min(100)
            );
            println!("  总体QPS: {:.0} req/s (累计)", overall_qps);
            println!(
                "  总请求: {} ({:.1}%)",
                total,
                (total as f64 / total_requests as f64 * 100.0)
            );
            println!("  成功: {} ({:.2}%)", success, overall_success_rate);
            println!("  失败: {} ({:.2}%)", failed, 100.0 - overall_success_rate);
            println!("  瞬时成功率: {:.2}% (最近5秒)", interval_success_rate);
            println!(
                "  平均成功率: {:.2}% (最近{}个间隔)",
                avg_success_rate,
                hist_len.min(100)
            );
            println!(
                "  吞吐量: {:.2} MB/s",
                (stats_monitor.total_bytes_sent.load(Ordering::Relaxed)
                    + stats_monitor.total_bytes_received.load(Ordering::Relaxed))
                    as f64
                    / 1024.0
                    / 1024.0
                    / elapsed
            );

            let tcp_stats = tcp_pool_monitor.stats();
            let udp_stats = udp_pool_monitor.stats();
            let reuse_rate_tcp = if tcp_stats.successful_gets > 0 {
                tcp_stats.total_connections_reused as f64 / tcp_stats.successful_gets as f64 * 100.0
            } else {
                0.0
            };
            let reuse_rate_udp = if udp_stats.successful_gets > 0 {
                udp_stats.total_connections_reused as f64 / udp_stats.successful_gets as f64 * 100.0
            } else {
                0.0
            };

            {
                let mut reuse_hist = reuse_rate_history_clone.lock().unwrap();
                reuse_hist.push((reuse_rate_tcp + reuse_rate_udp) / 2.0);
                if reuse_hist.len() > 100 {
                    reuse_hist.remove(0);
                }
            }
            let avg_reuse_rate = {
                let reuse_hist = reuse_rate_history_clone.lock().unwrap();
                if !reuse_hist.is_empty() {
                    reuse_hist.iter().sum::<f64>() / reuse_hist.len() as f64
                } else {
                    0.0
                }
            };

            println!("连接池:");
            println!(
                "  TCP: 活跃={}, 空闲={}, 复用率={:.2}%",
                tcp_stats.current_active_connections,
                tcp_stats.current_idle_connections,
                reuse_rate_tcp
            );
            println!(
                "  UDP: 活跃={}, 空闲={}, 复用率={:.2}%",
                udp_stats.current_active_connections,
                udp_stats.current_idle_connections,
                reuse_rate_udp
            );
            println!("  平均复用率: {:.2}%", avg_reuse_rate);

            println!("服务器:");
            println!(
                "  总连接: {}, 活跃: {}",
                server_monitor.total_connections.load(Ordering::Relaxed),
                server_monitor.active_connections.load(Ordering::Relaxed)
            );
            println!(
                "  总请求: {}, 错误: {}",
                server_monitor.total_requests.load(Ordering::Relaxed),
                server_monitor.errors.load(Ordering::Relaxed)
            );
            println!(
                "  吞吐量: {:.2} MB/s",
                (server_monitor.total_bytes_received.load(Ordering::Relaxed)
                    + server_monitor.total_bytes_sent.load(Ordering::Relaxed))
                    as f64
                    / 1024.0
                    / 1024.0
                    / elapsed
            );

            // 性能稳定性检查
            if interval_count >= 10 {
                // 至少运行50秒后开始检查
                let qps_variance = {
                    let qps_hist = qps_history_clone.lock().unwrap();
                    if qps_hist.len() > 1 {
                        let mean = avg_qps;
                        let variance = qps_hist.iter().map(|&x| (x - mean).powi(2)).sum::<f64>()
                            / qps_hist.len() as f64;
                        variance.sqrt() / mean * 100.0 // 变异系数（CV）
                    } else {
                        0.0
                    }
                };

                println!("性能稳定性:");
                println!("  QPS变异系数: {:.2}% (越小越稳定)", qps_variance);
                println!("  成功率稳定性: {:.2}% (平均)", avg_success_rate);
                println!("  复用率稳定性: {:.2}% (平均)", avg_reuse_rate);

                // 性能保证检查
                if avg_success_rate >= 99.9 && avg_reuse_rate >= 99.0 && qps_variance < 20.0 {
                    println!("  ✅ 性能稳定: 成功率>99.9%, 复用率>99%, QPS波动<20%");
                } else {
                    println!("  ⚠️  性能波动: 需要关注");
                }
            }

            last_total = total;
            last_success = success;
        }
    });

    // 启动客户端线程（TCP和UDP混合）
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let tcp_pool = tcp_pool.clone();
        let udp_pool = udp_pool.clone();
        let stats = client_stats.clone();
        let data = test_data.clone();
        let use_tcp = i % 2 == 0; // 50% TCP, 50% UDP

        let handle = thread::spawn(move || {
            for _ in 0..requests_per_thread {
                stats.total_requests.fetch_add(1, Ordering::Relaxed);

                if use_tcp {
                    match tcp_pool.get_tcp() {
                        Ok(conn) => {
                            if let Some(stream_ref) = conn.tcp_conn() {
                                // 克隆连接用于读写（TcpStream的try_clone创建新的句柄，共享同一个连接）
                                match stream_ref.try_clone() {
                                    Ok(mut stream) => {
                                        // 发送数据
                                        if stream.write_all(&data).is_err() {
                                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                        stats
                                            .total_bytes_sent
                                            .fetch_add(data_size, Ordering::Relaxed);

                                        // 接收响应
                                        let mut buffer = vec![0u8; data_size];
                                        if stream.read_exact(&mut buffer).is_err() {
                                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                        stats
                                            .total_bytes_received
                                            .fetch_add(data_size, Ordering::Relaxed);
                                        stats.success_requests.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(_) => {
                                        stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            } else {
                                stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(NetConnPoolError::GetConnectionTimeout { .. }) => {
                            stats.timeout_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                } else {
                    match udp_pool.get_udp() {
                        Ok(conn) => {
                            if let Some(socket) = conn.udp_conn() {
                                // 设置接收超时（避免无限阻塞）
                                let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));

                                // 发送数据
                                if socket.send(&data).is_err() {
                                    stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                    continue;
                                }
                                stats
                                    .total_bytes_sent
                                    .fetch_add(data_size, Ordering::Relaxed);

                                // 接收响应（带超时）
                                let mut buffer = vec![0u8; data_size + 100];
                                match socket.recv(&mut buffer) {
                                    Ok(n) if n >= data_size => {
                                        stats
                                            .total_bytes_received
                                            .fetch_add(data_size, Ordering::Relaxed);
                                        stats.success_requests.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Ok(_) => {
                                        // 接收到的数据不足
                                        stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(_) => {
                                        // 超时或其他错误
                                        stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            } else {
                                stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(NetConnPoolError::GetConnectionTimeout { .. }) => {
                            stats.timeout_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                            stats.failed_requests.fetch_add(1, Ordering::Relaxed);
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

    monitor_stop.store(true, Ordering::Relaxed);
    let total_time = start_time.elapsed();

    // 最终统计
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║                      测试完成 - 最终统计                        ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    println!("⏱️  总耗时: {:.2} 秒", total_time.as_secs_f64());

    println!("\n📊 客户端统计:");
    let total = client_stats.total_requests.load(Ordering::Relaxed);
    let success = client_stats.success_requests.load(Ordering::Relaxed);
    let failed = client_stats.failed_requests.load(Ordering::Relaxed);
    let qps = total as f64 / total_time.as_secs_f64();
    let success_rate = if total > 0 {
        success as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    println!("  总请求数: {}", total);
    println!("  成功请求: {} ({:.2}%)", success, success_rate);
    println!("  失败请求: {} ({:.2}%)", failed, 100.0 - success_rate);
    println!(
        "  连接错误: {}",
        client_stats.connection_errors.load(Ordering::Relaxed)
    );
    println!(
        "  超时错误: {}",
        client_stats.timeout_errors.load(Ordering::Relaxed)
    );
    println!("  QPS: {:.0} requests/sec", qps);
    println!(
        "  吞吐量: {:.2} MB/s",
        (client_stats.total_bytes_sent.load(Ordering::Relaxed)
            + client_stats.total_bytes_received.load(Ordering::Relaxed)) as f64
            / 1024.0
            / 1024.0
            / total_time.as_secs_f64()
    );

    println!("\n📊 连接池统计:");
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

    println!("  TCP连接池:");
    println!(
        "    创建: {}, 关闭: {}, 当前: {}",
        tcp_stats.total_connections_created,
        tcp_stats.total_connections_closed,
        tcp_stats.current_connections
    );
    println!(
        "    活跃: {}, 空闲: {}",
        tcp_stats.current_active_connections, tcp_stats.current_idle_connections
    );
    println!("    复用率: {:.2}%", tcp_reuse_rate);
    println!("    平均复用次数: {:.2}", tcp_stats.average_reuse_count);

    println!("  UDP连接池:");
    println!(
        "    创建: {}, 关闭: {}, 当前: {}",
        udp_stats.total_connections_created,
        udp_stats.total_connections_closed,
        udp_stats.current_connections
    );
    println!(
        "    活跃: {}, 空闲: {}",
        udp_stats.current_active_connections, udp_stats.current_idle_connections
    );
    println!("    复用率: {:.2}%", udp_reuse_rate);
    println!("    平均复用次数: {:.2}", udp_stats.average_reuse_count);

    println!("\n📊 服务器统计:");
    println!(
        "  总连接数: {}",
        server_stats.total_connections.load(Ordering::Relaxed)
    );
    println!(
        "  当前活跃: {}",
        server_stats.active_connections.load(Ordering::Relaxed)
    );
    println!(
        "  总请求数: {}",
        server_stats.total_requests.load(Ordering::Relaxed)
    );
    println!("  错误数: {}", server_stats.errors.load(Ordering::Relaxed));
    println!(
        "  吞吐量: {:.2} MB/s",
        (server_stats.total_bytes_received.load(Ordering::Relaxed)
            + server_stats.total_bytes_sent.load(Ordering::Relaxed)) as f64
            / 1024.0
            / 1024.0
            / total_time.as_secs_f64()
    );

    // 验证结果（长时间运行的高标准验证）
    println!("\n✅ 验证结果:");

    // 计算性能稳定性指标
    let (qps_variance, avg_success_rate, _avg_reuse_rate) = {
        let qps_hist = qps_history.lock().unwrap();
        let success_hist = success_rate_history.lock().unwrap();
        let reuse_hist = reuse_rate_history.lock().unwrap();

        let qps_var = if qps_hist.len() > 1 {
            let mean = qps_hist.iter().sum::<f64>() / qps_hist.len() as f64;
            let variance =
                qps_hist.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / qps_hist.len() as f64;
            variance.sqrt() / mean * 100.0
        } else {
            0.0
        };

        let avg_success = if !success_hist.is_empty() {
            success_hist.iter().sum::<f64>() / success_hist.len() as f64
        } else {
            success_rate
        };

        let avg_reuse = if !reuse_hist.is_empty() {
            reuse_hist.iter().sum::<f64>() / reuse_hist.len() as f64
        } else {
            (tcp_reuse_rate + udp_reuse_rate) / 2.0
        };

        (qps_var, avg_success, avg_reuse)
    };

    // 高标准验证（长时间运行要求）
    assert!(
        success_rate > 99.9,
        "成功率应该超过99.9%，实际: {:.2}%",
        success_rate
    );
    assert!(
        avg_success_rate > 99.9,
        "平均成功率应该超过99.9%，实际: {:.2}%",
        avg_success_rate
    );
    assert!(qps > 50000.0, "QPS应该超过50,000，实际: {:.0}", qps);
    assert!(
        tcp_reuse_rate > 99.0,
        "TCP连接复用率应该超过99%，实际: {:.2}%",
        tcp_reuse_rate
    );
    assert!(
        udp_reuse_rate > 99.0,
        "UDP连接复用率应该超过99%，实际: {:.2}%",
        udp_reuse_rate
    );
    assert!(
        qps_variance < 30.0,
        "QPS变异系数应该小于30%，实际: {:.2}%",
        qps_variance
    );

    println!("  ✅ 总体成功率: {:.2}% (要求 > 99.9%)", success_rate);
    println!("  ✅ 平均成功率: {:.2}% (要求 > 99.9%)", avg_success_rate);
    println!("  ✅ 总体QPS: {:.0} (要求 > 50,000)", qps);
    println!("  ✅ TCP复用率: {:.2}% (要求 > 99%)", tcp_reuse_rate);
    println!("  ✅ UDP复用率: {:.2}% (要求 > 99%)", udp_reuse_rate);
    println!("  ✅ QPS稳定性: 变异系数 {:.2}% (要求 < 30%)", qps_variance);

    // 长时间运行性能保证
    if success_rate > 99.9
        && avg_success_rate > 99.9
        && tcp_reuse_rate > 99.0
        && udp_reuse_rate > 99.0
        && qps_variance < 30.0
    {
        println!("\n🎉 长时间运行性能保证: ✅ 100% 通过");
        println!("   - 成功率稳定在 99.9% 以上");
        println!("   - 连接复用率稳定在 99% 以上");
        println!("   - QPS波动控制在 30% 以内");
        println!("   - 系统长时间运行稳定");
    } else {
        println!("\n⚠️  长时间运行性能保证: 部分指标未达标");
    }

    // 清理
    tcp_pool.close().unwrap();
    udp_pool.close().unwrap();

    println!("\n🎉 真实环境最大压力测试完成！");
}
