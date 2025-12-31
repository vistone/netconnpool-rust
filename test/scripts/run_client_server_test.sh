#!/bin/bash
# 运行客户端-服务器端到端测试

set -euo pipefail

echo "=========================================="
echo "客户端-服务器端到端压力测试"
echo "=========================================="
echo ""

# 测试持续时间（秒）- 默认1小时，可以通过参数修改
DURATION=${1:-3600}

echo "测试配置:"
echo "  持续时间: ${DURATION}秒 ($(($DURATION / 60))分钟)"
echo "  客户端线程: 100 (TCP: 50, UDP: 30, 混合: 20)"
echo "  服务器: TCP + UDP 回显服务器"
echo ""

read -p "是否继续？(y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "测试已取消"
    exit 0
fi

TS=$(date +%Y%m%d_%H%M%S)
LOG_FILE="/tmp/client_server_test_${TS}.log"

echo ""
echo "开始测试..."
echo "日志文件: $LOG_FILE"
echo "按 Ctrl+C 可以提前终止测试"
echo ""

# 运行测试（使用timeout防止无限运行）
timeout $((DURATION + 60)) cargo test --test comprehensive_client_test test_comprehensive_client_stress -- \
    --ignored --nocapture 2>&1 | tee "$LOG_FILE"

EXIT_CODE=${PIPESTATUS[0]}

echo ""
echo "=========================================="
if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ 测试完成"
elif [ $EXIT_CODE -eq 124 ]; then
    echo "⏱️  测试超时（可能仍在运行）"
else
    echo "❌ 测试失败 (退出码: $EXIT_CODE)"
fi
echo "=========================================="
echo ""
echo "日志文件: $LOG_FILE"
echo ""

# 显示最后的结果
if [ -f "$LOG_FILE" ]; then
    echo "最后的结果:"
    echo "----------------------------------------"
    tail -30 "$LOG_FILE" | grep -A 20 "全面客户端压力测试结果" || tail -20 "$LOG_FILE"
fi

