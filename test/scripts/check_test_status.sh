#!/bin/bash
# 快速检查测试状态

LOG_DIR=$(ls -td /tmp/comprehensive_stress_* 2>/dev/null | head -1)

if [ -z "$LOG_DIR" ]; then
    echo "❌ 未找到运行中的测试"
    exit 1
fi

LOG_FILE="$LOG_DIR/1_long_running.log"

if [ ! -f "$LOG_FILE" ]; then
    echo "❌ 日志文件不存在"
    exit 1
fi

echo "=========================================="
echo "测试状态检查"
echo "=========================================="
echo "日志目录: $LOG_DIR"
echo ""

# 检查测试进程
if pgrep -f "comprehensive_stress_test" > /dev/null; then
    echo "✅ 测试正在运行中"
    echo ""
    
    # 显示最新的状态报告
    echo "最新状态报告:"
    echo "----------------------------------------"
    tail -5 "$LOG_FILE" | grep -E "运行中|操作数|连接|创建|关闭|复用" | tail -1
    echo ""
    
    # 显示测试运行时间
    if [ -f "$LOG_FILE" ]; then
        FIRST_LINE=$(head -1 "$LOG_FILE" 2>/dev/null | grep -o "开始时间: [0-9-]* [0-9:]*" || echo "")
        if [ -n "$FIRST_LINE" ]; then
            echo "开始时间: $FIRST_LINE"
        fi
    fi
    
    # 显示进程信息
    echo ""
    echo "进程信息:"
    ps aux | grep -E "comprehensive_stress_test|test_long_running" | grep -v grep | head -2 | awk '{print "  PID:", $2, "CPU:", $3"%", "MEM:", $4"%"}'
    
else
    echo "❌ 测试未运行"
    echo ""
    echo "最后的状态:"
    echo "----------------------------------------"
    tail -20 "$LOG_FILE" | tail -10
fi

echo ""
echo "完整日志: $LOG_FILE"
echo "实时监控: ./test/monitor_stress_test.sh"

