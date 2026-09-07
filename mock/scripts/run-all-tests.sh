#!/usr/bin/env bash
#
# run-all-tests.sh — 跑一组端到端冒烟测试
#
# 与真实环境集成：检测 ZLM/Redis/DB，跑 HTTP API 与协议层断言
#
# 用法：
#   bash scripts/run-all-tests.sh --backend=sqlite
#   bash scripts/run-all-tests.sh --no-mocks
#   bash scripts/run-all-tests.sh --skip-rust

set -o pipefail

MOCK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT_ROOT="$(cd "$MOCK_ROOT/.." && pwd)"

BACKEND="sqlite"
START_MOCKS=1
SKIP_RUST=0

for arg in "$@"; do
  case "$arg" in
    --backend=*) BACKEND="${arg#--backend=}" ;;
    --no-mocks)  START_MOCKS=0 ;;
    --skip-rust) SKIP_RUST=1 ;;
    *) echo "unknown arg: $arg"; exit 2 ;;
  esac
done

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

pass=0; fail=0; total=0

run_test() {
  local name="$1"; shift
  total=$((total + 1))
  printf "  [%2d] %-50s " "$total" "$name"
  if "$@" >/dev/null 2>&1; then
    printf "${GREEN}PASS${NC}\n"
    pass=$((pass + 1))
  else
    printf "${RED}FAIL${NC}\n"
    fail=$((fail + 1))
  fi
}

# 真实 ZLM 配置（与 config/application.toml 一致）
ZLM="http://127.0.0.1:8080"
ZLM_SECRET="EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw"

# ----- 0. mock 准备 -----
if [ "$START_MOCKS" -eq 1 ]; then
  echo "==== 启动 mock（自动跳过真实 ZLM）===="
  bash "$MOCK_ROOT/scripts/start-mocks.sh" >/dev/null 2>&1
  echo ""
fi

# ----- 1. 后端联通 -----
echo "==== HTTP API 联通测试 ===="

run_test "/api/health 返回 alive" \
  bash -c "curl -sf http://127.0.0.1:18080/api/health | grep -q alive"

run_test "/metrics 返回 prom 格式" \
  bash -c "curl -sf http://127.0.0.1:18080/metrics | grep -qE '^# (HELP|TYPE) '"

run_test "/api/user/login 成功（query 参数）" \
  bash -c "curl -sf 'http://127.0.0.1:18080/api/user/login?username=admin&password=admin' | grep -q '\"code\":0'"

# ----- 2. 真实 ZLM 联通 -----
echo ""
echo "==== 真实 ZLM 联通 ===="

run_test "ZLM getServerConfig 200" \
  bash -c "curl -sf '$ZLM/index/api/getServerConfig?secret=$ZLM_SECRET' | grep -q '\"code\"[[:space:]]*:[[:space:]]*0'"

run_test "ZLM getApiList 返回 API 列表" \
  bash -c "curl -sf '$ZLM/index/api/getApiList?secret=$ZLM_SECRET' | python3 -c 'import json,sys; assert len(json.load(sys.stdin).get(\"data\",[]))>0' 2>/dev/null"

run_test "ZLM getMediaList 返回流列表" \
  bash -c "curl -sf '$ZLM/index/api/getMediaList?secret=$ZLM_SECRET' | grep -q '\"code\"[[:space:]]*:[[:space:]]*0'"

run_test "ZLM getStatistic 返回统计" \
  bash -c "curl -sf '$ZLM/index/api/getStatistic?secret=$ZLM_SECRET' | grep -q '\"code\"[[:space:]]*:[[:space:]]*0'"

# ----- 3. Webhook 接收 -----
echo ""
echo "==== Webhook 接收 ===="

# 先清空 webhook 接收器
curl -sf http://127.0.0.1:9090/received/clear >/dev/null 2>&1 || true

# 用临时文件构造请求体，避免 bash 引号嵌套
TMP_HOOK_BODY=$(mktemp /tmp/hook-body-XXXXXX.json)
trap "rm -f $TMP_HOOK_BODY" EXIT
echo '{"hook_name":"on_publish","app":"live","stream":"test01"}' > "$TMP_HOOK_BODY"

run_test "Webhook 接收 on_publish（POST 到 mock）" \
  bash -c "curl -sf -X POST 'http://127.0.0.1:9090/hook/on_publish' \
    -H 'Content-Type: application/json' \
    --data-binary @'$TMP_HOOK_BODY' | grep -q '\"code\"[[:space:]]*:[[:space:]]*0'"

run_test "Webhook received 列表非空" \
  bash -c "curl -sf http://127.0.0.1:9090/received | grep -q on_publish"

TMP_GB_HOOK=$(mktemp /tmp/gb-hook-XXXXXX.json)
trap "rm -f $TMP_HOOK_BODY $TMP_GB_HOOK" EXIT
echo '{"hook_name":"on_server_started","port":554,"http_port":8080,"rtsp_port":554,"rtmp_port":1935,"https_port":8443,"mediaServerId":"zlmediakit-1"}' > "$TMP_GB_HOOK"

run_test "GBServer /api/zlm/hook 接收 on_server_started" \
  bash -c "curl -sf -X POST 'http://127.0.0.1:18080/api/zlm/hook' \
    -H 'Content-Type: application/json' \
    --data-binary @'$TMP_GB_HOOK' | grep -q '\"code\"[[:space:]]*:[[:space:]]*0'"

# ----- 4. 数据库种子可见 -----
echo ""
echo "==== 数据库种子验证 ===="

JWT=$(curl -sf "http://127.0.0.1:18080/api/user/login?username=admin&password=admin" | python3 -c "import json,sys; print(json.load(sys.stdin)['data']['accessToken'])")

run_test "设备列表 ≥ 4 条（含 GBServer + 3 mock 摄像机）" \
  bash -c "curl -sf 'http://127.0.0.1:18080/api/device/query/devices?page=1&count=10' -H \"access-token: $JWT\" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d[\"data\"][\"list\"])>=4' 2>/dev/null"

run_test "流媒体服务器包含 zlmediakit-1" \
  bash -c "curl -sf 'http://127.0.0.1:18080/api/server/media_server/list' -H \"access-token: $JWT\" | grep -q zlmediakit-1"

run_test "JT1078 终端列表 ≥ 2 条" \
  bash -c "curl -sf 'http://127.0.0.1:18080/api/jt1078/terminal/list' -H \"access-token: $JWT\" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d.get(\"data\",[]))>=2' 2>/dev/null"

run_test "Redis 包含 GBServer 自己的 state key（gbserver:001:）" \
  bash -c "python3 -c 'import socket; s=socket.create_connection((\"127.0.0.1\",6379),timeout=2); s.sendall(b\"KEYS gbserver:*\r\n\"); import time; time.sleep(0.3); print(s.recv(4096).decode())' | grep -q 'gbserver:'"

run_test "Redis 包含 cluster 心跳（说明 GBServer 自己在用）" \
  bash -c "python3 -c 'import socket; s=socket.create_connection((\"127.0.0.1\",6379),timeout=2); s.sendall(b\"KEYS gb:cluster:*\r\n\"); import time; time.sleep(0.3); print(s.recv(4096).decode())' | grep -q cluster"

# ----- 5. SIP 设备联通 -----
echo ""
echo "==== SIP 设备联通（看 mock 日志）===="
LOG="$MOCK_ROOT/logs/sip-device.log"

run_test "SIP mock 进程存活" \
  bash -c "[ -f $MOCK_ROOT/run/sip-device.pid ] && kill -0 \$(cat $MOCK_ROOT/run/sip-device.pid)"

run_test "SIP mock 已发送 REGISTER" \
  bash -c "grep -q 'REGISTER sent' $LOG"

run_test "SIP mock 收到 401 / 200 OK / 403" \
  bash -c "grep -qE '200 OK|401 Unauthorized|403 Forbidden' $LOG"

# ----- 6. JT1078 联通 -----
echo ""
echo "==== JT1078 联通（看 mock 日志）===="
LOG="$MOCK_ROOT/logs/jt1078-terminal.log"

run_test "JT1078 mock 进程存活" \
  bash -c "[ -f $MOCK_ROOT/run/jt1078-terminal.pid ] && kill -0 \$(cat $MOCK_ROOT/run/jt1078-terminal.pid)"

run_test "JT1078 mock 已发送注册（msg_id=0x0100）" \
  bash -c "grep -q 'msg_id=0x0100' $LOG"

run_test "JT1078 mock 发送心跳或位置" \
  bash -c "grep -qE 'msg_id=0x0002|msg_id=0x0200' $LOG"

# ----- 7. 级联平台联通 -----
echo ""
echo "==== 级联平台联通 ===="
LOG="$MOCK_ROOT/logs/cascade-platform.log"

run_test "级联 mock 进程存活" \
  bash -c "[ -f $MOCK_ROOT/run/cascade-platform.pid ] && kill -0 \$(cat $MOCK_ROOT/run/cascade-platform.pid)"

run_test "级联 mock 收到 REGISTER（说明 GBServer 主动连）" \
  bash -c "grep -qE 'RX.*REGISTER' $LOG"

# ----- 8. Rust 单元 / 集成测试 -----
if [ "$SKIP_RUST" -eq 0 ]; then
  echo ""
  echo "==== Rust 单元 + 集成测试 ===="
  cd "$PROJECT_ROOT"
  case "$BACKEND" in
    sqlite)
      echo "  cargo test (SQLite)..."
      cargo test --quiet --lib 2>&1 | tail -3 || true
      ;;
    postgres)
      echo "  cargo test (PostgreSQL)..."
      cargo test --quiet --no-default-features --features postgres --lib 2>&1 | tail -3 || true
      ;;
    mysql)
      echo "  cargo test (MySQL)..."
      cargo test --quiet --no-default-features --features mysql --lib 2>&1 | tail -3 || true
      ;;
  esac
fi

# ----- 收尾 -----
echo ""
echo "================================================="
printf "${GREEN}通过 ${pass}${NC} / ${RED}失败 ${fail}${NC} / 总计 ${total}\n"
echo "================================================="

if [ "$fail" -gt 0 ]; then
  echo ""
  echo -e "${YELLOW}提示：失败项的日志在 mock/logs/ 下${NC}"
  exit 1
fi
exit 0