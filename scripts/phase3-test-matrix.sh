#!/usr/bin/env bash
# Phase 3 三库测试矩阵验收脚本
#
# 一键跑三路 cargo test（sqlite / postgres / mysql），
# 任一红即整体失败。Phase 3 期间所有 db 模块改动必须三库全绿。
#
# 用法：
#   bash scripts/phase3-test-matrix.sh
#
# 退出码：
#   0 — 三库全绿
#   1 — 至少一路失败

set -euo pipefail

# 默认先跑 sqlite（与项目 default feature 对齐）
echo "=== Phase 3 三库测试矩阵 ==="
echo ""
echo "[1/3] SQLite (default)..."
if cargo test --lib --no-fail-fast 2>&1 | tee /tmp/phase3_sqlite.log; then
  echo "[1/3] SQLite: OK"
else
  echo "[1/3] SQLite: FAILED — 见 /tmp/phase3_sqlite.log"
  exit 1
fi
echo ""

echo "[2/3] PostgreSQL..."
if cargo test --no-default-features --features postgres --lib --no-fail-fast 2>&1 | tee /tmp/phase3_pg.log; then
  echo "[2/3] PostgreSQL: OK"
else
  echo "[2/3] PostgreSQL: FAILED — 见 /tmp/phase3_pg.log"
  exit 1
fi
echo ""

echo "[3/3] MySQL..."
if cargo test --no-default-features --features mysql --lib --no-fail-fast 2>&1 | tee /tmp/phase3_mysql.log; then
  echo "[3/3] MySQL: OK"
else
  echo "[3/3] MySQL: FAILED — 见 /tmp/phase3_mysql.log"
  exit 1
fi
echo ""

echo "=== Phase 3 三库测试矩阵：全绿 ==="
