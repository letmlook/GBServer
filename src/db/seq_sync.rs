//! PostgreSQL `serial` 序列与表内容对齐。
//!
//! ## 为什么需要
//!
//! `database/init-postgresql-2.7.4.sql` 用**显式 id** 播了种子行：
//!
//! ```sql
//! INSERT INTO gb_user      VALUES (1, 'admin', ...);
//! INSERT INTO gb_user_role VALUES (1, 'admin', '0', ...);
//! ```
//!
//! 而这两张表的主键是 `serial primary key` —— 序列本身停在初始值 1 且
//! `is_called = false`，也就是说**下一次 `nextval` 仍然返回 1**。
//! 于是**第一次**往这两张表插数据就撞上种子行的主键：
//!
//! ```text
//! ERROR: duplicate key value violates unique constraint "gb_user_pkey"
//! ```
//!
//! 症状很像"重复数据"，真因是**序列没跟上表**。sqlite 没有这个问题
//! （`INTEGER PRIMARY KEY AUTOINCREMENT` 在插入时自己算下一个 id），
//! mysql 也没有（`AUTO_INCREMENT` 会扫描表内容），所以**只有 postgres 会炸**。
//!
//! ## 为什么放在启动时做
//!
//! 新装（后端建表路径 + docker-compose 的 initdb 路径）都会中招，而**已装**
//! 的库同样中招 —— 序列一旦停在 1，除非有人手动 `setval`，否则永远是坏的。
//! 所以这段在**每次启动**都跑，不是只在建表时。
//!
//! ## 怎么找序列
//!
//! 用 `pg_depend` 的**自动依赖**（`deptype = 'a'`）把表和它自增列所属的序列
//! 关联起来。**不要**用 `pg_get_serial_sequence(table, 'id')` —— 它在列不存在
//! 时是**抛错**而不是返回 NULL，对 `pg_class` 里的系统视图逐行调用会直接
//! 报 `column "id" of relation "pg_statistic" does not exist`。

use crate::db::Pool;

/// 把所有 `serial` 主键的序列推进到当前 `MAX(id)`。
///
/// 返回被调整过的序列数量；非 postgres 构建是空实现。
#[cfg(feature = "postgres")]
pub async fn sync_serial_sequences(pool: &Pool) -> sqlx::Result<u64> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT t.relname, s.relname \
         FROM pg_class t \
         JOIN pg_namespace n ON n.oid = t.relnamespace AND n.nspname = 'public' \
         JOIN pg_class s ON s.relkind = 'S' \
         JOIN pg_depend d ON d.objid = s.oid \
              AND d.refclassid = 'pg_class'::regclass \
              AND d.refobjid = t.oid AND d.deptype = 'a' \
         WHERE t.relkind = 'r'",
    )
    .fetch_all(pool)
    .await?;

    let mut adjusted = 0u64;
    for (table, seq) in rows {
        // 表名/序列名来自系统目录，不是用户输入；仍然只放行我们自己 schema 的
        // 命名风格，再用双引号包起来 —— 不给任何"拼进 SQL"的自由度。
        if !is_safe_ident(&table) || !is_safe_ident(&seq) {
            continue;
        }
        let sql = format!(
            "SELECT setval('{seq}', GREATEST(COALESCE((SELECT MAX(id) FROM \"{table}\"), 0), 1), true)"
        );
        match sqlx::query(&sql).execute(pool).await {
            Ok(_) => adjusted += 1,
            Err(e) => tracing::warn!("同步序列 {seq} 失败（表 {table}）: {e}"),
        }
    }
    if adjusted > 0 {
        tracing::info!("已把 {} 个 serial 序列同步到当前 MAX(id)", adjusted);
    }
    Ok(adjusted)
}

/// 非 postgres 构建是空实现（sqlite/mysql 的自增主键不依赖序列）。
#[cfg(not(feature = "postgres"))]
pub async fn sync_serial_sequences(_pool: &Pool) -> sqlx::Result<u64> {
    Ok(0)
}

/// 只放行小写字母、数字与下划线 —— 拼进 SQL 的标识符必须过这一关。
#[allow(dead_code)]
fn is_safe_ident(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
