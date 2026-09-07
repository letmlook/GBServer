#!/usr/bin/env bash
#
# discover-services.sh — 探测本机 / docker 中运行的真实服务
#
# 用法：
#   bash scripts/discover-services.sh            # 自动探测
#   bash scripts/discover-services.sh --quiet    # 仅输出关键结果
#
# 输出：哪些服务真实运行；哪些必须 mock；给 mock/ 工具提供默认目标

set -uo pipefail

QUIET=0
for arg in "$@"; do
  case "$arg" in
    --quiet|-q) QUIET=1 ;;
  esac
done

log() { [ "$QUIET" -eq 1 ] || echo "$@"; }

# ----- 探测函数 -----

probe_tcp() {
  local name="$1" host="$2" port="$3"
  if nc -z -w 1 "$host" "$port" 2>/dev/null; then
    log "  ✅ $name  TCP  $host:$port"
    return 0
  fi
  log "  ❌ $name  TCP  $host:$port  未监听"
  return 1
}

probe_udp() {
  local name="$1" host="$2" port="$3"
  if nc -uz -w 1 "$host" "$port" 2>/dev/null; then
    log "  ✅ $name  UDP  $host:$port"
    return 0
  fi
  log "  ❌ $name  UDP  $host:$port  未监听"
  return 1
}

probe_http() {
  local name="$1" url="$2" expected_key="$3"
  local body
  body=$(curl -sf --max-time 3 "$url" 2>/dev/null) || {
    log "  ❌ $name  HTTP  $url"
    return 1
  }
  if [ -n "$expected_key" ] && ! echo "$body" | grep -q "$expected_key"; then
    log "  ⚠️  $name  HTTP  $url  (响应无期望字段 $expected_key)"
    return 1
  fi
  log "  ✅ $name  HTTP  $url"
  echo "$body"
  return 0
}

# ----- 主流程 -----

log ""
echo "==== 本机真实服务探测 ===="
log ""

GB_SERVER_OK=0
ZLM_OK=0
REDIS_OK=0
PG_OK=0
MYSQL_OK=0
SQLITE_OK=0

# 1. GBServer 后端
if probe_http "GBServer" "http://127.0.0.1:18080/api/health" "alive"; then
  GB_SERVER_OK=1
fi

# 2. ZLMediaKit（探测时需要 secret；先尝试空 secret 再尝试 config 默认值）
ZLM_URL="http://127.0.0.1:8080/index/api/getApiList?secret="
ZLM_SECRET="${ZLM_SECRET:-EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw}"
if probe_http "ZLM (no secret)" "$ZLM_URL" "code" >/dev/null; then
  log "  ⚠️  ZLM 未启用 secret 鉴权（开放模式）"
  ZLM_OK=1
elif probe_http "ZLM (default secret)" "$ZLM_URL$ZLM_SECRET" "code" >/dev/null; then
  ZLM_OK=1
else
  log "  ❌ ZLM 8080 未启用或 secret 不匹配"
fi

# 3. Redis
if probe_tcp "Redis" "127.0.0.1" "6379"; then
  REDIS_OK=1
fi

# 4. PostgreSQL
if probe_tcp "PostgreSQL" "127.0.0.1" "5432"; then
  PG_OK=1
fi

# 5. MySQL
if probe_tcp "MySQL" "127.0.0.1" "3306"; then
  MYSQL_OK=1
fi

# 6. SQLite（检查 GBServer 默认文件位置）
SQLITE_FILE="${SQLITE_FILE:-$HOME/code/GBServer/data/gbserver.db}"
[ -z "${GBServer_ROOT:-}" ] && GBServer_ROOT="$(cd "$(dirname "$0")/../.." 2>/dev/null && pwd)"
SQLITE_FILE="$GBServer_ROOT/data/gbserver.db"
if [ -f "$SQLITE_FILE" ]; then
  log "  ✅ SQLite   $SQLITE_FILE"
  SQLITE_OK=1
fi

# ----- 总结 -----

log ""
echo "==== 探测结果 ===="
cat <<EOF | tee /dev/null
  GBServer HTTP : $([ "$GB_SERVER_OK" -eq 1 ] && echo "运行中" || echo "未运行")
  ZLM HTTP      : $([ "$ZLM_OK" -eq 1 ] && echo "运行中" || echo "未运行")
  Redis TCP     : $([ "$REDIS_OK" -eq 1 ] && echo "运行中" || echo "未运行")
  PostgreSQL    : $([ "$PG_OK" -eq 1 ] && echo "运行中" || echo "未运行")
  MySQL         : $([ "$MYSQL_OK" -eq 1 ] && echo "运行中" || echo "未运行")
  SQLite 文件   : $([ "$SQLITE_OK" -eq 1 ] && echo "存在" || echo "无")
EOF

log ""
echo "==== mock 建议 ===="
if [ "$ZLM_OK" -eq 1 ]; then
  echo "  ✅ ZLM 真实可用 → 不要启动 mock/tools/zlm/zlm_mock.py"
  echo "     如需触发 Webhook，直接对真实 ZLM 调用 GET /trigger/（mock 才有此端点）"
  echo "     或在 GBServer 后端手动触发流上下线"
else
  echo "  ⚠️  ZLM 不可用 → 必须启动 mock/tools/zlm/zlm_mock.py"
fi
echo ""
echo "  ⚠️  SIP / JT1078 / 级联平台 通常都没有真实硬件 → 必须启动对应的 mock"
echo "  ⚠️  数据库若无 → GBServer 默认 SQLite 自带；PG/MySQL 用 docker compose up -d"

log ""
echo "==== GBServer 数据库后端识别（建议）===="
if [ "$GB_SERVER_OK" -eq 1 ]; then
  echo "  检查 GBServer 实际连接的后端："
  echo "    curl -s http://127.0.0.1:18080/api/system/version | jq"
  echo "  或读配置："
  echo "    grep -E '^url|sqlite_max' $GBServer_ROOT/config/application.toml"
fi