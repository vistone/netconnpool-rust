#!/bin/bash
# 全面性能测试脚本 - 记录速度、时间、IO吞吐量等关键指标

set -euo pipefail

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║     NetConnPool Rust 全面性能测试套件                          ║"
echo "║     Comprehensive Performance Test Suite                       ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

# 设置测试超时时间（秒）
TIMEOUT=3600

# 测试线程数：性能/报告/基准必须串行，避免端口/资源竞争与指标抖动
TEST_THREADS=1

# 记录开始时间
START_TIME=$(date +%s)

TS=$(date +%Y%m%d_%H%M%S)
LOG_PREFIX="/tmp/perf_${TS}"

run_test() {
  local name="$1"
  local log="$2"
  shift 2

  echo ""
  echo ">>> ${name}"
  echo ">>> log: ${log}"

  # 统一用 release + 串行线程，保证性能阈值稳定
  timeout "$TIMEOUT" "$@" 2>&1 | tee "$log"
}

echo "=========================================="
echo "性能基准测试"
echo "=========================================="
echo ""

echo "1. 获取/归还操作吞吐量测试..."
run_test "1. 获取/归还操作吞吐量测试" "${LOG_PREFIX}_perf_test_1.log" \
  cargo test --release --test performance_test test_get_put_throughput -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "2. 并发吞吐量测试..."
run_test "2. 并发吞吐量测试" "${LOG_PREFIX}_perf_test_2.log" \
  cargo test --release --test performance_test test_concurrent_throughput -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "3. IO吞吐量测试..."
run_test "3. IO吞吐量测试" "${LOG_PREFIX}_perf_test_3.log" \
  cargo test --release --test performance_test test_io_throughput -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "4. 延迟分布测试..."
run_test "4. 延迟分布测试" "${LOG_PREFIX}_perf_test_4.log" \
  cargo test --release --test performance_test test_latency_distribution -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "5. 连接创建速度测试..."
run_test "5. 连接创建速度测试" "${LOG_PREFIX}_perf_test_5.log" \
  cargo test --release --test performance_test test_connection_creation_speed -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "6. 高负载IO吞吐量测试..."
run_test "6. 高负载IO吞吐量测试" "${LOG_PREFIX}_perf_test_6.log" \
  cargo test --release --test performance_test test_high_load_io_throughput -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "7. 统计信息收集性能测试..."
run_test "7. 统计信息收集性能测试" "${LOG_PREFIX}_perf_test_7.log" \
  cargo test --release --test performance_test test_stats_collection_performance -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "8. 综合性能测试..."
run_test "8. 综合性能测试" "${LOG_PREFIX}_perf_test_8.log" \
  cargo test --release --test performance_test test_comprehensive_performance -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "生成完整性能报告"
echo "=========================================="
echo ""

echo "9. 生成完整性能测试报告..."
run_test "9. 生成完整性能测试报告" "${LOG_PREFIX}_perf_report.log" \
  cargo test --release --test performance_report generate_performance_report -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "性能基准（Benchmark）"
echo "=========================================="

echo ""
echo "10. 运行全部 benchmark_test..."
run_test "10. benchmark_test（全部）" "${LOG_PREFIX}_benchmark.log" \
  cargo test --release --test benchmark_test -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "性能测试完成"
echo "=========================================="
echo ""

# 计算总耗时
END_TIME=$(date +%s)
TOTAL_TIME=$((END_TIME - START_TIME))

echo "总测试时间: ${TOTAL_TIME} 秒"
echo ""
echo "测试日志已保存到 ${LOG_PREFIX}_*.log"
echo ""
