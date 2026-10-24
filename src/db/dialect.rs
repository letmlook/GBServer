//! **运行期方言抽象**（SQLite / PostgreSQL / MySQL 三选一，进程内固定）
//!
//! ## 为什么从编译期搬到运行期
//!
//! 此前三个后端是**互斥的 cargo feature**：`db::Pool` 是编译期类型别名，
//! 每个 db 函数用 `#[cfg(feature = "postgres")]` 写三份 SQL。代价是：
//! - 部署方要为每种库各发一个二进制；
//! - 每改一条 SQL 就要三个 feature 各构建 + 三个真实库各冒烟一次。
//!
//! 改造后：三个驱动**同时编进同一个二进制**，由 `database.url` 的 scheme
//! 在**运行期**决定用哪个。SQL 文本随之按运行期方言生成。
//!
//! ## 为什么是「进程内固定」而不是「每条语句一个方言」
//!
//! 一个进程只连一个库 —— `create_pool` 建好池之后方言不再变。
//! 于是全局放一个 `Dialect`（原子量），SQL 生成函数直接读它，
//! 不必把 dialect 参数一路透传到每个 db 函数。
//!
//! ## 占位符
//!
//! - SQLite / MySQL：`?`
//! - PostgreSQL：`$1..$n`（pg 不认 `?`，直接发过去是 `syntax error`）
//!
//! 统一用 `?` 写、由 [`dialect_sql`] 改写，是本仓库的既定约定
//! （见 `docs/DB_DIALECT_NOTES.md` 规则 1）。

use std::sync::atomic::{AtomicU8, Ordering};

/// 数据库方言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// SQLite（`sqlite:` / `file:` / 裸文件路径）
    Sqlite,
    /// PostgreSQL（`postgres:` / `postgresql:`）
    Postgres,
    /// MySQL / MariaDB（`mysql:` / `mariadb:`）
    MySql,
}

impl Dialect {
    /// 按连接串的 scheme 推断方言。
    ///
    /// SQLite 的判定比别的宽松：仓库历史上允许**裸文件路径**
    /// （`config/application.toml` 里写 `data/gbserver.db`），
    /// 也允许 `file:` 前缀。判定顺序很重要 —— 先看显式 scheme，
    /// 再看是不是裸路径。
    pub fn from_url(url: &str) -> Option<Dialect> {
        let url = url.trim();
        if url.is_empty() {
            return None;
        }
        // 取 scheme：第一个 `:` 之前、且**不含 `/`** 的那段。
        // 没有 `:` 的**裸路径**（仓库历史配置写法 `data/gbserver.db`、
        // `/var/lib/gbserver.db`）也算 SQLite —— 注意不能只看
        // `split(':').next()`：裸路径没有 `:`，取出来的整段路径会被当成
        // 「未知 scheme」而返回 None。
        let scheme = match url.split_once(':') {
            Some((head, _)) if !head.contains('/') => head.to_ascii_lowercase(),
            _ => String::new(),
        };
        match scheme.as_str() {
            "sqlite" | "file" => Some(Dialect::Sqlite),
            "postgres" | "postgresql" => Some(Dialect::Postgres),
            "mysql" | "mariadb" => Some(Dialect::MySql),
            // 没有 scheme → 裸路径（SQLite）
            "" => Some(Dialect::Sqlite),
            _ => None,
        }
    }

    pub fn is_sqlite(self) -> bool {
        matches!(self, Dialect::Sqlite)
    }

    pub fn is_postgres(self) -> bool {
        matches!(self, Dialect::Postgres)
    }

    pub fn is_mysql(self) -> bool {
        matches!(self, Dialect::MySql)
    }

    /// 连接串里是否带**用户名**（pg / mysql 需要，sqlite 忽略）。
    pub fn is_sqlite_like(self) -> bool {
        self.is_sqlite()
    }
}

/// 全局当前方言。
///
/// `0` = 尚未设置；其余值对应 [`Dialect`] 的判别值 + 1。
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// 设置进程级方言。
///
/// 幂等：重复设置**同一个**方言是允许的（启动流程可能被调用两次），
/// 改成**不同**方言则拒绝 —— 一个进程同时连两个库没有意义，
/// 而静默接受会让 SQL 生成跟着错，风险远高于直接报错。
pub fn set_current(dialect: Dialect) -> Result<(), &'static str> {
    let prev = CURRENT.load(Ordering::SeqCst);
    if prev != 0 && decode(prev) != dialect {
        return Err("数据库方言已被设定为另一个后端，一个进程不能同时连两个库");
    }
    CURRENT.store(encode(dialect), Ordering::SeqCst);
    Ok(())
}

/// 取当前方言。
///
/// 未设置时**兜底为 SQLite**（本仓库的默认后端），而不是 panic：
/// 单元测试里可能不经过 `create_pool` 就直接调 SQL 生成函数。
/// 生产路径由 `create_pool` 保证一定设置过。
pub fn current() -> Dialect {
    match CURRENT.load(Ordering::SeqCst) {
        0 => Dialect::Sqlite,
        v => decode(v),
    }
}

/// 仅供测试：清掉全局方言。
pub fn reset_for_test() {
    CURRENT.store(0, Ordering::SeqCst);
}

fn encode(d: Dialect) -> u8 {
    match d {
        Dialect::Sqlite => 1,
        Dialect::Postgres => 2,
        Dialect::MySql => 3,
    }
}

fn decode(v: u8) -> Dialect {
    match v {
        2 => Dialect::Postgres,
        3 => Dialect::MySql,
        _ => Dialect::Sqlite,
    }
}

/// 把 SQL 里的 `?` 占位符改写成**当前方言**需要的形式。
///
/// - PostgreSQL：`?` → `$1`、`$2` ……
/// - MySQL / SQLite：原样返回（零拷贝）
///
/// 会跳过字符串字面量（`'...'`）与带引号标识符（`"..."`），
/// 避免把 `LIKE '%?%'` 里的问号也当成占位符。
pub fn dialect_sql(sql: &str) -> std::borrow::Cow<'_, str> {
    rewrite_placeholders(sql, current().is_postgres())
}

/// 按**全局当前方言**改写 SQL 文本（`?` → `$n`）。
///
/// 与 [`Pool::sql`](crate::db::Pool::sql) 等价，但不要求调用方手里有 pool ——
/// handlers 层的 pool 通常是 `state.pool`，脚本/宏不好统一拿到它的名字。
/// 进程内只有一个库，全局方言就是当前库的方言。
pub fn current_sql(sql: &str) -> std::borrow::Cow<'_, str> {
    dialect_sql(sql)
}

/// [`dialect_sql`] 的可测版本：方言显式传入。
pub fn dialect_sql_for(sql: &str, dialect: Dialect) -> std::borrow::Cow<'_, str> {
    rewrite_placeholders(sql, dialect.is_postgres())
}

/// 把一个整数列渲染成**64 位**表达式，让 Rust 侧的 `i64` 在三种方言下都能解出来。
///
/// PostgreSQL 的 `serial` 主键是 INT4，而 MySQL 的 `bigint auto_increment`
/// 与 SQLite 的 `INTEGER` 都是 64 位；postgres 对类型是**严格**的，
/// 用 `i64` 去解 INT4 列直接报类型不兼容。
///
/// 与占位符、布尔列是同一类问题，处理方式也一样：**在 SQL 侧统一**。
pub fn bigint_col(col: &str) -> String {
    bigint_col_for(col, current())
}

/// [`bigint_col`] 的可测版本。
pub fn bigint_col_for(col: &str, dialect: Dialect) -> String {
    if dialect.is_postgres() {
        format!("{col}::bigint")
    } else {
        col.to_string()
    }
}

/// 生成「N 天前」的时间表达式 —— **在数据库自己的时钟里**算。
///
/// 三种方言的写法：
/// - SQLite：`datetime('now', '-90 days')`（UTC）
/// - MySQL：`DATE_SUB(NOW(), INTERVAL 90 DAY)`（会话时区）
/// - PostgreSQL：`NOW() - INTERVAL '90 days'`（会话时区）
///
/// 必须在 Rust 里用同一个时钟算 —— 与列默认值的类型/时区一致，比较才有意义。
pub fn days_ago_expr(days: u32) -> String {
    days_ago_expr_for(days, current())
}

/// [`days_ago_expr`] 的可测版本。
pub fn days_ago_expr_for(days: u32, dialect: Dialect) -> String {
    if dialect.is_postgres() {
        format!("NOW() - INTERVAL '{days} days'")
    } else if dialect.is_mysql() {
        format!("DATE_SUB(NOW(), INTERVAL {days} DAY)")
    } else {
        format!("datetime('now', '-{days} days')")
    }
}

/// 把 `?` 改写成 `$n`（仅 PostgreSQL）。
fn rewrite_placeholders(sql: &str, postgres: bool) -> std::borrow::Cow<'_, str> {
    if !postgres || !sql.contains('?') {
        return std::borrow::Cow::Borrowed(sql);
    }
    let mut out = String::with_capacity(sql.len() + 16);
    let mut n = 0usize;
    // 跳过字符串字面量（'...'，'' 转义）与带引号标识符（"..."），
    // 避免把 `LIKE '%?%'` 里的问号也当成占位符改写。
    let mut quote: Option<char> = None;
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        match quote {
            Some(q) => {
                out.push(ch);
                if ch == q {
                    if chars.peek() == Some(&q) {
                        out.push(chars.next().unwrap());
                    } else {
                        quote = None;
                    }
                }
            }
            None => {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                    out.push(ch);
                } else if ch == '?' {
                    n += 1;
                    out.push('$');
                    out.push_str(&n.to_string());
                } else {
                    out.push(ch);
                }
            }
        }
    }
    std::borrow::Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_scheme_picks_dialect() {
        assert_eq!(
            Dialect::from_url("postgres://u:p@h:5432/db"),
            Some(Dialect::Postgres)
        );
        assert_eq!(
            Dialect::from_url("postgresql://u:p@h:5432/db"),
            Some(Dialect::Postgres)
        );
        assert_eq!(
            Dialect::from_url("mysql://u:p@h:3306/db"),
            Some(Dialect::MySql)
        );
        assert_eq!(
            Dialect::from_url("mariadb://u:p@h:3306/db"),
            Some(Dialect::MySql)
        );
        assert_eq!(Dialect::from_url("sqlite://data.db"), Some(Dialect::Sqlite));
        assert_eq!(Dialect::from_url("sqlite::memory:"), Some(Dialect::Sqlite));
        assert_eq!(Dialect::from_url("file:data.db"), Some(Dialect::Sqlite));
        // 裸路径 → SQLite（仓库历史配置写法）
        assert_eq!(Dialect::from_url("data/gbserver.db"), Some(Dialect::Sqlite));
        assert_eq!(
            Dialect::from_url("/var/lib/gbserver.db"),
            Some(Dialect::Sqlite)
        );
        // 未知 scheme
        assert_eq!(Dialect::from_url("mongodb://h/db"), None);
        assert_eq!(Dialect::from_url(""), None);
    }

    #[test]
    fn set_current_is_idempotent_but_rejects_conflict() {
        reset_for_test();
        assert_eq!(current(), Dialect::Sqlite);
        assert!(set_current(Dialect::Postgres).is_ok());
        assert_eq!(current(), Dialect::Postgres);
        // 同一个方言重复设置 → 允许
        assert!(set_current(Dialect::Postgres).is_ok());
        // 换成别的方言 → 拒绝
        assert!(set_current(Dialect::MySql).is_err());
        assert_eq!(current(), Dialect::Postgres);
        reset_for_test();
    }

    #[test]
    fn placeholder_rewrite_numbers_in_order() {
        let sql = "SELECT * FROM t WHERE a = ? AND b = ? OR c = ?";
        assert_eq!(
            dialect_sql_for(sql, Dialect::Postgres),
            "SELECT * FROM t WHERE a = $1 AND b = $2 OR c = $3"
        );
        assert_eq!(dialect_sql_for(sql, Dialect::Sqlite), sql);
        assert_eq!(dialect_sql_for(sql, Dialect::MySql), sql);
    }

    #[test]
    fn placeholder_rewrite_skips_quoted_regions() {
        let sql = "SELECT * FROM t WHERE a LIKE '%?%' AND b = ?";
        assert_eq!(
            dialect_sql_for(sql, Dialect::Postgres),
            "SELECT * FROM t WHERE a LIKE '%?%' AND b = $1"
        );
        let sql = "UPDATE t SET \"we?ird\" = ? WHERE id = ?";
        assert_eq!(
            dialect_sql_for(sql, Dialect::Postgres),
            "UPDATE t SET \"we?ird\" = $1 WHERE id = $2"
        );
        let sql = "SELECT 'it''s ?' , ?";
        assert_eq!(
            dialect_sql_for(sql, Dialect::Postgres),
            "SELECT 'it''s ?' , $1"
        );
        assert_eq!(dialect_sql_for("SELECT 1", Dialect::Postgres), "SELECT 1");
    }

    #[test]
    fn bigint_col_casts_only_on_postgres() {
        assert_eq!(bigint_col_for("c.id", Dialect::Postgres), "c.id::bigint");
        assert_eq!(bigint_col_for("c.id", Dialect::Sqlite), "c.id");
        assert_eq!(bigint_col_for("c.id", Dialect::MySql), "c.id");
    }

    #[test]
    fn days_ago_expr_uses_each_dialect_clock() {
        assert_eq!(
            days_ago_expr_for(90, Dialect::Postgres),
            "NOW() - INTERVAL '90 days'"
        );
        assert_eq!(
            days_ago_expr_for(90, Dialect::MySql),
            "DATE_SUB(NOW(), INTERVAL 90 DAY)"
        );
        assert_eq!(
            days_ago_expr_for(90, Dialect::Sqlite),
            "datetime('now', '-90 days')"
        );
    }
}
