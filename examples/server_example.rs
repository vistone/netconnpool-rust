use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// 服务器统计信息
struct ServerStats {
    total_connections: AtomicUsize,
    active_connections: AtomicUsize,
    total_bytes_received: AtomicUsize,
    total_bytes_sent: AtomicUsize,
}

struct UdpStats {
    total_packets_received: AtomicUsize,
    total_bytes_received: AtomicUsize,
    total_packets_sent: AtomicUsize,
    total_bytes_sent: AtomicUsize,
}

fn handle_client(mut stream: TcpStream, stats: Arc<ServerStats>) {
    stats.active_connections.fetch_add(1, Ordering::Relaxed);

    let mut buffer = [0; 4096]; // 4KB buffer
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Connection closed
            Ok(n) => {
                stats.total_bytes_received.fetch_add(n, Ordering::Relaxed);
                // Echo data back
                if let Err(_) = stream.write_all(&buffer[0..n]) {
                    break;
                }
                stats.total_bytes_sent.fetch_add(n, Ordering::Relaxed);
            }
            Err(_) => break,
        }
    }

    stats.active_connections.fetch_sub(1, Ordering::Relaxed);
}

fn main() -> std::io::Result<()> {
    let tcp_listener = TcpListener::bind("0.0.0.0:8081")?;
    let udp_socket = UdpSocket::bind("0.0.0.0:8081")?;
    println!("Server listening on TCP/UDP 0.0.0.0:8081");

    let stats = Arc::new(ServerStats {
        total_connections: AtomicUsize::new(0),
        active_connections: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
    });

    let udp_stats = Arc::new(UdpStats {
        total_packets_received: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
        total_packets_sent: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
    });

    // 统计打印线程
    let stats_clone = stats.clone();
    let udp_stats_clone = udp_stats.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));
        println!("--- Server Stats ---");
        println!(
            "TCP Connections: Total={}, Active={}",
            stats_clone.total_connections.load(Ordering::Relaxed),
            stats_clone.active_connections.load(Ordering::Relaxed)
        );
        println!(
            "TCP Traffic: Recv={} MB, Sent={} MB",
            stats_clone.total_bytes_received.load(Ordering::Relaxed) / 1024 / 1024,
            stats_clone.total_bytes_sent.load(Ordering::Relaxed) / 1024 / 1024
        );
        println!(
            "UDP Traffic: Recv={} MB ({} pkts), Sent={} MB ({} pkts)",
            udp_stats_clone.total_bytes_received.load(Ordering::Relaxed) / 1024 / 1024,
            udp_stats_clone
                .total_packets_received
                .load(Ordering::Relaxed),
            udp_stats_clone.total_bytes_sent.load(Ordering::Relaxed) / 1024 / 1024,
            udp_stats_clone.total_packets_sent.load(Ordering::Relaxed)
        );
        println!("--------------------");
    });

    // UDP 处理线程
    let udp_socket_clone = udp_socket.try_clone()?;
    let udp_stats_handler = udp_stats.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 65535];
        loop {
            match udp_socket_clone.recv_from(&mut buf) {
                Ok((n, src)) => {
                    udp_stats_handler
                        .total_packets_received
                        .fetch_add(1, Ordering::Relaxed);
                    udp_stats_handler
                        .total_bytes_received
                        .fetch_add(n, Ordering::Relaxed);

                    // Echo back
                    match udp_socket_clone.send_to(&buf[..n], src) {
                        Ok(sent) => {
                            udp_stats_handler
                                .total_packets_sent
                                .fetch_add(1, Ordering::Relaxed);
                            udp_stats_handler
                                .total_bytes_sent
                                .fetch_add(sent, Ordering::Relaxed);
                        }
                        Err(e) => eprintln!("UDP send error: {}", e),
                    }
                }
                Err(e) => eprintln!("UDP recv error: {}", e),
            }
        }
    });

    for stream in tcp_listener.incoming() {
        match stream {
            Ok(stream) => {
                stats.total_connections.fetch_add(1, Ordering::Relaxed);
                let stats_clone = stats.clone();
                thread::spawn(move || {
                    handle_client(stream, stats_clone);
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
    Ok(())
}
