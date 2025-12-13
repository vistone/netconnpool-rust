#!/bin/bash
# 运行压力测试脚本

echo "=========================================="
echo "NetConnPool Rust 压力测试套件"
echo "=========================================="
echo ""

# 设置测试超时时间（秒）
TIMEOUT=3600

# 运行并发压力测试
echo "1. 运行并发连接测试..."
timeout $TIMEOUT cargo test --test stress_test test_concurrent_connections -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "2. 运行长时间运行测试（60秒）..."
timeout $TIMEOUT cargo test --test stress_test test_long_running -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "3. 运行内存泄漏测试..."
timeout $TIMEOUT cargo test --test stress_test test_memory_leak -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "4. 运行连接池耗尽测试..."
timeout $TIMEOUT cargo test --test stress_test test_connection_pool_exhaustion -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "5. 运行快速获取释放测试..."
timeout $TIMEOUT cargo test --test stress_test test_rapid_acquire_release -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "6. 运行混合协议测试..."
timeout $TIMEOUT cargo test --test stress_test test_mixed_protocols -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "7. 运行连接生命周期测试..."
timeout $TIMEOUT cargo test --test stress_test test_connection_lifecycle -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "8. 运行高并发压力测试..."
timeout $TIMEOUT cargo test --test stress_test test_high_concurrency_stress -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "=========================================="
echo "性能基准测试"
echo "=========================================="
echo ""

echo "9. 运行获取/归还操作基准测试..."
timeout $TIMEOUT cargo test --test benchmark_test benchmark_get_put_operations -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "10. 运行并发获取/归还基准测试..."
timeout $TIMEOUT cargo test --test benchmark_test benchmark_concurrent_get_put -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "11. 运行连接创建基准测试..."
timeout $TIMEOUT cargo test --test benchmark_test benchmark_connection_creation -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "12. 运行统计信息收集基准测试..."
timeout $TIMEOUT cargo test --test benchmark_test benchmark_stats_collection -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "=========================================="
echo "集成测试"
echo "=========================================="
echo ""

echo "13. 运行完整生命周期测试..."
timeout $TIMEOUT cargo test --test integration_test test_full_lifecycle -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "14. 运行错误恢复测试..."
timeout $TIMEOUT cargo test --test integration_test test_error_recovery -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "15. 运行并发池操作测试..."
timeout $TIMEOUT cargo test --test integration_test test_concurrent_pool_operations -- --nocapture --ignored || echo "测试超时或失败"

echo ""
echo "=========================================="
echo "统计模块专项测试"
echo "=========================================="

echo "16. 运行统计模块并发更新测试..."
timeout $TIMEOUT cargo test --test stats_stress_test test_stats_concurrent_updates -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "17. 运行统计模块内存泄漏测试..."
timeout $TIMEOUT cargo test --test stats_stress_test test_stats_memory_leak -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "18. 运行统计模块竞争条件测试..."
timeout $TIMEOUT cargo test --test stats_stress_test test_stats_race_condition -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "19. 运行统计模块死循环防护测试..."
timeout $TIMEOUT cargo test --test stats_stress_test test_stats_infinite_loop_prevention -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "20. 运行统计模块锁竞争测试..."
timeout $TIMEOUT cargo test --test stats_stress_test test_stats_lock_contention -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "21. 运行统计模块详细竞争条件测试..."
timeout $TIMEOUT cargo test --test stats_race_test test_stats_race_condition_detailed -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "22. 运行统计模块读取一致性测试..."
timeout $TIMEOUT cargo test --test stats_race_test test_stats_get_stats_consistency -- --ignored --nocapture 2>&1 | head -100 || echo "测试超时或失败"

echo ""
echo "=========================================="
echo "所有压力测试完成"
echo "=========================================="
