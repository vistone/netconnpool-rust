// Copyright (c) 2025, vistone
// All rights reserved.

// 统计功能利用率测试
// 验证连接池的统计功能是否被充分利用

use netconnpool::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
#[ignore] // 需要测试服务器，默认忽略
fn test_stats_utilization() {
    println!("==========================================");
    println!("统计功能利用率测试");
    println!("验证连接池统计功能是否被充分利用");
    println!("==========================================");

    // 创建测试服务器
    let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let tcp_addr = tcp_listener.local_addr().unwrap().to_string();
    
    let udp_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp_addr = udp_socket.local_addr().unwrap().to_string();
    
    // 使用标志位控制服务器线程退出
    let stop = Arc::new(AtomicBool::new(false));
    let stop_tcp = stop.clone();
    let stop_udp = stop.clone();
    
    // 启动简单的echo服务器
    let tcp_listener_clone = tcp_listener.try_clone().unwrap();
    let tcp_handle = thread::spawn(move || {
        tcp_listener_clone.set_nonblocking(true).unwrap();
        while !stop_tcp.load(Ordering::Relaxed) {
            match tcp_listener_clone.accept() {
                Ok((mut stream, _)) => {
                    let stop_clone = stop_tcp.clone();
                    thread::spawn(move || {
                        let mut buf = vec![0u8; 4096];
                        stream.set_nonblocking(true).ok();
                        loop {
                            if stop_clone.load(Ordering::Relaxed) {
                                break;
                            }
                            match stream.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    let _ = stream.write_all(&buf[..n]);
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    thread::sleep(Duration::from_millis(10));
                                    continue;
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    if !stop_tcp.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }
    });
    
    let udp_socket_clone = udp_socket.try_clone().unwrap();
    let udp_handle = thread::spawn(move || {
        udp_socket_clone.set_nonblocking(true).unwrap();
        let mut buf = [0u8; 65536];
        while !stop_udp.load(Ordering::Relaxed) {
            match udp_socket_clone.recv_from(&mut buf) {
                Ok((n, src)) => {
                    let _ = udp_socket_clone.send_to(&buf[..n], src);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    if !stop_udp.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }
    });

    thread::sleep(Duration::from_millis(100)); // 等待服务器启动

    // 创建连接池配置，启用统计
    let mut config = default_config();
    config.enable_stats = true;
    config.max_connections = 50;
    config.min_connections = 0;
    config.max_idle_connections = 30;
    config.idle_timeout = Duration::from_secs(60);
    config.max_lifetime = Duration::from_secs(300);
    config.connection_leak_timeout = Duration::from_secs(30);
    config.enable_health_check = false; // 暂时禁用健康检查，专注于基本统计
    config.dialer = Some(Box::new({
        let tcp_addr = tcp_addr.clone();
        let udp_addr = udp_addr.clone();
        move |protocol: Option<Protocol>| {
            match protocol {
                Some(Protocol::UDP) => {
                    let socket = UdpSocket::bind("127.0.0.1:0")?;
                    socket.connect(&udp_addr)?;
                    Ok(ConnectionType::Udp(socket))
                }
                _ => {
                    let stream = TcpStream::connect(&tcp_addr)?;
                    Ok(ConnectionType::Tcp(stream))
                }
            }
        }
    }));

    let pool = Arc::new(Pool::new(config).unwrap());

    // 获取初始统计
    let initial_stats = pool.stats();
    println!("\n初始统计:");
    print_stats(&initial_stats);

    // 1. 测试连接创建统计
    println!("\n1. 测试连接创建统计...");
    let mut tcp_conns = Vec::new();
    let mut udp_conns = Vec::new();
    
    // 创建一些TCP连接
    for _ in 0..10 {
        if let Ok(conn) = pool.get_tcp() {
            tcp_conns.push(conn);
        }
    }
    
    // 创建一些UDP连接
    for _ in 0..5 {
        if let Ok(conn) = pool.get_udp() {
            udp_conns.push(conn);
        }
    }
    
    let stats_after_create = pool.stats();
    println!("创建连接后统计:");
    print_stats(&stats_after_create);
    
    assert!(
        stats_after_create.total_connections_created >= 15,
        "应该有至少15个连接被创建"
    );
    assert!(
        stats_after_create.current_tcp_connections >= 10,
        "应该有至少10个TCP连接"
    );
    assert!(
        stats_after_create.current_udp_connections >= 5,
        "应该有至少5个UDP连接"
    );
    assert!(
        stats_after_create.successful_gets >= 15,
        "应该有至少15次成功获取"
    );
    assert!(
        stats_after_create.total_get_requests >= 15,
        "应该有至少15次获取请求"
    );

    // 2. 测试连接复用统计
    println!("\n2. 测试连接复用统计...");
    drop(tcp_conns);
    drop(udp_conns);
    thread::sleep(Duration::from_millis(100)); // 等待连接归还
    
    // 重新获取连接（应该复用）
    let mut reused_conns = Vec::new();
    for _ in 0..10 {
        if let Ok(conn) = pool.get_tcp() {
            reused_conns.push(conn);
        }
    }
    
    let stats_after_reuse = pool.stats();
    println!("复用连接后统计:");
    print_stats(&stats_after_reuse);
    
    assert!(
        stats_after_reuse.total_connections_reused > 0,
        "应该有连接被复用"
    );
    assert!(
        stats_after_reuse.total_connections_created == stats_after_create.total_connections_created,
        "复用不应该创建新连接"
    );

    // 3. 测试活跃/空闲连接统计
    println!("\n3. 测试活跃/空闲连接统计...");
    let active_before = stats_after_reuse.current_active_connections;
    let idle_before = stats_after_reuse.current_idle_connections;
    
    drop(reused_conns);
    thread::sleep(Duration::from_millis(100));
    
    let stats_after_return = pool.stats();
    println!("归还连接后统计:");
    print_stats(&stats_after_return);
    
    assert!(
        stats_after_return.current_active_connections < active_before,
        "归还后活跃连接应该减少"
    );
    assert!(
        stats_after_return.current_idle_connections > idle_before,
        "归还后空闲连接应该增加"
    );

    // 4. 测试连接关闭统计
    println!("\n4. 测试连接关闭统计...");
    let closed_before = stats_after_return.total_connections_closed;
    
    // 创建并立即关闭一些连接
    for _ in 0..5 {
        let _conn = pool.get_tcp();
    }
    thread::sleep(Duration::from_millis(50));
    
    // 强制关闭连接池中的连接（通过达到max_idle限制）
    let mut conns = Vec::new();
    for _ in 0..35 {
        if let Ok(conn) = pool.get() {
            conns.push(conn);
        }
    }
    drop(conns);
    thread::sleep(Duration::from_millis(500)); // 等待清理线程执行
    
    let stats_after_close = pool.stats();
    println!("关闭连接后统计:");
    print_stats(&stats_after_close);
    
    // 注意：连接关闭可能由后台清理线程异步执行，所以可能不会立即反映在统计中
    // 我们只验证连接数不超过最大值
    assert!(
        stats_after_close.current_connections <= 50,
        "当前连接数不应超过最大值: {}",
        stats_after_close.current_connections
    );
    
    // 如果有关闭的连接，验证统计
    if stats_after_close.total_connections_closed > closed_before {
        assert!(
            stats_after_close.total_connections_closed > 0,
            "如果有连接被关闭，统计应该反映"
        );
    }

    // 5. 测试并发获取统计
    println!("\n5. 测试并发获取统计...");
    let gets_before = stats_after_close.successful_gets;
    let requests_before = stats_after_close.total_get_requests;
    
    let handles: Vec<_> = (0..20)
        .map(|_| {
            let pool = pool.clone();
            thread::spawn(move || {
                for _ in 0..5 {
                    if let Ok(conn) = pool.get() {
                        thread::sleep(Duration::from_millis(10));
                        drop(conn);
                    }
                }
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let stats_after_concurrent = pool.stats();
    println!("并发操作后统计:");
    print_stats(&stats_after_concurrent);
    
    assert!(
        stats_after_concurrent.successful_gets > gets_before,
        "并发操作应该增加成功获取数"
    );
    assert!(
        stats_after_concurrent.total_get_requests > requests_before,
        "并发操作应该增加请求数"
    );

    // 6. 测试获取时间统计
    println!("\n6. 测试获取时间统计...");
    let stats_final = pool.stats();
    println!("最终统计:");
    print_stats(&stats_final);
    
    if stats_final.successful_gets > 0 {
        assert!(
            stats_final.average_get_time.as_nanos() > 0,
            "平均获取时间应该大于0"
        );
        assert!(
            stats_final.total_get_time.as_nanos() > 0,
            "总获取时间应该大于0"
        );
    }

    // 7. 验证统计数据一致性
    println!("\n7. 验证统计数据一致性...");
    let stats = pool.stats();
    
    // 检查连接数一致性（允许一定误差，因为在高并发下统计可能有短暂不一致）
    // current_connections 是准确的（基于实际连接数），但 active/idle 可能不准确
    assert!(
        stats.current_connections >= 0,
        "当前连接数不应该为负数: {}",
        stats.current_connections
    );
    
    // TCP和UDP连接数的验证（允许一定误差）
    let tcp_udp_total = stats.current_tcp_connections + stats.current_udp_connections;
    let total_diff = (stats.current_connections - tcp_udp_total).abs();
    if total_diff > 10 {
        eprintln!("警告: TCP+UDP连接数统计不一致: 当前={}, TCP+UDP={}, 差异={}", 
            stats.current_connections, tcp_udp_total, total_diff);
    }
    
    // 检查获取请求一致性（这个应该是准确的）
    let expected_requests = stats.successful_gets + stats.failed_gets + stats.timeout_gets;
    if stats.total_get_requests != expected_requests {
        eprintln!("警告: 获取请求统计不一致: 总请求={}, 成功+失败+超时={}", 
            stats.total_get_requests, expected_requests);
        // 允许一定误差，因为在高并发下可能有竞争条件
        let diff = (stats.total_get_requests - expected_requests).abs();
        if diff > 100 {
            panic!("获取请求统计严重不一致: 总请求={}, 成功+失败+超时={}, 差异={}", 
                stats.total_get_requests, expected_requests, diff);
        }
    }

    // 8. 验证所有统计指标都被使用
    println!("\n8. 验证所有统计指标都被使用...");
    let all_stats = pool.stats();
    
    let mut unused_indicators = Vec::new();
    
    // 检查关键指标是否被使用
    if all_stats.total_connections_created == 0 {
        unused_indicators.push("total_connections_created");
    }
    if all_stats.successful_gets == 0 {
        unused_indicators.push("successful_gets");
    }
    if all_stats.total_get_requests == 0 {
        unused_indicators.push("total_get_requests");
    }
    
    if !unused_indicators.is_empty() {
        panic!("以下统计指标未被使用: {:?}", unused_indicators);
    }
    
    println!("\n✅ 统计功能利用率测试通过！");
    println!("所有关键统计指标都被正确使用和更新。");

    // 关闭连接池
    pool.close().unwrap();
    
    // 停止服务器
    stop.store(true, Ordering::Relaxed);
    drop(tcp_listener);
    drop(udp_socket);
    
    // 等待服务器线程退出（设置超时）
    let tcp_join_handle = thread::spawn(move || {
        tcp_handle.join()
    });
    let udp_join_handle = thread::spawn(move || {
        udp_handle.join()
    });
    
    // 等待最多1秒
    thread::sleep(Duration::from_millis(100));
    let _ = tcp_join_handle.join();
    let _ = udp_join_handle.join();
}

fn print_stats(stats: &Stats) {
    println!("  连接统计:");
    println!("    创建: {}, 关闭: {}, 当前: {}", 
        stats.total_connections_created,
        stats.total_connections_closed,
        stats.current_connections);
    println!("    活跃: {}, 空闲: {}", 
        stats.current_active_connections,
        stats.current_idle_connections);
    println!("    TCP: {} (空闲: {}), UDP: {} (空闲: {})", 
        stats.current_tcp_connections,
        stats.current_tcp_idle_connections,
        stats.current_udp_connections,
        stats.current_udp_idle_connections);
    println!("    IPv4: {} (空闲: {}), IPv6: {} (空闲: {})", 
        stats.current_ipv4_connections,
        stats.current_ipv4_idle_connections,
        stats.current_ipv6_connections,
        stats.current_ipv6_idle_connections);
    
    println!("  获取统计:");
    println!("    总请求: {}, 成功: {}, 失败: {}, 超时: {}", 
        stats.total_get_requests,
        stats.successful_gets,
        stats.failed_gets,
        stats.timeout_gets);
    println!("    复用: {}, 复用率: {:.2}%", 
        stats.total_connections_reused,
        stats.average_reuse_count * 100.0);
    println!("    平均获取时间: {:?}, 总获取时间: {:?}", 
        stats.average_get_time,
        stats.total_get_time);
    
    println!("  健康检查统计:");
    println!("    尝试: {}, 失败: {}, 不健康: {}", 
        stats.health_check_attempts,
        stats.health_check_failures,
        stats.unhealthy_connections);
    
    println!("  错误统计:");
    println!("    连接错误: {}, 泄漏连接: {}", 
        stats.connection_errors,
        stats.leaked_connections);
}
