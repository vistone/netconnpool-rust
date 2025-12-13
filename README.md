# NetConnPool - Rust 网络连接池管理库

[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)

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
netconnpool = { path = "." }
```

## 快速开始

### 客户端模式（默认）

客户端模式用于主动连接到服务器的场景，适用于HTTP客户端、数据库客户端、RPC客户端等。

```rust
use netconnpool::*;
use std::net::TcpStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建客户端连接池配置
    let mut config = DefaultConfig();
    config.MaxConnections = 10;
    config.MinConnections = 2; // 预热2个连接
    
    // 设置连接创建函数
    config.Dialer = Some(Box::new(|| {
        TcpStream::connect("127.0.0.1:8080")
            .map(|s| ConnectionType::Tcp(s))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }));
    
    // 创建连接池
    let pool = Pool::NewPool(config)?;
    
    // 获取连接
    let conn = pool.Get()?;
    
    // 使用连接进行网络操作
    if let Some(tcp_stream) = conn.GetTcpConn() {
        // ... 使用连接 ...
    }
    
    // 归还连接
    pool.Put(conn)?;
    
    // 关闭连接池
    pool.Close()?;
    
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
    let mut config = DefaultServerConfig();
    config.Listener = Some(listener);
    config.MaxConnections = 100;

    // 创建连接池
    let pool = Pool::NewPool(config)?;

    // 获取连接（等待接受客户端连接）
    let conn = pool.Get()?;

    // 使用连接处理客户端请求
    if let Some(tcp_stream) = conn.GetTcpConn() {
        // ... 处理客户端请求 ...
    }

    // 归还连接
    pool.Put(conn)?;

    // 关闭连接池
    pool.Close()?;

    Ok(())
}
```

## API 文档

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

## 测试

运行测试：

```bash
cargo test
```

运行特定测试：

```bash
cargo test --lib test_pool_creation
```

## 项目结构

```
netconnpool/
├── src/                    # 源代码
│   ├── config.rs          # 配置结构和验证
│   ├── connection.rs      # 连接封装和生命周期管理
│   ├── errors.rs          # 错误定义
│   ├── health.rs          # 健康检查管理器
│   ├── ipversion.rs        # IP 版本检测
│   ├── leak.rs            # 连接泄露检测器
│   ├── mode.rs            # 连接池模式定义
│   ├── pool.rs            # 核心连接池实现
│   ├── protocol.rs        # 协议类型检测
│   ├── stats.rs           # 统计信息收集器
│   └── udp_utils.rs        # UDP 工具函数
├── test/                  # 测试文件
├── examples/              # 示例代码
├── docs/                  # 文档
└── Cargo.toml            # 项目配置
```

## 许可证

BSD-3-Clause License

## 参考

- [Go 版本 netconnpool](https://github.com/vistone/netconnpool)
