// Copyright (c) 2025, vistone
// All rights reserved.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Stats 连接池统计信息
#[derive(Debug, Clone)]
pub struct Stats {
    /// TotalConnectionsCreated 累计创建的连接数
    pub total_connections_created: i64,
    /// TotalConnectionsClosed 累计关闭的连接数
    pub total_connections_closed: i64,
    /// CurrentConnections 当前连接数
    pub current_connections: i64,
    /// CurrentIdleConnections 当前空闲连接数
    pub current_idle_connections: i64,
    /// CurrentActiveConnections 当前活跃连接数
    pub current_active_connections: i64,

    /// CurrentIPv4Connections 当前IPv4连接数
    pub current_ipv4_connections: i64,
    /// CurrentIPv6Connections 当前IPv6连接数
    pub current_ipv6_connections: i64,
    /// CurrentIPv4IdleConnections 当前IPv4空闲连接数
    pub current_ipv4_idle_connections: i64,
    /// CurrentIPv6IdleConnections 当前IPv6空闲连接数
    pub current_ipv6_idle_connections: i64,

    /// CurrentTCPConnections 当前TCP连接数
    pub current_tcp_connections: i64,
    /// CurrentUDPConnections 当前UDP连接数
    pub current_udp_connections: i64,
    /// CurrentTCPIdleConnections 当前TCP空闲连接数
    pub current_tcp_idle_connections: i64,
    /// CurrentUDPIdleConnections 当前UDP空闲连接数
    pub current_udp_idle_connections: i64,

    /// TotalGetRequests 累计获取连接请求数
    pub total_get_requests: i64,
    /// SuccessfulGets 成功获取连接数
    pub successful_gets: i64,
    /// FailedGets 失败获取连接数
    pub failed_gets: i64,
    /// TimeoutGets 超时获取连接数
    pub timeout_gets: i64,

    /// HealthCheckAttempts 健康检查尝试次数
    pub health_check_attempts: i64,
    /// HealthCheckFailures 健康检查失败次数
    pub health_check_failures: i64,
    /// UnhealthyConnections 不健康连接数
    pub unhealthy_connections: i64,

    /// ConnectionErrors 连接错误数
    pub connection_errors: i64,
    /// LeakedConnections 泄漏的连接数
    pub leaked_connections: i64,

    /// TotalConnectionsReused 累计连接复用次数（从空闲池获取的次数）
    pub total_connections_reused: i64,
    /// AverageReuseCount 平均每个连接的复用次数
    pub average_reuse_count: f64,

    /// AverageGetTime 平均获取连接时间
    pub average_get_time: Duration,
    /// TotalGetTime 总获取连接时间
    pub total_get_time: Duration,

    /// LastUpdateTime 最后更新时间
    pub last_update_time: Instant,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            total_connections_created: 0,
            total_connections_closed: 0,
            current_connections: 0,
            current_idle_connections: 0,
            current_active_connections: 0,
            current_ipv4_connections: 0,
            current_ipv6_connections: 0,
            current_ipv4_idle_connections: 0,
            current_ipv6_idle_connections: 0,
            current_tcp_connections: 0,
            current_udp_connections: 0,
            current_tcp_idle_connections: 0,
            current_udp_idle_connections: 0,
            total_get_requests: 0,
            successful_gets: 0,
            failed_gets: 0,
            timeout_gets: 0,
            health_check_attempts: 0,
            health_check_failures: 0,
            unhealthy_connections: 0,
            connection_errors: 0,
            leaked_connections: 0,
            total_connections_reused: 0,
            average_reuse_count: 0.0,
            average_get_time: Duration::ZERO,
            total_get_time: Duration::ZERO,
            last_update_time: Instant::now(),
        }
    }
}

/// StatsCollector 统计收集器
pub struct StatsCollector {
    stats: StatsInternal,
    last_update_time: RwLock<Instant>,
}

struct StatsInternal {
    total_connections_created: AtomicI64,
    total_connections_closed: AtomicI64,
    current_connections: AtomicI64,
    current_idle_connections: AtomicI64,
    current_active_connections: AtomicI64,
    current_ipv4_connections: AtomicI64,
    current_ipv6_connections: AtomicI64,
    current_ipv4_idle_connections: AtomicI64,
    current_ipv6_idle_connections: AtomicI64,
    current_tcp_connections: AtomicI64,
    current_udp_connections: AtomicI64,
    current_tcp_idle_connections: AtomicI64,
    current_udp_idle_connections: AtomicI64,
    total_get_requests: AtomicI64,
    successful_gets: AtomicI64,
    failed_gets: AtomicI64,
    timeout_gets: AtomicI64,
    health_check_attempts: AtomicI64,
    health_check_failures: AtomicI64,
    unhealthy_connections: AtomicI64,
    connection_errors: AtomicI64,
    leaked_connections: AtomicI64,
    total_connections_reused: AtomicI64,
    average_get_time: AtomicU64, // Duration as nanoseconds
    total_get_time: AtomicU64,   // Duration as nanoseconds
}

impl StatsCollector {
    /// NewStatsCollector 创建统计收集器
    pub fn new() -> Self {
        Self {
            stats: StatsInternal {
                total_connections_created: AtomicI64::new(0),
                total_connections_closed: AtomicI64::new(0),
                current_connections: AtomicI64::new(0),
                current_idle_connections: AtomicI64::new(0),
                current_active_connections: AtomicI64::new(0),
                current_ipv4_connections: AtomicI64::new(0),
                current_ipv6_connections: AtomicI64::new(0),
                current_ipv4_idle_connections: AtomicI64::new(0),
                current_ipv6_idle_connections: AtomicI64::new(0),
                current_tcp_connections: AtomicI64::new(0),
                current_udp_connections: AtomicI64::new(0),
                current_tcp_idle_connections: AtomicI64::new(0),
                current_udp_idle_connections: AtomicI64::new(0),
                total_get_requests: AtomicI64::new(0),
                successful_gets: AtomicI64::new(0),
                failed_gets: AtomicI64::new(0),
                timeout_gets: AtomicI64::new(0),
                health_check_attempts: AtomicI64::new(0),
                health_check_failures: AtomicI64::new(0),
                unhealthy_connections: AtomicI64::new(0),
                connection_errors: AtomicI64::new(0),
                leaked_connections: AtomicI64::new(0),
                total_connections_reused: AtomicI64::new(0),
                average_get_time: AtomicU64::new(0),
                total_get_time: AtomicU64::new(0),
            },
            last_update_time: RwLock::new(Instant::now()),
        }
    }

    /// IncrementTotalConnectionsCreated 增加创建连接计数
    pub fn increment_total_connections_created(&self) {
        self.stats
            .total_connections_created
            .fetch_add(1, Ordering::Relaxed);
        self.stats
            .current_connections
            .fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTotalConnectionsClosed 增加关闭连接计数
    pub fn increment_total_connections_closed(&self) {
        self.stats
            .total_connections_closed
            .fetch_add(1, Ordering::Relaxed);
        self.stats
            .current_connections
            .fetch_sub(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIdleConnections 增加空闲连接计数
    pub fn increment_current_idle_connections(&self, delta: i64) {
        self.stats
            .current_idle_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentActiveConnections 增加活跃连接计数
    pub fn increment_current_active_connections(&self, delta: i64) {
        self.stats
            .current_active_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTotalGetRequests 增加获取请求计数
    pub fn increment_total_get_requests(&self) {
        self.stats
            .total_get_requests
            .fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementSuccessfulGets 增加成功获取计数
    pub fn increment_successful_gets(&self) {
        self.stats.successful_gets.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementFailedGets 增加失败获取计数
    pub fn increment_failed_gets(&self) {
        self.stats.failed_gets.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTimeoutGets 增加超时获取计数
    pub fn increment_timeout_gets(&self) {
        self.stats.timeout_gets.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementHealthCheckAttempts 增加健康检查尝试计数
    pub fn increment_health_check_attempts(&self) {
        self.stats
            .health_check_attempts
            .fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementHealthCheckFailures 增加健康检查失败计数
    pub fn increment_health_check_failures(&self) {
        self.stats
            .health_check_failures
            .fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementUnhealthyConnections 增加不健康连接计数
    pub fn increment_unhealthy_connections(&self) {
        self.stats
            .unhealthy_connections
            .fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementConnectionErrors 增加连接错误计数
    pub fn increment_connection_errors(&self) {
        self.stats.connection_errors.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementLeakedConnections 增加泄漏连接计数
    pub fn increment_leaked_connections(&self) {
        self.stats
            .leaked_connections
            .fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// RecordGetTime 记录获取连接的时间
    pub fn record_get_time(&self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;
        self.stats
            .total_get_time
            .fetch_add(nanos, Ordering::Relaxed);

        // 计算平均时间（使用重试机制避免竞争条件，最多重试3次）
        let max_retries = 3;
        for retry in 0..max_retries {
            let total_gets = self.stats.successful_gets.load(Ordering::Acquire);
            if total_gets > 0 {
                let total_time = self.stats.total_get_time.load(Ordering::Acquire);
                // 再次检查，确保值没有变化
                let total_gets2 = self.stats.successful_gets.load(Ordering::Acquire);
                if total_gets == total_gets2 {
                    // 值稳定，可以安全计算平均值
                    if total_gets2 > 0 {
                        let avg_time = total_time / total_gets2 as u64;
                        self.stats
                            .average_get_time
                            .store(avg_time, Ordering::Release);
                    }
                    break;
                }
                // 如果值变化了，且不是最后一次重试，继续重试
                if retry < max_retries - 1 {
                    continue;
                }
                // 最后一次重试，使用当前值计算
                if total_gets2 > 0 {
                    let total_time2 = self.stats.total_get_time.load(Ordering::Acquire);
                    let avg_time = total_time2 / total_gets2 as u64;
                    self.stats
                        .average_get_time
                        .store(avg_time, Ordering::Release);
                }
                break;
            } else {
                // total_gets 为 0，不需要计算平均值
                break;
            }
        }
        self.update_time();
    }

    /// GetStats 获取当前统计信息快照
    pub fn get_stats(&self) -> Stats {
        let total_created = self.stats.total_connections_created.load(Ordering::Relaxed);
        let total_reused = self.stats.total_connections_reused.load(Ordering::Relaxed);
        let avg_reuse = if total_created > 0 {
            total_reused as f64 / total_created as f64
        } else {
            0.0
        };

        Stats {
            total_connections_created: self.stats.total_connections_created.load(Ordering::Relaxed),
            total_connections_closed: self.stats.total_connections_closed.load(Ordering::Relaxed),
            current_connections: self.stats.current_connections.load(Ordering::Relaxed),
            current_idle_connections: self.stats.current_idle_connections.load(Ordering::Relaxed),
            current_active_connections: self
                .stats
                .current_active_connections
                .load(Ordering::Relaxed),
            current_ipv4_connections: self.stats.current_ipv4_connections.load(Ordering::Relaxed),
            current_ipv6_connections: self.stats.current_ipv6_connections.load(Ordering::Relaxed),
            current_ipv4_idle_connections: self
                .stats
                .current_ipv4_idle_connections
                .load(Ordering::Relaxed),
            current_ipv6_idle_connections: self
                .stats
                .current_ipv6_idle_connections
                .load(Ordering::Relaxed),
            current_tcp_connections: self.stats.current_tcp_connections.load(Ordering::Relaxed),
            current_udp_connections: self.stats.current_udp_connections.load(Ordering::Relaxed),
            current_tcp_idle_connections: self
                .stats
                .current_tcp_idle_connections
                .load(Ordering::Relaxed),
            current_udp_idle_connections: self
                .stats
                .current_udp_idle_connections
                .load(Ordering::Relaxed),
            total_get_requests: self.stats.total_get_requests.load(Ordering::Relaxed),
            successful_gets: self.stats.successful_gets.load(Ordering::Relaxed),
            failed_gets: self.stats.failed_gets.load(Ordering::Relaxed),
            timeout_gets: self.stats.timeout_gets.load(Ordering::Relaxed),
            health_check_attempts: self.stats.health_check_attempts.load(Ordering::Relaxed),
            health_check_failures: self.stats.health_check_failures.load(Ordering::Relaxed),
            unhealthy_connections: self.stats.unhealthy_connections.load(Ordering::Relaxed),
            connection_errors: self.stats.connection_errors.load(Ordering::Relaxed),
            leaked_connections: self.stats.leaked_connections.load(Ordering::Relaxed),
            total_connections_reused: total_reused,
            average_reuse_count: avg_reuse,
            average_get_time: Duration::from_nanos(
                self.stats.average_get_time.load(Ordering::Relaxed),
            ),
            total_get_time: Duration::from_nanos(self.stats.total_get_time.load(Ordering::Relaxed)),
            last_update_time: *self.last_update_time.read().unwrap(),
        }
    }

    /// IncrementCurrentIPv4Connections 增加IPv4连接计数
    pub fn increment_current_ipv4_connections(&self, delta: i64) {
        self.stats
            .current_ipv4_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIPv6Connections 增加IPv6连接计数
    pub fn increment_current_ipv6_connections(&self, delta: i64) {
        self.stats
            .current_ipv6_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIPv4IdleConnections 增加IPv4空闲连接计数
    pub fn increment_current_ipv4_idle_connections(&self, delta: i64) {
        self.stats
            .current_ipv4_idle_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIPv6IdleConnections 增加IPv6空闲连接计数
    pub fn increment_current_ipv6_idle_connections(&self, delta: i64) {
        self.stats
            .current_ipv6_idle_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentTCPConnections 增加TCP连接计数
    pub fn increment_current_tcp_connections(&self, delta: i64) {
        self.stats
            .current_tcp_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentUDPConnections 增加UDP连接计数
    pub fn increment_current_udp_connections(&self, delta: i64) {
        self.stats
            .current_udp_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentTCPIdleConnections 增加TCP空闲连接计数
    pub fn increment_current_tcp_idle_connections(&self, delta: i64) {
        self.stats
            .current_tcp_idle_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentUDPIdleConnections 增加UDP空闲连接计数
    pub fn increment_current_udp_idle_connections(&self, delta: i64) {
        self.stats
            .current_udp_idle_connections
            .fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTotalConnectionsReused 增加连接复用计数
    pub fn increment_total_connections_reused(&self) {
        self.stats
            .total_connections_reused
            .fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    fn update_time(&self) {
        // 使用 try_write 避免在高并发下阻塞
        // 如果无法获取写锁，说明其他线程正在更新，可以跳过本次更新
        if let Ok(mut last_time) = self.last_update_time.try_write() {
            let now = Instant::now();
            // 减少时间更新频率，每100ms更新一次
            if now.duration_since(*last_time) >= Duration::from_millis(100) {
                *last_time = now;
            }
        }
        // 如果无法获取锁，说明其他线程正在更新，跳过本次更新是安全的
    }
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}
