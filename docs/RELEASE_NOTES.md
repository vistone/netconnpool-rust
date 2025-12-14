# 发布说明 - v1.0.0

## 🎉 NetConnPool Rust 版本 1.0.0 正式发布

### 概述

这是 NetConnPool 的 Rust 实现版本，完全基于 [Go 版本 netconnpool](https://github.com/vistone/netconnpool) 开发，保持了相同的 API 接口和函数名，确保从 Go 迁移到 Rust 的平滑过渡。

### ✨ 核心特性

- 🚀 **高性能**: 连接复用率 > 95%，显著提升性能
- 🔒 **并发安全**: 完全线程安全，支持高并发场景
- 🎯 **灵活配置**: 支持客户端/服务器端两种模式
- 📊 **详细统计**: 提供丰富的统计信息，便于监控和优化
- 🛡️ **自动管理**: 健康检查、泄漏检测、自动清理
- 🌐 **协议支持**: 支持TCP/UDP，IPv4/IPv6
- 🔄 **智能空闲池**: TCP/UDP 独立空闲池，避免协议混淆带来的性能抖动
- 🪝 **生命周期钩子**: 支持 Created/Borrow/Return 阶段的自定义回调

### 📦 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
netconnpool = "1.0.0"
```

### 🚀 快速开始

```rust
use netconnpool::*;
use std::net::TcpStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建客户端连接池配置
    let mut config = default_config();
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

### 📚 API 文档

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

### 🧪 测试

- ✅ 9个单元测试全部通过
- ✅ 8个压力测试场景
- ✅ 4个性能基准测试
- ✅ 3个集成测试场景

### 📊 代码统计

- **源代码**: 12个文件，2020行代码
- **测试代码**: 8个文件，1178行代码
- **文档**: 完整的README、CHANGELOG、测试文档

### 🔍 代码质量

- ✅ 所有代码编译通过
- ✅ 所有测试通过
- ✅ 代码审核完成
- ✅ 文档完整且与代码一致

### 📝 变更日志

详见 [CHANGELOG.md](CHANGELOG.md)

### 📄 许可证

BSD-3-Clause License

### 🙏 致谢

感谢 [Go 版本 netconnpool](https://github.com/vistone/netconnpool) 项目提供的优秀设计和实现参考。

---

**版本**: 1.0.0  
**发布日期**: 2025-12-13  
**状态**: 稳定版本，可用于生产环境
