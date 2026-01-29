// Copyright (c) 2025, vistone
// All rights reserved.

// 真实数据传输测试
// 验证连接池中的连接能够进行真实的数据传输，而不是仅仅进行连接计数

use netconnpool::config::default_config;
use netconnpool::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// 创建一个真正处理数据的 TCP 回声服务器
fn create_tcp_echo_server() -> (TcpListener, Arc<AtomicBool>, Arc<AtomicU64>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).expect("Failed to set non-blocking mode on TCP listener");
    let stop = Arc::new(AtomicBool::new(false));
    let bytes_processed = Arc::new(AtomicU64::new(0));

    let stop_clone = stop.clone();
    let bytes_clone = bytes_processed.clone();
    let listener_clone = listener.try_clone().unwrap();

    thread::spawn(move || {
        while !stop_clone.load(Ordering::Relaxed) {
            match listener_clone.accept() {
                Ok((stream, _)) => {
                    let bytes = bytes_clone.clone();
                    let stop = stop_clone.clone();
                    thread::spawn(move || {
                        handle_tcp_client(stream, bytes, stop);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    });

    (listener, stop, bytes_processed)
}

fn handle_tcp_client(mut stream: TcpStream, bytes: Arc<AtomicU64>, stop: Arc<AtomicBool>) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
    let mut buf = [0u8; 4096];

    while !stop.load(Ordering::Relaxed) {
        match stream.read(&mut buf) {
            Ok(0) => break, // 连接关闭
            Ok(n) => {
                bytes.fetch_add(n as u64, Ordering::Relaxed);
                if stream.write_all(&buf[..n]).is_err() {
                    break;
                }
                let _ = stream.flush();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(_) => break,
        }
    }
}

/// 创建一个真正处理数据的 UDP 回声服务器
fn create_udp_echo_server() -> (UdpSocket, Arc<AtomicBool>, Arc<AtomicU64>) {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.set_nonblocking(true).expect("Failed to set non-blocking mode on UDP socket");
    let stop = Arc::new(AtomicBool::new(false));
    let bytes_processed = Arc::new(AtomicU64::new(0));

    let stop_clone = stop.clone();
    let bytes_clone = bytes_processed.clone();
    let socket_clone = socket.try_clone().unwrap();

    thread::spawn(move || {
        let mut buf = [0u8; 65536];
        while !stop_clone.load(Ordering::Relaxed) {
            match socket_clone.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    bytes_clone.fetch_add(n as u64, Ordering::Relaxed);
                    let _ = socket_clone.send_to(&buf[..n], addr);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    });

    (socket, stop, bytes_processed)
}

/// 测试1: TCP 连接真实数据传输验证
#[test]
fn test_tcp_real_data_transmission() {
    println!("\n========================================");
    println!("TCP 真实数据传输测试");
    println!("========================================");

    let (listener, stop, server_bytes) = create_tcp_echo_server();
    let addr = listener.local_addr().unwrap().to_string();

    let mut config = default_config();
    config.max_connections = 10;
    config.min_connections = 2;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    thread::sleep(Duration::from_millis(100)); // 等待预热

    let mut total_sent = 0u64;
    let mut total_received = 0u64;
    let test_data = b"Hello, this is real TCP data for testing the connection pool!";

    // 进行多次数据传输
    for i in 0..100 {
        let conn = pool.get_tcp().expect("应该能获取 TCP 连接");
        
        if let Some(stream) = conn.tcp_conn() {
            // 克隆 stream 用于读写（因为我们只有不可变引用）
            let mut stream_clone = stream.try_clone().expect("应该能克隆 stream");
            stream_clone.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            stream_clone.set_write_timeout(Some(Duration::from_secs(1))).unwrap();

            // 发送数据
            let send_data = format!("{}-{:?}", i, test_data);
            let bytes_to_send = send_data.as_bytes();
            
            match stream_clone.write_all(bytes_to_send) {
                Ok(_) => {
                    total_sent += bytes_to_send.len() as u64;
                    stream_clone.flush().unwrap();

                    // 接收回显数据
                    let mut response = vec![0u8; bytes_to_send.len()];
                    match stream_clone.read_exact(&mut response) {
                        Ok(_) => {
                            total_received += response.len() as u64;
                            // 验证数据完整性
                            assert_eq!(response, bytes_to_send, "回显数据应该与发送数据匹配");
                        }
                        Err(e) => {
                            println!("读取响应失败 (迭代 {}): {}", i, e);
                        }
                    }
                }
                Err(e) => {
                    println!("发送数据失败 (迭代 {}): {}", i, e);
                }
            }
        }
        // 连接自动归还
    }

    stop.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(100));

    let server_processed = server_bytes.load(Ordering::Relaxed);
    let stats = pool.stats();

    println!("测试结果:");
    println!("  客户端发送: {} 字节", total_sent);
    println!("  客户端接收: {} 字节", total_received);
    println!("  服务器处理: {} 字节", server_processed);
    println!("  连接池统计:");
    println!("    成功获取: {}", stats.successful_gets);
    println!("    连接复用: {}", stats.total_connections_reused);

    // 验证真实数据传输
    assert!(total_sent > 0, "应该发送了数据");
    assert!(total_received > 0, "应该接收了数据");
    assert_eq!(total_sent, total_received, "发送和接收的数据量应该相等");
    assert!(server_processed > 0, "服务器应该处理了数据");
    
    println!("\n✅ TCP 真实数据传输测试通过！");
}

/// 测试2: UDP 连接真实数据传输验证
#[test]
fn test_udp_real_data_transmission() {
    println!("\n========================================");
    println!("UDP 真实数据传输测试");
    println!("========================================");

    let (server_socket, stop, server_bytes) = create_udp_echo_server();
    let server_addr = server_socket.local_addr().unwrap().to_string();

    let mut config = default_config();
    config.max_connections = 10;
    config.min_connections = 0;
    let addr_clone = server_addr.clone();
    config.dialer = Some(Box::new(move |_| {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.connect(&addr_clone)?;
        Ok(ConnectionType::Udp(socket))
    }));

    let pool = Pool::new(config).unwrap();

    let mut total_sent = 0u64;
    let mut total_received = 0u64;

    // 进行多次数据传输
    for i in 0..50 {
        let conn = pool.get_udp().expect("应该能获取 UDP 连接");
        
        if let Some(socket) = conn.udp_conn() {
            socket.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            socket.set_write_timeout(Some(Duration::from_secs(1))).unwrap();

            // 发送数据
            let send_data = format!("UDP packet #{}: Hello from test!", i);
            let bytes_to_send = send_data.as_bytes();
            
            match socket.send(bytes_to_send) {
                Ok(n) => {
                    total_sent += n as u64;

                    // 接收回显数据
                    let mut response = vec![0u8; 1024];
                    match socket.recv(&mut response) {
                        Ok(n) => {
                            total_received += n as u64;
                            // 验证数据完整性
                            assert_eq!(&response[..n], bytes_to_send, "回显数据应该与发送数据匹配");
                        }
                        Err(e) => {
                            println!("接收响应失败 (迭代 {}): {}", i, e);
                        }
                    }
                }
                Err(e) => {
                    println!("发送数据失败 (迭代 {}): {}", i, e);
                }
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(100));

    let server_processed = server_bytes.load(Ordering::Relaxed);
    let stats = pool.stats();

    println!("测试结果:");
    println!("  客户端发送: {} 字节", total_sent);
    println!("  客户端接收: {} 字节", total_received);
    println!("  服务器处理: {} 字节", server_processed);
    println!("  连接池统计:");
    println!("    成功获取: {}", stats.successful_gets);
    println!("    连接复用: {}", stats.total_connections_reused);

    // 验证真实数据传输
    assert!(total_sent > 0, "应该发送了数据");
    assert!(total_received > 0, "应该接收了数据");
    assert!(server_processed > 0, "服务器应该处理了数据");
    
    println!("\n✅ UDP 真实数据传输测试通过！");
}

/// 测试3: 并发环境下的真实数据传输
#[test]
fn test_concurrent_real_data_transmission() {
    println!("\n========================================");
    println!("并发真实数据传输测试");
    println!("========================================");

    let (listener, stop, server_bytes) = create_tcp_echo_server();
    let addr = listener.local_addr().unwrap().to_string();

    let mut config = default_config();
    config.max_connections = 50;
    config.min_connections = 10;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Arc::new(Pool::new(config).unwrap());
    thread::sleep(Duration::from_millis(100)); // 等待预热

    let total_sent = Arc::new(AtomicU64::new(0));
    let total_received = Arc::new(AtomicU64::new(0));
    let successful_ops = Arc::new(AtomicU64::new(0));

    let num_threads = 10;
    let ops_per_thread = 20;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let pool = pool.clone();
            let sent = total_sent.clone();
            let received = total_received.clone();
            let success = successful_ops.clone();

            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    if let Ok(conn) = pool.get_tcp() {
                        if let Some(stream) = conn.tcp_conn() {
                            if let Ok(mut stream_clone) = stream.try_clone() {
                                let _ = stream_clone.set_read_timeout(Some(Duration::from_secs(2)));
                                let _ = stream_clone.set_write_timeout(Some(Duration::from_secs(2)));

                                let data = format!("Thread-{}-Op-{}: Test data for concurrent transmission!", thread_id, i);
                                let bytes = data.as_bytes();

                                if stream_clone.write_all(bytes).is_ok() {
                                    sent.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                                    let _ = stream_clone.flush();

                                    let mut response = vec![0u8; bytes.len()];
                                    if stream_clone.read_exact(&mut response).is_ok() {
                                        received.fetch_add(response.len() as u64, Ordering::Relaxed);
                                        if response == bytes {
                                            success.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    stop.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(100));

    let final_sent = total_sent.load(Ordering::Relaxed);
    let final_received = total_received.load(Ordering::Relaxed);
    let final_success = successful_ops.load(Ordering::Relaxed);
    let server_processed = server_bytes.load(Ordering::Relaxed);
    let stats = pool.stats();

    println!("并发测试结果:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", ops_per_thread);
    println!("  成功数据验证: {}/{}", final_success, num_threads * ops_per_thread);
    println!("  客户端发送: {} 字节", final_sent);
    println!("  客户端接收: {} 字节", final_received);
    println!("  服务器处理: {} 字节", server_processed);
    println!("  连接池统计:");
    println!("    成功获取: {}", stats.successful_gets);
    println!("    连接复用: {}", stats.total_connections_reused);

    // 验证真实数据传输
    assert!(final_sent > 0, "应该发送了数据");
    assert!(final_received > 0, "应该接收了数据");
    assert!(final_success > 0, "应该有成功的数据验证");
    assert!(server_processed > 0, "服务器应该处理了数据");
    
    // 验证成功率（在本地回环测试中应该非常高）
    let success_rate = final_success as f64 / (num_threads * ops_per_thread) as f64 * 100.0;
    println!("  成功率: {:.2}%", success_rate);
    assert!(success_rate > 95.0, "成功率应该超过 95%，实际: {:.2}%", success_rate);

    println!("\n✅ 并发真实数据传输测试通过！");
}

/// 测试4: 连接复用时的数据隔离验证
#[test]
fn test_connection_reuse_data_isolation() {
    println!("\n========================================");
    println!("连接复用数据隔离测试");
    println!("========================================");

    let (listener, stop, _) = create_tcp_echo_server();
    let addr = listener.local_addr().unwrap().to_string();

    let mut config = default_config();
    config.max_connections = 1;  // 强制只有一个连接，确保复用
    config.min_connections = 1;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    thread::sleep(Duration::from_millis(100)); // 等待预热

    // 第一次使用连接
    {
        let conn = pool.get_tcp().expect("应该能获取连接");
        if let Some(stream) = conn.tcp_conn() {
            let mut stream_clone = stream.try_clone().unwrap();
            stream_clone.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            stream_clone.set_write_timeout(Some(Duration::from_secs(1))).unwrap();

            let data1 = b"First message from first user";
            stream_clone.write_all(data1).unwrap();
            stream_clone.flush().unwrap();

            let mut response = vec![0u8; data1.len()];
            stream_clone.read_exact(&mut response).unwrap();
            assert_eq!(&response, data1, "第一次回显应该正确");
        }
    } // 连接归还

    // 第二次使用同一个连接（复用）
    {
        let conn = pool.get_tcp().expect("应该能获取连接");
        let stats = pool.stats();
        assert!(stats.total_connections_reused >= 1, "应该发生了连接复用");

        if let Some(stream) = conn.tcp_conn() {
            let mut stream_clone = stream.try_clone().unwrap();
            stream_clone.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            stream_clone.set_write_timeout(Some(Duration::from_secs(1))).unwrap();

            let data2 = b"Second message from second user - different data";
            stream_clone.write_all(data2).unwrap();
            stream_clone.flush().unwrap();

            let mut response = vec![0u8; data2.len()];
            stream_clone.read_exact(&mut response).unwrap();
            
            // 关键验证：复用的连接不应该返回上一次的数据
            assert_eq!(&response, data2, "复用连接的回显应该是当前发送的数据，不是上一次的数据");
        }
    }

    stop.store(true, Ordering::Relaxed);

    println!("✅ 连接复用数据隔离测试通过！");
}

/// 测试5: 大数据量传输测试
#[test]
fn test_large_data_transmission() {
    println!("\n========================================");
    println!("大数据量传输测试");
    println!("========================================");

    let (listener, stop, server_bytes) = create_tcp_echo_server();
    let addr = listener.local_addr().unwrap().to_string();

    let mut config = default_config();
    config.max_connections = 5;
    config.min_connections = 2;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(&addr)
            .map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    thread::sleep(Duration::from_millis(100));

    // 生成一个较大的数据块 (1KB)
    let large_data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let mut total_sent = 0u64;
    let mut total_received = 0u64;

    for _ in 0..10 {
        let conn = pool.get_tcp().expect("应该能获取连接");
        if let Some(stream) = conn.tcp_conn() {
            let mut stream_clone = stream.try_clone().unwrap();
            stream_clone.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            stream_clone.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

            // 发送大数据
            if stream_clone.write_all(&large_data).is_ok() {
                total_sent += large_data.len() as u64;
                let _ = stream_clone.flush();

                // 接收回显
                let mut response = vec![0u8; large_data.len()];
                if stream_clone.read_exact(&mut response).is_ok() {
                    total_received += response.len() as u64;
                    assert_eq!(response, large_data, "大数据回显应该完全匹配");
                }
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(100));

    let server_processed = server_bytes.load(Ordering::Relaxed);

    println!("测试结果:");
    println!("  发送: {} 字节 ({:.2} KB)", total_sent, total_sent as f64 / 1024.0);
    println!("  接收: {} 字节 ({:.2} KB)", total_received, total_received as f64 / 1024.0);
    println!("  服务器处理: {} 字节", server_processed);

    assert!(total_sent >= 10 * 1024, "应该发送了至少 10KB 数据");
    assert!(total_received >= 10 * 1024, "应该接收了至少 10KB 数据");
    assert_eq!(total_sent, total_received, "发送和接收的数据量应该相等");

    println!("\n✅ 大数据量传输测试通过！");
}
