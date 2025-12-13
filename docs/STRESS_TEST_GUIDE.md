# 压力测试指南

本文档说明如何运行 NetConnPool Rust 版本的全面压力测试和长时间测试。

## 测试套件概述

我们提供了三个主要的测试套件：

1. **压力测试** (`tests/stress_test.rs`) - 8个压力测试场景
2. **性能基准测试** (`tests/benchmark_test.rs`) - 4个性能基准测试
3. **集成测试** (`tests/integration_test.rs`) - 3个集成测试场景

## 快速开始

### 运行所有压力测试

```bash
# 运行所有压力测试（需要较长时间）
cargo test --test stress_test -- --ignored --nocapture

# 运行所有性能基准测试
cargo test --test benchmark_test -- --ignored --nocapture

# 运行所有集成测试
cargo test --test integration_test -- --ignored --nocapture
```

### 使用测试脚本

```bash
# 运行完整的测试套件（包括所有压力测试）
./test/run_stress_tests.sh
```

## 压力测试详情

### 1. test_concurrent_connections
**目的**: 测试高并发场景下的连接池性能

**参数**:
- 50个线程
- 每线程100次操作
- 最大连接数: 100

**验证点**:
- 所有操作都能成功完成
- 连接数不超过最大值
- 连接复用率合理

**运行时间**: 约10-30秒

### 2. test_long_running
**目的**: 测试连接池长时间运行的稳定性

**参数**:
- 运行60秒
- 10个线程持续操作
- 连接超时: 5秒
- 连接生命周期: 30秒

**验证点**:
- 长时间运行无崩溃
- 连接正确清理
- 统计信息准确

**运行时间**: 60秒

### 3. test_memory_leak
**目的**: 检测内存泄漏

**参数**:
- 10000次迭代
- 每次获取后立即归还

**验证点**:
- 连接数不会无限增长
- 内存使用稳定

**运行时间**: 约5-10秒

### 4. test_connection_pool_exhaustion
**目的**: 测试连接池耗尽时的行为

**参数**:
- 最大连接数: 10
- 尝试获取超过最大值的连接

**验证点**:
- 连接池耗尽时正确返回错误
- 归还连接后能重新获取

**运行时间**: 约1秒

### 5. test_rapid_acquire_release
**目的**: 测试快速获取和释放的性能

**参数**:
- 10000次操作
- 立即获取和归还

**验证点**:
- 连接复用率 > 10
- 吞吐量高

**运行时间**: 约1-2秒

### 6. test_mixed_protocols
**目的**: 测试TCP和UDP混合使用

**参数**:
- 20个线程
- 每线程50次操作

**验证点**:
- 不同协议连接正确管理
- 统计信息准确

**运行时间**: 约5-10秒

### 7. test_connection_lifecycle
**目的**: 测试连接生命周期管理

**参数**:
- 连接生命周期: 2秒
- 空闲超时: 1秒

**验证点**:
- 过期连接正确清理
- 新连接正确创建

**运行时间**: 约5秒

### 8. test_high_concurrency_stress
**目的**: 极限并发压力测试

**参数**:
- 200个线程
- 每线程100次操作
- 最大连接数: 200

**验证点**:
- 成功率 > 90%
- 连接数不超过最大值

**运行时间**: 约30-60秒

## 性能基准测试详情

### 1. benchmark_get_put_operations
**目的**: 测量单线程获取/归还操作的性能

**参数**:
- 100000次操作
- 预热10个连接

**预期指标**:
- 吞吐量 > 10,000 ops/sec
- 连接复用率 > 95%

### 2. benchmark_concurrent_get_put
**目的**: 测量并发获取/归还操作的性能

**参数**:
- 50个线程
- 每线程2000次操作

**预期指标**:
- 吞吐量 > 50,000 ops/sec

### 3. benchmark_connection_creation
**目的**: 测量连接创建的性能

**参数**:
- 创建100个连接

**预期指标**:
- 创建时间 < 10秒

### 4. benchmark_stats_collection
**目的**: 测量统计信息收集的性能开销

**参数**:
- 100000次统计信息获取

**预期指标**:
- 平均耗时 < 10微秒

## 集成测试详情

### 1. test_full_lifecycle
**目的**: 测试连接池的完整生命周期

**阶段**:
1. 预热阶段
2. 正常使用阶段
3. 高负载阶段
4. 清理和关闭

**验证点**:
- 各阶段正常运行
- 统计数据准确
- 正确关闭

### 2. test_error_recovery
**目的**: 测试错误恢复机制

**场景**:
- 模拟连接失败（每10次失败1次）

**验证点**:
- 错误正确处理
- 能继续正常工作

### 3. test_concurrent_pool_operations
**目的**: 测试多种操作的并发执行

**操作类型**:
- 获取和归还
- 获取TCP连接
- 获取IPv4连接
- 获取统计信息

**验证点**:
- 所有操作都能正常完成
- 无死锁或竞态条件

## 长时间测试建议

### 运行24小时测试

```bash
# 创建一个长时间运行的测试脚本
cat > long_test.sh << 'EOF'
#!/bin/bash
DURATION=86400  # 24小时
START=$(date +%s)

while [ $(($(date +%s) - START)) -lt $DURATION ]; do
    echo "运行测试循环 $(date)"
    cargo test --test stress_test test_long_running -- --ignored --nocapture
    sleep 60
done
EOF

chmod +x long_test.sh
./long_test.sh
```

### 监控指标

在长时间测试中，建议监控：

1. **内存使用**: 不应持续增长
2. **连接数**: 应在配置范围内
3. **CPU使用率**: 应在合理范围
4. **错误率**: 应保持较低水平

## 故障排查

### 测试失败常见原因

1. **端口被占用**: 确保测试端口可用
2. **资源不足**: 增加系统资源或减少并发数
3. **超时**: 增加超时时间或优化测试参数

### 调试技巧

```bash
# 运行单个测试并显示详细输出
RUST_BACKTRACE=1 cargo test --test stress_test test_concurrent_connections -- --ignored --nocapture

# 只编译不运行（检查编译错误）
cargo test --test stress_test --no-run

# 运行特定测试并设置超时
timeout 300 cargo test --test stress_test test_long_running -- --ignored --nocapture
```

## 性能调优建议

1. **调整连接池大小**: 根据实际负载调整 MaxConnections
2. **预热连接**: 设置 MinConnections 以减少首次请求延迟
3. **调整超时时间**: 根据网络条件调整各种超时参数
4. **监控统计信息**: 使用 Stats() 方法监控连接池状态

## 持续集成

在CI/CD流程中，建议：

1. 每次提交运行单元测试
2. 每日运行压力测试
3. 每周运行长时间测试（可以设置较短的时间）
4. 监控性能指标的变化趋势
