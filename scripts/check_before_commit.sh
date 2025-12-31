#!/bin/bash
# 提交前检查脚本
# 确保所有代码质量检查通过后才能提交

set -e  # 遇到错误立即退出

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "  提交前检查脚本"
echo "=========================================="
echo ""

ERRORS=0
WARNINGS=0

# 函数：检查命令是否成功
check_command() {
    local cmd="$1"
    local name="$2"
    
    echo -n "检查 $name... "
    if eval "$cmd" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ 通过${NC}"
        return 0
    else
        echo -e "${RED}✗ 失败${NC}"
        ERRORS=$((ERRORS + 1))
        return 1
    fi
}

# 函数：检查命令并显示输出
check_command_verbose() {
    local cmd="$1"
    local name="$2"
    
    echo "检查 $name..."
    if eval "$cmd" 2>&1; then
        echo -e "${GREEN}✓ $name 通过${NC}"
        echo ""
        return 0
    else
        echo -e "${RED}✗ $name 失败${NC}"
        echo ""
        ERRORS=$((ERRORS + 1))
        return 1
    fi
}

# 1. 代码格式检查
echo "1. 代码格式检查"
if cargo fmt --check 2>&1 | grep -q "Diff"; then
    echo -e "${RED}✗ 代码格式不符合标准${NC}"
    echo "请运行: cargo fmt"
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓ 代码格式正确${NC}"
fi
echo ""

# 2. Clippy 检查
echo "2. Clippy 代码质量检查"
if cargo clippy --all-targets -- -W clippy::all 2>&1 | grep -q "warning:"; then
    echo -e "${RED}✗ 发现 Clippy 警告${NC}"
    echo "请运行: cargo clippy --all-targets -- -W clippy::all"
    cargo clippy --all-targets -- -W clippy::all 2>&1 | grep "warning:" | head -5
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓ Clippy 检查通过${NC}"
fi
echo ""

# 3. Cargo deny 检查
echo "3. 依赖检查 (cargo deny)"
if command -v cargo-deny > /dev/null 2>&1; then
    if cargo deny check 2>&1 | grep -qE "(error|denied)"; then
        echo -e "${RED}✗ 依赖检查失败${NC}"
        cargo deny check
        ERRORS=$((ERRORS + 1))
    else
        echo -e "${GREEN}✓ 依赖检查通过${NC}"
    fi
else
    echo -e "${YELLOW}⚠ cargo-deny 未安装，跳过${NC}"
    WARNINGS=$((WARNINGS + 1))
fi
echo ""

# 4. 编译检查
echo "4. 编译检查 (Debug)"
if cargo check --all-targets 2>&1 | grep -qE "error|warning:"; then
    echo -e "${RED}✗ Debug 编译失败或有警告${NC}"
    cargo check --all-targets 2>&1 | grep -E "error|warning:" | head -5
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓ Debug 编译成功${NC}"
fi
echo ""

echo "5. 编译检查 (Release)"
if cargo build --release 2>&1 | grep -qE "error|warning:"; then
    echo -e "${RED}✗ Release 编译失败或有警告${NC}"
    cargo build --release 2>&1 | grep -E "error|warning:" | head -5
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓ Release 编译成功${NC}"
fi
echo ""

# 5. 单元测试
echo "6. 单元测试"
if cargo test --lib 2>&1 | grep -q "test result:.*FAILED"; then
    echo -e "${RED}✗ 单元测试失败${NC}"
    cargo test --lib 2>&1 | grep -A 10 "test result:.*FAILED"
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓ 单元测试通过${NC}"
fi
echo ""

# 6. 版本号一致性检查
echo "7. 版本号一致性检查"
CARGO_VERSION=$(grep "^version" Cargo.toml | cut -d'"' -f2)
echo "  Cargo.toml 版本: $CARGO_VERSION"

# 检查主要文档中的版本号
DOCS_TO_CHECK=(
    "README.md:当前版本"
    "docs/README.md:版本"
    "docs/design/STRUCTURE.md:当前版本"
    "docs/reports/SECURITY.md:版本"
    "docs/reports/ANALYSIS.md:版本"
    "docs/guides/TEST_GUIDE.md:版本"
)

VERSION_MISMATCH=0
for doc_check in "${DOCS_TO_CHECK[@]}"; do
    file=$(echo $doc_check | cut -d':' -f1)
    pattern=$(echo $doc_check | cut -d':' -f2)
    
    if [ -f "$file" ]; then
        doc_version=$(grep "$pattern" "$file" | grep -oE "1\.0\.[0-9]+" | head -1)
        if [ -n "$doc_version" ] && [ "$doc_version" != "$CARGO_VERSION" ]; then
            echo -e "  ${RED}✗ $file: $doc_version (期望: $CARGO_VERSION)${NC}"
            VERSION_MISMATCH=1
        fi
    fi
done

if [ $VERSION_MISMATCH -eq 1 ]; then
    echo -e "${RED}✗ 版本号不一致${NC}"
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓ 版本号一致${NC}"
fi
echo ""

# 7. 检查根目录临时文件
echo "8. 根目录文件检查"
ROOT_MD_FILES=$(find . -maxdepth 1 -name "*.md" -type f ! -name "README.md" ! -name "CONTRIBUTING.md" ! -name "LICENSE" 2>/dev/null || true)
if [ -n "$ROOT_MD_FILES" ]; then
    echo -e "${RED}✗ 发现根目录临时文件:${NC}"
    echo "$ROOT_MD_FILES"
    echo "请将文件移动到 docs/ 目录下"
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓ 根目录文件正确${NC}"
fi
echo ""

# 总结
echo "=========================================="
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}✓ 所有检查通过！可以提交代码。${NC}"
    if [ $WARNINGS -gt 0 ]; then
        echo -e "${YELLOW}⚠ 有 $WARNINGS 个警告${NC}"
    fi
    exit 0
else
    echo -e "${RED}✗ 发现 $ERRORS 个错误，请修复后再提交${NC}"
    exit 1
fi
