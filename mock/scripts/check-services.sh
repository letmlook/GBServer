#!/usr/bin/env bash
#
# check-services.sh — 检查真实服务 + mock 工具的健康
#
# 与 check-mocks.sh 的差别：check-services.sh 同时检查真实与 mock；
#                     check-mocks.sh 只检查 mock 进程（pid 文件）
#
# 用法：
#   bash scripts/check-services.sh

set -uo pipefail

MOCK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT_ROOT="$(cd "$MOCK_ROOT/.." && pwd)"
RUN_DIR="$MOCK_ROOT/run"

overall=0

# ===== 真实服务 =====
echo "==== 真实服务 ===="

if curl -sf --max-time 2 http://127.0.0.1:18080/api/health >/dev/null 2>&1; then
  echo "  [GBServer]  ✅ http://127.0.0.1:18080/api/health"
else
  echo "  [GBServer]  ❌ http://127.0.0.1:18080/api/health（未启动？）"
  overall=1
fi

# ZLM 探测（用真实 secret）
if curl -sf --max-time 2 "http://127.0.0.1:8080/index/api/getServerConfig?secret=EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw" >/dev/null 2>&1; then
  echo "  [ZLM 真实]   ✅ http://127.0.0.1:8080（使用真实 ZLM）"
else
  echo "  [ZLM 真实]   ❌ http://127.0.0.1:8080（未启动或 secret 不匹配）"
  # 看是否启动了 mock
  if [ -f "$RUN_DIR/zlm.pid" ] && kill -0 "$(cat "$RUN_DIR/zlm.pid")" 2>/dev/null; then
    echo "  [ZLM mock]   ✅ http://127.0.0.1:8080（mock 进程）"
  fi
fi

# Redis
if nc -z -w 1 127.0.0.1 6379 2>/dev/null; then
  echo "  [Redis]      ✅ 127.0.0.1:6379"
else
  echo "  [Redis]      ❌ 127.0.0.1:6379（未启动）"
fi

# PostgreSQL
if nc -z -w 1 127.0.0.1 5432 2>/dev/null; then
  echo "  [PostgreSQL] ✅ 127.0.0.1:5432"
else
  echo "  [PostgreSQL] ❌ 127.0.0.1:5432（未启动）"
fi

# MySQL
if nc -z -w 1 127.0.0.1 3306 2>/dev/null; then
  echo "  [MySQL]      ✅ 127.0.0.1:3306"
else
  echo "  [MySQL]      ❌ 127.0.0.1:3306（未启动）"
fi

# SQLite 文件
SQLITE_FILE="$PROJECT_ROOT/data/gbserver.db"
if [ -f "$SQLITE_FILE" ]; then
  size=$(du -h "$SQLITE_FILE" | cut -f1)
  echo "  [SQLite]     ✅ $SQLITE_FILE ($size)"
else
  echo "  [SQLite]     ⚠️  $SQLITE_FILE（不存在）"
fi

# ===== Mock 进程 =====
echo ""
echo "==== Mock 进程（mock/run/*.pid）===="
for pid_file in "$RUN_DIR"/*.pid; do
  [ -f "$pid_file" ] || continue
  key=$(basename "$pid_file" .pid)
  pid=$(cat "$pid_file")
  if kill -0 "$pid" 2>/dev/null; then
    case "$key" in
      sip-device)    target="UDP 15060" ;;
      jt1078-terminal) target="UDP 16000" ;;
      webhook-receiver) target="HTTP 9090" ;;
      cascade-platform) target="UDP 5062" ;;
      zlm)            target="HTTP 8080" ;;
      *)              target="?" ;;
    esac
    echo "  [$key] ✅ pid=$pid  $target"
  else
    echo "  [$key] ❌ pid=$pid not alive（僵尸进程）"
    overall=1
  fi
done

# ===== 总结 =====
echo ""
if [ "$overall" -eq 0 ]; then
  echo "==== ✅ 全部健康 ===="
  exit 0
else
  echo "==== ⚠️  有服务不可用 ===="
  exit 1
fi