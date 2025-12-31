#!/bin/bash
# 全面长时间运行压力测试脚本
# 测试内存溢出、资源泄漏、长时间稳定性

set -euo pipefail

echo "=========================================="
echo "NetConnPool Rust 全面压力测试套件"
echo "长时间运行、内存溢出、资源管理测试"
echo "=========================================="
echo ""

# 设置测试超时时间（秒）- 3小时
TIMEOUT=10800

TEST_THREADS=1
TS=$(date +%Y%m%d_%H%M%S)
LOG_DIR="/tmp/comprehensive_stress_${TS}"
mkdir -p "$LOG_DIR"

run_test() {
  local name="$1"
  local log="$2"
  shift 2

  echo ""
  echo ">>> ${name}"
  echo ">>> 日志文件: ${log}"
  echo ">>> 开始时间: $(date '+%Y-%m-%d %H:%M:%S')"
  
  # 使用 timeout 但设置较长的超时时间
  timeout "$TIMEOUT" "$@" 2>&1 | tee "$log"
  
  local exit_code=${PIPESTATUS[0]}
  echo ">>> 结束时间: $(date '+%Y-%m-%d %H:%M:%S')"
  
  if [ $exit_code -eq 0 ]; then
    echo ">>> ✅ ${name} 通过"
  elif [ $exit_code -eq 124 ]; then
    echo ">>> ⏱️  ${name} 超时（但可能仍在运行）"
  else
    echo ">>> ❌ ${name} 失败 (退出码: $exit_code)"
  fi
  
  return $exit_code
}

echo "=========================================="
echo "1. 长时间运行全面测试（2小时）"
echo "=========================================="
echo "此测试将运行2小时，测试："
echo "  - 内存泄漏"
echo "  - 资源管理"
echo "  - 长时间稳定性"
echo "  - 统计计数器溢出检测"
echo ""
read -p "是否继续？(y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    run_test "长时间运行全面测试（2小时）" "${LOG_DIR}/1_long_running.log" \
      cargo test --test comprehensive_stress_test test_long_running_comprehensive -- \
      --ignored --nocapture --test-threads="$TEST_THREADS"
else
    echo "跳过长时间运行测试"
fi

echo ""
echo "=========================================="
echo "2. 整数溢出边界测试"
echo "=========================================="
echo "此测试将执行100万次操作，测试统计计数器溢出检测"
echo ""
run_test "整数溢出边界测试" "${LOG_DIR}/2_integer_overflow.log" \
  cargo test --test comprehensive_stress_test test_integer_overflow_boundary -- \
  --ignored --nocapture --test-threads="$TEST_THREADS"

echo ""
echo "=========================================="
echo "3. 资源耗尽测试（30分钟）"
echo "=========================================="
echo "此测试将运行30分钟，测试连接池在资源耗尽时的行为"
echo ""
read -p "是否继续？(y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    run_test "资源耗尽测试（30分钟）" "${LOG_DIR}/3_resource_exhaustion.log" \
      cargo test --test comprehensive_stress_test test_resource_exhaustion -- \
      --ignored --nocapture --test-threads="$TEST_THREADS"
else
    echo "跳过资源耗尽测试"
fi

echo ""
echo "=========================================="
echo "4. 内存泄漏专项测试（1小时）"
echo "=========================================="
echo "此测试将运行1小时，专门测试内存泄漏"
echo ""
read -p "是否继续？(y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    run_test "内存泄漏专项测试（1小时）" "${LOG_DIR}/4_memory_leak.log" \
      cargo test --test stress_test test_memory_leak -- \
      --ignored --nocapture --test-threads="$TEST_THREADS" &
    
    # 在后台运行，定期检查内存使用
    TEST_PID=$!
    echo "测试进程 PID: $TEST_PID"
    
    # 每5分钟记录一次内存使用
    for i in {1..12}; do
        sleep 300  # 5分钟
        if ps -p $TEST_PID > /dev/null; then
            echo "[$(date '+%H:%M:%S')] 测试运行中... (已运行 $((i * 5)) 分钟)"
            # 可以在这里添加内存使用检查
            # ps -p $TEST_PID -o rss= | awk '{print "内存使用: " $1/1024 " MB"}'
        else
            echo "测试已完成"
            break
        fi
    done
    
    wait $TEST_PID
else
    echo "跳过内存泄漏专项测试"
fi

echo ""
echo "=========================================="
echo "5. 高并发持续压力测试（1小时）"
echo "=========================================="
echo "此测试将运行1小时，持续高并发压力"
echo ""
read -p "是否继续？(y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    run_test "高并发持续压力测试（1小时）" "${LOG_DIR}/5_high_concurrency.log" \
      cargo test --test stress_test test_long_running -- \
      --ignored --nocapture --test-threads="$TEST_THREADS" &
    
    TEST_PID=$!
    echo "测试进程 PID: $TEST_PID"
    
    # 每10分钟报告一次
    for i in {1..6}; do
        sleep 600  # 10分钟
        if ps -p $TEST_PID > /dev/null; then
            echo "[$(date '+%H:%M:%S')] 高并发测试运行中... (已运行 $((i * 10)) 分钟)"
        else
            echo "测试已完成"
            break
        fi
    done
    
    wait $TEST_PID
else
    echo "跳过高并发持续压力测试"
fi

echo ""
echo "=========================================="
echo "测试完成总结"
echo "=========================================="
echo ""
echo "所有测试日志已保存到: ${LOG_DIR}/"
echo ""
echo "日志文件列表:"
ls -lh "${LOG_DIR}"/*.log 2>/dev/null || echo "无日志文件"
echo ""
echo "请检查日志文件以查看详细测试结果"
echo ""

