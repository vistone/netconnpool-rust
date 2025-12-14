use netconnpool::{default_config, ConnectionType, Pool, Protocol};
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// 客户端统计信息
struct ClientStats {
    total_tcp_requests: AtomicUsize,
    success_tcp_requests: AtomicUsize,
    failed_tcp_requests: AtomicUsize,
    total_udp_requests: AtomicUsize,
    success_udp_requests: AtomicUsize,
    failed_udp_requests: AtomicUsize,
    total_bytes_sent: AtomicUsize,
    total_bytes_received: AtomicUsize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting High Concurrency TCP/UDP Client Stress Test...");

    let mut config = default_config();
    config.max_connections = 100; // 高并发连接数
    config.min_connections = 10;
    config.max_idle_connections = 50;
    config.connection_timeout = Duration::from_secs(5);
    config.enable_stats = true;

    // 连接服务器，支持 TCP 和 UDP
    config.dialer = Some(Box::new(|protocol| {
        let addr = "127.0.0.1:8081";
        match protocol {
            Some(Protocol::UDP) => {
                let socket = UdpSocket::bind("0.0.0.0:0")?;
                socket.connect(addr)?;
                Ok(ConnectionType::Udp(socket))
            }
            _ => TcpStream::connect(addr)
                .map(|s| ConnectionType::Tcp(s))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }));

    let pool = Pool::new(config)?;
    let pool = Arc::new(pool); // Share pool among threads

    let stats = Arc::new(ClientStats {
        total_tcp_requests: AtomicUsize::new(0),
        success_tcp_requests: AtomicUsize::new(0),
        failed_tcp_requests: AtomicUsize::new(0),
        total_udp_requests: AtomicUsize::new(0),
        success_udp_requests: AtomicUsize::new(0),
        failed_udp_requests: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
    });

    let num_threads = 50;
    let requests_per_thread = 1000;
    let data_size = 1024 * 10; // 10KB per request

    println!("Configuration:");
    println!("  Threads: {}", num_threads);
    println!("  Requests per thread: {}", requests_per_thread);
    println!("  Data size per request: {} KB", data_size / 1024);
    println!("  Total requests: {}", num_threads * requests_per_thread);

    let start_time = Instant::now();
    let mut handles = vec![];

    // 启动统计打印线程
    let stats_monitor = stats.clone();
    let pool_monitor = pool.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(2));
        let elapsed = start_time.elapsed().as_secs_f64();
        let tcp_total = stats_monitor.total_tcp_requests.load(Ordering::Relaxed);
        let udp_total = stats_monitor.total_udp_requests.load(Ordering::Relaxed);
        let total = tcp_total + udp_total;
        let tcp_success = stats_monitor.success_tcp_requests.load(Ordering::Relaxed);
        let udp_success = stats_monitor.success_udp_requests.load(Ordering::Relaxed);

        println!("\n--- Real-time Client Stats ({:.1}s) ---", elapsed);
        println!("QPS: {:.2}", total as f64 / elapsed);
        println!(
            "TCP: Total={}, Success={}, Failed={}",
            tcp_total,
            tcp_success,
            stats_monitor.failed_tcp_requests.load(Ordering::Relaxed)
        );
        println!(
            "UDP: Total={}, Success={}, Failed={}",
            udp_total,
            udp_success,
            stats_monitor.failed_udp_requests.load(Ordering::Relaxed)
        );
        println!(
            "Traffic: Sent={} MB, Recv={} MB",
            stats_monitor.total_bytes_sent.load(Ordering::Relaxed) / 1024 / 1024,
            stats_monitor.total_bytes_received.load(Ordering::Relaxed) / 1024 / 1024
        );

        let pool_stats = pool_monitor.stats();
        println!("Pool Stats:");
        println!(
            "  Active: {}, Idle: {} (TCP:{}, UDP:{})",
            pool_stats.current_active_connections,
            pool_stats.current_idle_connections,
            pool_stats.current_tcp_idle_connections,
            pool_stats.current_udp_idle_connections
        );
        println!(
            "  Created: TCP={}, UDP={}",
            pool_stats.current_tcp_connections, pool_stats.current_udp_connections
        );
        println!(
            "  Reuse Count: {}, Successful Gets: {}",
            pool_stats.total_connections_reused, pool_stats.successful_gets
        );
        println!("---------------------------------------");
    });

    for i in 0..num_threads {
        let pool = pool.clone();
        let stats = stats.clone();
        let data = vec![b'X'; data_size]; // Pre-allocate data
        let is_udp_thread = i % 2 == 1; // 偶数线程TCP，奇数线程UDP

        let handle = thread::spawn(move || {
            for _ in 0..requests_per_thread {
                if is_udp_thread {
                    stats.total_udp_requests.fetch_add(1, Ordering::Relaxed);
                    match pool.get_udp() {
                        Ok(conn) => {
                            if let Some(socket) = conn.udp_conn() {
                                // UDP send
                                if let Err(_) = socket.send(&data) {
                                    stats.failed_udp_requests.fetch_add(1, Ordering::Relaxed);
                                    continue;
                                }
                                stats
                                    .total_bytes_sent
                                    .fetch_add(data_size, Ordering::Relaxed);

                                // UDP recv
                                let mut buffer = vec![0; data_size + 100];
                                match socket.recv(&mut buffer) {
                                    Ok(_) => {
                                        stats
                                            .total_bytes_received
                                            .fetch_add(data_size, Ordering::Relaxed);
                                        stats.success_udp_requests.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(_) => {
                                        stats.failed_udp_requests.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            } else {
                                stats.failed_udp_requests.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(_) => {
                            stats.failed_udp_requests.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                } else {
                    stats.total_tcp_requests.fetch_add(1, Ordering::Relaxed);
                    match pool.get_tcp() {
                        Ok(conn) => {
                            if let Some(mut stream) = conn.tcp_conn() {
                                // Send data
                                if let Err(_) = stream.write_all(&data) {
                                    stats.failed_tcp_requests.fetch_add(1, Ordering::Relaxed);
                                    continue;
                                }
                                stats
                                    .total_bytes_sent
                                    .fetch_add(data_size, Ordering::Relaxed);

                                // Read echo
                                let mut buffer = vec![0; data_size];
                                if let Err(_) = stream.read_exact(&mut buffer) {
                                    stats.failed_tcp_requests.fetch_add(1, Ordering::Relaxed);
                                    continue;
                                }
                                stats
                                    .total_bytes_received
                                    .fetch_add(data_size, Ordering::Relaxed);
                                stats.success_tcp_requests.fetch_add(1, Ordering::Relaxed);
                            } else {
                                stats.failed_tcp_requests.fetch_add(1, Ordering::Relaxed);
                            }
                            // conn drops here, returning to pool
                        }
                        Err(_) => {
                            stats.failed_tcp_requests.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    let total_time = start_time.elapsed();
    let total_requests = stats.total_tcp_requests.load(Ordering::Relaxed)
        + stats.total_udp_requests.load(Ordering::Relaxed);

    println!("\nTest Completed in {:?}", total_time);
    println!("Final Stats:");
    println!("  Total Requests: {}", total_requests);
    println!(
        "  TCP Success: {}",
        stats.success_tcp_requests.load(Ordering::Relaxed)
    );
    println!(
        "  UDP Success: {}",
        stats.success_udp_requests.load(Ordering::Relaxed)
    );
    println!(
        "  Throughput: {:.2} MB/s",
        (stats.total_bytes_sent.load(Ordering::Relaxed)
            + stats.total_bytes_received.load(Ordering::Relaxed)) as f64
            / 1024.0
            / 1024.0
            / total_time.as_secs_f64()
    );

    // Final Pool Stats
    let pool_stats = pool.stats();
    println!("\nFinal Pool Internal Stats:");
    println!("{:#?}", pool_stats);

    Ok(())
}
