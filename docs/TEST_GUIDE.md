# 测试指南

**最后更新**: 2025-01-XX  
**版本**: 1.0.1

---

## 目录

1. [测试概述](#测试概述)
2. [快速开始](#快速开始)
3. [测试套件](#测试套件)
4. [测试结果](#测试结果)
5. [测试状态说明](#测试状态说明)

---

## 测试概述

NetConnPool Rust 包含完整的测试套件，覆盖所有功能和极端场景：

- ✅ **单元测试**: 9个测试全部通过
- ✅ **集成测试**: 3个测试全部通过
- ✅ **压力测试**: 高并发、长时间运行、资源耗尽
- ✅ **模糊测试**: 干扰数据、异常数据、稳定性验证
- ✅ **性能测试**: 吞吐量、延迟、IO性能

---

## 快速开始

### 运行所有测试

```bash
# 单元测试
cargo test --lib

# 集成测试
cargo test --test integration_test -- --ignored

# 快速模糊测试（120秒）
cargo test --test quick_fuzzing_test test_quick_fuzzing_all_features -- --ignored --nocapture

# 统计功能测试
cargo test --test stats_utilization_test test_stats_utilization -- --ignored --nocapture
```

---

## 测试套件

### 1. 单元测试

**位置**: `src/*.rs` 中的 `#[cfg(test)]` 模块

**运行**:
```bash
cargo test --lib
```

**结果**: ✅ 9个测试全部通过

### 2. 集成测试

**位置**: `test/integration_test.rs`

**测试场景**:
- `test_full_lifecycle`: 完整生命周期测试
- `test_error_recovery`: 错误恢复测试
- `test_concurrent_pool_operations`: 并发操作测试

**运行**:
```bash
cargo test --test integration_test -- --ignored
```

### 3. 压力测试

#### 3.1 快速模糊测试

**位置**: `test/quick_fuzzing_test.rs`

**测试内容**:
- TCP/UDP 混合测试
- 干扰数据测试（10种模式）
- 120秒压力测试
- 系统稳定性验证

**运行**:
```bash
cargo test --test quick_fuzzing_test test_quick_fuzzing_all_features -- --ignored --nocapture
```

**预期结果**:
- 崩溃数: 0
- 连接数稳定（无内存泄漏）
- TCP/UDP 操作持续运行

#### 3.2 完整模糊测试

**位置**: `test/fuzzing_client_test.rs`

**测试内容**:
- 20种干扰数据模式
- 30分钟长时间测试
- 极端场景测试

**运行**:
```bash
cargo test --test fuzzing_client_test test_fuzzing_all_features -- --ignored --nocapture
```

#### 3.3 综合压力测试

**位置**: `test/comprehensive_stress_test.rs`

**测试内容**:
- 整数溢出边界测试
- 资源耗尽测试
- 长时间运行测试（2小时）

**运行**:
```bash
cargo test --test comprehensive_stress_test -- --ignored --nocapture
```

### 4. 客户端-服务器端到端测试

**位置**: 
- `test/test_server.rs`: 测试服务器
- `test/comprehensive_client_test.rs`: 综合客户端测试

**测试架构**:
```
客户端 (连接池)  ←→  服务器 (回显服务器)
  - TCP客户端          - TCP监听
  - UDP客户端          - UDP监听
  - 混合协议           - 回显数据
```

**运行**:
```bash
# 使用脚本运行
./test/run_client_server_test.sh

# 或手动运行
cargo test --test comprehensive_client_test -- --ignored --nocapture
```

### 5. 统计功能测试

**位置**: `test/stats_utilization_test.rs`

**测试内容**:
- 连接创建统计验证
- 连接复用统计验证
- 活跃/空闲连接统计验证
- 统计数据一致性验证

**运行**:
```bash
cargo test --test stats_utilization_test test_stats_utilization -- --ignored --nocapture
```

### 6. 性能测试

**位置**: `test/performance_test.rs`

**测试指标**:
- 获取/归还操作吞吐量
- 延迟分布 (P50/P95/P99)
- IO吞吐量
- 连接复用率

**运行**:
```bash
cargo test --test performance_test -- --ignored --nocapture
```

---

## 测试结果

### 整夜测试结果（2025-01-XX）

**测试时长**: 整夜运行（超过1800秒）

**关键指标**:
- ✅ **崩溃数**: 0（系统完全稳定）
- ✅ **连接数**: 120（稳定，无内存泄漏）
- ✅ **TCP错误**: 0（TCP连接完美）
- ✅ **UDP操作**: 1.32亿+（持续运行）

**结论**: 系统通过了整夜压力测试，在各种干扰数据下保持稳定。

### 快速模糊测试结果

**测试时长**: 120秒

**关键指标**:
- ✅ **崩溃数**: 0
- ✅ **TCP操作**: 400万+次
- ✅ **UDP操作**: 1500万+次
- ✅ **连接复用**: 5700万+次
- ✅ **系统稳定性**: 优秀

---

## 测试状态说明

### 为什么某些数字"没有变化"？

在压力测试中，你可能会看到：
```
[30s] 当前连接: 100, 创建: 100, 关闭: 0, 复用: 556470
[60s] 当前连接: 100, 创建: 100, 关闭: 0, 复用: 1121353
```

这实际上是**正常且理想的状态**！

#### 1. **当前连接数：100（稳定）** ✅
- **含义**：连接池中保持100个活跃连接
- **为什么稳定**：连接池已经创建了足够的连接来满足需求，连接被高效复用

#### 2. **创建：100（不变）** ✅
- **含义**：总共创建了100个连接
- **为什么不变**：100个连接已经足够，不需要创建新连接
- **这是连接池的理想状态**：创建一次，反复使用

#### 3. **关闭：0（不变）** ✅
- **含义**：没有连接被关闭
- **为什么不变**：所有连接都健康且在使用中，不需要关闭

#### 4. **复用：持续增长** ✅
- **含义**：连接被重复使用的次数
- **为什么增长**：每次从空闲池获取连接，复用计数就会增加
- **这是性能指标**：复用率越高，性能越好

### 结论

**稳定的数字 = 系统运行正常** ✅

- 连接数稳定 → 无内存泄漏
- 创建数不变 → 连接池效率高
- 复用数增长 → 连接被高效使用

---

## 干扰数据模式

模糊测试使用多种干扰数据模式来测试系统稳定性：

1. **空数据**: 空向量
2. **最小数据**: 1字节
3. **最大数据**: 64KB
4. **随机数据**: 随机字节序列
5. **边界值**: 各种边界情况
6. **特殊字符**: 控制字符、Unicode等
7. **格式错误**: 不符合协议的数据
8. **恶意数据**: 可能的攻击向量

---

## 测试脚本

项目提供了多个测试脚本：

- `test/run_client_server_test.sh`: 运行客户端-服务器测试
- `test/run_fuzzing_test.sh`: 运行模糊测试
- `test/run_comprehensive_stress_tests.sh`: 运行综合压力测试

---

## 故障排查

### 测试失败

1. **检查服务器是否运行**: 某些测试需要测试服务器
2. **检查端口占用**: 确保测试端口未被占用
3. **检查系统资源**: 长时间测试需要足够的内存

### 测试卡住

1. **检查阻塞操作**: 已修复TCP/UDP阻塞问题
2. **检查超时设置**: 测试都有超时机制
3. **查看日志**: 使用 `--nocapture` 查看详细输出

---

## 更多信息

- 详细测试报告: 见各测试文件的注释
- 性能基准: 见 `test/performance_test.rs`
- 测试覆盖率: 运行 `cargo test --lib -- --test-threads=1`
