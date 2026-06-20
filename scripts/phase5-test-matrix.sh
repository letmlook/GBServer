#!/usr/bin/env bash
# Phase 5 三库测试矩阵验收脚本
#
# 一键跑三路 cargo test（sqlite / postgres / mysql），
# 任一红即整体失败。Phase 5 期间所有 db 模块改动必须三库全绿。
#
# 用法：
#   bash scripts/phase5-test-matrix.sh
#
# 退出码：
#   0 — 三库全绿
#   1 — 至少一路失败

set -euo pipefail

echo "=== Phase 5 三库测试矩阵 ==="
echo ""
echo "[1/3] SQLite (default) — 重点: cascade::register / cascade_forward / sip::server::upstream_message_tests (phase5_*)..."
if cargo test --lib --no-fail-fast 2>&1 | tee /tmp/phase5_sqlite.log; then
  echo "[1/3] SQLite: OK"
else
  echo "[1/3] SQLite: FAILED — 见 /tmp/phase5_sqlite.log"
  exit 1
fi
echo ""

echo "[2/3] PostgreSQL..."
if cargo test --no-default-features --features postgres --lib --no-fail-fast 2>&1 | tee /tmp/phase5_pg.log; then
  echo "[2/3] PostgreSQL: OK"
else
  echo "[2/3] PostgreSQL: FAILED — 见 /tmp/phase5_pg.log"
  exit 1
fi
echo ""

echo "[3/3] MySQL..."
if cargo test --no-default-features --features mysql --lib --no-fail-fast 2>&1 | tee /tmp/phase5_mysql.log; then
  echo "[3/3] MySQL: OK"
else
  echo "[3/3] MySQL: FAILED — 见 /tmp/phase5_mysql.log"
  exit 1
fi
echo ""

# Phase 5 关键单测汇总
echo "=== Phase 5 关键单测汇总（应全绿）==="
echo ""
echo "  cascade::register::c3_tests::phase5_build_digest_response_*  (3 个)"
echo "  sip::gb28181::cascade_forward::tests::phase5_close_by_stream_*  (4 个)"
echo "  sip::server::upstream_message_tests::phase5_parse_cascade_invite_sdp_*  (6 个)"
echo "  sip::server::upstream_message_tests::phase5_register_cascade_invite_*  (2 个)"
echo "  sip::server::upstream_message_tests::phase5_forward_mobile_position_*  (2 个)"
echo "  sip::server::upstream_message_tests::phase5_forward_alarm_*  (2 个)"
echo ""
echo "=== Phase 5 三库测试矩阵：全绿 ==="
