#!/bin/bash
# =============================================================================
# 仅编译：前端 + 后端（不运行）
# 用法: 在 GBServer 根目录执行  ./scripts/build.sh
#       ./scripts/build.sh --skip-frontend   # 仅编译后端
#       ./scripts/build.sh --skip-backend    # 仅编译前端
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $1"; }
ok()   { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail() { echo -e "${RED}[FAIL]${NC} $1"; exit 1; }

SKIP_FRONTEND=0
SKIP_BACKEND=0
for arg in "$@"; do
    case "$arg" in
        --skip-frontend) SKIP_FRONTEND=1 ;;
        --skip-backend)  SKIP_BACKEND=1  ;;
        -h|--help)
            echo "用法: $0 [--skip-frontend] [--skip-backend]"
            exit 0 ;;
        *) fail "未知参数: $arg" ;;
    esac
done

# ── 前置检查 ───────────────────────────────────────────────────────
[ -f "Cargo.toml" ]  || fail "未找到 Cargo.toml，请在 GBServer 根目录执行"
[ -d "web" ]          || fail "未找到 web/ 目录"

if [ "$SKIP_FRONTEND" -eq 0 ]; then
    command -v node &>/dev/null || fail "未找到 node，请先安装 Node.js (推荐 18.x)"
    command -v npm  &>/dev/null || fail "未找到 npm"
fi
if [ "$SKIP_BACKEND" -eq 0 ]; then
    command -v cargo &>/dev/null || fail "未找到 cargo，请先安装 Rust: https://rustup.rs"
fi

# ── 1. 编译前端 ────────────────────────────────────────────────────
if [ "$SKIP_FRONTEND" -eq 0 ]; then
    log "=== 1/2 编译前端 (web/dist) ==="
    pushd web > /dev/null
    if [ ! -d node_modules ]; then
        log "安装前端依赖 (npm install)..."
        npm install --no-audit --no-fund
    fi
    npm run build:prod
    ok "前端编译完成 → web/dist"
    popd > /dev/null
fi

# ── 2. 编译后端 ────────────────────────────────────────────────────
if [ "$SKIP_BACKEND" -eq 0 ]; then
    log "=== 2/2 编译后端 (target/release/gbserver) ==="
    cargo build --release
    ok "后端编译完成 → target/release/gbserver"
fi

echo ""
ok "全部编译完成"
[ "$SKIP_FRONTEND" -eq 0 ] && echo -e "  前端产物: ${YELLOW}web/dist/${NC}"
[ "$SKIP_BACKEND"  -eq 0 ] && echo -e "  后端可执行: ${YELLOW}target/release/gbserver${NC}"