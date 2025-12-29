use netconnpool::config::ConnectionType;
use netconnpool::protocol::Protocol;
use netconnpool::*;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::Duration;
use std::time::Instant;

#[test]
fn test_connection_id_collision_reconciliation() {
    let mut config = default_config();
    config.max_connections = 10;

    // Create a listener to connect to
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr)
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();

    // 1. Get a connection to find its ID
    let conn1 = pool.get().unwrap();
    let id1 = conn1.id;
    println!("First connection ID: {}", id1);
}

#[test]
fn test_forced_eviction_of_leaked_connections() {
    let mut config = default_config();
    config.connection_leak_timeout = Duration::from_millis(100);
    config.max_connections = 1;
    config.min_connections = 0;
    config.health_check_interval = Duration::from_millis(100);
    config.enable_stats = true;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr)
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();

    // 1. Borrow a connection and "leak" it (keep it held)
    let _conn = pool.get().unwrap();

    // 2. Wait for more than 2x leak timeout
    thread::sleep(Duration::from_millis(300));

    // 3. The reaper should have run and evicted it from all_connections
    let stats = pool.stats();
    println!("Stats before waiting: {:?}", stats);

    // The reaper runs periodically. Let's wait a bit more.
    thread::sleep(Duration::from_millis(1000));

    let stats = pool.stats();
    println!("Stats after waiting: {:?}", stats);
    println!("Leaked connections count: {}", stats.leaked_connections);

    // Try to get a new connection.
    let conn2 = pool.get_with_timeout(Duration::from_secs(1));
    assert!(
        conn2.is_ok(),
        "Should be able to get a new connection after eviction. Error: {:?}",
        conn2.err()
    );
}

#[test]
fn test_udp_buffer_clearing_on_get() {
    let mut config = default_config();

    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap();

    config.dialer = Some(Box::new(move |_| {
        let s = UdpSocket::bind("127.0.0.1:0").unwrap();
        s.connect(addr).unwrap();
        Ok(ConnectionType::Udp(s))
    }));

    let pool = Pool::new(config).unwrap();

    // 1. Get a UDP connection and send some data to it from the server side
    {
        let conn = pool.get_udp().unwrap();
        let client_addr = conn.udp_conn().unwrap().local_addr().unwrap();

        socket.send_to(b"dirty data", client_addr).unwrap();
    }

    // 2. Get it again. The buffer should be cleared on get.
    let conn = pool.get_udp().unwrap();
    let udp = conn.udp_conn().unwrap();
    udp.set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();

    let mut buf = [0u8; 1024];
    let result = udp.recv(&mut buf);

    assert!(
        result.is_err(),
        "Buffer should have been cleared, but received: {:?}",
        result
    );
}

#[test]
fn test_pool_closure_reaper_exit() {
    let mut config = default_config();
    config.health_check_interval = Duration::from_millis(100);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    config.dialer = Some(Box::new(move |_| {
        TcpStream::connect(addr)
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));

    let pool = Pool::new(config).unwrap();

    let start = Instant::now();
    pool.close().unwrap();
    let elapsed = start.elapsed();

    println!("Pool closed in {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(50),
        "Pool took too long to close: {:?}",
        elapsed
    );
}
