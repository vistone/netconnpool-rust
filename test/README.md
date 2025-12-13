# 测试说明

本目录包含 NetConnPool Rust 版本的全面测试套件。

## 测试分类

### 1. 单元测试 (`*_test.rs`)
- `pool_test.rs` - 连接池基本功能测试
- `mode_test.rs` - 模式定义测试
- `protocol_test.rs` - 协议类型测试
- `ipversion_test.rs` - IP版本测试
- `stats_test.rs` - 统计信息测试

### 2. 压力测试 (`stress_test.rs`)
包含以下压力测试场景：

- **test_concurrent_connections** - 并发连接测试
  - 50个线程，每线程100次操作
  - 测试高并发场景下的连接池性能

- **test_long_running** - 长时间运行测试
  - 运行60秒，10个线程持续操作
  - 测试连接池的稳定性和资源管理

- **test_memory_leak** - 内存泄漏测试
  - 10000次迭代，检查连接是否正确释放
  - 验证没有内存泄漏

- **test_connection_pool_exhaustion** - 连接池耗尽测试
  - 测试连接池达到最大连接数时的行为
  - 验证错误处理和连接归还机制

- **test_rapid_acquire_release** - 快速获取释放测试
  - 10000次快速获取和释放操作
  - 测试连接复用效率

- **test_mixed_protocols** - 混合协议测试
  - 测试TCP和UDP连接的混合使用

- **test_connection_lifecycle** - 连接生命周期测试
  - 测试连接的创建、使用、过期和清理

- **test_high_concurrency_stress** - 高并发压力测试
  - 200个线程，每线程100次操作
  - 测试极限并发场景

### 3. 性能基准测试 (`benchmark_test.rs`)
包含以下性能基准测试：

- **benchmark_get_put_operations** - 获取/归还操作基准
  - 100000次操作，测量单线程性能

- **benchmark_concurrent_get_put** - 并发获取/归还基准
  - 50个线程，每线程2000次操作
  - 测量并发性能

- **benchmark_connection_creation** - 连接创建基准
  - 测量创建100个连接的耗时

- **benchmark_stats_collection** - 统计信息收集基准
  - 测量获取统计信息的性能开销

### 4. 集成测试 (`integration_test.rs`)
包含以下集成测试：

- **test_full_lifecycle** - 完整生命周期测试
  - 测试连接池从创建到关闭的完整流程

- **test_error_recovery** - 错误恢复测试
  - 测试连接失败时的错误处理和恢复机制

- **test_concurrent_pool_operations** - 并发池操作测试
  - 测试多种操作的并发执行

## 运行测试

### 运行所有单元测试
```bash
cargo test --lib
```

### 运行特定测试文件
```bash
cargo test --test stress_test
cargo test --test benchmark_test
cargo test --test integration_test
```

### 运行压力测试（需要标记为 ignored）
```bash
# 运行单个压力测试
cargo test --test stress_test test_concurrent_connections -- --ignored --nocapture

# 运行所有压力测试
cargo test --test stress_test -- --ignored --nocapture
```

### 运行所有测试（包括压力测试）
```bash
./test/run_stress_tests.sh
```

## 测试注意事项

1. **压力测试默认被忽略**：压力测试使用 `#[ignore]` 标记，需要使用 `--ignored` 标志运行
2. **需要测试服务器**：某些测试需要运行本地TCP服务器
3. **运行时间**：压力测试可能需要较长时间运行
4. **资源消耗**：高并发测试会消耗较多系统资源

## 性能指标

### 预期性能指标

- **单线程吞吐量**: > 10,000 ops/sec
- **并发吞吐量**: > 50,000 ops/sec
- **连接复用率**: > 95%
- **统计信息获取**: < 10 微秒

### 资源使用

- **内存**: 连接池本身内存占用应该很小
- **CPU**: 高并发场景下CPU使用率应该合理
- **连接数**: 不应超过配置的最大连接数

## 持续集成

建议在CI/CD流程中：
1. 运行所有单元测试
2. 定期运行压力测试（可以设置较短的超时时间）
3. 监控性能指标的变化
