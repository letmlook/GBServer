#!/usr/bin/env bash
#
# seed-database.sh — 把测试种子灌入运行中的数据库
#
# 自动检测 GBServer 实际使用的数据库后端（SQLite / PostgreSQL / MySQL）
# 并调用对应工具加载 mock/seed/{sqlite,postgres,mysql}/seed-test-data.sql
#
# 用法：
#   bash scripts/seed-database.sh            # 自动检测
#   bash scripts/seed-database.sh sqlite    # 强制 SQLite
#   bash scripts/seed-database.sh postgres  # 强制 PostgreSQL（需要 DATABASE_URL 环境变量）

set -uo pipefail

MOCK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT_ROOT="$(cd "$MOCK_ROOT/.." && pwd)"

SEED_DIR="$MOCK_ROOT/seed"

detect_backend() {
  # 优先：环境变量覆盖
  if [ -n "${BACKEND:-}" ]; then
    echo "$BACKEND"
    return
  fi
  # 其次：GBServer 默认 SQLite 文件
  if [ -f "$PROJECT_ROOT/data/gbserver.db" ]; then
    echo "sqlite"
    return
  fi
  # 再次：探测 TCP 端口
  if nc -z -w 1 127.0.0.1 5432 2>/dev/null; then
    echo "postgres"; return
  fi
  if nc -z -w 1 127.0.0.1 3306 2>/dev/null; then
    echo "mysql"; return
  fi
  echo "sqlite"  # fallback
}

BACKEND="${1:-$(detect_backend)}"
echo "==== 后端: $BACKEND ===="

run_sqlite() {
  local file="$SEED_DIR/sqlite/seed-test-data.sql"
  local db="$PROJECT_ROOT/data/gbserver.db"
  if [ ! -f "$db" ]; then
    echo "  ❌ SQLite 数据库文件不存在: $db"
    echo "  请先 cargo run 一次，让 GBServer 自动建表"
    return 1
  fi
  if [ ! -f "$file" ]; then
    echo "  ❌ 种子文件不存在: $file"
    return 1
  fi
  echo "  → 灌入 $file 到 $db"
  if command -v sqlite3 >/dev/null; then
    sqlite3 "$db" < "$file"
  else
    echo "  ⚠️  sqlite3 命令未安装；请安装后重试（brew install sqlite3 / apt install sqlite3）"
    return 1
  fi
}

run_postgres() {
  local file="$SEED_DIR/postgres/seed-test-data.sql"
  if [ -z "${DATABASE_URL:-}" ]; then
    echo "  ❌ 未设置 DATABASE_URL 环境变量"
    echo "  例子: export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/gbserver"
    return 1
  fi
  echo "  → 灌入 $file 到 $DATABASE_URL"
  if command -v psql >/dev/null; then
    psql "$DATABASE_URL" -f "$file"
  else
    echo "  ⚠️  psql 命令未安装；请安装后重试"
    return 1
  fi
}

run_mysql() {
  local file="$SEED_DIR/mysql/seed-test-data.sql"
  if [ -z "${DATABASE_URL:-}" ]; then
    echo "  ❌ 未设置 DATABASE_URL 环境变量"
    echo "  例子: export DATABASE_URL=mysql://root:Fitow2022@127.0.0.1:3306/gbserver"
    return 1
  fi
  echo "  → 灌入 $file 到 $DATABASE_URL"
  # mysql CLI 用 -h/-u/-p 风格；从 URL 提取
  local url="$DATABASE_URL"
  url="${url#mysql://}"
  local userpass="${url%@*}"
  local hostdb="${url#*@}"
  local user="${userpass%:*}"
  local pass="${userpass#*:}"
  local host="${hostdb%:*}"
  local db="${hostdb#*/}"
  if command -v mysql >/dev/null; then
    mysql -h "$host" -u "$user" -p"$pass" "$db" < "$file"
  else
    echo "  ⚠️  mysql CLI 未安装"
    return 1
  fi
}

case "$BACKEND" in
  sqlite)   run_sqlite ;;
  postgres) run_postgres ;;
  mysql)    run_mysql ;;
  *) echo "未知后端: $BACKEND"; exit 1 ;;
esac