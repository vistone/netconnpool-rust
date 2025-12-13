#!/bin/bash
# 全面性能测试脚本 - 记录速度、时间、IO吞吐量等关键指标

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║     NetConnPool Rust 全面性能测试套件                          ║"
echo "║     Comprehensive Performance Test Suite                       ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

# 设置测试超时时间（秒）
TIMEOUT=3600

# 记录开始时间
START_TIME=$(date +%s)

echo "=========================================="
echo "性能基准测试"
echo "=========================================="
echo ""

echo "1. 获取/归还操作吞吐量测试..."
timeout $TIMEOUT cargo test --test performance_test test_get_put_throughput -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_1.log || echo "测试超时或失败"

echo ""
echo "2. 并发吞吐量测试..."
timeout $TIMEOUT cargo test --test performance_test test_concurrent_throughput -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_2.log || echo "测试超时或失败"

echo ""
echo "3. IO吞吐量测试..."
timeout $TIMEOUT cargo test --test performance_test test_io_throughput -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_3.log || echo "测试超时或失败"

echo ""
echo "4. 延迟分布测试..."
timeout $TIMEOUT cargo test --test performance_test test_latency_distribution -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_4.log || echo "测试超时或失败"

echo ""
echo "5. 连接创建速度测试..."
timeout $TIMEOUT cargo test --test performance_test test_connection_creation_speed -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_5.log || echo "测试超时或失败"

echo ""
echo "6. 高负载IO吞吐量测试..."
timeout $TIMEOUT cargo test --test performance_test test_high_load_io_throughput -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_6.log || echo "测试超时或失败"

echo ""
echo "7. 统计信息收集性能测试..."
timeout $TIMEOUT cargo test --test performance_test test_stats_collection_performance -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_7.log || echo "测试超时或失败"

echo ""
echo "8. 综合性能测试..."
timeout $TIMEOUT cargo test --test performance_test test_comprehensive_performance -- --ignored --nocapture 2>&1 | tee /tmp/perf_test_8.log || echo "测试超时或失败"

echo ""
echo "=========================================="
echo "生成完整性能报告"
echo "=========================================="
echo ""

echo "9. 生成完整性能测试报告..."
timeout $TIMEOUT cargo test --test performance_report generate_performance_report -- --ignored --nocapture 2>&1 | tee /tmp/perf_report.log || echo "测试超时或失败"

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
echo "测试日志已保存到 /tmp/perf_test_*.log"
echo "完整报告已保存到 /tmp/perf_report.log"
echo ""
