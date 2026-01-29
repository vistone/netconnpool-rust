# 手动更新 GitHub 仓库描述和标签指南

如果脚本遇到权限问题（403 错误），您可以手动在 GitHub 网页上更新。

## 问题原因

403 错误通常是因为：
1. **Token 权限不足**：需要 `repo` 或 `public_repo` 权限
2. **Token 类型**：Fine-grained token 需要额外配置权限
3. **Token 过期**：需要重新生成 token

## 解决方案

### 方案 1: 修复 Token 权限（推荐）

1. **访问 Token 设置**：
   - https://github.com/settings/tokens

2. **创建新的 Classic Token**：
   - 点击 "Generate new token" → "Generate new token (classic)"
   - 勾选以下权限：
     - ✅ `repo` (完整仓库访问权限)
     - ✅ `public_repo` (如果仓库是公开的)
   - 点击 "Generate token"
   - 复制生成的 token

3. **重新运行脚本**：
   ```bash
   ./update_github_repo.sh your_new_token
   ```

### 方案 2: 手动更新（无需 Token）

#### 更新仓库描述

1. 访问仓库设置：
   - https://github.com/vistone/netconnpool-rust/settings

2. 在 "Repository name" 下方找到 "Description" 字段

3. 输入以下描述：
   ```
   一个功能全面、高性能的 Rust 语言网络连接池管理库，提供连接生命周期管理、健康检查、统计监控等功能。支持 TCP/UDP、IPv4/IPv6，完全线程安全，适用于高并发场景。
   ```

4. 点击 "Save changes"

#### 添加仓库标签（Topics）

1. 访问仓库主页：
   - https://github.com/vistone/netconnpool-rust

2. 在仓库名称下方找到 "About" 部分

3. 点击 "⚙️" 图标（设置图标）

4. 在 "Topics" 字段中添加以下标签（每行一个或逗号分隔）：

```
rust
connection-pool
network
tcp
udp
ipv4
ipv6
async
concurrent
high-performance
thread-safe
network-programming
pool
connection-management
health-check
statistics
client-server
networking
rust-library
rust-crate
```

5. 点击 "Save changes"

## 标签说明

### 核心标签
- `rust` - Rust 语言
- `connection-pool` - 连接池
- `network` - 网络编程
- `tcp`, `udp` - 协议支持
- `ipv4`, `ipv6` - IP 版本支持

### 技术特性
- `async` - 异步支持
- `concurrent` - 并发
- `high-performance` - 高性能
- `thread-safe` - 线程安全
- `network-programming` - 网络编程

### 功能特性
- `pool` - 连接池
- `connection-management` - 连接管理
- `health-check` - 健康检查
- `statistics` - 统计功能
- `client-server` - 客户端/服务器

### 分类
- `networking` - 网络
- `rust-library` - Rust 库
- `rust-crate` - Rust crate

## 验证

更新完成后，访问仓库主页查看：
- https://github.com/vistone/netconnpool-rust

您应该能看到：
- ✅ 仓库描述已更新
- ✅ 所有标签已显示在 "About" 部分
