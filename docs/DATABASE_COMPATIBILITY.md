# GBServer 数据库兼容性方案

> **范围**：评估并实施 SQLite 数据库兼容性。`PostgreSQL` 仍是默认生产后端；`MySQL` 作为 WVP 平迁选项保留；`SQLite` 用于开发 / 演示 / 边缘节点 / 小规模生产。
>
> **部署拓扑与集群化判断**请参阅 [`DEPLOYMENT_GUIDE.md`](./DEPLOYMENT_GUIDE.md)。

---

## 1. 目标与定位

### 1.1 现状

| 后端 | Cargo feature | 状态 |
|------|---------------|------|
| PostgreSQL | `default = ["postgres"]` | **生产主力** |
| MySQL | `--no-default-features --features mysql` | WVP 兼容保留 |
| SQLite | ❌ 缺失 | 本次新增 |

`src/db/mod.rs:42-50` 当前用 `#[cfg]` 在编译期二选一确定 `Pool` 类型，`create_pool()` 也是二分支。

### 1.2 目标

引入 SQLite 作为**第三种编译期可选后端**，与现有 PG/MySQL 互斥：

- ✅ 开发与 CI：开箱即用，免装 PG / Redis
- ✅ 演示与边缘盒子：单文件 DB，简化备份（`VACUUM INTO` / 文件拷贝）
- ✅ 小规模生产（< 500 设备、< 50 并发直播、无 HA 要求）
- ❌ 不试图让 SQLite 进入集群路径（LiteFS / rqlite 复杂度 > 直接迁 PG）

### 1.3 非目标

- 不引入 SQL 方言抽象层（Dialect trait）——当前差异点少且分散，抽象层维护成本 > 收益
- 不启用 `sqlx::query!` 编译期宏（SQLite 的 `DATABASE_URL` 流程与 PG 冲突）
- 不要求 SQLite 100% 覆盖所有 SQL（少数边缘 SQL 走 cfg 兜底）

---

## 2. 兼容性调研结论

### 2.1 当前代码未依赖 PG 独占特性

| 特性 | 是否使用 | 影响 |
|------|----------|------|
| `LISTEN/NOTIFY` | ❌ | 跨节点通知全靠 Redis pub/sub |
| `RETURNING` | ❌ | 无 |
| `jsonb` | ❌ | 无（schema 中是 `text`） |
| CTEs 递归 | ❌ | 无 |
| `FOR UPDATE SKIP LOCKED` | ❌ | 无 |
| `generate_series` | ❌ | 无 |
| 临时表 / GUC 参数 | ❌ | 无 |

**结论**：不存在"必须保留 PG"的功能阻塞点。

### 2.2 必须为 SQLite 改写的 SQL

| 优先级 | 文件:行号 | PG/MySQL 语法 | SQLite 替代 |
|--------|----------|---------------|-------------|
| **P1** | `db/device.rs:815` | `ON CONFLICT (device_id) DO UPDATE SET ...`（PG） | SQLite 3.24+ **同样支持** `ON CONFLICT(col) DO UPDATE SET`，可直接统一 |
| **P1** | `db/device.rs:799,1173`、`db/stream_push.rs:96`、`db/media_server.rs:182` | MySQL `ON DUPLICATE KEY UPDATE ... VALUES(col)` | 改为 PG 写法 `ON CONFLICT(col) DO UPDATE SET col = excluded.col` |
| **P1** | `db/audit_log.rs:141` | PG `ILIKE ?` | `LIKE ? COLLATE NOCASE` |
| **P1** | `db/audit_log.rs:182` | PG `create_time::text` | `CAST(create_time AS TEXT)`，三库通用 |
| **P2** | `db/audit_log.rs:8,17` | PG `bool` / MySQL `tinyint(1)` | SQLite `INTEGER`（0/1） |
| **P2** | `db/position_history.rs:23` | 时间分区 | SQLite 不支持原生分区，应用层按月分表 |
| **P3** | `db/device.rs:1217` | `EXTRACT(EPOCH FROM now())` | `strftime('%s','now')` |

### 2.3 cfg 分支统计

| 模块 | `#[cfg(feature="postgres")]` | `#[cfg(feature="mysql")]` |
|------|----:|----:|
| `db/audit_log.rs` | 11 | 11 |
| `db/jt1078.rs` | 24 | — |
| `db/group.rs` | 11 | 11 |
| `db/platform_group.rs` | 5 | 5 |
| `db/platform_channel.rs` | 5 | 5 |
| **合计** | **约 60+** | |

---

## 3. 实施方案

### 3.1 总体策略

延续现有的 `#[cfg]` 三态互斥模式，不引入抽象层。SQL 改写优先采用"三库兼容写法"（如 `CAST`、`ON CONFLICT`），只对少数边缘 SQL 用 cfg 兜底。

```
[features]
default = ["postgres"]
mysql = ["sqlx/mysql"]
postgres = ["sqlx/postgres"]
sqlite = ["sqlx/sqlite"]     # 新增
```

### 3.2 Phase 拆分

| Phase | 范围 | 工时 |
|-------|------|------|
| **Phase 1** 基础设施 | Cargo feature + Pool + create_pool + init_db_tables + 最小 init SQL | 0.5 人天 |
| **Phase 2** SQL 方言适配 | 60+ cfg 扩为三态 + 关键 SQL 改写 | 3–5 人天 |
| **Phase 3** 测试与文档 | docker compose sqlite 变体 + CI 矩阵 + 文档 | 1–2 人天 |
| **合计** | | **约 5–8 人天** |

当前 PR 提交 **Phase 1**（基础设施 + 最小 schema），后续 PR 按模块渐进引入 SQL 适配。

---

## 4. Phase 1 详细步骤

### 4.1 Cargo.toml

```toml
[features]
default = ["postgres"]
# 数据库三选一：默认 PostgreSQL
mysql = ["sqlx/mysql"]
postgres = ["sqlx/postgres"]
sqlite = ["sqlx/sqlite"]   # 新增
```

### 4.2 src/db/mod.rs

```rust
#[cfg(feature = "sqlite")]
pub type Pool = sqlx::SqlitePool;

#[cfg(all(feature = "mysql", not(feature = "postgres"), not(feature = "sqlite")))]
pub type Pool = sqlx::MySqlPool;

#[cfg(all(feature = "postgres", not(feature = "mysql"), not(feature = "sqlite")))]
pub type Pool = sqlx::PgPool;

#[cfg(all(not(feature = "mysql"), not(feature = "postgres"), not(feature = "sqlite")))]
pub type Pool = sqlx::PgPool;  // 默认

pub async fn create_pool(cfg: &AppConfig) -> anyhow::Result<Pool> {
    #[cfg(feature = "sqlite")]
    {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        let opts = SqliteConnectOptions::from_str(&cfg.database.url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(opts)
            .await?;
        Ok(pool)
    }
    /* PG / MySQL 分支保持不变 */
}
```

### 4.3 src/lib.rs::init_db_tables

```rust
#[cfg(feature = "sqlite")]
{
    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='gb_device')"
    )
    .fetch_one(pool)
    .await.unwrap_or(false);

    if !table_exists {
        tracing::info!("[sqlite] schema tables not found, initializing...");
        let sql = include_str!("../database/init-sqlite-2.7.4.sql");
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() || stmt.starts_with("--") { continue; }
            if !stmt.to_uppercase().starts_with("CREATE") && !stmt.to_uppercase().starts_with("INSERT") {
                continue;
            }
            let _ = sqlx::query(stmt).execute(pool).await;
        }
        tracing::info!("[sqlite] schema initialization complete");
    }
}
```

### 4.4 database/init-sqlite-2.7.4.sql

Phase 1 仅覆盖**核心 6 表**最小集合（设备、通道、用户、角色、媒体服务器、流代理），保证 `cargo run --features sqlite` 能起服务；其余表的 cfg 适配在 Phase 2 推进时按需补齐。

---

## 5. 风险与缓解

| 风险 | 影响面 | 缓解 |
|------|--------|------|
| SQLite 单写者锁 | 高并发写阻塞 | WAL + `busy_timeout=5000ms`；超过 500 设备回退 PG |
| 文件损坏 | 异常断电 | 启用 WAL + `synchronous=NORMAL` |
| ALTER TABLE 限制 | schema 演进复杂 | 提供 `migrate` 子命令或重建表 |
| 并发读仍 OK | 读多写少 | 视频列表/录像查询类业务不受影响 |
| JT1078 / Redis 在 SQLite 模式下 | 行为相同 | StateStore / cache 与 DB 解耦，无影响 |

---

## 6. 验收清单

- [x] Cargo feature `sqlite` 添加，编译可通过
- [x] `cargo run --no-default-features --features sqlite` 启动成功
- [x] SQLite 数据库文件自动生成
- [x] `init-sqlite-2.7.4.sql` 覆盖核心表
- [x] 现有 PG/MySQL 路径不受影响
- [ ] Phase 2：所有 SQL 走通 cfg 三态
- [ ] Phase 3：CI 矩阵包含 sqlite

---

## 7. 相关文件

- `Cargo.toml:7-11` — feature 定义
- `src/db/mod.rs:42-72` — Pool 与 create_pool
- `src/lib.rs:24-85` — init_db_tables
- `database/init-postgresql-2.7.4.sql` — PG schema
- `database/init-mysql-2.7.4.sql` — MySQL schema
- `database/init-sqlite-2.7.4.sql`（本次新增）— SQLite schema