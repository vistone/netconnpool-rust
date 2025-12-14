# 项目结构说明

```
netconnpool-rust/
├── src/                           # 源代码目录
│   ├── lib.rs                    # 库入口，导出所有公共 API
│   ├── config.rs                 # 配置结构和验证
│   ├── connection.rs             # 连接封装和生命周期管理
│   ├── errors.rs                 # 错误定义
│   ├── health.rs                 # 健康检查管理器
│   ├── ipversion.rs              # IP 版本检测
│   ├── leak.rs                   # 连接泄露检测器
│   ├── mode.rs                   # 连接池模式定义
│   ├── pool.rs                   # 核心连接池实现
│   ├── protocol.rs               # 协议类型检测
│   ├── stats.rs                  # 统计信息收集器
│   └── udp_utils.rs              # UDP 工具函数
│
├── test/                          # 测试文件目录
│   ├── README.md                 # 测试说明文档
│   │
│   ├── 单元测试/
│   │   ├── pool_test.rs         # 连接池基本功能测试
│   │   ├── mode_test.rs         # 模式定义测试
│   │   ├── protocol_test.rs      # 协议类型测试
│   │   ├── ipversion_test.rs    # IP版本测试
│   │   └── stats_test.rs        # 统计信息测试
│   │
│   ├── 集成测试/
│   │   └── integration_test.rs  # 集成测试
│   │
│   ├── 压力测试/
│   │   └── stress_test.rs       # 压力测试套件
│   │
│   ├── 性能测试/
│   │   ├── benchmark_test.rs    # 性能基准测试
│   │   ├── performance_test.rs  # 性能测试
│   │   └── performance_report.rs # 性能报告
│   │
│   ├── 统计模块专项测试/
│   │   ├── stats_stress_test.rs # 统计模块压力测试
│   │   └── stats_race_test.rs   # 统计模块竞争条件测试
│   │
│   └── 测试脚本/
│       ├── run_stress_tests.sh      # 运行压力测试
│       ├── run_performance_tests.sh # 运行性能测试
│       ├── run_comprehensive_tests.sh # 综合测试脚本
│       └── run_final_tests.sh       # 最终测试验证脚本
│
├── examples/                      # 示例代码目录
│   ├── basic_example.rs          # 基本使用示例
│   ├── client_stress.rs         # 客户端压力测试示例
│   └── server_example.rs        # 服务器端示例
│
├── docs/                          # 文档目录
│   ├── INDEX.md                  # 文档索引（本目录）
│   ├── STRUCTURE.md              # 项目结构说明（本文件）
│   ├── CHANGELOG.md              # 变更日志
│   ├── RELEASE_NOTES.md          # 版本发布说明
│   │
│   ├── TEST_REPORT.md            # 全面测试报告
│   ├── TEST_SUMMARY.md           # 测试总结
│   ├── PERFORMANCE_TEST_GUIDE.md  # 性能测试指南
│   ├── STRESS_TEST_GUIDE.md      # 压力测试指南
│   ├── STRESS_TEST_COMPLETE.md   # 压力测试完成报告
│   │
│   ├── CODE_REVIEW.md            # 代码审查记录
│   └── STATS_SECURITY_AUDIT.md   # 统计模块安全审计
│
├── .github/                       # GitHub 配置
│   └── workflows/
│       └── rust.yml              # CI/CD 工作流
│
├── Cargo.toml                    # Rust 项目配置文件
├── Cargo.lock                    # 依赖锁定文件（自动生成）
├── .gitignore                    # Git 忽略文件配置
└── README.md                     # 项目主文档
```

## 核心文件说明

### 连接池核心
- **pool.rs**: 连接池的核心实现，包含连接获取、归还、创建等逻辑
- **connection.rs**: 连接对象的封装，提供线程安全的连接信息访问
- **config.rs**: 配置结构体和默认配置，支持客户端/服务器端两种模式

### 工具和辅助
- **stats.rs**: 统计信息收集器，提供详细的连接池使用统计
- **protocol.rs**: 协议类型检测（TCP/UDP）
- **ipversion.rs**: IP 版本检测（IPv4/IPv6）
- **udp_utils.rs**: UDP 特定的工具函数，如缓冲区清理
- **errors.rs**: 错误定义和常量
- **mode.rs**: 连接池模式定义（客户端/服务器端）

### 管理器（待完善）
- **health.rs**: 健康检查管理器（占位，功能在 pool.rs 中实现）
- **leak.rs**: 泄露检测器（占位，功能在 pool.rs 中实现）

## 目录说明

### src/
包含所有源代码文件，按照功能模块组织。

### test/
包含所有测试文件：
- 单元测试覆盖各个模块
- 集成测试验证整体功能

### examples/
包含各种使用场景的示例代码：
- 基本的 TCP/UDP 连接池使用
- 客户端和服务器端模式示例

### docs/
包含项目文档：
- README.md: 项目说明和使用指南
- STRUCTURE.md: 项目结构说明（本文件）

## 代码组织原则

1. **关注点分离**: 每个文件负责特定的功能模块
2. **清晰的命名**: 文件名直接反映其功能
3. **模块化设计**: 管理器独立实现，易于测试和维护
4. **文档齐全**: 每个目录都有相应的说明文档
5. **函数名一致**: 与原 Go 版本保持相同的函数名

## 开发建议

- 核心逻辑修改：主要关注 `pool.rs` 和 `connection.rs`
- 添加新功能：考虑创建新的管理器文件
- 性能优化：关注 `stats.rs` 和锁的使用
- 添加测试：在 `test/` 目录下创建新的测试文件
- 添加示例：在 `examples/` 目录下创建新的示例程序
