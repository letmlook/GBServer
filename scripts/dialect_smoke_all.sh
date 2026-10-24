#!/usr/bin/env bash
# 三方言真实库冒烟：**一个二进制**起三个实例，只改 GBSERVER__DATABASE__URL。
#
# 这是本次改造的核心验收点 —— 改造前需要分别
# `cargo build --no-default-features --features postgres|mysql|sqlite` 三次，
# 改造后一个二进制（三个驱动同时编进）起三个实例即可。
set -uo pipefail
cd "$(dirname "$0")/.."

BIN=./target/debug/gbserver
LOG=/tmp/dialect_smoke_logs
mkdir -p "$LOG"

# 端口：HTTP / SIP(udp) / SIP(tcp) / JT1078 tcp / JT1078 udp
declare -a HTTP=(18081 18082 18083)
declare -a SIPU=(25060 25061 25062)
declare -a SIPT=(25070 25071 25072)
declare -a JTT=(60001 60002 60003)
declare -a JTU=(60011 60012 60013)

NAMES=(sqlite postgres mysql)
URLS=(
  "sqlite:data/smoke_sqlite.db"
  "postgres://postgres:postgrespw@127.0.0.1:5432/gbserver"
  "mysql://root:rootpw@127.0.0.1:3306/gbserver"
)

echo "==> 1/3 构建（default features = sqlite + mysql + postgres 三个驱动同时编进）"
cargo build 2>&1 | tail -3 || exit 1

pids=()
for i in 0 1 2; do
  echo "==> 启动 ${NAMES[$i]} 实例（:${HTTP[$i]}）"
  GBSERVER__DATABASE__URL="${URLS[$i]}" \
  GBSERVER__SERVER__PORT="${HTTP[$i]}" \
  GBSERVER__SIP__PORT="${SIPU[$i]}" GBSERVER__SIP__TCP_PORT="${SIPT[$i]}" \
  GBSERVER__JT1078__TCP_PORT="${JTT[$i]}" GBSERVER__JT1078__UDP_PORT="${JTU[$i]}" \
  NO_PROXY=localhost,127.0.0.1,::1 \
    "$BIN" >"$LOG/${NAMES[$i]}.log" 2>&1 &
  pids+=($!)
  sleep 4
done

rc=0
for i in 0 1 2; do
  n=${NAMES[$i]}
  base="http://127.0.0.1:${HTTP[$i]}/api"
  echo "==> 冒烟：$n ($base)"
  # 登录是 **POST + query 参数**（GET → 405；JSON body → 400「缺少 username」）
  TOKEN=$(curl -s -X POST \
    "$base/user/login?username=admin&password=21232f297a57a5a743894a0e4a801fc3" \
    | python3 -c 'import json,sys
try:
    print(json.load(sys.stdin)["data"]["accessToken"])
except Exception:
    print("")')
  if [ -z "$TOKEN" ]; then
    echo "  !! 登录失败（token 为空）—— 日志尾部："
    tail -20 "$LOG/$n.log"
    rc=1
    continue
  fi
  python3 scripts/dialect_smoke.py --base "$base" --token "$TOKEN" || rc=1
done

echo "==> 关闭实例"
for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done
sleep 1
for p in "${pids[@]}"; do kill -9 "$p" 2>/dev/null; done

echo "==> 日志在 $LOG"
exit $rc
