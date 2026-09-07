#!/usr/bin/env bash
#
# start-mocks.sh — 一键启动模拟工具（**自动跳过真实运行的服务**）
#
# 设计思路：
#   - ZLM、Redis、PostgreSQL、MySQL、SQLite 是真实环境 → 跳过 mock
#   - SIP 设备 / JT1078 终端 / 级联平台 通常没有真实硬件 → 必须 mock
#   - Webhook 接收器可选（用于验证回调）
#
# 用法：
#   bash scripts/start-mocks.sh                      # 自动探测 + 启动
#   bash scripts/start-mocks.sh --force-mock-zlm     # 强制启动 ZLM mock（覆盖真实 ZLM）
#   bash scripts/start-mocks.sh sip-device jt1078    # 仅启动指定
#

set -o pipefail

MOCK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT_ROOT="$(cd "$MOCK_ROOT/.." && pwd)"
RUN_DIR="$MOCK_ROOT/run"
LOG_DIR="$MOCK_ROOT/logs"
mkdir -p "$RUN_DIR" "$LOG_DIR"

FORCE_MOCK_ZLM=0
for arg in "$@"; do
  case "$arg" in
    --force-mock-zlm) FORCE_MOCK_ZLM=1 ;;
    --help|-h)
      echo "Usage: $0 [--force-mock-zlm] [service ...]"
      echo "Services: zlm sip-device jt1078-terminal webhook-receiver cascade-platform"
      exit 0
      ;;
  esac
done

# ----- 探测 -----

is_zlm_real() {
  nc -z -w 1 127.0.0.1 8080 2>/dev/null && \
    curl -sf --max-time 2 "http://127.0.0.1:8080/index/api/getServerConfig?secret=EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw" \
    | grep -q "code"
}

# ----- 服务定义 -----

# 格式：key|dir|python_file|args
declare -a SERVICES=(
  "zlm|zlm|zlm_mock.py|--port 8080 --secret demo --hook-url http://127.0.0.1:18080/api/zlm/hook"
  "sip-device|sip-device|sip_device_mock.py|--server 127.0.0.1:5060 --auto-register --auto-keepalive 30"
  "jt1078-terminal|jt1078-terminal|jt1078_terminal_mock.py|--server 127.0.0.1:60000 --phone-decimal 13912345678 --plate 京A12345 --simulate-loss 0.0"
  "webhook-receiver|webhook-receiver|webhook_receiver.py|--port 9090 --log $LOG_DIR/webhook.log"
  "cascade-platform|cascade-platform|cascade_mock.py|--port 5062 --server-id 34020000002000000099"
)

# ----- 启动 -----

start_one() {
  local key="$1" dir="$2" pyfile="$3" args="$4"
  local pid_file="$RUN_DIR/$key.pid"
  local log_file="$LOG_DIR/$key.log"

  if [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "[$key] already running pid=$(cat "$pid_file")"
    return
  fi

  echo "[$key] starting..."
  (
    cd "$MOCK_ROOT/tools/$dir"
    nohup python3 "$pyfile" $args >"$log_file" 2>&1 &
    echo $! > "$pid_file"
  )

  case "$key" in
    zlm)
      sleep 1
      if curl -sf "http://127.0.0.1:8080/index/api/getServerConfig?secret=demo" >/dev/null 2>&1; then
        echo "[$key] ✅ started pid=$(cat "$pid_file")"
      else
        echo "[$key] ⚠️  started but health check failed; see $log_file"
      fi
      ;;
    webhook-receiver)
      sleep 1
      if curl -sf "http://127.0.0.1:9090/healthz" >/dev/null 2>&1; then
        echo "[$key] ✅ started pid=$(cat "$pid_file")"
      else
        echo "[$key] ⚠️  started but health check failed; see $log_file"
      fi
      ;;
    sip-device|jt1078-terminal|cascade-platform)
      sleep 0.3
      if [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
        echo "[$key] ✅ started pid=$(cat "$pid_file") (UDP, no HTTP health)"
      else
        echo "[$key] ❌ failed to start; see $log_file"
      fi
      ;;
  esac
}

# ----- 入口 -----

REQUESTED=()
if [ $# -gt 0 ]; then
  for arg in "$@"; do
    case "$arg" in
      --*) ;;
      *) REQUESTED+=("$arg") ;;
    esac
  done
fi

if [ ${#REQUESTED[@]} -eq 0 ]; then
  REQUESTED=(zlm sip-device jt1078-terminal webhook-receiver cascade-platform)
fi

# ----- 自动跳过真实服务 -----

if [ "$FORCE_MOCK_ZLM" -eq 0 ] && is_zlm_real; then
  echo "==== 检测到真实 ZLM (127.0.0.1:8080) → 跳过 ZLM mock ===="
  echo "  如需强制使用 mock，请加 --force-mock-zlm"
  NEW_REQ=()
  if [ ${#REQUESTED[@]} -gt 0 ]; then
    for r in "${REQUESTED[@]}"; do
      [ "$r" != "zlm" ] && NEW_REQ+=("$r")
    done
  fi
  if [ ${#NEW_REQ[@]} -gt 0 ]; then
    REQUESTED=("${NEW_REQ[@]}")
  else
    REQUESTED=()
  fi
fi

echo ""

if [ ${#REQUESTED[@]} -eq 0 ]; then
  echo "  （无可启动项；ZLM 已跳过）"
  exit 0
fi

for entry in "${SERVICES[@]}"; do
  IFS='|' read -r key dir pyfile args <<<"$entry"
  matched=0
  if [ ${#REQUESTED[@]} -gt 0 ]; then
    for r in "${REQUESTED[@]}"; do
      if [ "$r" = "$key" ] || [ "$r" = "all" ]; then
        matched=1
        break
      fi
    done
  fi
  if [ "$matched" -eq 1 ]; then
    start_one "$key" "$dir" "$pyfile" "$args"
  fi
done

echo ""
echo "==== mock 进程清单 ===="
for pid_file in "$RUN_DIR"/*.pid; do
  [ -f "$pid_file" ] || continue
  key=$(basename "$pid_file" .pid)
  pid=$(cat "$pid_file")
  if kill -0 "$pid" 2>/dev/null; then
    cmdline=$(ps -p "$pid" -o command= 2>/dev/null | head -c 80)
    printf "  %-20s pid=%-6s %s\n" "$key" "$pid" "$cmdline"
  else
    echo "  $key ❌ pid=$pid DEAD"
  fi
done