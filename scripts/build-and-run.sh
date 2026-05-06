#!/bin/bash
set -e

# ── 一键编译 & 运行 GBServer（前后端）──────────────────────────────
# 用法:
#   ./scripts/build-and-run.sh          # 开发模式：前后端同时启动（前端 dev server + cargo run）
#   ./scripts/build-and-run.sh --prod   # 生产模式：编译前端 → 编译后端 → 启动后端（serve 静态文件）
#   ./scripts/build-and-run.sh --build-only  # 仅编译，不运行

MODE="${1:-dev}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $1"; }
ok()   { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail() { echo -e "${RED}[FAIL]${NC} $1"; exit 1; }

# ── 前置检查 ───────────────────────────────────────────────────────
command -v cargo  &>/dev/null || fail "未找到 cargo，请先安装 Rust: https://rustup.rs"
command -v node   &>/dev/null || fail "未找到 node，请先安装 Node.js"
command -v npm    &>/dev/null || fail "未找到 npm"

cd "$PROJECT_DIR"

# ── 编译前端 ───────────────────────────────────────────────────────
build_frontend() {
    log "编译前端..."
    cd "$PROJECT_DIR/web"
    if [ ! -d "node_modules" ]; then
        log "安装前端依赖..."
        npm install
    fi
    npm run build:prod
    ok "前端编译完成 → web/dist"
    cd "$PROJECT_DIR"
}

# ── 编译后端 ───────────────────────────────────────────────────────
build_backend() {
    log "编译后端（release）..."
    cargo build --release
    ok "后端编译完成 → target/release/GBServer"
}

# ── 启动后端（生产模式，serve 前端静态文件）─────────────────────────
run_prod() {
    log "启动后端（生产模式，端口 18080）..."
    cargo run --release
}

# ── 开发模式（前端 dev server + 后端同时启动）───────────────────────
run_dev() {
    log "开发模式启动..."
    log "前端 dev server → http://localhost:9528"
    log "后端 API       → http://localhost:18080"
    echo ""

    # 后台启动后端
    cargo run &
    BACKEND_PID=$!

    # 前台启动前端 dev server
    cd "$PROJECT_DIR/web"
    if [ ! -d "node_modules" ]; then
        log "安装前端依赖..."
        npm install
    fi
    npm run dev &
    FRONTEND_PID=$!

    # 捕获退出信号，清理子进程
    cleanup() {
        echo ""
        log "正在停止服务..."
        kill $BACKEND_PID 2>/dev/null || true
        kill $FRONTEND_PID 2>/dev/null || true
        wait $BACKEND_PID 2>/dev/null || true
        wait $FRONTEND_PID 2>/dev/null || true
        ok "服务已停止"
    }
    trap cleanup EXIT INT TERM

    wait
}

# ── 主流程 ─────────────────────────────────────────────────────────
case "$MODE" in
    --prod)
        build_frontend
        build_backend
        run_prod
        ;;
    --build-only)
        build_frontend
        build_backend
        ok "编译完成，未启动服务"
        ;;
    dev|--dev)
        run_dev
        ;;
    *)
        echo "用法: $0 [--dev|--prod|--build-only]"
        echo "  --dev        开发模式（默认）：前端 dev server + 后端同时启动"
        echo "  --prod       生产模式：编译前后端 → 启动后端 serve 静态文件"
        echo "  --build-only 仅编译前后端，不启动"
        exit 1
        ;;
esac
