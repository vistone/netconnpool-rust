#!/bin/bash
# 最终测试脚本 - 运行所有关键测试

set -e

echo "=========================================="
echo "NetConnPool Rust 最终测试验证"
echo "=========================================="
echo ""

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

FAILED=0

# 运行测试并检查结果
run_test() {
    local name=$1
    local cmd=$2
    echo "运行: $name"
    if eval "$cmd" > /tmp/test_$$.log 2>&1; then
        if grep -q "test result: ok" /tmp/test_$$.log && ! grep -q "test result: FAILED" /tmp/test_$$.log; then
            echo -e "${GREEN}✓ $name 通过${NC}"
            return 0
        else
            echo -e "${RED}✗ $name 失败${NC}"
            tail -20 /tmp/test_$$.log
            FAILED=$((FAILED + 1))
            return 1
        fi
    else
        echo -e "${RED}✗ $name 执行失败${NC}"
        tail -20 /tmp/test_$$.log
        FAILED=$((FAILED + 1))
        return 1
    fi
}

# 1. 单元测试
run_test "单元测试" "cargo test --lib"

# 2. 统计模块单元测试
run_test "统计模块单元测试" "cargo test --test stats_test"

# 3. 集成测试
run_test "集成测试" "cargo test --test integration_test -- --ignored"

# 4. 统计模块竞争条件测试
run_test "统计模块竞争条件测试" "cargo test --test stats_race_test -- --ignored"

# 5. 压力测试（快速版本）
run_test "压力测试-连接池耗尽" "cargo test --test stress_test test_connection_pool_exhaustion -- --ignored"
run_test "压力测试-快速获取释放" "cargo test --test stress_test test_rapid_acquire_release -- --ignored"

# 6. 性能基准测试（快速版本）
run_test "性能基准测试-统计收集" "cargo test --test benchmark_test benchmark_stats_collection -- --ignored"
run_test "性能基准测试-连接创建" "cargo test --test benchmark_test benchmark_connection_creation -- --ignored"

rm -f /tmp/test_$$.log

echo ""
echo "=========================================="
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}所有测试通过！${NC}"
    exit 0
else
    echo -e "${RED}有 $FAILED 个测试失败${NC}"
    exit 1
fi
