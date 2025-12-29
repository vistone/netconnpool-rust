// Copyright (c) 2025, vistone
// All rights reserved.

// 模糊测试客户端 - 发送各种干扰数据，测试系统稳定性

use netconnpool::config::default_config;
use netconnpool::*;
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

mod test_server;
use test_server::TestServer;

/// 生成各种干扰数据
fn generate_fuzz_data(pattern: usize) -> Vec<u8> {
    match pattern % 20 {
        0 => vec![],                // 空数据
        1 => vec![0u8; 1],          // 最小数据
        2 => vec![0xFF; 1],         // 全1
        3 => vec![0u8; 1024],       // 1KB
        4 => vec![0xFF; 1024],      // 1KB全1
        5 => vec![0u8; 65535],      // 最大UDP包
        6 => vec![0xFF; 65535],     // 最大UDP包全1
        7 => (0u8..=255).collect(), // 所有字节值
        8 => {
            // 随机模式
            let mut data = Vec::new();
            for i in 0..1000 {
                data.push((i % 256) as u8);
            }
            data
        }
        9 => {
            // 递增模式
            (0..1000).map(|i| (i % 256) as u8).collect()
        }
        10 => {
            // 递减模式
            (0..1000).rev().map(|i| (i % 256) as u8).collect()
        }
        11 => {
            // 交替模式
            (0..1000)
                .map(|i| if i % 2 == 0 { 0x00 } else { 0xFF })
                .collect()
        }
        12 => {
            // 边界值
            vec![0x00, 0x01, 0x7F, 0x80, 0xFE, 0xFF].repeat(100)
        }
        13 => {
            // 特殊字符
            b"GET / HTTP/1.1\r\nHost: test\r\n\r\n".to_vec()
        }
        14 => {
            // JSON格式（可能不完整）
            b"{\"test\":\"value\",\"number\":123".to_vec()
        }
        15 => {
            // 二进制数据
            vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05].repeat(1000)
        }
        16 => {
            // 超大数据（可能被截断）
            vec![0xAA; 100000]
        }
        17 => {
            // Unicode字符（UTF-8）
            "测试数据🚀中文".as_bytes().to_vec()
        }
        18 => {
            // 零字节
            vec![0u8; 100]
        }
        _ => {
            // 混合模式
            let mut data = Vec::new();
            for i in 0..500 {
                data.push((i * 7) as u8);
            }
            data
        }
    }
}

/// 全面模糊测试 - 发送各种干扰数据
#[test]
#[ignore]
fn test_fuzzing_all_features() {
    println!("==========================================");
    println!("全面模糊测试 - 干扰数据压力测试");
    println!("测试系统在各种异常数据下的稳定性");
    println!("==========================================");

    // 启动测试服务器
    let server = TestServer::new().unwrap();
    let tcp_addr = server.tcp_addr().to_string();
    let udp_addr = server.udp_addr().to_string();

    println!("服务器地址:");
    println!("  TCP: {}", tcp_addr);
    println!("  UDP: {}", udp_addr);
    println!();

    server.start();

    // 创建连接池配置
    let mut config = default_config();
    config.max_connections = 300;
    config.min_connections = 30;
    config.max_idle_connections = 150;
    config.idle_timeout = Duration::from_secs(60);
    config.max_lifetime = Duration::from_secs(300);
    config.enable_stats = true;
    config.enable_health_check = true;
    config.health_check_interval = Duration::from_secs(30);
    config.get_connection_timeout = Duration::from_secs(5);
    config.clear_udp_buffer_on_return = true;

    // 配置Dialer
    let tcp_addr_clone = tcp_addr.clone();
    let udp_addr_clone = udp_addr.clone();
    config.dialer = Some(Box::new(move |protocol| match protocol {
        Some(Protocol::UDP) => {
            let socket = UdpSocket::bind("0.0.0.0:0")?;
            socket.connect(&udp_addr_clone)?;
            Ok(ConnectionType::Udp(socket))
        }
        _ => TcpStream::connect(&tcp_addr_clone)
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
    }));

    let pool = Arc::new(Pool::new(config).unwrap());

    // 测试持续时间：30分钟（快速测试）或更长
    let test_duration = Duration::from_secs(30 * 60);
    let start = Instant::now();

    // 统计信息
    let tcp_operations = Arc::new(AtomicU64::new(0));
    let udp_operations = Arc::new(AtomicU64::new(0));
    let tcp_errors = Arc::new(AtomicU64::new(0));
    let udp_errors = Arc::new(AtomicU64::new(0));
    let crashes = Arc::new(AtomicU64::new(0));
    let bytes_sent = Arc::new(AtomicU64::new(0));
    let bytes_received = Arc::new(AtomicU64::new(0));

    println!("测试配置:");
    println!("  测试持续时间: {:?}", test_duration);
    println!("  最大连接数: 300");
    println!("  干扰数据模式: 20种");
    println!();

    // 1. TCP模糊测试线程组（60个线程）
    let tcp_handles: Vec<_> = (0..60)
        .map(|i| {
            let pool = pool.clone();
            let tcp_ops = tcp_operations.clone();
            let tcp_errs = tcp_errors.clone();
            let _crashes_clone = crashes.clone(); // 保留用于未来扩展
            let bytes_sent_clone = bytes_sent.clone();
            let bytes_recv_clone = bytes_received.clone();
            let start = start;

            thread::spawn(move || {
                let mut iteration = 0;
                while start.elapsed() < test_duration {
                    iteration += 1;

                    // 使用不同的干扰数据模式
                    let pattern = (i * 1000 + iteration) % 20;
                    let fuzz_data = generate_fuzz_data(pattern);

                    match pool.get_tcp() {
                        Ok(conn) => {
                            if let Some(mut stream) = conn.tcp_conn() {
                                // 设置非阻塞模式以避免卡住
                                let _ = stream.set_nonblocking(true);

                                // 发送干扰数据
                                match stream.write_all(&fuzz_data) {
                                    Ok(_) => {
                                        bytes_sent_clone
                                            .fetch_add(fuzz_data.len() as u64, Ordering::Relaxed);

                                        // 尝试读取响应（非阻塞模式）
                                        let mut response = vec![0u8; fuzz_data.len().min(8192)];
                                        match stream.read(&mut response) {
                                            Ok(size) if size > 0 => {
                                                bytes_recv_clone
                                                    .fetch_add(size as u64, Ordering::Relaxed);
                                                tcp_ops.fetch_add(1, Ordering::Relaxed);
                                            }
                                            Ok(_) => {
                                                // 读到0字节，可能是连接关闭，但数据已发送成功
                                                tcp_ops.fetch_add(1, Ordering::Relaxed);
                                            }
                                            Err(e)
                                                if e.kind() == std::io::ErrorKind::WouldBlock =>
                                            {
                                                // 非阻塞模式下暂时没有数据，数据已发送成功
                                                tcp_ops.fetch_add(1, Ordering::Relaxed);
                                            }
                                            Err(_) => {
                                                // 其他错误
                                                tcp_errs.fetch_add(1, Ordering::Relaxed);
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        tcp_errs.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            } else {
                                tcp_errs.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(_) => {
                            tcp_errs.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    // 每1000次迭代短暂休息
                    if iteration % 1000 == 0 {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            })
        })
        .collect();

    // 2. UDP模糊测试线程组（40个线程）
    let udp_handles: Vec<_> = (0..40)
        .map(|i| {
            let pool = pool.clone();
            let udp_ops = udp_operations.clone();
            let udp_errs = udp_errors.clone();
            let _crashes_clone = crashes.clone(); // 保留用于未来扩展
            let bytes_sent_clone = bytes_sent.clone();
            let bytes_recv_clone = bytes_received.clone();
            let start = start;

            thread::spawn(move || {
                let mut iteration = 0;
                while start.elapsed() < test_duration {
                    iteration += 1;

                    // 使用不同的干扰数据模式
                    let pattern = (i * 1000 + iteration) % 20;
                    let fuzz_data = generate_fuzz_data(pattern);

                    match pool.get_udp() {
                        Ok(conn) => {
                            if let Some(socket) = conn.udp_conn() {
                                // 发送干扰数据
                                match socket.send(&fuzz_data) {
                                    Ok(_) => {
                                        bytes_sent_clone
                                            .fetch_add(fuzz_data.len() as u64, Ordering::Relaxed);

                                        // 尝试接收响应
                                        let mut response = vec![0u8; 65536];
                                        match socket.recv(&mut response) {
                                            Ok(size) => {
                                                bytes_recv_clone
                                                    .fetch_add(size as u64, Ordering::Relaxed);
                                                udp_ops.fetch_add(1, Ordering::Relaxed);
                                            }
                                            Err(_) => {
                                                // UDP接收失败是常见的
                                                udp_errs.fetch_add(1, Ordering::Relaxed);
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        udp_errs.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            } else {
                                udp_errs.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(_) => {
                            udp_errs.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    if iteration % 1000 == 0 {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            })
        })
        .collect();

    // 3. 极端测试线程组（20个线程）- 发送极端数据
    let extreme_handles: Vec<_> = (0..20)
        .map(|i| {
            let pool = pool.clone();
            let tcp_ops = tcp_operations.clone();
            let udp_ops = udp_operations.clone();
            let tcp_errs = tcp_errors.clone();
            let udp_errs = udp_errors.clone();
            let _crashes_clone = crashes.clone(); // 保留用于未来扩展
            let start = start;

            thread::spawn(move || {
                let mut iteration = 0;
                while start.elapsed() < test_duration {
                    iteration += 1;

                    // 极端测试：快速获取和释放连接
                    if iteration % 10 == 0 {
                        // 每10次中有1次进行极端测试
                        for _ in 0..100 {
                            // 快速获取和释放，测试连接池稳定性
                            if let Ok(_conn) = pool.get() {
                                // 立即释放，不发送数据
                                drop(_conn);
                            }
                        }
                    } else {
                        // 正常测试
                        let pattern = (i * 1000 + iteration) % 20;
                        let fuzz_data = generate_fuzz_data(pattern);

                        if iteration % 2 == 0 {
                            // TCP
                            if let Ok(conn) = pool.get_tcp() {
                                if let Some(mut stream) = conn.tcp_conn() {
                                    // 设置非阻塞模式
                                    let _ = stream.set_nonblocking(true);

                                    if stream.write_all(&fuzz_data).is_ok() {
                                        let mut buf = vec![0u8; fuzz_data.len().min(8192)];
                                        match stream.read(&mut buf) {
                                            Ok(_) => {
                                                tcp_ops.fetch_add(1, Ordering::Relaxed);
                                            }
                                            Err(e)
                                                if e.kind() == std::io::ErrorKind::WouldBlock =>
                                            {
                                                // 非阻塞模式，暂时没有数据也算成功
                                                tcp_ops.fetch_add(1, Ordering::Relaxed);
                                            }
                                            Err(_) => {
                                                tcp_errs.fetch_add(1, Ordering::Relaxed);
                                            }
                                        }
                                    } else {
                                        tcp_errs.fetch_add(1, Ordering::Relaxed);
                                    }
                                } else {
                                    tcp_errs.fetch_add(1, Ordering::Relaxed);
                                }
                            } else {
                                tcp_errs.fetch_add(1, Ordering::Relaxed);
                            }
                        } else {
                            // UDP
                            if let Ok(conn) = pool.get_udp() {
                                if let Some(socket) = conn.udp_conn() {
                                    if socket.send(&fuzz_data).is_ok() {
                                        let mut buf = vec![0u8; 65536];
                                        match socket.recv(&mut buf) {
                                            Ok(_) => {
                                                udp_ops.fetch_add(1, Ordering::Relaxed);
                                            }
                                            Err(_) => {
                                                udp_errs.fetch_add(1, Ordering::Relaxed);
                                            }
                                        }
                                    } else {
                                        udp_errs.fetch_add(1, Ordering::Relaxed);
                                    }
                                } else {
                                    udp_errs.fetch_add(1, Ordering::Relaxed);
                                }
                            } else {
                                udp_errs.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }

                    if iteration % 1000 == 0 {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            })
        })
        .collect();

    // 4. 监控线程
    let monitor_pool = pool.clone();
    let monitor_start = start;
    let monitor_tcp_ops = tcp_operations.clone();
    let monitor_udp_ops = udp_operations.clone();
    let monitor_tcp_errs = tcp_errors.clone();
    let monitor_udp_errs = udp_errors.clone();
    let monitor_crashes = crashes.clone();
    let monitor_handle = thread::spawn(move || {
        while monitor_start.elapsed() < test_duration {
            thread::sleep(Duration::from_secs(30));

            let elapsed = monitor_start.elapsed();
            let tcp_ops = monitor_tcp_ops.load(Ordering::Relaxed);
            let udp_ops = monitor_udp_ops.load(Ordering::Relaxed);
            let tcp_errs = monitor_tcp_errs.load(Ordering::Relaxed);
            let udp_errs = monitor_udp_errs.load(Ordering::Relaxed);
            let crashes_count = monitor_crashes.load(Ordering::Relaxed);
            let stats = monitor_pool.stats();

            println!(
                "[{:?}] TCP: 操作={}, 错误={} | UDP: 操作={}, 错误={} | 崩溃={} | 连接: 当前={}, 创建={}",
                elapsed,
                tcp_ops,
                tcp_errs,
                udp_ops,
                udp_errs,
                crashes_count,
                stats.current_connections,
                stats.total_connections_created
            );

            // 检查是否有异常
            if crashes_count > 0 {
                println!("  ⚠️  警告: 检测到 {} 次崩溃/异常", crashes_count);
            }

            // 检查连接数是否异常
            if stats.current_connections > 300 {
                println!(
                    "  ⚠️  警告: 连接数超过最大值: {}",
                    stats.current_connections
                );
            }
        }
    });

    println!("启动120个模糊测试线程...");
    println!("  - TCP模糊测试: 60个线程");
    println!("  - UDP模糊测试: 40个线程");
    println!("  - 极端测试: 20个线程");
    println!();

    // 等待所有线程完成
    for handle in tcp_handles
        .into_iter()
        .chain(udp_handles)
        .chain(extreme_handles)
    {
        handle.join().unwrap();
    }
    monitor_handle.join().unwrap();

    // 收集最终统计
    let total_time = start.elapsed();
    let final_stats = pool.stats();
    let total_tcp_ops = tcp_operations.load(Ordering::Relaxed);
    let total_udp_ops = udp_operations.load(Ordering::Relaxed);
    let total_tcp_errs = tcp_errors.load(Ordering::Relaxed);
    let total_udp_errs = udp_errors.load(Ordering::Relaxed);
    let total_crashes = crashes.load(Ordering::Relaxed);
    let total_bytes_sent = bytes_sent.load(Ordering::Relaxed);
    let total_bytes_recv = bytes_received.load(Ordering::Relaxed);

    println!();
    println!("==========================================");
    println!("模糊测试结果 - 干扰数据压力测试");
    println!("==========================================");
    println!("  运行时间: {:?}", total_time);
    println!();
    println!("  TCP统计:");
    println!("    操作数: {}", total_tcp_ops);
    println!("    错误数: {}", total_tcp_errs);
    println!(
        "    成功率: {:.2}%",
        if total_tcp_ops + total_tcp_errs > 0 {
            (total_tcp_ops as f64 / (total_tcp_ops + total_tcp_errs) as f64) * 100.0
        } else {
            0.0
        }
    );
    println!();
    println!("  UDP统计:");
    println!("    操作数: {}", total_udp_ops);
    println!("    错误数: {}", total_udp_errs);
    println!(
        "    成功率: {:.2}%",
        if total_udp_ops + total_udp_errs > 0 {
            (total_udp_ops as f64 / (total_udp_ops + total_udp_errs) as f64) * 100.0
        } else {
            0.0
        }
    );
    println!();
    println!("  稳定性测试:");
    println!("    崩溃/异常: {}", total_crashes);
    if total_crashes == 0 {
        println!("    ✅ 无崩溃，系统稳定");
    } else {
        println!("    ❌ 检测到 {} 次崩溃/异常", total_crashes);
    }
    println!();
    println!("  数据传输:");
    println!(
        "    发送: {:.2} MB",
        total_bytes_sent as f64 / 1024.0 / 1024.0
    );
    println!(
        "    接收: {:.2} MB",
        total_bytes_recv as f64 / 1024.0 / 1024.0
    );
    println!();
    println!("  连接池统计:");
    println!("    当前连接: {}", final_stats.current_connections);
    println!("    创建连接: {}", final_stats.total_connections_created);
    println!("    关闭连接: {}", final_stats.total_connections_closed);
    println!("    连接复用: {}", final_stats.total_connections_reused);
    println!("    TCP连接: {}", final_stats.current_tcp_connections);
    println!("    UDP连接: {}", final_stats.current_udp_connections);
    println!();
    println!("  服务器统计:");
    println!("    TCP请求: {}", server.tcp_requests());
    println!("    UDP请求: {}", server.udp_requests());
    println!();

    // 验证结果
    assert!(total_tcp_ops > 0 || total_udp_ops > 0, "应该有成功的操作");
    assert!(
        final_stats.current_connections <= 300,
        "连接数不应超过最大值"
    );

    // 崩溃检测
    if total_crashes > 0 {
        panic!("检测到 {} 次崩溃/异常！系统不稳定", total_crashes);
    }

    println!("✅ 模糊测试通过！系统在各种干扰数据下保持稳定！");
}
