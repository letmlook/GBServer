#!/usr/bin/env bash
#
# stop-mocks.sh — 停止一组模拟工具
#
# 用法：
#   bash scripts/stop-mocks.sh                # 停止所有
#   bash scripts/stop-mocks.sh zlm sip-device # 仅停止指定

set -uo pipefail

MOCK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUN_DIR="$MOCK_ROOT/run"

stop_one() {
  local key="$1"
  local pid_file="$RUN_DIR/$key.pid"
  if [ ! -f "$pid_file" ]; then
    echo "[$key] not running (no pid file)"
    return
  fi
  local pid
  pid=$(cat "$pid_file")
  if kill -0 "$pid" 2>/dev/null; then
    echo "[$key] stopping pid=$pid..."
    kill "$pid" || true
    # 等待退出
    for i in 1 2 3 4 5; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.5
    done
    kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
    echo "[$key] stopped"
  else
    echo "[$key] pid=$pid not alive"
  fi
  rm -f "$pid_file"
}

REQUESTED=("$@")
if [ ${#REQUESTED[@]} -eq 0 ]; then
  REQUESTED=(zlm sip-device jt1078-terminal webhook-receiver cascade-platform)
fi

for key in "${REQUESTED[@]}"; do
  stop_one "$key"
done

echo "==== 剩余 mock 进程（应为空）===="
ps -p $(cat $RUN_DIR/*.pid 2>/dev/null) -o pid,command 2>/dev/null || echo "（无）"