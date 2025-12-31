#!/bin/bash
# 运行压力测试脚本

set -euo pipefail

echo "=========================================="
echo "NetConnPool Rust 压力测试套件"
echo "=========================================="
echo ""

# 设置测试超时时间（秒）
TIMEOUT=3600

TEST_THREADS=1
TS=$(date +%Y%m%d_%H%M%S)
LOG_PREFIX="/tmp/stress_${TS}"

run_test() {
  local name="$1"
  local log="$2"
  shift 2

  echo ""
  echo ">>> ${name}"
  echo ">>> log: ${log}"
  timeout "$TIMEOUT" "$@" 2>&1 | tee "$log"
}

# 运行并发压力测试
echo "1. 运行并发连接测试..."
run_test "1. 并发连接测试" "${LOG_PREFIX}_1_concurrent.log" \
  cargo test --test stress_test test_concurrent_connections -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "2. 运行长时间运行测试（60秒）..."
run_test "2. 长时间运行测试（60秒）" "${LOG_PREFIX}_2_long_running.log" \
  cargo test --test stress_test test_long_running -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "3. 运行内存泄漏测试..."
run_test "3. 内存泄漏测试" "${LOG_PREFIX}_3_memory_leak.log" \
  cargo test --test stress_test test_memory_leak -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "4. 运行连接池耗尽测试..."
run_test "4. 连接池耗尽测试" "${LOG_PREFIX}_4_exhaustion.log" \
  cargo test --test stress_test test_connection_pool_exhaustion -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "5. 运行快速获取释放测试..."
run_test "5. 快速获取释放测试" "${LOG_PREFIX}_5_rapid.log" \
  cargo test --test stress_test test_rapid_acquire_release -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "6. 运行混合协议测试..."
run_test "6. 混合协议测试" "${LOG_PREFIX}_6_mixed.log" \
  cargo test --test stress_test test_mixed_protocols -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "7. 运行连接生命周期测试..."
run_test "7. 连接生命周期测试" "${LOG_PREFIX}_7_lifecycle.log" \
  cargo test --test stress_test test_connection_lifecycle -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "8. 运行高并发压力测试..."
run_test "8. 高并发压力测试" "${LOG_PREFIX}_8_high_concurrency.log" \
  cargo test --test stress_test test_high_concurrency_stress -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "性能基准测试"
echo "=========================================="
echo ""

echo "9. 运行获取/归还操作基准测试..."
run_test "9. 获取/归还操作基准测试" "${LOG_PREFIX}_9_bench_get_put.log" \
  cargo test --release --test benchmark_test benchmark_get_put_operations -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "10. 运行并发获取/归还基准测试..."
run_test "10. 并发获取/归还基准测试" "${LOG_PREFIX}_10_bench_concurrent.log" \
  cargo test --release --test benchmark_test benchmark_concurrent_get_put -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "11. 运行连接创建基准测试..."
run_test "11. 连接创建基准测试" "${LOG_PREFIX}_11_bench_create.log" \
  cargo test --release --test benchmark_test benchmark_connection_creation -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "12. 运行统计信息收集基准测试..."
run_test "12. 统计信息收集基准测试" "${LOG_PREFIX}_12_bench_stats.log" \
  cargo test --release --test benchmark_test benchmark_stats_collection -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "集成测试"
echo "=========================================="
echo ""

echo "13. 运行完整生命周期测试..."
run_test "13. 完整生命周期测试" "${LOG_PREFIX}_13_it_lifecycle.log" \
  cargo test --test integration_test test_full_lifecycle -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "14. 运行错误恢复测试..."
run_test "14. 错误恢复测试" "${LOG_PREFIX}_14_it_recovery.log" \
  cargo test --test integration_test test_error_recovery -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "15. 运行并发池操作测试..."
run_test "15. 并发池操作测试" "${LOG_PREFIX}_15_it_concurrent.log" \
  cargo test --test integration_test test_concurrent_pool_operations -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "统计模块专项测试"
echo "=========================================="

echo "16. 运行统计模块并发更新测试..."
run_test "16. 统计模块并发更新测试" "${LOG_PREFIX}_16_stats_updates.log" \
  cargo test --test stats_stress_test test_stats_concurrent_updates -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "17. 运行统计模块内存泄漏测试..."
run_test "17. 统计模块内存泄漏测试" "${LOG_PREFIX}_17_stats_mem.log" \
  cargo test --test stats_stress_test test_stats_memory_leak -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "18. 运行统计模块竞争条件测试..."
run_test "18. 统计模块竞争条件测试" "${LOG_PREFIX}_18_stats_race.log" \
  cargo test --test stats_stress_test test_stats_race_condition -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "19. 运行统计模块死循环防护测试..."
run_test "19. 统计模块死循环防护测试" "${LOG_PREFIX}_19_stats_infinite.log" \
  cargo test --test stats_stress_test test_stats_infinite_loop_prevention -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "20. 运行统计模块锁竞争测试..."
run_test "20. 统计模块锁竞争测试" "${LOG_PREFIX}_20_stats_lock.log" \
  cargo test --test stats_stress_test test_stats_lock_contention -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "21. 运行统计模块详细竞争条件测试..."
run_test "21. 统计模块详细竞争条件测试" "${LOG_PREFIX}_21_stats_race_detail.log" \
  cargo test --test stats_race_test test_stats_race_condition_detailed -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "22. 运行统计模块读取一致性测试..."
run_test "22. 统计模块读取一致性测试" "${LOG_PREFIX}_22_stats_consistency.log" \
  cargo test --test stats_race_test test_stats_get_stats_consistency -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "所有压力测试完成"
echo "=========================================="
echo ""
echo "测试日志已保存到 ${LOG_PREFIX}_*.log"
