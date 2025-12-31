// Copyright (c) 2025, vistone
// All rights reserved.

// 测试服务器 - 支持TCP和UDP，用于压力测试

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct TestServer {
    tcp_listener: TcpListener,
    udp_socket: UdpSocket,
    tcp_addr: String,
    udp_addr: String,
    stop: Arc<AtomicBool>,
    tcp_requests: Arc<AtomicU64>,
    udp_requests: Arc<AtomicU64>,
}

impl TestServer {
    pub fn new() -> std::io::Result<Self> {
        // TCP服务器
        let tcp_listener = TcpListener::bind("127.0.0.1:0")?;
        let tcp_addr = format!("{}", tcp_listener.local_addr()?);
        tcp_listener.set_nonblocking(true)?;

        // UDP服务器
        let udp_socket = UdpSocket::bind("127.0.0.1:0")?;
        let udp_addr = format!("{}", udp_socket.local_addr()?);
        udp_socket.set_nonblocking(true)?;

        Ok(Self {
            tcp_listener,
            udp_socket,
            tcp_addr,
            udp_addr,
            stop: Arc::new(AtomicBool::new(false)),
            tcp_requests: Arc::new(AtomicU64::new(0)),
            udp_requests: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn tcp_addr(&self) -> &str {
        &self.tcp_addr
    }

    pub fn udp_addr(&self) -> &str {
        &self.udp_addr
    }

    pub fn tcp_requests(&self) -> u64 {
        self.tcp_requests.load(Ordering::Relaxed)
    }

    pub fn udp_requests(&self) -> u64 {
        self.udp_requests.load(Ordering::Relaxed)
    }

    pub fn start(&self) {
        let tcp_listener = self.tcp_listener.try_clone().unwrap();
        let udp_socket = self.udp_socket.try_clone().unwrap();
        let stop_tcp = self.stop.clone();
        let stop_udp = self.stop.clone();
        let tcp_requests = self.tcp_requests.clone();
        let udp_requests = self.udp_requests.clone();

        // TCP处理线程
        let tcp_handle = thread::spawn(move || {
            while !stop_tcp.load(Ordering::Relaxed) {
                // 接受新连接
                match tcp_listener.accept() {
                    Ok((stream, _)) => {
                        let stream_clone = stream.try_clone().unwrap();
                        let tcp_requests_clone = tcp_requests.clone();
                        let stop_clone = stop_tcp.clone();

                        thread::spawn(move || {
                            Self::handle_tcp_client(stream_clone, tcp_requests_clone, stop_clone);
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

        // UDP处理线程
        let udp_handle = thread::spawn(move || {
            let mut buf = [0u8; 65536];
            while !stop_udp.load(Ordering::Relaxed) {
                match udp_socket.recv_from(&mut buf) {
                    Ok((size, addr)) => {
                        udp_requests.fetch_add(1, Ordering::Relaxed);

                        // 回显数据
                        let data = &buf[..size];
                        let _ = udp_socket.send_to(data, addr);
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

        // 等待线程（实际上会一直运行直到stop）
        let _ = (tcp_handle, udp_handle);
    }

    fn handle_tcp_client(mut stream: TcpStream, requests: Arc<AtomicU64>, stop: Arc<AtomicBool>) {
        let mut buf = [0u8; 8192];

        while !stop.load(Ordering::Relaxed) {
            match stream.read(&mut buf) {
                Ok(0) => break, // 连接关闭
                Ok(size) => {
                    requests.fetch_add(1, Ordering::Relaxed);

                    // 回显数据
                    if stream.write_all(&buf[..size]).is_err() {
                        break;
                    }
                    stream.flush().ok();
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => break,
            }
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}
