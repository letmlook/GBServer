pub mod alarm;
pub mod audit_log;
pub mod cloud_record;
pub mod common_channel;
pub mod device;
pub mod dialect;
pub mod group;
pub mod jt1078;
pub mod log;
pub mod media_server;
pub mod mobile_position;
pub mod platform;
pub mod platform_channel;
pub mod platform_group;
pub mod platform_region;
pub mod position_history;
#[cfg(test)]
mod read_smoke;
pub mod record_plan;
pub mod region;
pub mod role;
pub mod seq_sync;
pub mod stream_proxy;
pub mod stream_push;
mod user;
pub mod user_api_key;

// 跨模块同名函数 re-export（如 `count_all`、`get_by_id` 等多模块共有）。
// 调用方写 `db::count_all(&pool)` 时使用"先 import 的优先"语义；触发 warning 但不编译失败。
// 业务层实际依赖调用路径明确（`db::device::get_by_id` vs `db::region::get_by_id`）。
pub use alarm::Alarm;
pub use cloud_record::CloudRecord;
#[allow(ambiguous_glob_reexports)]
pub use device::*;
#[allow(ambiguous_glob_reexports)]
pub use group::*;
pub use jt1078::{JtChannel, JtTerminal};
#[allow(ambiguous_glob_reexports)]
pub use log::*;
#[allow(ambiguous_glob_reexports)]
pub use media_server::*;
pub use mobile_position::MobilePosition;
pub use platform::Platform;
pub use position_history::PositionHistory;
#[allow(ambiguous_glob_reexports)]
pub use record_plan::*;
#[allow(ambiguous_glob_reexports)]
pub use region::*;
#[allow(ambiguous_glob_reexports)]
pub use role::*;
pub use stream_proxy::{StreamProxy, StreamProxyWrite};
pub use stream_push::StreamPush;
#[allow(ambiguous_glob_reexports)]
pub use user::*;
#[allow(ambiguous_glob_reexports)]
pub use user_api_key::*;

use crate::config::AppConfig;
pub use dialect::Dialect;

// ---------------------------------------------------------------------------
// 运行期多态的数据库池
// ---------------------------------------------------------------------------
//
// **一个二进制，三种后端**：三个驱动同时编进来，运行时按 `database.url`
// 的 scheme 选库（`sqlite:` / `postgres:` / `mysql:`）。
//
// 此前是**互斥 cargo feature** + 编译期类型别名：`Pool` 只有一种具体类型，
// 每个 db 函数用 `#[cfg(feature = ...)]` 抄三份 SQL。改造后：
//
// - 部署方只发一个二进制，配置改一行就换库；
// - SQL 文本**一份**，由 [`dialect::dialect_sql`] 按运行期方言改写占位符；
// - 各方言**原生** pool —— 布尔列、TINYINT、REAL 的解码语义与各自
//   原生驱动完全一致（这是不用 sqlx `Any` 驱动的原因，详见 `docs/DB_DIALECT_NOTES.md`）。
//
// 三个驱动必须同时启用（`Pool` 枚举引用了三种具体 pool 类型）。
#[cfg(not(all(feature = "sqlite", feature = "mysql", feature = "postgres")))]
compile_error!(
    "三个数据库驱动必须同时启用：\n\
     cargo build                                  # default 已含 sqlite+mysql+postgres\n\
     cargo build --no-default-features --features \"sqlite,mysql,postgres\""
);

/// 运行期多态的数据库池。
#[derive(Clone)]
pub enum Pool {
    /// SQLite（`sqlite:` / `file:` / 裸文件路径）
    Sqlite(sqlx::SqlitePool),
    /// PostgreSQL（`postgres:` / `postgresql:`）
    Postgres(sqlx::PgPool),
    /// MySQL / MariaDB（`mysql:` / `mariadb:`）
    MySql(sqlx::MySqlPool),
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pool::Sqlite(_) => write!(f, "Pool::Sqlite"),
            Pool::Postgres(_) => write!(f, "Pool::Postgres"),
            Pool::MySql(_) => write!(f, "Pool::MySql"),
        }
    }
}

impl Pool {
    /// 本池对应的方言。
    pub fn dialect(&self) -> Dialect {
        match self {
            Pool::Sqlite(_) => Dialect::Sqlite,
            Pool::Postgres(_) => Dialect::Postgres,
            Pool::MySql(_) => Dialect::MySql,
        }
    }

    /// 把**统一用 `?` 写**的 SQL 改写成当前方言的占位符形式。
    ///
    /// - PostgreSQL：`?` → `$1`、`$2` ……
    /// - MySQL / SQLite：原样返回（零拷贝）
    ///
    /// 这是仓库的既定约定（`docs/DB_DIALECT_NOTES.md` 规则 1）：
    /// **一份 SQL 文本服务三种方言**，避免"改了这里忘了那里"。
    pub fn sql<'a>(&self, sql: &'a str) -> std::borrow::Cow<'a, str> {
        dialect::dialect_sql_for(sql, self.dialect())
    }
}

/// 按运行期方言**分发同一段查询代码**。
///
/// 宏体展开三次，每次 `$p` 是**各自方言的具体 pool 引用**，
/// 于是 `sqlx::query_as::<_, T>(&sql).bind(..).fetch_all($p)`
/// 在每个分支里各自完成类型推导 —— 一份 SQL 文本走三种后端。
///
/// ```ignore
/// let sql = pool.sql("SELECT id FROM t WHERE id = ?");
/// let rows = crate::dispatch!(pool, |p| {
///     sqlx::query_as::<_, MyRow>(&sql).bind(id).fetch_all(p)
///  });
/// ```
///
/// 方言**专属语法**（`RETURNING` / `ON CONFLICT` / `CAST`）不能靠占位符统一，
/// 在宏体内用 `pool.dialect()` 判断即可。
#[macro_export]
macro_rules! dispatch {
    ($pool:expr, |$p:ident| $q:block) => {
        match &$pool {
            $crate::db::Pool::MySql($p) => ($q).await,
            $crate::db::Pool::Postgres($p) => ($q).await,
            $crate::db::Pool::Sqlite($p) => ($q).await,
        }
    };
}

/// 按运行期方言分发**执行语句**（`INSERT` / `UPDATE` / `DELETE` / DDL）。
///
/// 与 [`dispatch!`] 分开的原因：`execute()` 返回的 **QueryResult 类型是方言相关的**
/// （`MySqlQueryResult` / `PgQueryResult` / `SqliteQueryResult`），
/// 三个分支直接放进同一个 `match` 会得到
/// "match arms have incompatible types"。这里统一映射成 `rows_affected()`
/// —— 三方言语义一致（= 命中行数，与"值有没有变"无关）。
#[macro_export]
macro_rules! dispatch_exec {
    ($pool:expr, |$p:ident| $q:block) => {
        match &$pool {
            $crate::db::Pool::MySql($p) => ($q).await.map(|r| $crate::db::ExecResult {
                rows_affected: r.rows_affected(),
                // MySQL 的自增 id 是**方言专有 API**（`QueryResult` trait 上没有），
                // 只能在各方言自己的分支里取。
                last_insert_id: Some(r.last_insert_id() as i64),
            }),
            $crate::db::Pool::Postgres($p) => ($q).await.map(|r| $crate::db::ExecResult {
                rows_affected: r.rows_affected(),
                // PostgreSQL 没有 `last_insert_id` —— 取新 id 要用 `RETURNING id`。
                last_insert_id: None,
            }),
            $crate::db::Pool::Sqlite($p) => ($q).await.map(|r| $crate::db::ExecResult {
                rows_affected: r.rows_affected(),
                last_insert_id: Some(r.last_insert_rowid()),
            }),
        }
    };
}

/// 方言**专属**分支：每个方言一份自己的 SQL / 逻辑。
///
/// [`dispatch!`] 的宏体在三个分支里展开的是**同一份代码**，所以宏体里的 SQL
/// 文本必须是三种方言都能解析的 —— 一旦遇到方言专属语法就会编译不过
/// （`UNNEST` / `ON DUPLICATE KEY UPDATE` / `INSERT ... RETURNING` /
/// `ILIKE` / `LAST_INSERT_ID()` / `::text[]` …）。
///
/// 那种地方用本宏：**每个分支带自己的 SQL**，只有对应方言的那一份会被
/// 编译、也只会在运行期真正执行。
///
/// ```ignore
/// crate::dispatch_dialect!(
///     pool,
///     |p| { sqlx::query(PG_SQL).bind(x).execute(p).await },   // PostgreSQL
///     |p| { sqlx::query(MY_SQL).bind(x).execute(p).await },   // MySQL
///     |p| { sqlx::query(SQ_SQL).bind(x).execute(p).await },   // SQLite
/// );
/// ```
///
/// 三个块的类型必须一致（都是 `Result<..>` 或都是 `..`）。
///
/// 每个块前面的 `|p|` 是**池绑定名**（三个块用同一个名字即可），宏会在
/// 对应的方言分支里把它绑成该方言的**具体 pool 引用**。
#[macro_export]
macro_rules! dispatch_dialect {
    ($pool:expr, |$pg_p:ident| $pg:block, |$my_p:ident| $my:block, |$sq_p:ident| $sq:block $(,)?) => {
        match &$pool {
            $crate::db::Pool::Postgres(__mbv_pg) => {
                #[allow(unused_variables)]
                let $pg_p = __mbv_pg;
                // 必须加括号：`block` 片段直接放在语句位置会被当成**块语句**
                // （值是 `()`），加括号后才是块**表达式**（值是它自己的值）。
                ($pg)
            }
            $crate::db::Pool::MySql(__mbv_my) => {
                #[allow(unused_variables)]
                let $my_p = __mbv_my;
                ($my)
            }
            $crate::db::Pool::Sqlite(__mbv_sq) => {
                #[allow(unused_variables)]
                let $sq_p = __mbv_sq;
                ($sq)
            }
        }
    };
}

/// 方言**专属**的「执行语句」分支：每个方言一份自己的 SQL。
///
/// 与 [`dispatch_dialect!`] 的区别：执行语句的返回类型（`QueryResult`）是
/// **方言相关**的，三个分支不能直接放进同一个 `match` —— 这里统一映射成
/// [`ExecResult`]（`rows_affected()` + 方言专有的 `last_insert_id()`）。
///
/// 宏体末尾**不要**写 `.await` —— 宏自己会 await。
#[macro_export]
macro_rules! dispatch_exec_dialect {
    ($pool:expr, |$pg_p:ident| $pg:block, |$my_p:ident| $my:block, |$sq_p:ident| $sq:block $(,)?) => {
        match &$pool {
            $crate::db::Pool::Postgres(__mbv_pg) => {
                #[allow(unused_variables)]
                let $pg_p = __mbv_pg;
                ($pg).await.map(|r| $crate::db::ExecResult {
                    rows_affected: r.rows_affected(),
                    // PostgreSQL 没有 `last_insert_id` —— 取新 id 要用 `RETURNING id`。
                    last_insert_id: None,
                })
            }
            $crate::db::Pool::MySql(__mbv_my) => {
                #[allow(unused_variables)]
                let $my_p = __mbv_my;
                ($my).await.map(|r| $crate::db::ExecResult {
                    rows_affected: r.rows_affected(),
                    last_insert_id: Some(r.last_insert_id() as i64),
                })
            }
            $crate::db::Pool::Sqlite(__mbv_sq) => {
                #[allow(unused_variables)]
                let $sq_p = __mbv_sq;
                ($sq).await.map(|r| $crate::db::ExecResult {
                    rows_affected: r.rows_affected(),
                    last_insert_id: Some(r.last_insert_rowid()),
                })
            }
        }
    };
}

/// 执行语句的统一结果。
///
/// 各方言的 QueryResult 类型不同（`MySqlQueryResult` / `PgQueryResult` /
/// `SqliteQueryResult`），不能直接放进同一个 `match`。这里收敛成一个
/// 跨方言的壳，让调用点原有的 `r.rows_affected()` 写法**保持不变**。
#[derive(Debug, Clone, Copy)]
pub struct ExecResult {
    /// 命中行数（三方语义一致：= 命中行数，与"值有没有变"无关）
    pub rows_affected: u64,
    /// 本次插入生成的自增 id。
    ///
    /// - MySQL：`last_insert_id()`
    /// - SQLite：`last_insert_rowid()`
    /// - PostgreSQL：**没有** —— pg 要取新 id 得用 `INSERT ... RETURNING id`，
    ///   所以这里是 `None`（调用点需自行用 RETURNING 那一支）。
    pub last_insert_id: Option<i64>,
}

impl ExecResult {
    /// 命中行数（保持与 sqlx `QueryResult` 同样的方法名，调用点无需改写）
    pub fn rows_affected(&self) -> u64 {
        self.rows_affected
    }

    /// 本次插入生成的自增 id（PostgreSQL 为 `None`，要用 `RETURNING id` 那一支）
    pub fn last_insert_id(&self) -> Option<i64> {
        self.last_insert_id
    }
}

/// 建立连接池：按 `database.url` 的 scheme 选后端。
///
/// 顺带把**进程级方言**写进全局（[`dialect::set_current`]）——
/// SQL 文本生成函数没有 pool 参数，靠它决定占位符形式。
pub async fn create_pool(cfg: &AppConfig) -> anyhow::Result<Pool> {
    let dialect = Dialect::from_url(&cfg.database.url).ok_or_else(|| {
        anyhow::anyhow!(
            "无法识别数据库方言（url={}）：支持的 scheme 为 sqlite: / file: / postgres: / postgresql: / mysql: / mariadb:",
            cfg.database.url
        )
    })?;
    dialect::set_current(dialect).map_err(|e| anyhow::anyhow!("设置数据库方言失败：{}", e))?;

    match dialect {
        Dialect::Sqlite => {
            let pool = create_sqlite_pool(&cfg.database.url).await?;
            tracing::info!("SQLite pool 初始化成功 (WAL mode): {}", &cfg.database.url);
            Ok(Pool::Sqlite(pool))
        }
        Dialect::Postgres => {
            let pool = create_postgres_pool(&cfg.database.url).await?;
            tracing::info!("PostgreSQL pool 初始化成功");
            Ok(Pool::Postgres(pool))
        }
        Dialect::MySql => {
            let pool = create_mysql_pool(&cfg.database.url).await?;
            tracing::info!("MySQL pool 初始化成功");
            Ok(Pool::MySql(pool))
        }
    }
}

/// SQLite 连接池。
///
/// URL 兼容：接受 `sqlite://path`、`sqlite::memory:`，或裸文件路径。
/// 裸路径自动补 `sqlite://` 前缀；绝对路径以 `/` 开头时，URL 用三斜杠 `sqlite:///abs/path`。
async fn create_sqlite_pool(url: &str) -> anyhow::Result<sqlx::SqlitePool> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let url_owned: String;
    let normalized: &str = if url.starts_with("sqlite:") || url.starts_with("file:") {
        url
    } else if url == ":memory:" || url.is_empty() {
        "sqlite::memory:"
    } else {
        // 绝对路径补三斜杠（保留根 `/`），相对路径补双斜杠
        if url.starts_with('/') {
            url_owned = format!("sqlite:///{}", url.trim_start_matches('/'));
        } else {
            url_owned = format!("sqlite://{}", url);
        }
        url_owned.as_str()
    };
    let opts = SqliteConnectOptions::from_str(normalized)
        .map_err(|e| anyhow::anyhow!("解析 SQLite 连接串失败: {} (url={})", e, url))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(5));
    Ok(SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await?)
}

/// PostgreSQL 连接池。
///
/// **必须关闭 prepared statement 缓存**（capacity = 0）。
///
/// 原因（2026-09-12 postgres 运行时冒烟实测）：
/// sqlx-postgres 的语句缓存以 **SQL 文本**为 key，命中缓存时
/// **不校验参数类型**（见 sqlx-postgres `get_or_prepare`：
/// `if let Some(statement) = self.cache_statement.get_mut(sql) { return ... }`）。
/// 于是"同一份 SQL 文本被两个不同 Rust 类型绑定"的地方，
/// 第二次会按第一次 PARSE 时声明的 OID 发送二进制参数，服务端直接报：
///   - `insufficient data left in message`
///   - `incorrect binary data format in bind parameter 1`
/// 具体撞车实例（实测报 500 的接口）：
///   `DELETE FROM gb_record_plan_item WHERE plan_id = $1`
///     —— db/record_plan.rs 的 `delete_by_id` 绑 i32、`replace_items` 绑 i64
///     → `/api/record/plan/delete`、`/api/record/plan/update` 500。
///     （该处参数类型已一并统一为 i32，作为第二道防线。）
/// sqlite（动态类型）与 mysql（宽进严出）都不受影响，所以这个问题
/// 只在 postgres 构建里、且只在"先跑 A 再跑 B"时才炸，隐藏得很深。
///
/// 关掉缓存后每次查询都带真实参数类型重新 PARSE，代价是每条语句多一个
/// 往返；对本项目（管理面 + 协议面，QPS 不高）换取的是**跨方言一致的
/// 正确性**。根治办法是把同文本 SQL 的参数类型统一，但那需要全仓约束，
/// 这里先用最可靠的开关兜住。
async fn create_postgres_pool(url: &str) -> anyhow::Result<sqlx::PgPool> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::str::FromStr;

    let opts = PgConnectOptions::from_str(url)
        .map_err(|e| anyhow::anyhow!("解析 PostgreSQL 连接串失败: {}", e))?
        .statement_cache_capacity(0);
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await?)
}

/// MySQL 连接池。
async fn create_mysql_pool(url: &str) -> anyhow::Result<sqlx::MySqlPool> {
    use sqlx::mysql::MySqlPoolOptions;
    Ok(MySqlPoolOptions::new()
        .max_connections(10)
        .connect(url)
        .await?)
}
