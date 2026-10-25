# GBServer 开发者快捷命令（just / cargo-make 兼容）
#
# 安装 just: cargo install just
# 或直接用 cargo run / cargo test 替代。
#
# 用法：
#   just                  # 默认任务：fmt + clippy + test
#   just dev              # SQLite 默认开发模式
#   just pg               # PostgreSQL 模式（改 database.url，不改编译）
#   just mysql            # MySQL 模式（同上）
#   just test             # 全量测试
#   just dialect-check    # 三驱动同编 + 方言占位符守门人
#
# ⚠️ 2026-10-25 起：**三个数据库驱动同时编进同一个二进制**
# （`Cargo.toml` 的 `default = ["sqlite", "mysql", "postgres"]`），
# 运行期按 `config/application.toml` 里 `[database] url` 的 scheme 选库。
# 因此**不再有**「按方言分别编译」这件事：
#   - `--no-default-features --features postgres` 之类的命令**会直接编译失败**
#     （`src/db/mod.rs` 有 `compile_error!` 守卫：三个驱动必须同时启用）；
#   - 换库 = 改一行 `database.url` + **重启进程**（方言在启动时确定）。

# 默认任务
default: fmt clippy test

# === 构建 ===

# SQLite 默认模式（开发 / 演示 / 边缘）
dev:
    cargo run

# PostgreSQL 模式（生产主力）—— 改 url，不改编译
# 可用环境变量覆盖：PG_URL=postgres://user:pass@host:5432/gbserver just pg
pg:
    GBSERVER__DATABASE__URL="$${PG_URL:-postgres://postgres:postgres@127.0.0.1:5432/gbserver}" cargo run

# MySQL 模式（MySQL 平迁 / 兼容历史部署）—— 改 url，不改编译
mysql:
    GBSERVER__DATABASE__URL="$${MYSQL_URL:-mysql://root:root@127.0.0.1:3306/gbserver}" cargo run

# Release 构建（三个驱动都在这个二进制里）
build:
    cargo build --release

# === 测试 ===

test:
    cargo test --no-fail-fast

# 仅 SQLite 兼容层的专项用例
test-sqlite:
    cargo test --test sqlite_compat -- --nocapture

# 真实库专项用例（需要本地跑着 PostgreSQL / MySQL 容器）
# 用法：TEST_POSTGRES_URL=... TEST_MYSQL_URL=... just test-realdb
test-realdb:
    cargo test --locked \
      --test postgres_jt1078 \
      --test postgres_device_lookup \
      --test postgres_platform_channel \
      --test mysql_catalog_channel \
      --test mysql_platform_channel \
      -- --ignored

# === 方言验证 ===

# 三驱动同编 + 方言占位符守门人
#
# 改造前这里叫 feature-check，跑的是「三种 feature 各自编译一遍」——
# 那已经是**编译期互斥**时代的做法，现在三个驱动必须同开，分开编会失败。
dialect-check:
    @echo "==> check（三驱动同编：sqlite + mysql + postgres）"
    cargo check --all-targets
    @echo "==> 方言占位符守门人（裸 ? 在 postgres 上必然 syntax error）"
    cargo test --lib dialect_guard
    @echo "✅ 三驱动同编 + 守门人通过"

# === 代码质量 ===

fmt:
    cargo fmt --all -- --check

# Clippy（与 CI 的 hygiene job 一致）
#
# `--all-features` 现在是**正确**的：三个驱动本来就必须同时启用。
# （改造前这里写着「不要用 --all-features，三选一互斥，同时开启产生 162 个编译错误」
#  —— 那条注释已作废。）
clippy:
    cargo clippy --all-targets --all-features

# 严格模式：warning 视为错误。
# 当前基线尚有 353 个 warning（多为风格类），清理后再接入 CI 门禁。
clippy-strict:
    cargo clippy --all-targets --all-features -- -D warnings

# === Docker ===

docker-sqlite:
    docker compose -f docker-compose.sqlite.yml up -d

docker-pg:
    docker compose up -d

docker-down:
    docker compose down

# === 数据库初始化 ===
#
# 三个后端**首次启动都会自动建表**（按运行期方言重放对应初始化脚本的
# CREATE TABLE / CREATE INDEX，不重放 drop table 与种子 INSERT）。
# 下面的手动初始化仅在需要预置种子数据时使用。

init-sqlite:
    @echo "SQLite 数据库自动初始化；表结构见 database/init-sqlite-2.7.4.sql"

init-pg:
    docker exec -i gbserver-postgres psql -U postgres -d gbserver < database/init-postgresql-2.7.4.sql

init-mysql:
    docker exec -i gbserver-mysql mysql -uroot -p$$MYSQL_ROOT_PASSWORD gbserver < database/init-mysql-2.7.4.sql

# === 清理 ===

clean:
    cargo clean
    rm -f data/gbserver.db data/gbserver.db-wal data/gbserver.db-shm

# === 信息 ===

info:
    @echo "GBServer 数据库后端：一个二进制支持三种"
    @echo ""
    @echo "编译: cargo build --release   （sqlite + mysql + postgres 三驱动同编）"
    @echo "选库: 改 config/application.toml 的 [database] url，或用 GBSERVER__DATABASE__URL 覆盖"
    @echo ""
    @echo "SQLite（默认配置）:"
    @echo "  启动: just dev  或  cargo run"
    @echo "  Docker: just docker-sqlite"
    @echo ""
    @echo "PostgreSQL:"
    @echo "  启动: just pg  或  GBSERVER__DATABASE__URL=postgres://... cargo run"
    @echo "  Docker: just docker-pg"
    @echo ""
    @echo "MySQL:"
    @echo "  启动: just mysql  或  GBSERVER__DATABASE__URL=mysql://... cargo run"
    @echo ""
    @echo "方言验证: just dialect-check（真实库冒烟见 scripts/dialect_smoke_all.sh）"
    @echo "详见 docs/DEPLOYMENT_GUIDE.md 与 docs/DB_RUNTIME_DIALECT_PLAN.md"
