#!/bin/bash
# =============================================================================
# 仅运行已构建的后端服务（不编译）
# 用法: 在 GBServer 根目录执行  ./scripts/run.sh
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log() { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $1"; }
ok()  { echo -e "${GREEN}[OK]${NC} $1"; }
warn(){ echo -e "${YELLOW}[WARN]${NC} $1"; }
fail(){ echo -e "${RED}[FAIL]${NC} $1"; exit 1; }

[ -f "Cargo.toml" ] || fail "未找到 Cargo.toml，请在 GBServer 根目录执行"

EXE="target/release/gbserver"
if [ ! -x "$EXE" ]; then
    fail "未找到可执行文件: $EXE
请先执行:  ./scripts/build.sh
      或:   docker compose up -d --build"
fi

# ── 前端静态文件检查 ───────────────────────────────────────────────
if [ ! -d "web/dist" ]; then
    warn "未找到 web/dist 目录，前端页面将无法访问"
    warn "如需前端，请先执行:  cd web && npm install && npm run build:prod"
fi

# ── 数据库 / Redis 检查 ────────────────────────────────────────────
if ! command -v docker &>/dev/null; then
    warn "未检测到 docker；请确保 PostgreSQL 与 Redis 已启动"
fi

# ── 显示关键信息 ───────────────────────────────────────────────────
HTTP_PORT="${GBSERVER__SERVER__PORT:-18080}"
log "=== 启动服务 ==="
echo -e "  可执行文件: ${YELLOW}${EXE}${NC}"
echo -e "  HTTP 地址 : ${YELLOW}http://0.0.0.0:${HTTP_PORT}${NC}"
echo -e "  健康检查  : ${YELLOW}http://127.0.0.1:${HTTP_PORT}/api/health${NC}"
echo -e "  提示: 需先启动 PostgreSQL/Redis (如 docker compose up -d) 并正确配置 ${YELLOW}config/application.yaml${NC}"
echo ""

exec "./$EXE"