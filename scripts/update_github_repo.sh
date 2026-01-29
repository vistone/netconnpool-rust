#!/bin/bash

# 更新 GitHub 仓库描述和标签的脚本
# 使用方法: ./update_github_repo.sh [GITHUB_TOKEN]

set -e

GITHUB_TOKEN=${1:-${GITHUB_TOKEN}}
REPO="vistone/netconnpool-rust"

if [ -z "$GITHUB_TOKEN" ]; then
    echo "错误: 需要 GitHub token"
    echo ""
    echo "使用方法:"
    echo "  1. 设置环境变量: export GITHUB_TOKEN=your_token"
    echo "  2. 运行脚本: ./update_github_repo.sh"
    echo ""
    echo "或者直接传递 token:"
    echo "  ./update_github_repo.sh your_token"
    echo ""
    echo "获取 token: https://github.com/settings/tokens"
    echo "需要权限: repo (更新仓库设置)"
    exit 1
fi

# 项目描述（移除控制字符，使用纯文本）
DESCRIPTION="A comprehensive, high-performance Rust network connection pool library with connection lifecycle management, health checks, and statistics monitoring. Supports TCP/UDP, IPv4/IPv6, fully thread-safe for high-concurrency scenarios."

# 标签列表
TOPICS=(
    "rust"
    "connection-pool"
    "network"
    "tcp"
    "udp"
    "ipv4"
    "ipv6"
    "async"
    "concurrent"
    "high-performance"
    "thread-safe"
    "network-programming"
    "pool"
    "connection-management"
    "health-check"
    "statistics"
    "client-server"
    "networking"
    "rust-library"
    "rust-crate"
)

echo "正在更新 GitHub 仓库描述和标签..."
echo "仓库: $REPO"
echo ""

# 更新仓库描述
echo "1. 更新仓库描述..."
# 使用 jq 构建完整的 JSON payload，确保正确编码和转义
DESC_PAYLOAD=$(jq -n --arg desc "$DESCRIPTION" '{description: $desc}')

DESC_RESPONSE=$(curl -s -w "\n%{http_code}" -X PATCH \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -H "Content-Type: application/json" \
  https://api.github.com/repos/$REPO \
  -d "$DESC_PAYLOAD")

DESC_HTTP_CODE=$(echo "$DESC_RESPONSE" | tail -n1)
DESC_BODY=$(echo "$DESC_RESPONSE" | sed '$d')

if [ "$DESC_HTTP_CODE" -eq 200 ]; then
    echo "✅ 仓库描述更新成功"
elif [ "$DESC_HTTP_CODE" -eq 403 ]; then
    echo "❌ 权限不足 (HTTP 403)"
    echo ""
    echo "可能的原因："
    echo "  1. Token 没有 'repo' 或 'public_repo' 权限"
    echo "  2. Token 已过期"
    echo "  3. 使用的是 Fine-grained token，需要添加 'Repository metadata' 权限"
    echo ""
    echo "解决方案："
    echo "  1. 访问 https://github.com/settings/tokens"
    echo "  2. 创建新的 token (classic) 并勾选 'repo' 权限"
    echo "  3. 或者使用 Fine-grained token 并添加 'Repository metadata: Read and write' 权限"
    echo ""
    echo "错误详情:"
    echo "$DESC_BODY" | jq .
    echo ""
    echo "💡 提示: 您也可以手动在 GitHub 网页上更新："
    echo "  https://github.com/$REPO/settings"
    exit 1
else
    echo "❌ 更新描述失败 (HTTP $DESC_HTTP_CODE)"
    echo "$DESC_BODY" | jq .
    exit 1
fi

# 更新标签
echo ""
echo "2. 更新仓库标签..."
TOPICS_JSON=$(printf '%s\n' "${TOPICS[@]}" | jq -R . | jq -s .)

TOPICS_RESPONSE=$(curl -s -w "\n%{http_code}" -X PUT \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.mercy-preview+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  https://api.github.com/repos/$REPO/topics \
  -d "{
    \"names\": $TOPICS_JSON
  }")

TOPICS_HTTP_CODE=$(echo "$TOPICS_RESPONSE" | tail -n1)
TOPICS_BODY=$(echo "$TOPICS_RESPONSE" | sed '$d')

if [ "$TOPICS_HTTP_CODE" -eq 200 ]; then
    echo "✅ 仓库标签更新成功"
    echo ""
    echo "已添加的标签:"
    echo "$TOPICS_BODY" | jq -r '.names[]' | sed 's/^/  - /'
elif [ "$TOPICS_HTTP_CODE" -eq 403 ]; then
    echo "❌ 权限不足 (HTTP 403)"
    echo ""
    echo "可能的原因："
    echo "  1. Token 没有 'repo' 或 'public_repo' 权限"
    echo "  2. Token 已过期"
    echo "  3. 使用的是 Fine-grained token，需要添加 'Repository metadata' 权限"
    echo ""
    echo "解决方案："
    echo "  1. 访问 https://github.com/settings/tokens"
    echo "  2. 创建新的 token (classic) 并勾选 'repo' 权限"
    echo "  3. 或者使用 Fine-grained token 并添加 'Repository metadata: Read and write' 权限"
    echo ""
    echo "错误详情:"
    echo "$TOPICS_BODY" | jq .
    echo ""
    echo "💡 提示: 您也可以手动在 GitHub 网页上更新："
    echo "  https://github.com/$REPO"
    exit 1
else
    echo "❌ 更新标签失败 (HTTP $TOPICS_HTTP_CODE)"
    echo "$TOPICS_BODY" | jq .
    exit 1
fi

echo ""
echo "✅ 所有更新完成！"
echo ""
echo "查看仓库: https://github.com/$REPO"
