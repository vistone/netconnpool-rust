use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// 服务器统计信息
struct ServerStats {
    total_connections: AtomicUsize,
    active_connections: AtomicUsize,
    total_bytes_received: AtomicUsize,
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
    let listener = TcpListener::bind("0.0.0.0:8081")?;
    println!("Server listening on 0.0.0.0:8081");

    let stats = Arc::new(ServerStats {
        total_connections: AtomicUsize::new(0),
        active_connections: AtomicUsize::new(0),
        total_bytes_received: AtomicUsize::new(0),
        total_bytes_sent: AtomicUsize::new(0),
    });

    // 统计打印线程
    let stats_clone = stats.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(5));
            println!("--- Server Stats ---");
            println!("Total Connections: {}", stats_clone.total_connections.load(Ordering::Relaxed));
            println!("Active Connections: {}", stats_clone.active_connections.load(Ordering::Relaxed));
            println!("Total Bytes Received: {} MB", stats_clone.total_bytes_received.load(Ordering::Relaxed) / 1024 / 1024);
            println!("Total Bytes Sent: {} MB", stats_clone.total_bytes_sent.load(Ordering::Relaxed) / 1024 / 1024);
            println!("--------------------");
        }
    });

    for stream in listener.incoming() {
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
