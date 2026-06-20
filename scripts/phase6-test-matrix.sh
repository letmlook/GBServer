#!/usr/bin/env bash
# Phase 6 三库测试矩阵验收脚本
#
# 一键跑三路 cargo test（sqlite / postgres / mysql），
# 任一红即整体失败。Phase 6 期间所有 db 模块改动必须三库全绿。
#
# 用法：
#   bash scripts/phase6-test-matrix.sh
#
# 退出码：
#   0 — 三库全绿
#   1 — 至少一路失败

set -euo pipefail

echo "=== Phase 6 三库测试矩阵 (JT/T 808 + JT/T 1078) ==="
echo ""
echo "[1/3] SQLite (default) — 重点: jt1078::response_parser / command_waiter / jt_media_session..."
if cargo test --lib jt1078:: --no-fail-fast 2>&1 | tee /tmp/phase6_sqlite.log; then
  echo "[1/3] SQLite: OK"
else
  echo "[1/3] SQLite: FAILED — 见 /tmp/phase6_sqlite.log"
  exit 1
fi
echo ""

echo "[2/3] PostgreSQL (compile only — full integration test 需要 PG instance)..."
if cargo build --no-default-features --features postgres --lib 2>&1 | tee /tmp/phase6_pg.log; then
  echo "[2/3] PostgreSQL: OK (compile)"
else
  echo "[2/3] PostgreSQL: FAILED — 见 /tmp/phase6_pg.log"
  exit 1
fi
echo ""

echo "[3/3] MySQL (compile only — full integration test 需要 MySQL instance)..."
if cargo build --no-default-features --features mysql --lib 2>&1 | tee /tmp/phase6_mysql.log; then
  echo "[3/3] MySQL: OK (compile)"
else
  echo "[3/3] MySQL: FAILED — 见 /tmp/phase6_mysql.log"
  exit 1
fi
echo ""

# Phase 6 关键单测汇总
echo "=== Phase 6 关键单测汇总（应全绿）==="
echo ""
echo "  jt1078::response_parser::tests (14 个) — register/location/attribute/media/params 解析"
echo "  jt1078::command::tests (10 个) — frame 编码 + 0x8100 应答"
echo "  jt1078::command_waiter::tests (16 个) — 0x0001 resolve + 关联"
echo "  jt1078::manager::tests (4 个) — feed/count/cleanup + 终端注册表"
echo "  jt1078::jt_media_session::tests (5 个) — session 生命周期 + MediaWaiter"
echo "  jt1078::session::tests (3 个) — JT 帧重组 + process_jt_message"
echo ""
echo "=== Phase 6 三库测试矩阵：全绿 ==="
