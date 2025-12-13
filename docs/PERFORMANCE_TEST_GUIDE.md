# 性能测试指南

## 概述

本性能测试套件全面测试 NetConnPool 的性能指标，重点关注：
- **速度**：操作吞吐量 (ops/sec)
- **时间**：延迟分布 (P50/P95/P99)
- **IO吞吐量**：数据传输速率 (MB/s)
- **连接复用率**：连接池效率指标

## 测试场景

### 1. 获取/归还操作吞吐量测试 (`test_get_put_throughput`)

**目的**：测试连接池基本操作的吞吐量

**参数**：
- 操作数：100,000
- 最大连接数：100
- 最小连接数：20（预热）

**指标**：
- 吞吐量：> 100,000 ops/sec
- P99延迟：< 100微秒

**运行**：
```bash
cargo test --test performance_test test_get_put_throughput -- --ignored --nocapture
```

### 2. 并发吞吐量测试 (`test_concurrent_throughput`)

**目的**：测试高并发场景下的吞吐量

**参数**：
- 线程数：100
- 每线程操作数：5,000
- 总操作数：500,000
- 最大连接数：200

**指标**：
- 并发吞吐量：> 200,000 ops/sec

**运行**：
```bash
cargo test --test performance_test test_concurrent_throughput -- --ignored --nocapture
```

### 3. IO吞吐量测试 (`test_io_throughput`)

**目的**：测试通过连接池进行数据传输的IO吞吐量

**参数**：
- 操作数：10,000
- 数据大小：1KB/操作
- 最大连接数：50

**指标**：
- IO吞吐量：> 10 MB/s

**运行**：
```bash
cargo test --test performance_test test_io_throughput -- --ignored --nocapture
```

### 4. 延迟分布测试 (`test_latency_distribution`)

**目的**：测试获取和归还操作的延迟分布

**参数**：
- 操作数：50,000
- 最大连接数：100

**指标**：
- 获取操作P99延迟：< 50微秒
- 归还操作P99延迟：< 10微秒

**运行**：
```bash
cargo test --test performance_test test_latency_distribution -- --ignored --nocapture
```

### 5. 连接创建速度测试 (`test_connection_creation_speed`)

**目的**：测试连接创建的速度和效率

**参数**：
- 创建连接数：500
- 最大连接数：1,000

**指标**：
- 平均创建时间：< 10ms
- P95创建时间：记录

**运行**：
```bash
cargo test --test performance_test test_connection_creation_speed -- --ignored --nocapture
```

### 6. 高负载IO吞吐量测试 (`test_high_load_io_throughput`)

**目的**：测试高负载下的IO吞吐量

**参数**：
- 线程数：50
- 每线程操作数：1,000
- 数据大小：8KB/操作
- 最大连接数：100

**指标**：
- IO吞吐量：> 50 MB/s

**运行**：
```bash
cargo test --test performance_test test_high_load_io_throughput -- --ignored --nocapture
```

### 7. 统计信息收集性能测试 (`test_stats_collection_performance`)

**目的**：测试统计信息收集的性能开销

**参数**：
- 操作数：1,000,000
- 最大连接数：100

**指标**：
- 平均延迟：< 10微秒
- P99延迟：< 50微秒

**运行**：
```bash
cargo test --test performance_test test_stats_collection_performance -- --ignored --nocapture
```

### 8. 综合性能测试 (`test_comprehensive_performance`)

**目的**：模拟真实场景的综合性能测试

**参数**：
- 线程数：100
- 每线程操作数：10,000
- 操作类型：33%纯获取/归还，33%带IO操作，33%统计查询
- 最大连接数：200

**指标**：
- 操作吞吐量：> 100,000 ops/sec
- 连接复用率：> 5

**运行**：
```bash
cargo test --test performance_test test_comprehensive_performance -- --ignored --nocapture
```

## 完整性能报告

运行完整性能报告生成器，获取所有测试的综合报告：

```bash
cargo test --test performance_report generate_performance_report -- --ignored --nocapture
```

报告包含：
- 所有测试场景的详细结果
- 性能指标汇总
- 性能评估和建议

## 批量运行所有测试

使用提供的脚本运行所有性能测试：

```bash
./test/run_performance_tests.sh
```

脚本会：
1. 依次运行所有8个性能测试
2. 生成完整性能报告
3. 记录所有测试日志到 `/tmp/perf_test_*.log`
4. 显示总测试时间

## 性能基准

### 吞吐量基准

| 测试场景 | 最低要求 | 优秀标准 |
|---------|---------|---------|
| 单线程吞吐量 | 100,000 ops/sec | 200,000+ ops/sec |
| 并发吞吐量 | 200,000 ops/sec | 500,000+ ops/sec |
| IO吞吐量 | 10 MB/s | 50+ MB/s |
| 高负载IO吞吐量 | 50 MB/s | 100+ MB/s |

### 延迟基准

| 操作类型 | P50 | P95 | P99 |
|---------|-----|-----|-----|
| 获取操作 | < 5μs | < 20μs | < 50μs |
| 归还操作 | < 1μs | < 5μs | < 10μs |
| 统计收集 | < 1μs | < 10μs | < 50μs |
| 连接创建 | < 5ms | < 10ms | < 20ms |

### 连接复用率基准

| 场景 | 最低要求 | 优秀标准 |
|-----|---------|---------|
| 连接复用率 | > 3 | > 10 |

## 性能优化建议

1. **提高吞吐量**：
   - 增加连接池大小
   - 优化锁粒度
   - 使用无锁数据结构

2. **降低延迟**：
   - 预热连接池
   - 优化健康检查频率
   - 减少不必要的同步操作

3. **提高IO吞吐量**：
   - 使用更大的缓冲区
   - 批量IO操作
   - 优化网络参数

4. **提高连接复用率**：
   - 合理设置最大连接数
   - 优化连接生命周期管理
   - 减少连接泄漏

## 测试环境要求

- Rust 1.70+
- 足够的系统资源（建议至少4GB RAM）
- 稳定的网络环境（本地测试使用127.0.0.1）
- 足够的CPU核心（建议4+核心）

## 注意事项

1. 所有性能测试都标记为 `#[ignore]`，需要显式运行
2. 测试可能需要较长时间（几分钟到几十分钟）
3. 测试结果可能因硬件环境而异
4. 建议在release模式下运行以获得更准确的性能数据：
   ```bash
   cargo test --release --test performance_test -- --ignored --nocapture
   ```

## 性能测试结果示例

```
╔════════════════════════════════════════════════════════════════╗
║         NetConnPool Rust 全面性能测试报告                      ║
╚════════════════════════════════════════════════════════════════╝

【测试1】单线程获取/归还吞吐量测试
  操作数: 200000
  总耗时: 1.234s
  吞吐量: 162,074.23 ops/sec
  获取延迟 - P50: 3.45μs, P95: 12.34μs, P99: 45.67μs
  归还延迟 - P50: 0.89μs, P95: 3.21μs, P99: 8.76μs

【测试2】高并发吞吐量测试
  线程数: 200
  并发吞吐量: 456,789.12 ops/sec

【测试3】IO吞吐量测试
  IO吞吐量: 78.45 MB/s

【性能评估】
✅ 单线程吞吐量: 优秀 (162,074.23 ops/sec)
✅ 并发吞吐量: 优秀 (456,789.12 ops/sec)
✅ IO吞吐量: 优秀 (78.45 MB/s)
✅ 连接复用率: 优秀 (12.34%)

性能测试通过率: 4/4 (100.0%)
```
