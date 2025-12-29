#!/bin/bash
# 实时监控压力测试进度

LOG_DIR=$(ls -td /tmp/comprehensive_stress_* 2>/dev/null | head -1)

if [ -z "$LOG_DIR" ]; then
    echo "未找到测试日志目录"
    exit 1
fi

LOG_FILE="$LOG_DIR/1_long_running.log"

if [ ! -f "$LOG_FILE" ]; then
    echo "日志文件不存在: $LOG_FILE"
    exit 1
fi

echo "=========================================="
echo "实时监控压力测试进度"
echo "=========================================="
echo "日志文件: $LOG_FILE"
echo "按 Ctrl+C 退出监控"
echo ""

# 使用 tail -f 实时跟踪日志
tail -f "$LOG_FILE" | while IFS= read -r line; do
    # 高亮显示关键信息
    if echo "$line" | grep -q "运行中"; then
        echo -e "\033[1;32m$line\033[0m"
    elif echo "$line" | grep -q "警告"; then
        echo -e "\033[1;33m$line\033[0m"
    elif echo "$line" | grep -q "溢出\|错误\|失败"; then
        echo -e "\033[1;31m$line\033[0m"
    elif echo "$line" | grep -q "测试结果\|最终"; then
        echo -e "\033[1;36m$line\033[0m"
    else
        echo "$line"
    fi
done

