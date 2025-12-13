// Copyright (c) 2025, vistone
// All rights reserved.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Stats 连接池统计信息
#[derive(Debug, Clone)]
pub struct Stats {
    /// TotalConnectionsCreated 累计创建的连接数
    pub TotalConnectionsCreated: i64,
    /// TotalConnectionsClosed 累计关闭的连接数
    pub TotalConnectionsClosed: i64,
    /// CurrentConnections 当前连接数
    pub CurrentConnections: i64,
    /// CurrentIdleConnections 当前空闲连接数
    pub CurrentIdleConnections: i64,
    /// CurrentActiveConnections 当前活跃连接数
    pub CurrentActiveConnections: i64,

    /// CurrentIPv4Connections 当前IPv4连接数
    pub CurrentIPv4Connections: i64,
    /// CurrentIPv6Connections 当前IPv6连接数
    pub CurrentIPv6Connections: i64,
    /// CurrentIPv4IdleConnections 当前IPv4空闲连接数
    pub CurrentIPv4IdleConnections: i64,
    /// CurrentIPv6IdleConnections 当前IPv6空闲连接数
    pub CurrentIPv6IdleConnections: i64,

    /// CurrentTCPConnections 当前TCP连接数
    pub CurrentTCPConnections: i64,
    /// CurrentUDPConnections 当前UDP连接数
    pub CurrentUDPConnections: i64,
    /// CurrentTCPIdleConnections 当前TCP空闲连接数
    pub CurrentTCPIdleConnections: i64,
    /// CurrentUDPIdleConnections 当前UDP空闲连接数
    pub CurrentUDPIdleConnections: i64,

    /// TotalGetRequests 累计获取连接请求数
    pub TotalGetRequests: i64,
    /// SuccessfulGets 成功获取连接数
    pub SuccessfulGets: i64,
    /// FailedGets 失败获取连接数
    pub FailedGets: i64,
    /// TimeoutGets 超时获取连接数
    pub TimeoutGets: i64,

    /// HealthCheckAttempts 健康检查尝试次数
    pub HealthCheckAttempts: i64,
    /// HealthCheckFailures 健康检查失败次数
    pub HealthCheckFailures: i64,
    /// UnhealthyConnections 不健康连接数
    pub UnhealthyConnections: i64,

    /// ConnectionErrors 连接错误数
    pub ConnectionErrors: i64,
    /// LeakedConnections 泄漏的连接数
    pub LeakedConnections: i64,

    /// TotalConnectionsReused 累计连接复用次数（从空闲池获取的次数）
    pub TotalConnectionsReused: i64,
    /// AverageReuseCount 平均每个连接的复用次数
    pub AverageReuseCount: f64,

    /// AverageGetTime 平均获取连接时间
    pub AverageGetTime: Duration,
    /// TotalGetTime 总获取连接时间
    pub TotalGetTime: Duration,

    /// LastUpdateTime 最后更新时间
    pub LastUpdateTime: Instant,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            TotalConnectionsCreated: 0,
            TotalConnectionsClosed: 0,
            CurrentConnections: 0,
            CurrentIdleConnections: 0,
            CurrentActiveConnections: 0,
            CurrentIPv4Connections: 0,
            CurrentIPv6Connections: 0,
            CurrentIPv4IdleConnections: 0,
            CurrentIPv6IdleConnections: 0,
            CurrentTCPConnections: 0,
            CurrentUDPConnections: 0,
            CurrentTCPIdleConnections: 0,
            CurrentUDPIdleConnections: 0,
            TotalGetRequests: 0,
            SuccessfulGets: 0,
            FailedGets: 0,
            TimeoutGets: 0,
            HealthCheckAttempts: 0,
            HealthCheckFailures: 0,
            UnhealthyConnections: 0,
            ConnectionErrors: 0,
            LeakedConnections: 0,
            TotalConnectionsReused: 0,
            AverageReuseCount: 0.0,
            AverageGetTime: Duration::ZERO,
            TotalGetTime: Duration::ZERO,
            LastUpdateTime: Instant::now(),
        }
    }
}

/// StatsCollector 统计收集器
pub struct StatsCollector {
    stats: StatsInternal,
    last_update_time: RwLock<Instant>,
}

struct StatsInternal {
    TotalConnectionsCreated: AtomicI64,
    TotalConnectionsClosed: AtomicI64,
    CurrentConnections: AtomicI64,
    CurrentIdleConnections: AtomicI64,
    CurrentActiveConnections: AtomicI64,
    CurrentIPv4Connections: AtomicI64,
    CurrentIPv6Connections: AtomicI64,
    CurrentIPv4IdleConnections: AtomicI64,
    CurrentIPv6IdleConnections: AtomicI64,
    CurrentTCPConnections: AtomicI64,
    CurrentUDPConnections: AtomicI64,
    CurrentTCPIdleConnections: AtomicI64,
    CurrentUDPIdleConnections: AtomicI64,
    TotalGetRequests: AtomicI64,
    SuccessfulGets: AtomicI64,
    FailedGets: AtomicI64,
    TimeoutGets: AtomicI64,
    HealthCheckAttempts: AtomicI64,
    HealthCheckFailures: AtomicI64,
    UnhealthyConnections: AtomicI64,
    ConnectionErrors: AtomicI64,
    LeakedConnections: AtomicI64,
    TotalConnectionsReused: AtomicI64,
    AverageGetTime: AtomicU64, // Duration as nanoseconds
    TotalGetTime: AtomicU64,     // Duration as nanoseconds
}

impl StatsCollector {
    /// NewStatsCollector 创建统计收集器
    pub fn new() -> Self {
        Self {
            stats: StatsInternal {
                TotalConnectionsCreated: AtomicI64::new(0),
                TotalConnectionsClosed: AtomicI64::new(0),
                CurrentConnections: AtomicI64::new(0),
                CurrentIdleConnections: AtomicI64::new(0),
                CurrentActiveConnections: AtomicI64::new(0),
                CurrentIPv4Connections: AtomicI64::new(0),
                CurrentIPv6Connections: AtomicI64::new(0),
                CurrentIPv4IdleConnections: AtomicI64::new(0),
                CurrentIPv6IdleConnections: AtomicI64::new(0),
                CurrentTCPConnections: AtomicI64::new(0),
                CurrentUDPConnections: AtomicI64::new(0),
                CurrentTCPIdleConnections: AtomicI64::new(0),
                CurrentUDPIdleConnections: AtomicI64::new(0),
                TotalGetRequests: AtomicI64::new(0),
                SuccessfulGets: AtomicI64::new(0),
                FailedGets: AtomicI64::new(0),
                TimeoutGets: AtomicI64::new(0),
                HealthCheckAttempts: AtomicI64::new(0),
                HealthCheckFailures: AtomicI64::new(0),
                UnhealthyConnections: AtomicI64::new(0),
                ConnectionErrors: AtomicI64::new(0),
                LeakedConnections: AtomicI64::new(0),
                TotalConnectionsReused: AtomicI64::new(0),
                AverageGetTime: AtomicU64::new(0),
                TotalGetTime: AtomicU64::new(0),
            },
            last_update_time: RwLock::new(Instant::now()),
        }
    }

    /// IncrementTotalConnectionsCreated 增加创建连接计数
    pub fn IncrementTotalConnectionsCreated(&self) {
        self.stats.TotalConnectionsCreated.fetch_add(1, Ordering::Relaxed);
        self.stats.CurrentConnections.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTotalConnectionsClosed 增加关闭连接计数
    pub fn IncrementTotalConnectionsClosed(&self) {
        self.stats.TotalConnectionsClosed.fetch_add(1, Ordering::Relaxed);
        self.stats.CurrentConnections.fetch_sub(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIdleConnections 增加空闲连接计数
    pub fn IncrementCurrentIdleConnections(&self, delta: i64) {
        self.stats.CurrentIdleConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentActiveConnections 增加活跃连接计数
    pub fn IncrementCurrentActiveConnections(&self, delta: i64) {
        self.stats.CurrentActiveConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTotalGetRequests 增加获取请求计数
    pub fn IncrementTotalGetRequests(&self) {
        self.stats.TotalGetRequests.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementSuccessfulGets 增加成功获取计数
    pub fn IncrementSuccessfulGets(&self) {
        self.stats.SuccessfulGets.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementFailedGets 增加失败获取计数
    pub fn IncrementFailedGets(&self) {
        self.stats.FailedGets.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTimeoutGets 增加超时获取计数
    pub fn IncrementTimeoutGets(&self) {
        self.stats.TimeoutGets.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementHealthCheckAttempts 增加健康检查尝试计数
    pub fn IncrementHealthCheckAttempts(&self) {
        self.stats.HealthCheckAttempts.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementHealthCheckFailures 增加健康检查失败计数
    pub fn IncrementHealthCheckFailures(&self) {
        self.stats.HealthCheckFailures.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementUnhealthyConnections 增加不健康连接计数
    pub fn IncrementUnhealthyConnections(&self) {
        self.stats.UnhealthyConnections.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementConnectionErrors 增加连接错误计数
    pub fn IncrementConnectionErrors(&self) {
        self.stats.ConnectionErrors.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementLeakedConnections 增加泄漏连接计数
    pub fn IncrementLeakedConnections(&self) {
        self.stats.LeakedConnections.fetch_add(1, Ordering::Relaxed);
        self.update_time();
    }

    /// RecordGetTime 记录获取连接的时间
    pub fn RecordGetTime(&self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;
        self.stats.TotalGetTime.fetch_add(nanos, Ordering::Relaxed);

        // 计算平均时间（使用重试机制避免竞争条件，最多重试3次）
        let max_retries = 3;
        for retry in 0..max_retries {
            let total_gets = self.stats.SuccessfulGets.load(Ordering::Acquire);
            if total_gets > 0 {
                let total_time = self.stats.TotalGetTime.load(Ordering::Acquire);
                // 再次检查，确保值没有变化
                let total_gets2 = self.stats.SuccessfulGets.load(Ordering::Acquire);
                if total_gets == total_gets2 {
                    // 值稳定，可以安全计算平均值
                    if total_gets2 > 0 {
                        let avg_time = total_time / total_gets2 as u64;
                        self.stats.AverageGetTime.store(avg_time, Ordering::Release);
                    }
                    break;
                }
                // 如果值变化了，且不是最后一次重试，继续重试
                if retry < max_retries - 1 {
                    continue;
                }
                // 最后一次重试，使用当前值计算
                if total_gets2 > 0 {
                    let total_time2 = self.stats.TotalGetTime.load(Ordering::Acquire);
                    let avg_time = total_time2 / total_gets2 as u64;
                    self.stats.AverageGetTime.store(avg_time, Ordering::Release);
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
    pub fn GetStats(&self) -> Stats {
        let total_created = self.stats.TotalConnectionsCreated.load(Ordering::Relaxed);
        let total_reused = self.stats.TotalConnectionsReused.load(Ordering::Relaxed);
        let avg_reuse = if total_created > 0 {
            total_reused as f64 / total_created as f64
        } else {
            0.0
        };

        Stats {
            TotalConnectionsCreated: self.stats.TotalConnectionsCreated.load(Ordering::Relaxed),
            TotalConnectionsClosed: self.stats.TotalConnectionsClosed.load(Ordering::Relaxed),
            CurrentConnections: self.stats.CurrentConnections.load(Ordering::Relaxed),
            CurrentIdleConnections: self.stats.CurrentIdleConnections.load(Ordering::Relaxed),
            CurrentActiveConnections: self.stats.CurrentActiveConnections.load(Ordering::Relaxed),
            CurrentIPv4Connections: self.stats.CurrentIPv4Connections.load(Ordering::Relaxed),
            CurrentIPv6Connections: self.stats.CurrentIPv6Connections.load(Ordering::Relaxed),
            CurrentIPv4IdleConnections: self.stats.CurrentIPv4IdleConnections.load(Ordering::Relaxed),
            CurrentIPv6IdleConnections: self.stats.CurrentIPv6IdleConnections.load(Ordering::Relaxed),
            CurrentTCPConnections: self.stats.CurrentTCPConnections.load(Ordering::Relaxed),
            CurrentUDPConnections: self.stats.CurrentUDPConnections.load(Ordering::Relaxed),
            CurrentTCPIdleConnections: self.stats.CurrentTCPIdleConnections.load(Ordering::Relaxed),
            CurrentUDPIdleConnections: self.stats.CurrentUDPIdleConnections.load(Ordering::Relaxed),
            TotalGetRequests: self.stats.TotalGetRequests.load(Ordering::Relaxed),
            SuccessfulGets: self.stats.SuccessfulGets.load(Ordering::Relaxed),
            FailedGets: self.stats.FailedGets.load(Ordering::Relaxed),
            TimeoutGets: self.stats.TimeoutGets.load(Ordering::Relaxed),
            HealthCheckAttempts: self.stats.HealthCheckAttempts.load(Ordering::Relaxed),
            HealthCheckFailures: self.stats.HealthCheckFailures.load(Ordering::Relaxed),
            UnhealthyConnections: self.stats.UnhealthyConnections.load(Ordering::Relaxed),
            ConnectionErrors: self.stats.ConnectionErrors.load(Ordering::Relaxed),
            LeakedConnections: self.stats.LeakedConnections.load(Ordering::Relaxed),
            TotalConnectionsReused: total_reused,
            AverageReuseCount: avg_reuse,
            AverageGetTime: Duration::from_nanos(
                self.stats.AverageGetTime.load(Ordering::Relaxed),
            ),
            TotalGetTime: Duration::from_nanos(
                self.stats.TotalGetTime.load(Ordering::Relaxed),
            ),
            LastUpdateTime: *self.last_update_time.read().unwrap(),
        }
    }

    /// IncrementCurrentIPv4Connections 增加IPv4连接计数
    pub fn IncrementCurrentIPv4Connections(&self, delta: i64) {
        self.stats.CurrentIPv4Connections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIPv6Connections 增加IPv6连接计数
    pub fn IncrementCurrentIPv6Connections(&self, delta: i64) {
        self.stats.CurrentIPv6Connections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIPv4IdleConnections 增加IPv4空闲连接计数
    pub fn IncrementCurrentIPv4IdleConnections(&self, delta: i64) {
        self.stats.CurrentIPv4IdleConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentIPv6IdleConnections 增加IPv6空闲连接计数
    pub fn IncrementCurrentIPv6IdleConnections(&self, delta: i64) {
        self.stats.CurrentIPv6IdleConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentTCPConnections 增加TCP连接计数
    pub fn IncrementCurrentTCPConnections(&self, delta: i64) {
        self.stats.CurrentTCPConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentUDPConnections 增加UDP连接计数
    pub fn IncrementCurrentUDPConnections(&self, delta: i64) {
        self.stats.CurrentUDPConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentTCPIdleConnections 增加TCP空闲连接计数
    pub fn IncrementCurrentTCPIdleConnections(&self, delta: i64) {
        self.stats.CurrentTCPIdleConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementCurrentUDPIdleConnections 增加UDP空闲连接计数
    pub fn IncrementCurrentUDPIdleConnections(&self, delta: i64) {
        self.stats.CurrentUDPIdleConnections.fetch_add(delta, Ordering::Relaxed);
        self.update_time();
    }

    /// IncrementTotalConnectionsReused 增加连接复用计数
    pub fn IncrementTotalConnectionsReused(&self) {
        self.stats.TotalConnectionsReused.fetch_add(1, Ordering::Relaxed);
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
