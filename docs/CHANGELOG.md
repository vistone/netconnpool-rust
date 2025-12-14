# Changelog

所有重要的项目变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [1.0.0] - 2025-12-14

### 修复
- 修复 `stats_test.rs` 作为集成测试的配置问题
- 修复 `test_stats_get_stats_consistency` 测试中的断言过于严格的问题
- 优化 `test_stats_long_running` 测试的输出频率

### 文档
- 添加文档索引 (docs/INDEX.md)
- 更新项目结构文档，反映完整的项目结构
- 更新 README.md，添加文档链接
- 更新所有文档中的 API 说明，使其与实际代码一致

---

## [1.0.0] - 2025-12-13

### 新增
- 完整的 Rust 网络连接池实现
- 支持客户端和服务器端两种模式
- 支持 TCP/UDP 协议
- 支持 IPv4/IPv6
- 连接生命周期管理
- 健康检查功能
- 连接泄漏检测
- 详细的统计信息收集
- UDP 缓冲区清理功能
- 完整的测试套件（单元测试、压力测试、性能基准测试、集成测试）
- 全面的文档和示例代码

### 特性
- 🚀 高性能：连接复用率 > 95%
- 🔒 并发安全：完全线程安全
- 🎯 灵活配置：丰富的配置选项
- 📊 详细统计：提供丰富的统计信息
- 🛡️ 自动管理：健康检查、泄漏检测、自动清理
- 🌐 协议支持：TCP/UDP，IPv4/IPv6
- 🔄 智能空闲池：TCP/UDP 独立空闲池

### API
主要 API（Rust 风格 snake_case）：
- `Pool::new` - 创建新的连接池
- `Pool::get` - 获取一个连接（自动选择IP版本）
- `Pool::get_ipv4` / `Pool::get_ipv6` - 获取指定 IP 版本连接
- `Pool::get_tcp` / `Pool::get_udp` - 获取指定协议连接
- `Pool::get_with_protocol` - 获取指定协议连接（可自定义超时）
- `Pool::get_with_ip_version` - 获取指定 IP 版本连接（可自定义超时）
- `Pool::get_with_timeout` - 获取连接（带超时）
- `Pool::close` - 关闭连接池
- `Pool::stats` - 获取统计信息

**注意**: 连接归还采用 RAII 机制，`PooledConnection` 在 `drop` 时自动归还到池中，无需手动调用 `Put` 方法。

### 测试
- 9个单元测试全部通过
- 2个统计模块单元测试全部通过
- 3个集成测试全部通过
- 8个压力测试场景
- 11个统计模块压力测试
- 4个统计模块竞争条件测试
- 4个性能基准测试

### 文档
- 完整的 README.md
- 项目结构文档 (docs/STRUCTURE.md)
- 文档索引 (docs/INDEX.md)
- 全面测试报告 (docs/TEST_REPORT.md)
- 测试总结 (docs/TEST_SUMMARY.md)
- 压力测试指南 (docs/STRESS_TEST_GUIDE.md)
- 性能测试指南 (docs/PERFORMANCE_TEST_GUIDE.md)
- 测试说明文档 (test/README.md)
