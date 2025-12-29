# Changelog

所有重要的项目变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [1.0.1] - 2025-01-XX

### 修复
- 修复所有安全漏洞（panic风险、整数溢出、资源泄漏）
- 优化后台线程退出机制，使用可中断的sleep
- 修复统计功能在高并发下的不准确问题
- 完善错误处理，替换所有 `unwrap()` 调用

### 测试
- 新增模糊测试套件（fuzzing_client_test, quick_fuzzing_test）
- 新增客户端-服务器端到端测试（comprehensive_client_test）
- 新增统计功能测试（stats_utilization_test）
- 新增极端压力测试（extreme_stress_test, real_world_stress_test）

### 文档
- 整理docs目录，合并重复文档
- 更新所有文档以反映实际代码结构
- 完善测试指南和安全审计报告

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
所有函数名与原 Go 版本保持一致：
- `NewPool` - 创建新的连接池
- `Get` - 获取一个连接（自动选择IP版本）
- `GetIPv4` - 获取一个IPv4连接
- `GetIPv6` - 获取一个IPv6连接
- `GetTCP` - 获取一个TCP连接
- `GetUDP` - 获取一个UDP连接
- `GetWithProtocol` - 获取指定协议的连接
- `GetWithIPVersion` - 获取指定IP版本的连接
- `GetWithTimeout` - 获取一个连接（带超时）
- `Put` - 归还连接
- `Close` - 关闭连接池
- `Stats` - 获取统计信息

### 测试
- 9个单元测试全部通过
- 8个压力测试场景
- 4个性能基准测试
- 3个集成测试场景

### 文档
- 完整的 README.md
- 项目结构文档
- 压力测试指南
- 测试说明文档