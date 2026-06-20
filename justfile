# GBServer 开发者快捷命令（just / cargo-make 兼容）
#
# 安装 just: cargo install just
# 或直接用 cargo run / cargo test 替代。
#
# 用法：
#   just                  # 默认任务：fmt + clippy + test
#   just dev              # SQLite 默认开发模式
#   just pg               # PostgreSQL 模式
#   just mysql            # MySQL 模式
#   just test-sqlite      # 仅 SQLite 测试
#   just feature-check    # 三 feature 编译验证

# 默认任务
default: fmt clippy test

# === 构建 ===

# SQLite 默认模式（开发 / 演示 / 边缘）
dev:
    cargo run

# PostgreSQL 模式（生产主力）
pg:
    cargo run --no-default-features --features postgres

# MySQL 模式（MySQL 平迁 / 兼容历史部署）
mysql:
    cargo run --no-default-features --features mysql

# Release 构建
build-sqlite:
    cargo build --release

build-pg:
    cargo build --release --no-default-features --features postgres

build-mysql:
    cargo build --release --no-default-features --features mysql

# === 测试 ===

test:
    cargo test --features sqlite --test sqlite_compat

test-sqlite:
    cargo test --features sqlite --test sqlite_compat -- --nocapture

test-all:
    cargo test --features sqlite --test sqlite_compat

# === CI 验证 ===

# 三 feature 编译验证（CI matrix）
feature-check:
    @echo "==> check default (sqlite)"
    cargo check
    @echo "==> check postgres"
    cargo check --no-default-features --features postgres
    @echo "==> check mysql"
    cargo check --no-default-features --features mysql
    @echo "==> check sqlite"
    cargo check --no-default-features --features sqlite
    @echo "✅ all 4 feature combos compile"

# === 代码质量 ===

fmt:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# === Docker ===

docker-sqlite:
    docker compose -f docker-compose.sqlite.yml up -d

docker-pg:
    docker compose up -d

docker-down:
    docker compose down

# === 数据库初始化 ===

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
    @echo "GBServer 数据库三选一"
    @echo ""
    @echo "默认: SQLite (Cargo.toml default = ['sqlite'])"
    @echo "  启动: just dev  或  cargo run"
    @echo "  测试: just test-sqlite"
    @echo "  Docker: just docker-sqlite"
    @echo ""
    @echo "PostgreSQL:"
    @echo "  启动: just pg  或  cargo run --no-default-features --features postgres"
    @echo "  Docker: just docker-pg"
    @echo ""
    @echo "MySQL:"
    @echo "  启动: just mysql  或  cargo run --no-default-features --features mysql"
    @echo ""
    @echo "详见 docs/DATABASE_COMPATIBILITY.md 与 docs/DEPLOYMENT_GUIDE.md"