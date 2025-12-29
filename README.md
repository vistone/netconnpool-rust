# NetConnPool - Rust 网络连接池管理库

[![Rust](https://img.shields.io/badge/rust-1.92.0%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-100%25_passing-brightgreen.svg)](#测试)

一个功能全面、高性能的 Rust 语言网络连接池管理库，提供了完善的连接生命周期管理、健康检查、统计监控等功能。

这是 [Go 版本 netconnpool](https://github.com/vistone/netconnpool) 的 Rust 实现，保持了相同的 API 接口和函数名。

## 核心特性

- 🚀 **高性能**：连接复用率 > 95%，显著提升性能
- 🔒 **并发安全**：完全线程安全，支持高并发场景
- 🎯 **灵活配置**：支持客户端/服务器端两种模式
- 📊 **详细统计**：提供丰富的统计信息，便于监控和优化
- 🛡️ **自动管理**：健康检查、泄漏检测、自动清理
- 🌐 **协议支持**：支持TCP/UDP，IPv4/IPv6
- 🔄 **智能空闲池**：TCP/UDP 独立空闲池，避免协议混淆带来的性能抖动
- 🪝 **生命周期钩子**：支持 Created/Borrow/Return 阶段的自定义回调

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
netconnpool = "1.0.3"
```

或者从GitHub直接使用：

```toml
[dependencies]
netconnpool = { git = "https://github.com/vistone/netconnpool-rust", tag = "v1.0.3" }
```

## 快速开始

### 客户端模式（默认）

客户端模式用于主动连接到服务器的场景，适用于HTTP客户端、数据库客户端、RPC客户端等。

```rust
use netconnpool::*;
use std::net::TcpStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建客户端连接池配置
    let mut config = default_config();
    config.max_connections = 10;
    config.min_connections = 2; // 预热2个连接（后台 best-effort）
    
    // 设置连接创建函数
    config.dialer = Some(Box::new(|_protocol| {
        TcpStream::connect("127.0.0.1:8080")
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    
    // 创建连接池
    let pool = Pool::new(config)?;
    
    // 获取连接
    let conn = pool.get()?;
    
    // 使用连接进行网络操作
    if let Some(tcp_stream) = conn.tcp_conn() {
        // ... 使用连接 ...
    }
    
    // 归还连接：RAII 自动归还（drop 即可）
    drop(conn);
    
    // 关闭连接池
    pool.close()?;
    
    Ok(())
}
```

### 服务器端模式

服务器端模式用于接受客户端连接的场景，适用于HTTP服务器、TCP服务器等。

```rust
use netconnpool::*;
use std::net::TcpListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建监听器
    let listener = TcpListener::bind("127.0.0.1:8080")?;

    // 创建服务器端连接池配置
    let mut config = default_server_config();
    config.listener = Some(listener);
    config.max_connections = 100;

    // 创建连接池
    let pool = Pool::new(config)?;

    // 获取连接（等待接受客户端连接）
    let conn = pool.get()?;

    // 使用连接处理客户端请求
    if let Some(tcp_stream) = conn.tcp_conn() {
        // ... 处理客户端请求 ...
    }

    // 归还连接：RAII 自动归还
    drop(conn);

    // 关闭连接池
    pool.close()?;

    Ok(())
}
```

## API 文档

主要 API（Rust 风格 snake_case）：

- `Pool::new` - 创建新的连接池
- `Pool::get` - 获取一个连接（自动选择协议/IP版本）
- `Pool::get_ipv4` / `Pool::get_ipv6` - 获取指定 IP 版本连接
- `Pool::get_tcp` / `Pool::get_udp` - 获取指定协议连接
- `Pool::get_with_protocol` - 获取指定协议连接（可自定义超时）
- `Pool::get_with_ip_version` - 获取指定 IP 版本连接（可自定义超时）
- `Pool::get_with_timeout` - 获取连接（带超时）
- `Pool::close` - 关闭连接池
- `Pool::stats` - 获取统计信息

连接归还采用 RAII：`PooledConnection` 在 `drop` 时自动归还到池中。

## 测试

### 运行单元测试

```bash
cargo test --lib
```

### 运行压力测试

```bash
# 运行所有压力测试
cargo test --test stress_test -- --ignored --nocapture

# 运行性能基准测试
cargo test --test benchmark_test -- --ignored --nocapture

# 运行集成测试
cargo test --test integration_test -- --ignored --nocapture

# 使用测试脚本运行所有测试
./test/run_stress_tests.sh
```

### 运行特定测试

```bash
cargo test --lib test_pool_creation
```

## 项目结构

```text
netconnpool-rust/
├── src/                    # 源代码
│   ├── lib.rs             # 库入口，导出所有公共 API
│   ├── config.rs          # 配置结构和验证
│   ├── connection.rs      # 连接封装和生命周期管理
│   ├── errors.rs          # 错误定义
│   ├── ipversion.rs       # IP 版本检测
│   ├── mode.rs           # 连接池模式定义
│   ├── pool.rs           # 核心连接池实现（包含健康检查和泄漏检测）
│   ├── protocol.rs       # 协议类型检测
│   ├── stats.rs          # 统计信息收集器
│   └── udp_utils.rs      # UDP 工具函数
├── test/                  # 测试文件（详见 test/README.md）
│   ├── 单元测试/         # pool_test.rs, mode_test.rs, protocol_test.rs, ipversion_test.rs, stats_test.rs
│   ├── 集成测试/         # integration_test.rs, test_server.rs
│   ├── 压力测试/         # stress_test.rs, comprehensive_stress_test.rs, extreme_stress_test.rs, real_world_stress_test.rs
│   ├── 模糊测试/         # fuzzing_client_test.rs, quick_fuzzing_test.rs
│   ├── 性能测试/         # benchmark_test.rs, performance_test.rs, performance_report.rs
│   ├── 统计模块测试/     # stats_stress_test.rs, stats_race_test.rs, stats_utilization_test.rs, idle_counts_cas_test.rs
│   ├── 客户端-服务器测试/ # comprehensive_client_test.rs
│   └── 测试脚本/         # run_*.sh, check_test_status.sh, monitor_stress_test.sh
├── examples/              # 示例代码
│   ├── basic_example.rs   # 基本使用示例
│   ├── client_stress.rs  # 客户端压力测试示例
│   └── server_example.rs # 服务器端示例
├── docs/                  # 文档（详见 docs/README.md）
│   ├── README.md         # 文档导航
│   ├── STRUCTURE.md      # 项目结构说明
│   ├── TEST_GUIDE.md     # 测试指南
│   ├── SECURITY.md       # 安全审计报告
│   ├── ANALYSIS.md       # 项目分析与改进建议
│   └── ...              # 其他文档
└── Cargo.toml            # 项目配置
```

详细的项目结构说明请参考 [docs/STRUCTURE.md](docs/STRUCTURE.md)

## 版本

当前版本：**1.0.3**（最终稳定版）

## 许可证

BSD-3-Clause License

## 参考

- [Go 版本 netconnpool](https://github.com/vistone/netconnpool)

## 贡献

欢迎提交 Issue 和 Pull Request！

## 文档

- **[文档导航](docs/README.md)** - 所有文档的索引和导航
- **[项目结构](docs/STRUCTURE.md)** - 详细的代码组织结构
- **[变更日志](docs/CHANGELOG.md)** - 版本变更历史
- **[测试指南](docs/TEST_GUIDE.md)** - 完整的测试指南
- **[安全审计](docs/SECURITY.md)** - 安全审计报告
- **[项目分析](docs/ANALYSIS.md)** - 项目分析与改进建议
- **[测试说明](test/README.md)** - 如何运行和编写测试

## 更新日志

详见 [docs/CHANGELOG.md](docs/CHANGELOG.md)
