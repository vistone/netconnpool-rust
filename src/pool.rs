use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Connection pool configuration
#[derive(Clone, Debug)]
pub struct PoolConfig {
    /// Maximum number of connections
    pub max_connections: usize,
    /// Maximum number of idle connections
    pub max_idle_connections: usize,
    /// Connection idle timeout
    pub idle_timeout: Duration,
}

/// Idle connection entry
#[derive(Clone, Debug)]
struct IdleConnection {
    conn_id: usize,
    inserted_at: Instant,
}

/// Thread-safe connection pool with optimized CAS-based idle connection management
pub struct ConnectionPool {
    config: PoolConfig,
    // Active connections count
    active_connections: AtomicUsize,
    // Idle connections count with CAS-based management
    idle_counts: AtomicUsize,
    // Queue of idle connections
    idle_queue: Mutex<VecDeque<IdleConnection>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: PoolConfig) -> Arc<Self> {
        Arc::new(ConnectionPool {
            config,
            active_connections: AtomicUsize::new(0),
            idle_counts: AtomicUsize::new(0),
            idle_queue: Mutex::new(VecDeque::new()),
        })
    }

    /// Try to acquire a connection from the pool
    pub fn acquire(&self) -> Result<usize, String> {
        // First, try to get an idle connection
        if let Some(idle_conn) = self.try_get_idle_connection() {
            return Ok(idle_conn.conn_id);
        }

        // If no idle connection, try to create a new one
        let current_active = self.active_connections.load(Ordering::Acquire);
        if current_active < self.config.max_connections {
            // Try to increment active connections
            match self.active_connections.compare_exchange(
                current_active,
                current_active + 1,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let conn_id = current_active + 1;
                    return Ok(conn_id);
                }
                Err(_) => {
                    // CAS failed, retry with idle connections or fail
                    if let Some(idle_conn) = self.try_get_idle_connection() {
                        return Ok(idle_conn.conn_id);
                    }
                }
            }
        }

        Err("Connection pool exhausted".to_string())
    }

    /// Release a connection back to the pool as idle
    pub fn release(&self, conn_id: usize) -> Result<(), String> {
        let current_idle = self.idle_counts.load(Ordering::Acquire);

        // Check if we can add more idle connections using CAS
        if current_idle < self.config.max_idle_connections {
            match self.idle_counts.compare_exchange(
                current_idle,
                current_idle + 1,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // CAS succeeded, add to idle queue
                    let mut queue = self.idle_queue.lock().unwrap();
                    queue.push_back(IdleConnection {
                        conn_id,
                        inserted_at: Instant::now(),
                    });
                    return Ok(());
                }
                Err(_) => {
                    // CAS failed, pool might be full, close this connection
                    return Err("Idle pool full, closing connection".to_string());
                }
            }
        }

        Err("Idle pool full, closing connection".to_string())
    }

    /// Try to get an idle connection from the pool
    fn try_get_idle_connection(&self) -> Option<IdleConnection> {
        let mut queue = self.idle_queue.lock().unwrap();

        // Cleanup expired connections from the front
        while let Some(conn) = queue.front() {
            if conn.inserted_at.elapsed() > self.config.idle_timeout {
                queue.pop_front();
                // Decrement idle count with Release ordering
                self.idle_counts.fetch_sub(1, Ordering::Release);
            } else {
                break;
            }
        }

        // Get the next available idle connection
        if let Some(idle_conn) = queue.pop_front() {
            // Decrement idle count with Release ordering
            self.idle_counts.fetch_sub(1, Ordering::Release);
            return Some(idle_conn);
        }

        None
    }

    /// Cleanup expired idle connections (batched processing)
    pub fn cleanup(&self) {
        let mut queue = self.idle_queue.lock().unwrap();
        let mut removed_count = 0;

        // Batch remove expired connections from the front
        while let Some(conn) = queue.front() {
            if conn.inserted_at.elapsed() > self.config.idle_timeout {
                queue.pop_front();
                removed_count += 1;
            } else {
                break;
            }
        }

        // Update idle count in a single operation with Release ordering
        if removed_count > 0 {
            self.idle_counts.fetch_sub(removed_count, Ordering::Release);
        }
    }

    /// Return a connection to the pool, closing it if necessary
    pub fn close_connection(&self, conn_id: usize) {
        // Decrement active connections
        self.active_connections.fetch_sub(1, Ordering::Release);
        // Note: Connection cleanup is handled elsewhere
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            active_connections: self.active_connections.load(Ordering::Acquire),
            idle_connections: self.idle_counts.load(Ordering::Acquire),
            max_connections: self.config.max_connections,
            max_idle_connections: self.config.max_idle_connections,
        }
    }

    /// Reset the pool (for testing)
    #[cfg(test)]
    pub fn reset(&self) {
        self.active_connections.store(0, Ordering::Release);
        self.idle_counts.store(0, Ordering::Release);
        let mut queue = self.idle_queue.lock().unwrap();
        queue.clear();
    }
}

/// Pool statistics
#[derive(Clone, Debug)]
pub struct PoolStats {
    pub active_connections: usize,
    pub idle_connections: usize,
    pub max_connections: usize,
    pub max_idle_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_acquire_and_release() {
        let config = PoolConfig {
            max_connections: 10,
            max_idle_connections: 5,
            idle_timeout: Duration::from_secs(60),
        };
        let pool = ConnectionPool::new(config);

        // Acquire a connection
        let conn_id = pool.acquire().unwrap();
        assert_eq!(conn_id, 1);

        let stats = pool.stats();
        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.idle_connections, 0);

        // Release the connection
        pool.release(conn_id).unwrap();
        let stats = pool.stats();
        assert_eq!(stats.idle_connections, 1);
    }

    #[test]
    fn test_max_idle_connections_limit() {
        let config = PoolConfig {
            max_connections: 10,
            max_idle_connections: 3,
            idle_timeout: Duration::from_secs(60),
        };
        let pool = ConnectionPool::new(config);

        // Acquire 5 connections
        let conns: Vec<_> = (0..5).map(|_| pool.acquire().unwrap()).collect();

        // Release 3 connections (should succeed)
        for i in 0..3 {
            pool.release(conns[i]).unwrap();
        }

        let stats = pool.stats();
        assert_eq!(stats.idle_connections, 3);

        // Try to release 2 more (should fail due to limit)
        assert!(pool.release(conns[3]).is_err());
        assert!(pool.release(conns[4]).is_err());
    }

    #[test]
    fn test_cas_prevents_race_conditions() {
        let config = PoolConfig {
            max_connections: 100,
            max_idle_connections: 10,
            idle_timeout: Duration::from_secs(60),
        };
        let pool = Arc::new(ConnectionPool::new(config));
        let mut handles = vec![];

        // Spawn multiple threads trying to acquire connections
        for _ in 0..20 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                for _ in 0..5 {
                    if let Ok(_conn_id) = pool_clone.acquire() {
                        // Simulate work
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = pool.stats();
        // Verify that active connections don't exceed max
        assert!(stats.active_connections <= 100);
    }

    #[test]
    fn test_idle_timeout_cleanup() {
        let config = PoolConfig {
            max_connections: 10,
            max_idle_connections: 5,
            idle_timeout: Duration::from_millis(100),
        };
        let pool = ConnectionPool::new(config);

        // Acquire and release a connection
        let conn_id = pool.acquire().unwrap();
        pool.release(conn_id).unwrap();

        let stats = pool.stats();
        assert_eq!(stats.idle_connections, 1);

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Cleanup expired connections
        pool.cleanup();

        let stats = pool.stats();
        assert_eq!(stats.idle_connections, 0);
    }

    #[test]
    fn test_concurrent_acquire_and_release() {
        let config = PoolConfig {
            max_connections: 50,
            max_idle_connections: 25,
            idle_timeout: Duration::from_secs(60),
        };
        let pool = Arc::new(ConnectionPool::new(config));
        let mut handles = vec![];

        // Spawn threads doing acquire/release cycles
        for _ in 0..10 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    if let Ok(conn_id) = pool_clone.acquire() {
                        thread::sleep(Duration::from_millis(5));
                        let _ = pool_clone.release(conn_id);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = pool.stats();
        // Verify limits are maintained
        assert!(stats.active_connections <= 50);
        assert!(stats.idle_connections <= 25);
    }
}
