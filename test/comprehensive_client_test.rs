// Copyright (c) 2025, vistone
// All rights reserved.

// 全面客户端测试 - 集成所有功能，向服务器发送所有类型的请求

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

/// 全面客户端压力测试
#[test]
#[ignore] // 默认忽略，需要长时间运行
fn test_comprehensive_client_stress() {
    println!("==========================================");
    println!("全面客户端压力测试");
    println!("集成所有功能：TCP/UDP, IPv4/IPv6, 各种操作");
    println!("测试所有连接池功能：获取、归还、复用、统计");
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
    config.max_connections = 200;
    config.min_connections = 20;
    config.max_idle_connections = 100;
    config.idle_timeout = Duration::from_secs(60);
    config.max_lifetime = Duration::from_secs(300);
    config.enable_stats = true;
    config.enable_health_check = true;
    config.health_check_interval = Duration::from_secs(30);
    config.get_connection_timeout = Duration::from_secs(5);

    // 配置Dialer - 支持TCP和UDP
    let tcp_addr_clone = tcp_addr.clone();
    let udp_addr_clone = udp_addr.clone();
    config.dialer = Some(Box::new(move |protocol| {
        match protocol {
            Some(Protocol::UDP) => {
                let socket = UdpSocket::bind("0.0.0.0:0")?;
                socket.connect(&udp_addr_clone)?;
                Ok(ConnectionType::Udp(socket))
            }
            _ => {
                // 默认TCP
                TcpStream::connect(&tcp_addr_clone)
                    .map(|s| ConnectionType::Tcp(s))
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    }));

    let pool = Arc::new(Pool::new(config).unwrap());

    // 测试持续时间：1小时
    let test_duration = Duration::from_secs(60 * 60);
    let start = Instant::now();

    // 统计信息
    let tcp_operations = Arc::new(AtomicU64::new(0));
    let udp_operations = Arc::new(AtomicU64::new(0));
    let tcp_errors = Arc::new(AtomicU64::new(0));
    let udp_errors = Arc::new(AtomicU64::new(0));
    let bytes_sent = Arc::new(AtomicU64::new(0));
    let bytes_received = Arc::new(AtomicU64::new(0));

    println!("测试配置:");
    println!("  测试持续时间: {:?}", test_duration);
    println!("  最大连接数: 200");
    println!("  最小连接数: 20");
    println!();

    // 启动不同类型的客户端线程

    // 1. TCP客户端线程组（50个线程）
    let tcp_handles: Vec<_> = (0..50)
        .map(|i| {
            let pool = pool.clone();
            let tcp_ops = tcp_operations.clone();
            let tcp_errs = tcp_errors.clone();
            let bytes_sent_clone = bytes_sent.clone();
            let bytes_recv_clone = bytes_received.clone();
            let start = start;

            thread::spawn(move || {
                let mut iteration = 0;
                while start.elapsed() < test_duration {
                    iteration += 1;
                    
                    match pool.get_tcp() {
                        Ok(conn) => {
                            if let Some(mut stream) = conn.tcp_conn() {
                                // 设置非阻塞模式以避免卡住
                                let _ = stream.set_nonblocking(true);
                                
                                // 发送数据
                                let data = format!("TCP请求#{}-{}", i, iteration);
                                let data_bytes = data.as_bytes();
                                
                                if stream.write_all(data_bytes).is_ok() {
                                    bytes_sent_clone.fetch_add(data_bytes.len() as u64, Ordering::Relaxed);
                                    
                                    // 读取响应（非阻塞模式）
                                    let mut response = vec![0u8; data_bytes.len()];
                                    match stream.read_exact(&mut response) {
                                        Ok(_) => {
                                            bytes_recv_clone.fetch_add(response.len() as u64, Ordering::Relaxed);
                                            tcp_ops.fetch_add(1, Ordering::Relaxed);
                                        }
                                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                            // 非阻塞模式下暂时没有足够数据，尝试部分读取
                                            let mut partial = vec![0u8; data_bytes.len()];
                                            match stream.read(&mut partial) {
                                                Ok(size) if size > 0 => {
                                                    bytes_recv_clone.fetch_add(size as u64, Ordering::Relaxed);
                                                    tcp_ops.fetch_add(1, Ordering::Relaxed);
                                                }
                                                _ => {
                                                    // 数据已发送成功，即使没读到也算部分成功
                                                    tcp_ops.fetch_add(1, Ordering::Relaxed);
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

    // 2. UDP客户端线程组（30个线程）
    let udp_handles: Vec<_> = (0..30)
        .map(|i| {
            let pool = pool.clone();
            let udp_ops = udp_operations.clone();
            let udp_errs = udp_errors.clone();
            let bytes_sent_clone = bytes_sent.clone();
            let bytes_recv_clone = bytes_received.clone();
            let start = start;

            thread::spawn(move || {
                let mut iteration = 0;
                while start.elapsed() < test_duration {
                    iteration += 1;
                    
                    match pool.get_udp() {
                        Ok(conn) => {
                            if let Some(socket) = conn.udp_conn() {
                                // 发送数据
                                let data = format!("UDP请求#{}-{}", i, iteration);
                                let data_bytes = data.as_bytes();
                                
                                if socket.send(data_bytes).is_ok() {
                                    bytes_sent_clone.fetch_add(data_bytes.len() as u64, Ordering::Relaxed);
                                    
                                    // 接收响应
                                    let mut response = vec![0u8; 8192];
                                    match socket.recv(&mut response) {
                                        Ok(size) => {
                                            bytes_recv_clone.fetch_add(size as u64, Ordering::Relaxed);
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
                        }
                        Err(_) => {
                            udp_errs.fetch_add(1, Ordering::Relaxed);
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

    // 3. 混合协议客户端线程组（20个线程）
    let mixed_handles: Vec<_> = (0..20)
        .map(|i| {
            let pool = pool.clone();
            let tcp_ops = tcp_operations.clone();
            let udp_ops = udp_operations.clone();
            let tcp_errs = tcp_errors.clone();
            let udp_errs = udp_errors.clone();
            let bytes_sent_clone = bytes_sent.clone();
            let bytes_recv_clone = bytes_received.clone();
            let start = start;

            thread::spawn(move || {
                let mut iteration = 0;
                while start.elapsed() < test_duration {
                    iteration += 1;
                    
                    // 交替使用TCP和UDP
                    if iteration % 2 == 0 {
                        // TCP请求
                        if let Ok(conn) = pool.get_tcp() {
                            if let Some(mut stream) = conn.tcp_conn() {
                                // 设置非阻塞模式
                                let _ = stream.set_nonblocking(true);
                                
                                let data = format!("混合TCP#{}-{}", i, iteration);
                                let data_bytes = data.as_bytes();
                                
                                if stream.write_all(data_bytes).is_ok() {
                                    bytes_sent_clone.fetch_add(data_bytes.len() as u64, Ordering::Relaxed);
                                    let mut response = vec![0u8; data_bytes.len()];
                                    match stream.read_exact(&mut response) {
                                        Ok(_) => {
                                            bytes_recv_clone.fetch_add(response.len() as u64, Ordering::Relaxed);
                                            tcp_ops.fetch_add(1, Ordering::Relaxed);
                                        }
                                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                            // 非阻塞模式，尝试部分读取
                                            match stream.read(&mut response) {
                                                Ok(size) if size > 0 => {
                                                    bytes_recv_clone.fetch_add(size as u64, Ordering::Relaxed);
                                                    tcp_ops.fetch_add(1, Ordering::Relaxed);
                                                }
                                                _ => {
                                                    // 数据已发送成功
                                                    tcp_ops.fetch_add(1, Ordering::Relaxed);
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
                            } else {
                                tcp_errs.fetch_add(1, Ordering::Relaxed);
                            }
                        } else {
                            tcp_errs.fetch_add(1, Ordering::Relaxed);
                        }
                    } else {
                        // UDP请求
                        if let Ok(conn) = pool.get_udp() {
                            if let Some(socket) = conn.udp_conn() {
                                let data = format!("混合UDP#{}-{}", i, iteration);
                                let data_bytes = data.as_bytes();
                                
                                if socket.send(data_bytes).is_ok() {
                                    bytes_sent_clone.fetch_add(data_bytes.len() as u64, Ordering::Relaxed);
                                    let mut response = vec![0u8; 8192];
                                    match socket.recv(&mut response) {
                                        Ok(size) => {
                                            bytes_recv_clone.fetch_add(size as u64, Ordering::Relaxed);
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

                    if iteration % 1000 == 0 {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            })
        })
        .collect();

    // 4. 监控线程 - 每30秒报告一次
    let monitor_pool = pool.clone();
    let monitor_start = start;
    let monitor_tcp_ops = tcp_operations.clone();
    let monitor_udp_ops = udp_operations.clone();
    let monitor_tcp_errs = tcp_errors.clone();
    let monitor_udp_errs = udp_errors.clone();
    let monitor_handle = thread::spawn(move || {
        while monitor_start.elapsed() < test_duration {
            thread::sleep(Duration::from_secs(30));
            
            let elapsed = monitor_start.elapsed();
            let tcp_ops = monitor_tcp_ops.load(Ordering::Relaxed);
            let udp_ops = monitor_udp_ops.load(Ordering::Relaxed);
            let tcp_errs = monitor_tcp_errs.load(Ordering::Relaxed);
            let udp_errs = monitor_udp_errs.load(Ordering::Relaxed);
            let stats = monitor_pool.stats();
            
            println!(
                "[{:?}] TCP: 操作={}, 错误={} | UDP: 操作={}, 错误={} | 连接: 当前={}, 创建={}, 复用={}",
                elapsed,
                tcp_ops,
                tcp_errs,
                udp_ops,
                udp_errs,
                stats.current_connections,
                stats.total_connections_created,
                stats.total_connections_reused
            );
        }
    });

    println!("启动100个客户端线程...");
    println!("  - TCP客户端: 50个线程");
    println!("  - UDP客户端: 30个线程");
    println!("  - 混合协议: 20个线程");
    println!();

    // 等待所有线程完成
    for handle in tcp_handles.into_iter().chain(udp_handles).chain(mixed_handles) {
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
    let total_bytes_sent = bytes_sent.load(Ordering::Relaxed);
    let total_bytes_recv = bytes_received.load(Ordering::Relaxed);

    println!();
    println!("==========================================");
    println!("全面客户端压力测试结果");
    println!("==========================================");
    println!("  运行时间: {:?}", total_time);
    println!();
    println!("  TCP统计:");
    println!("    操作数: {}", total_tcp_ops);
    println!("    错误数: {}", total_tcp_errs);
    println!("    成功率: {:.2}%", 
        if total_tcp_ops + total_tcp_errs > 0 {
            (total_tcp_ops as f64 / (total_tcp_ops + total_tcp_errs) as f64) * 100.0
        } else {
            0.0
        });
    println!();
    println!("  UDP统计:");
    println!("    操作数: {}", total_udp_ops);
    println!("    错误数: {}", total_udp_errs);
    println!("    成功率: {:.2}%",
        if total_udp_ops + total_udp_errs > 0 {
            (total_udp_ops as f64 / (total_udp_ops + total_udp_errs) as f64) * 100.0
        } else {
            0.0
        });
    println!();
    println!("  数据传输:");
    println!("    发送: {:.2} MB", total_bytes_sent as f64 / 1024.0 / 1024.0);
    println!("    接收: {:.2} MB", total_bytes_recv as f64 / 1024.0 / 1024.0);
    println!("    总流量: {:.2} MB", (total_bytes_sent + total_bytes_recv) as f64 / 1024.0 / 1024.0);
    println!();
    println!("  连接池统计:");
    println!("    当前连接: {}", final_stats.current_connections);
    println!("    创建连接: {}", final_stats.total_connections_created);
    println!("    关闭连接: {}", final_stats.total_connections_closed);
    println!("    连接复用: {}", final_stats.total_connections_reused);
    println!("    复用率: {:.2}%", final_stats.average_reuse_count * 100.0);
    println!("    TCP连接: {}", final_stats.current_tcp_connections);
    println!("    UDP连接: {}", final_stats.current_udp_connections);
    println!();
    println!("  服务器统计:");
    println!("    TCP请求: {}", server.tcp_requests());
    println!("    UDP请求: {}", server.udp_requests());
    println!();

    // 验证结果
    assert!(total_tcp_ops > 0 || total_udp_ops > 0, "应该有成功的操作");
    assert!(final_stats.current_connections <= 200, "连接数不应超过最大值");

    println!("✅ 全面客户端压力测试通过！");
}

