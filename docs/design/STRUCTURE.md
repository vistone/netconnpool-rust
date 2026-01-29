# 项目结构说明

```
netconnpool-rust/
├── src/                           # 源代码目录（9个模块文件）
│   ├── lib.rs                    # 库入口，导出所有公共 API
│   ├── config.rs                 # 配置结构和验证
│   ├── connection.rs             # 连接封装和生命周期管理
│   ├── errors.rs                 # 错误定义
│   ├── ipversion.rs              # IP 版本检测
│   ├── mode.rs                   # 连接池模式定义
│   ├── pool.rs                   # 核心连接池实现（包含健康检查和泄漏检测）
│   ├── protocol.rs               # 协议类型检测
│   ├── stats.rs                  # 统计信息收集器
│   └── udp_utils.rs              # UDP 工具函数
│
├── test/                          # 测试文件目录
│   ├── README.md                 # 测试说明文档
│   │
│   ├── 单元测试/
│   │   ├── pool_test.rs          # 连接池基本功能测试
│   │   ├── mode_test.rs          # 模式定义测试
│   │   ├── protocol_test.rs     # 协议类型测试
│   │   ├── ipversion_test.rs     # IP版本测试
│   │   └── stats_test.rs        # 统计信息测试
│   │
│   ├── 集成测试/
│   │   ├── integration_test.rs  # 集成测试
│   │   └── test_server.rs       # 测试服务器（用于端到端测试）
│   │
│   ├── 压力测试/
│   │   ├── stress_test.rs        # 基础压力测试
│   │   ├── comprehensive_stress_test.rs  # 综合压力测试
│   │   ├── extreme_stress_test.rs       # 极端压力测试
│   │   └── real_world_stress_test.rs    # 真实场景压力测试
│   │
│   ├── 模糊测试/
│   │   ├── fuzzing_client_test.rs      # 完整模糊测试
│   │   └── quick_fuzzing_test.rs       # 快速模糊测试
│   │
│   ├── 性能测试/
│   │   ├── benchmark_test.rs     # 性能基准测试
│   │   ├── performance_test.rs   # 性能测试
│   │   └── performance_report.rs # 性能报告
│   │
│   ├── 统计模块专项测试/
│   │   ├── stats_stress_test.rs  # 统计模块压力测试
│   │   ├── stats_race_test.rs    # 统计模块竞争条件测试
│   │   ├── stats_utilization_test.rs  # 统计功能使用测试
│   │   └── idle_counts_cas_test.rs   # 空闲计数CAS测试
│   │
│   ├── 客户端-服务器测试/
│   │   └── comprehensive_client_test.rs  # 综合客户端测试
│   │
│   └── 测试脚本/
│       ├── run_stress_tests.sh           # 运行压力测试
│       ├── run_performance_tests.sh      # 运行性能测试
│       ├── run_comprehensive_tests.sh     # 综合测试脚本
│       ├── run_comprehensive_stress_tests.sh  # 综合压力测试
│       ├── run_fuzzing_test.sh           # 模糊测试脚本
│       ├── run_client_server_test.sh      # 客户端-服务器测试
│       ├── run_final_tests.sh            # 最终测试验证脚本
│       ├── check_test_status.sh          # 检查测试状态
│       └── monitor_stress_test.sh        # 监控压力测试
│
├── examples/                      # 示例代码目录
│   ├── basic_example.rs          # 基本使用示例
│   ├── client_stress.rs         # 客户端压力测试示例
│   └── server_example.rs         # 服务器端示例
│
├── docs/                          # 文档目录
│   ├── README.md                 # 文档导航
│   ├── CHANGELOG.md               # 变更日志
│   ├── RELEASE_NOTES.md           # 版本发布说明
│   ├── GITHUB_TOPICS.md           # GitHub 仓库标签
│   ├── design/                    # 设计文档
│   │   ├── STRUCTURE.md           # 项目结构说明（本文件）
│   │   └── LOCK_FREE_OPTIMIZATION.md  # 无锁优化文档
│   ├── guides/                    # 指南文档
│   │   ├── TEST_GUIDE.md          # 测试指南
│   │   └── MANUAL_UPDATE_GUIDE.md # 手动更新指南
│   └── reports/                   # 报告文档
│       ├── SECURITY.md            # 安全审计报告
│       └── ANALYSIS.md            # 项目分析与改进建议
│
├── Cargo.toml                    # Rust 项目配置文件
├── Cargo.lock                    # 依赖锁定文件（自动生成）
├── .gitignore                    # Git 忽略文件配置
└── README.md                     # 项目主文档
```

## 核心文件说明

### 连接池核心
- **pool.rs**: 连接池的核心实现，包含连接获取、归还、创建、健康检查、泄漏检测等逻辑
- **connection.rs**: 连接对象的封装，提供线程安全的连接信息访问
- **config.rs**: 配置结构体和默认配置，支持客户端/服务器端两种模式

### 工具和辅助
- **stats.rs**: 统计信息收集器，提供详细的连接池使用统计
- **protocol.rs**: 协议类型检测（TCP/UDP）
- **ipversion.rs**: IP 版本检测（IPv4/IPv6）
- **udp_utils.rs**: UDP 特定的工具函数，如缓冲区清理
- **errors.rs**: 错误定义和常量
- **mode.rs**: 连接池模式定义（客户端/服务器端）

## 测试文件说明

### 单元测试
- `pool_test.rs`: 连接池基本功能测试
- `mode_test.rs`: 模式定义测试
- `protocol_test.rs`: 协议类型测试
- `ipversion_test.rs`: IP版本测试
- `stats_test.rs`: 统计信息测试

### 集成测试
- `integration_test.rs`: 集成测试，验证完整生命周期
- `test_server.rs`: 测试服务器，用于客户端-服务器端到端测试

### 压力测试
- `stress_test.rs`: 基础压力测试（8个场景）
- `comprehensive_stress_test.rs`: 综合压力测试（长时间运行、资源耗尽）
- `extreme_stress_test.rs`: 极端压力测试
- `real_world_stress_test.rs`: 真实场景压力测试

### 模糊测试
- `fuzzing_client_test.rs`: 完整模糊测试（20种干扰数据模式，30分钟）
- `quick_fuzzing_test.rs`: 快速模糊测试（10种模式，120秒）

### 性能测试
- `benchmark_test.rs`: 性能基准测试
- `performance_test.rs`: 性能测试（吞吐量、延迟）
- `performance_report.rs`: 性能报告生成

### 统计模块专项测试
- `stats_stress_test.rs`: 统计模块压力测试
- `stats_race_test.rs`: 统计模块竞争条件测试
- `stats_utilization_test.rs`: 统计功能使用测试
- `idle_counts_cas_test.rs`: 空闲计数CAS测试

### 客户端-服务器测试
- `comprehensive_client_test.rs`: 综合客户端测试，集成所有功能

## 目录说明

### src/
包含所有源代码文件，按照功能模块组织。注意：健康检查和泄漏检测功能已集成到 `pool.rs` 中，不再有独立的 `health.rs` 和 `leak.rs` 文件。

### test/
包含所有测试文件，按类型组织在子目录中：
- **unit/**: 单元测试
- **integration/**: 集成测试
- **stress/**: 压力测试和性能测试
- **fuzzing/**: 模糊测试
- **scripts/**: 测试脚本

使用 `Cargo.toml` 中的 `[[test]]` 配置来组织不同的测试套件。

### examples/
包含各种使用场景的示例代码：
- 基本的 TCP/UDP 连接池使用
- 客户端和服务器端模式示例

### docs/
包含项目文档，按类型组织：
- **README.md**: 文档导航和快速开始
- **CHANGELOG.md**: 变更日志
- **RELEASE_NOTES.md**: 版本发布说明
- **design/**: 设计相关文档
  - STRUCTURE.md: 项目结构说明（本文件）
  - LOCK_FREE_OPTIMIZATION.md: 无锁队列优化说明
- **guides/**: 指南文档
  - TEST_GUIDE.md: 完整的测试指南
  - MANUAL_UPDATE_GUIDE.md: 手动更新 GitHub 指南
- **reports/**: 报告文档
  - SECURITY.md: 安全审计报告
  - ANALYSIS.md: 项目分析与改进建议

## 代码组织原则

1. **关注点分离**: 每个文件负责特定的功能模块
2. **清晰的命名**: 文件名直接反映其功能
3. **模块化设计**: 功能独立实现，易于测试和维护
4. **文档齐全**: 每个目录都有相应的说明文档
5. **函数名一致**: 与原 Go 版本保持相同的函数名

## 开发建议

- 核心逻辑修改：主要关注 `pool.rs` 和 `connection.rs`
- 添加新功能：考虑创建新的模块文件
- 性能优化：关注 `stats.rs` 和锁的使用
- 添加测试：在 `test/` 目录下创建新的测试文件，并在 `Cargo.toml` 中添加 `[[test]]` 配置
- 添加示例：在 `examples/` 目录下创建新的示例程序

## 版本信息

- **当前版本**: 1.0.4
- **最后更新**: 2025-01-XX
