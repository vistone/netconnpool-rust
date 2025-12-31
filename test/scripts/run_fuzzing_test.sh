#!/bin/bash
# 运行模糊测试 - 干扰数据压力测试

set -euo pipefail

echo "=========================================="
echo "模糊测试 - 干扰数据压力测试"
echo "测试系统在各种异常数据下的稳定性"
echo "=========================================="
echo ""

# 测试持续时间（秒）- 默认30分钟，可以通过参数修改
DURATION=${1:-1800}

echo "测试配置:"
echo "  持续时间: ${DURATION}秒 ($(($DURATION / 60))分钟)"
echo "  客户端线程: 120 (TCP: 60, UDP: 40, 极端: 20)"
echo "  干扰数据模式: 20种"
echo "  测试目标: 验证系统在异常数据下不会崩溃"
echo ""

read -p "是否继续？(y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "测试已取消"
    exit 0
fi

TS=$(date +%Y%m%d_%H%M%S)
LOG_FILE="/tmp/fuzzing_test_${TS}.log"

echo ""
echo "开始模糊测试..."
echo "日志文件: $LOG_FILE"
echo "按 Ctrl+C 可以提前终止测试"
echo ""

# 运行测试
timeout $((DURATION + 120)) cargo test --test fuzzing_client_test test_fuzzing_all_features -- \
    --ignored --nocapture 2>&1 | tee "$LOG_FILE"

EXIT_CODE=${PIPESTATUS[0]}

echo ""
echo "=========================================="
if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ 模糊测试完成 - 系统稳定"
elif [ $EXIT_CODE -eq 124 ]; then
    echo "⏱️  测试超时（可能仍在运行）"
else
    echo "❌ 测试失败 (退出码: $EXIT_CODE)"
    echo "可能检测到崩溃或异常"
fi
echo "=========================================="
echo ""
echo "日志文件: $LOG_FILE"
echo ""

# 显示最后的结果
if [ -f "$LOG_FILE" ]; then
    echo "测试结果摘要:"
    echo "----------------------------------------"
    tail -40 "$LOG_FILE" | grep -A 30 "模糊测试结果" || tail -30 "$LOG_FILE"
fi

