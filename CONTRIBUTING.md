# 贡献指南

感谢您对 NetConnPool Rust 项目的关注！本文档提供了项目开发规范和贡献指南。

## 📋 开发规范

### 1. 文件组织结构

**严格遵循以下目录结构，不得在根目录随意放置文件：**

```
netconnpool-rust/
├── src/                    # 源代码目录
│   └── *.rs               # 源代码文件
├── test/                   # 测试文件目录
│   ├── unit/              # 单元测试
│   ├── integration/       # 集成测试
│   ├── stress/            # 压力测试
│   ├── fuzzing/           # 模糊测试
│   └── scripts/           # 测试脚本
├── docs/                   # 文档目录（所有文档必须在此目录下）
│   ├── design/            # 设计文档
│   ├── guides/            # 指南文档
│   └── reports/           # 报告文档
├── examples/               # 示例代码
├── scripts/                # 脚本文件
├── Cargo.toml             # 项目配置
├── deny.toml              # cargo-deny 配置
├── README.md              # 项目说明（仅此一个README在根目录）
└── CONTRIBUTING.md        # 本文件
```

**禁止行为：**
- ❌ 在根目录创建临时的 .md 文件（如 TEST_REPORT.md、ANALYSIS.md 等）
- ❌ 在根目录创建临时文档或报告文件
- ❌ 随意创建新的顶级目录
- ✅ 所有文档必须在 `docs/` 目录下，按类型归类

### 2. 文档与代码同步

**强制要求：**

1. **版本号同步**：
   - `Cargo.toml` 中的版本号是唯一来源
   - 所有文档中的版本号必须与 `Cargo.toml` 保持一致
   - 修改版本号时，必须同步更新所有相关文档

2. **代码变更同步**：
   - 任何代码修改必须同步更新相关文档
   - API 变更必须更新 `docs/RELEASE_NOTES.md`
   - 结构变更必须更新 `docs/design/STRUCTURE.md`
   - 配置变更必须更新 `README.md` 和相应文档

3. **变更记录**：
   - 所有修改必须在 `docs/CHANGELOG.md` 中记录
   - 使用标准的变更日志格式（基于 Keep a Changelog）
   - 必须包含：修复、新增、改进、变更等分类

### 3. Rust 编码规范

**严格遵循 Rust 官方编码标准：**

1. **代码格式**：
   ```bash
   cargo fmt
   ```
   - 提交前必须运行 `cargo fmt` 确保代码格式统一
   - CI/CD 会检查格式，不通过将拒绝提交

2. **代码质量检查**：
   ```bash
   cargo clippy --all-targets -- -W clippy::all
   ```
   - 必须修复所有 Clippy 警告
   - 不允许有任何警告或错误
   - GitHub Actions 会运行 Clippy 检查

3. **依赖检查**：
   ```bash
   cargo deny check
   ```
   - 必须通过 cargo-deny 检查
   - 确保许可证兼容性
   - 检查安全漏洞

4. **编译检查**：
   ```bash
   cargo check --all-targets
   cargo build --release
   ```
   - Debug 和 Release 模式都必须编译成功
   - 不允许有任何编译错误或警告

### 4. 测试要求

**提交前必须运行完整的测试套件：**

```bash
# 1. 单元测试
cargo test --lib

# 2. 集成测试
cargo test --test integration_test -- --ignored

# 3. 统计测试
cargo test --test stats_test

# 4. 压力测试（至少运行一次）
cargo test --test stats_stress_test -- --ignored
cargo test --test stats_race_test -- --ignored
cargo test --test idle_counts_cas_test

# 5. 完整测试验证
cargo test --all
```

**测试通过标准：**
- ✅ 所有测试必须通过（0 failed）
- ✅ 所有测试套件必须运行
- ✅ 压力测试必须稳定运行
- ✅ 不允许跳过任何应该运行的测试

### 5. 提交前检查清单

在提交代码到 GitHub 之前，**必须**完成以下检查：

```bash
# 1. 代码格式检查
cargo fmt --check

# 2. 代码质量检查
cargo clippy --all-targets -- -W clippy::all

# 3. 依赖检查
cargo deny check

# 4. 编译检查
cargo check --all-targets
cargo build --release

# 5. 测试检查
cargo test --all

# 6. 版本号一致性检查
grep -r "版本.*1\.0\." docs/ README.md
# 确保所有版本号与 Cargo.toml 一致
```

**检查清单：**
- [ ] 代码格式正确（`cargo fmt --check` 通过）
- [ ] 无 Clippy 警告（`cargo clippy` 无警告）
- [ ] 依赖检查通过（`cargo deny check` 通过）
- [ ] 编译成功（Debug 和 Release）
- [ ] 所有测试通过（`cargo test --all` 全部通过）
- [ ] 版本号统一（所有文档与 Cargo.toml 一致）
- [ ] 文档已更新（CHANGELOG.md 记录了所有变更）
- [ ] 无临时文件（根目录无临时 .md 文件）
- [ ] 文件归类正确（所有文件在正确目录）

### 6. Git 提交规范

**提交信息格式：**

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Type 类型：**
- `fix`: 修复 bug
- `feat`: 新功能
- `docs`: 文档更新
- `test`: 测试相关
- `refactor`: 代码重构
- `style`: 代码格式（不影响代码运行）
- `chore`: 构建过程或辅助工具的变动

**示例：**
```
fix(pool): 修复连接泄漏问题

- 修复了连接归还时的泄漏检测逻辑
- 更新了相关文档

Fixes #123
```

### 7. Pull Request 要求

**提交 PR 前必须确保：**

1. ✅ 所有检查清单项目都已完成
2. ✅ 代码已通过本地全面测试
3. ✅ 文档已同步更新
4. ✅ CHANGELOG.md 已记录变更
5. ✅ 版本号已统一（如涉及版本变更）
6. ✅ 无临时文件或无关文件

**PR 描述必须包含：**
- 变更说明
- 测试结果
- 相关 Issue 编号（如有）
- 检查清单完成情况

## 🚫 禁止事项

1. **禁止在根目录创建临时文件**
   - 所有文档必须在 `docs/` 目录下
   - 所有报告必须在 `docs/reports/` 目录下
   - 临时文件必须及时清理

2. **禁止跳过测试**
   - 不允许提交未测试的代码
   - 不允许跳过应该运行的测试
   - 压力测试必须定期运行

3. **禁止版本号不一致**
   - 不允许文档版本号与代码版本号不一致
   - 不允许随意更改版本号

4. **禁止忽略警告**
   - 不允许有 Clippy 警告
   - 不允许有编译警告（除非有明确理由）

5. **禁止文档与代码不同步**
   - 代码变更必须同步文档
   - 不允许文档描述与实际代码不一致

## 📝 文档更新指南

### 何时更新文档

1. **代码变更时**：
   - 修改 API → 更新 `docs/RELEASE_NOTES.md`
   - 修改结构 → 更新 `docs/design/STRUCTURE.md`
   - 修改配置 → 更新 `README.md`

2. **功能变更时**：
   - 新增功能 → 更新 `README.md` 和 `docs/CHANGELOG.md`
   - 修复 Bug → 更新 `docs/CHANGELOG.md`
   - 性能优化 → 更新 `docs/CHANGELOG.md`

3. **版本发布时**：
   - 更新 `Cargo.toml` 版本号
   - 同步所有文档中的版本号
   - 在 `docs/CHANGELOG.md` 添加新版本条目
   - 更新 `docs/RELEASE_NOTES.md`

### 文档组织规则

```
docs/
├── README.md              # 文档导航
├── CHANGELOG.md           # 变更日志（所有变更必须记录）
├── RELEASE_NOTES.md       # 发布说明
├── GITHUB_TOPICS.md       # GitHub 标签
├── design/                # 设计文档
│   ├── STRUCTURE.md       # 项目结构
│   └── *.md              # 其他设计文档
├── guides/                # 指南文档
│   ├── TEST_GUIDE.md      # 测试指南
│   └── *.md              # 其他指南
└── reports/               # 报告文档
    ├── SECURITY.md        # 安全报告
    └── *.md              # 其他报告
```

## 🔄 工作流程

### 标准开发流程

1. **创建分支**
   ```bash
   git checkout -b fix/your-fix-name
   ```

2. **编写代码**
   - 遵循 Rust 编码规范
   - 添加必要的测试
   - 更新相关文档

3. **本地验证**
   ```bash
   # 运行完整检查清单
   ./scripts/check_before_commit.sh  # 如果存在
   ```

4. **提交代码**
   ```bash
   git add .
   git commit -m "fix(scope): description"
   ```

5. **推送到远程**
   ```bash
   git push origin fix/your-fix-name
   ```

6. **创建 Pull Request**
   - 填写完整的 PR 描述
   - 等待 CI/CD 通过
   - 等待代码审查

### 版本发布流程

1. **更新版本号**
   - 在 `Cargo.toml` 中更新版本号

2. **同步文档版本号**
   - 更新所有文档中的版本号
   - 确保完全一致

3. **更新变更日志**
   - 在 `docs/CHANGELOG.md` 添加新版本条目
   - 更新 `docs/RELEASE_NOTES.md`

4. **运行完整测试**
   - 运行所有测试套件
   - 确保全部通过

5. **创建 Git 标签**
   ```bash
   git tag v1.0.x
   git push origin v1.0.x
   ```

## 📞 获取帮助

如有疑问，请：
1. 查看 `README.md` 了解项目概述
2. 查看 `docs/README.md` 了解文档结构
3. 查看 `docs/design/STRUCTURE.md` 了解代码结构
4. 提交 Issue 询问

---

**记住：在提交到 GitHub 之前，必须完成所有检查清单项目！**
