#!/bin/bash
# 全面测试脚本 - 运行所有测试并报告结果

set -e

echo "=========================================="
echo "NetConnPool Rust 全面测试"
echo "=========================================="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

FAILED_TESTS=0
PASSED_TESTS=0

# 运行测试函数
run_test_suite() {
    local suite_name=$1
    local test_command=$2
    local timeout=${3:-300}
    
    echo -e "${YELLOW}运行测试套件: ${suite_name}${NC}"
    
    if timeout $timeout bash -c "$test_command" > /tmp/test_${suite_name}_$$.log 2>&1; then
        local passed
        local failed
        passed=$(grep -c "test result: ok" /tmp/test_${suite_name}_$$.log 2>/dev/null || true)
        failed=$(grep -c "test result: FAILED" /tmp/test_${suite_name}_$$.log 2>/dev/null || true)
        passed=${passed:-0}
        failed=${failed:-0}
        
        if [ "$failed" -gt 0 ]; then
            echo -e "${RED}✗ ${suite_name} 有失败的测试${NC}"
            FAILED_TESTS=$((FAILED_TESTS + failed))
            tail -50 /tmp/test_${suite_name}_$$.log
            return 1
        else
            echo -e "${GREEN}✓ ${suite_name} 通过${NC}"
            PASSED_TESTS=$((PASSED_TESTS + 1))
            return 0
        fi
    else
        echo -e "${RED}✗ ${suite_name} 执行失败或超时${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        tail -50 /tmp/test_${suite_name}_$$.log
        return 1
    fi
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# 1. 单元测试
run_test_suite "单元测试" "cd \"$ROOT_DIR\" && cargo test --lib" 60

# 2. 统计模块单元测试
run_test_suite "统计模块单元测试" "cd \"$ROOT_DIR\" && cargo test --test stats_test" 60

# 3. 集成测试
run_test_suite "集成测试" "cd \"$ROOT_DIR\" && cargo test --test integration_test -- --ignored" 120

# 4. 压力测试（快速版本）
run_test_suite "压力测试-连接池耗尽" "cd \"$ROOT_DIR\" && cargo test --test stress_test test_connection_pool_exhaustion -- --ignored" 60
run_test_suite "压力测试-快速获取释放" "cd \"$ROOT_DIR\" && cargo test --test stress_test test_rapid_acquire_release -- --ignored" 60

# 5. 统计模块竞争条件测试
run_test_suite "统计模块竞争条件测试" "cd \"$ROOT_DIR\" && cargo test --test stats_race_test -- --ignored" 60

# 6. 性能基准测试（快速版本，跳过长时间运行的测试）
run_test_suite "性能基准测试" "cd \"$ROOT_DIR\" && cargo test --test benchmark_test benchmark_stats_collection -- --ignored && cargo test --test benchmark_test benchmark_connection_creation -- --ignored" 120

# 清理临时文件
rm -f /tmp/test_*_$$.log

# 输出总结
echo ""
echo "=========================================="
echo "测试总结"
echo "=========================================="
echo -e "${GREEN}通过的测试套件: ${PASSED_TESTS}${NC}"
if [ $FAILED_TESTS -gt 0 ]; then
    echo -e "${RED}失败的测试套件: ${FAILED_TESTS}${NC}"
    exit 1
else
    echo -e "${GREEN}失败的测试套件: ${FAILED_TESTS}${NC}"
    echo ""
    echo -e "${GREEN}所有测试通过！${NC}"
    exit 0
fi
