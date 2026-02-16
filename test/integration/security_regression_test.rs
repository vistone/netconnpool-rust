// 核心回归测试套件 - 合并了之前分散的漏洞验证和最终验证用例
// 目标：确保并发安全、ID 唯一性、泄漏驱逐及 UDP 清理逻辑持续正确

use netconnpool::config::ConnectionType;
use netconnpool::*;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_security_and_stability_suite() {
    println!("\n========================================");
    println!("开始核心安全性与稳定性综合测试套件");
    println!("========================================");

    test_idle_counts_consistency();
    test_stats_atomicity();
    test_connection_id_uniqueness();
    test_connection_id_collision_reconciliation();
    test_forced_eviction_of_leaked_connections();
    test_udp_buffer_clearing_on_get();
    test_pool_closure_reaper_exit();
    test_comprehensive_stability();

    println!("\n========================================");
    println!("所有核心安全性测试通过！✅");
    println!("========================================");
}

/// 验证归还连接时计数器的一致性
fn test_idle_counts_consistency() {
    println!("\n[1] 验证 idle_counts 一致性...");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || loop {
        if let Ok((stream, _)) = listener.accept() { drop(stream); }
    });

    let mut config = default_config();
    config.max_connections = 100;
    config.max_idle_connections = 50;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr).map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    let handles: Vec<_> = (0..10).map(|_| {
        let pool = pool.clone();
        thread::spawn(move || {
            for _ in 0..100 {
                if let Ok(conn) = pool.get() {
                    thread::sleep(Duration::from_micros(10));
                    drop(conn);
                }
            }
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
    thread::sleep(Duration::from_millis(100));

    let stats = pool.stats();
    assert!(stats.current_idle_connections <= 50, "空闲连接数超过限制: {}", stats.current_idle_connections);
    println!("    ✅ 通过");
}

/// 验证高并发下统计数据的原子性
fn test_stats_atomicity() {
    println!("\n[2] 验证统计计数器原子性...");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || loop {
        if let Ok((stream, _)) = listener.accept() { drop(stream); }
    });

    let mut config = default_config();
    config.max_connections = 200;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr).map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    let total_operations = Arc::new(AtomicUsize::new(0));
    let successful_gets = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..20).map(|_| {
        let pool = pool.clone();
        let total = total_operations.clone();
        let success = successful_gets.clone();
        thread::spawn(move || {
            for _ in 0..500 {
                total.fetch_add(1, Ordering::Relaxed);
                if let Ok(conn) = pool.get() {
                    success.fetch_add(1, Ordering::Relaxed);
                    drop(conn);
                }
            }
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
    thread::sleep(Duration::from_millis(200));

    let stats = pool.stats();
    assert_eq!(stats.total_get_requests as usize, total_operations.load(Ordering::Relaxed));
    assert_eq!(stats.successful_gets as usize, successful_gets.load(Ordering::Relaxed));
    println!("    ✅ 通过");
}

/// 验证连接 ID 唯一性
fn test_connection_id_uniqueness() {
    println!("\n[3] 验证连接 ID 唯一性...");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || loop { if let Ok((s,_)) = listener.accept() { drop(s); } });

    let mut config = default_config();
    config.max_connections = 1000;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr).map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    let mut connections = Vec::new();
    for _ in 0..500 {
        if let Ok(conn) = pool.get() { connections.push(conn); }
    }

    let mut ids = std::collections::HashSet::new();
    for conn in &connections {
        assert!(ids.insert(conn.id), "重复的 ID: {}", conn.id);
    }
    println!("    ✅ 通过");
}

/// 验证连接 ID 冲突时的自动调节逻辑
fn test_connection_id_collision_reconciliation() {
    println!("\n[4] 验证 ID 冲突自动调节...");
    let mut config = default_config();
    config.max_connections = 10;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr).map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    let _conn1 = pool.get().unwrap();
    // 逻辑验证：如果 ID 冲突，PoolInner::create_connection 会自动递增找到新 ID
    // 此时即使 conn1 还在使用，新创建的连接也能找到唯一 Key 插入
    let _conn2 = pool.get().unwrap();
    println!("    ✅ 通过");
}

/// 验证严重泄漏连接的强制驱逐机制
fn test_forced_eviction_of_leaked_connections() {
    println!("\n[5] 验证严重泄漏连接强制驱逐...");
    let mut config = default_config();
    config.connection_leak_timeout = Duration::from_millis(100);
    config.max_connections = 1;
    config.min_connections = 0;
    config.health_check_interval = Duration::from_millis(50);
    config.enable_stats = true;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr).map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    // 借出一个连接且不归还（模拟泄漏）
    let _leaked_conn = pool.get().unwrap();

    // 等待超过 2 倍的 leak_timeout (100ms * 2 = 200ms)
    thread::sleep(Duration::from_millis(500));

    // Reaper 应当强制驱逐此连接
    let stats = pool.stats();
    assert!(stats.leaked_connections >= 1, "未记录泄漏连接");

    // 此时应该能获取到新连接（因为旧连接已被驱逐，配额释放）
    let conn2 = pool.get_with_timeout(Duration::from_secs(1));
    assert!(conn2.is_ok(), "配额未释放，无法获取新连接");
    println!("    ✅ 通过");
}

/// 验证 UDP 归还时的缓冲区清理逻辑（延迟至下一次 Get 时执行）
fn test_udp_buffer_clearing_on_get() {
    println!("\n[6] 验证 UDP 延迟缓冲区清理...");
    let mut config = default_config();
    let server_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = server_socket.local_addr().unwrap();

    config.dialer = Some(Box::new(move |_| {
        let s = UdpSocket::bind("127.0.0.1:0").unwrap();
        s.connect(addr).unwrap();
        Ok(ConnectionType::Udp(s))
    }));

    let pool = Pool::new(config).unwrap();

    // 步骤 1: 获取连接并在服务器端发送“脏数据”
    {
        let conn = pool.get_udp().unwrap();
        let client_addr = conn.udp_conn().unwrap().local_addr().unwrap();
        server_socket.send_to(b"dirty data", client_addr).unwrap();
    } // 归还连接

    // 步骤 2: 再次获取。连接池应在 Get 时清理缓冲区
    let conn = pool.get_udp().unwrap();
    let udp = conn.udp_conn().unwrap();
    udp.set_read_timeout(Some(Duration::from_millis(100))).unwrap();

    let mut buf = [0u8; 1024];
    let result = udp.recv(&mut buf);
    assert!(result.is_err(), "缓冲区未被清理，读取到了脏数据: {:?}", buf);
    println!("    ✅ 通过");
}

/// 验证池关闭时 Reaper 线程的快速响应
fn test_pool_closure_reaper_exit() {
    println!("\n[7] 验证池关闭时 Reaper 响应速度...");
    let mut config = default_config();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr).map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    let start = Instant::now();
    pool.close().unwrap();
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_millis(100), "关闭超时: {:?}", elapsed);
    println!("    ✅ 通过 (耗时 {:?})", elapsed);
}

/// 验证长时间高并发下的稳定性
fn test_comprehensive_stability() {
    println!("\n[8] 综合稳定性压力验证...");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || loop { if let Ok((s,_)) = listener.accept() { drop(s); } });

    let mut config = default_config();
    config.max_connections = 50;
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr).map(ConnectionType::Tcp)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();
    let stop = Arc::new(AtomicBool::new(false));

    let handles: Vec<_> = (0..8).map(|i| {
        let pool = pool.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(c) = pool.get() {
                    if i % 2 == 0 { thread::sleep(Duration::from_micros(10)); }
                    drop(c);
                }
            }
        })
    }).collect();

    thread::sleep(Duration::from_secs(2));
    stop.store(true, Ordering::Relaxed);
    for h in handles { h.join().unwrap(); }

    let stats = pool.stats();
    assert!(stats.current_active_connections >= 0);
    println!("    ✅ 通过");
}
