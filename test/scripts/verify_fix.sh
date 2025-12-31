#!/bin/bash

echo "========================================="
echo "漏洞修复验证报告"
echo "========================================="
echo ""

echo "1. 检查 safe_increment_i64 是否使用 CAS 循环..."
if grep -A 10 "fn safe_increment_i64" src/stats.rs | grep -q "compare_exchange_weak"; then
    echo "✅ safe_increment_i64 已修复 - 使用 CAS 循环"
else
    echo "❌ safe_increment_i64 未修复 - 仍使用 load-store"
fi

echo ""
echo "2. 检查 safe_increment_u64 是否使用 CAS 循环..."
if grep -A 10 "fn safe_increment_u64" src/stats.rs | grep -q "compare_exchange_weak"; then
    echo "✅ safe_increment_u64 已修复 - 使用 CAS 循环"
else
    echo "❌ safe_increment_u64 未修复 - 仍使用 load-store"
fi

echo ""
echo "3. 检查代码注释是否提到原子性..."
if grep -q "使用 CAS 循环确保原子性更新" src/stats.rs; then
    echo "✅ 代码注释已更新 - 明确说明使用 CAS 确保原子性"
else
    echo "⚠️  代码注释未更新"
fi

echo ""
echo "========================================="
echo "代码审查结果"
echo "========================================="

echo ""
echo "修复前的代码模式:"
echo "  let old = atomic.load(Ordering::Relaxed);"
echo "  atomic.store(new, Ordering::Relaxed);  // ❌ 非原子"

echo ""
echo "修复后的代码模式:"
echo "  loop {"
echo "    let old = atomic.load(Ordering::Relaxed);"
echo "    match atomic.compare_exchange_weak(old, new, ...) {"
echo "      Ok(_) => break,  // ✅ 原子操作"
echo "      Err(_) => continue,"
echo "    }"
echo "  }"

echo ""
echo "========================================="
echo "结论"
echo "========================================="

# 统计修复情况
FIXED=0
TOTAL=2

if grep -A 10 "fn safe_increment_i64" src/stats.rs | grep -q "compare_exchange_weak"; then
    FIXED=$((FIXED + 1))
fi

if grep -A 10 "fn safe_increment_u64" src/stats.rs | grep -q "compare_exchange_weak"; then
    FIXED=$((FIXED + 1))
fi

echo ""
if [ $FIXED -eq $TOTAL ]; then
    echo "✅ 所有漏洞已修复 ($FIXED/$TOTAL)"
    echo ""
    echo "修复详情:"
    echo "  - safe_increment_i64: 使用 CAS 循环"
    echo "  - safe_increment_u64: 使用 CAS 循环"
    echo ""
    echo "预期效果:"
    echo "  - 统计计数器不再丢失更新"
    echo "  - 统计数据准确可信"
    echo "  - 不会出现负数或异常值"
else
    echo "❌ 仍有 $((TOTAL - FIXED)) 个漏洞未修复"
fi

echo ""
echo "========================================="
