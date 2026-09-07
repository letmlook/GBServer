#!/usr/bin/env bash
#
# check-mocks.sh — 检查各 mock 服务的健康状态
#
# 用法：
#   bash scripts/check-mocks.sh                # 全部检查
#   bash scripts/check-mocks.sh zlm sip-device # 指定服务
#
# 退出码：0=全部 OK，1=有失败

set -uo pipefail

MOCK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUN_DIR="$MOCK_ROOT/run"

check_http() {
  local name="$1" url="$2"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" "$url" --max-time 3 || echo "000")
  if [ "$code" = "200" ]; then
    echo "  [$name] ✅ HTTP $code  $url"
    return 0
  else
    echo "  [$name] ❌ HTTP $code  $url"
    return 1
  fi
}

check_udp() {
  local name="$1" host="$2" port="$3"
  # 简单检查端口是否在监听（UDP 用 lsof）
  if command -v lsof >/dev/null; then
    if lsof -iUDP:"$port" -sUDP:LISTEN >/dev/null 2>&1; then
      echo "  [$name] ✅ UDP listening on $host:$port"
      return 0
    fi
  fi
  # fallback：用 nc 或 ss
  if command -v nc >/dev/null; then
    if nc -uz -w 1 "$host" "$port" 2>/dev/null; then
      echo "  [$name] ✅ UDP port open $host:$port"
      return 0
    fi
  fi
  echo "  [$name] ⚠️  UDP port check inconclusive: $host:$port"
  return 0
}

overall=0
echo "==== Mock 服务健康检查 ===="

# 如果没有传参，默认检查全部
if [ $# -eq 0 ]; then
  set -- zlm sip-device jt1078-terminal webhook-receiver cascade-platform
fi

for key in "$@"; do
  pid_file="$RUN_DIR/$key.pid"
  pid="(no pid)"
  if [ -f "$pid_file" ]; then
    pid=$(cat "$pid_file")
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "[$key] ❌ pid=$pid not alive"
      overall=1
      continue
    fi
  fi

  case "$key" in
    zlm)
      check_http "zlm" "http://127.0.0.1:8080/index/api/getServerConfig?secret=demo" || overall=1
      ;;
    sip-device)
      check_udp "sip-device" "127.0.0.1" "15060" || overall=1
      ;;
    jt1078-terminal)
      check_udp "jt1078-terminal" "127.0.0.1" "16000" || overall=1
      ;;
    webhook-receiver)
      check_http "webhook-receiver" "http://127.0.0.1:9090/healthz" || overall=1
      ;;
    cascade-platform)
      check_udp "cascade-platform" "127.0.0.1" "5062" || overall=1
      ;;
    *)
      echo "[$key] ❓ unknown service"
      overall=1
      ;;
  esac
done

echo ""
echo "==== GBServer 后端健康（独立）===="
curl -s -o /dev/null -w "  /api/health → HTTP %{http_code}\n" http://127.0.0.1:18080/api/health || echo "  /api/health → ⚠️  unreachable（后端可能未启动）"

exit $overall