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
//! ## 只前进，不后退
//!
//! postgres 的 `nextval` **不是事务性的**：回滚的 INSERT、或删掉了最大 id 的
//! 那一行，都会让序列**停在 `MAX(id)` 之上**。这是常态，不是异常。
//!
//! 所以这里**不能**简单地 `setval(seq, MAX(id), true)` —— 那会把序列往回拨，
//! 接下来几次插入会**复用已经发放过的 id**。本 schema 没有任何外键
//! （`init-postgresql-2.7.4.sql` 里没有 `REFERENCES`），引用完整性全靠应用层
//! （`gb_device_channel.data_device_id` → `gb_device.id`、
//! `gb_user_api_key.user_id` → `gb_user.id`），id 一旦复用，残留引用就会
//! **静默挂到另一行上**。
//!
//! 正确做法：只在序列**落后于**表内容时把它推上去，绝不把已经领先的序列拉回来。

use crate::db::Pool;

/// 一个 `serial` 列及其所属的序列。
///
/// 只在 **postgres 方言**下用得上（sqlite/mysql 的自增主键不依赖序列），
/// 但**门在运行期**而不是编译期 —— 三个驱动同时编进一个二进制之后，
/// 编译期 feature 门永远为真，非 postgres 的那一份也会被执行。
struct SerialColumn {
    /// 表所在 schema（不用硬编码 `public` —— 部署可能改过 `search_path`）。
    schema: String,
    /// 表名。
    table: String,
    /// **自增列名**（从依赖关系推出，不写死 `id`）。
    column: String,
    /// 序列名。
    sequence: String,
}

/// 把所有落后的 `serial` 序列推进到当前 `MAX(列)`。
///
/// 返回被调整过的序列数量；**非 postgres 方言是空实现**
/// （sqlite/mysql 的自增主键不依赖序列）。
pub async fn sync_serial_sequences(pool: &Pool) -> sqlx::Result<u64> {
    // 运行期守卫：三个驱动同时编进一个二进制之后，编译期 feature 门已失效，
    // 这里必须按**当前连接的方言**决定要不要真的同步序列。
    if !pool.dialect().is_postgres() {
        return Ok(0);
    }

    let columns = discover_serial_columns(pool).await?;
    let mut adjusted = 0u64;
    for col in columns {
        match advance_sequence(pool, &col).await {
            Ok(true) => adjusted += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!("同步序列 {}.{} 失败: {}", col.schema, col.sequence, e),
        }
    }
    if adjusted > 0 {
        tracing::info!("已把 {} 个落后的 serial 序列同步到当前 MAX(列)", adjusted);
    }
    Ok(adjusted)
}

/// 找出所有「表 ← 自增列 ← 序列」的自动依赖关系。
///
/// 用 `pg_depend` 的 `deptype = 'a'`（auto）关联：序列**拥有**那一列。
/// `refobjsubid` 就是自增列的 `attnum`，据此能拿到**真实的列名**，
/// 不用假设主键一定叫 `id`。
///
/// **不要**用 `pg_get_serial_sequence(table, 'id')` —— 它在列不存在时是
/// **抛错**而不是返回 NULL，对 `pg_class` 里的系统视图逐行调用会直接报
/// `column "id" of relation "pg_statistic" does not exist`。
async fn discover_serial_columns(pool: &Pool) -> sqlx::Result<Vec<SerialColumn>> {
    let rows: Vec<(String, String, String, String)> = crate::dispatch!(pool, |p| {
        sqlx::query_as(
            "SELECT n.nspname, t.relname, a.attname, s.relname \
         FROM pg_class t \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_class s ON s.relkind = 'S' \
         JOIN pg_depend d ON d.objid = s.oid \
              AND d.refclassid = 'pg_class'::regclass \
              AND d.refobjid = t.oid AND d.deptype = 'a' \
         JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = d.refobjsubid \
         WHERE t.relkind = 'r' AND a.attnum > 0 AND NOT a.attisdropped",
        )
        .fetch_all(p)
    })?;

    Ok(rows
        .into_iter()
        .map(|(schema, table, column, sequence)| SerialColumn {
            schema,
            table,
            column,
            sequence,
        })
        .collect())
}

/// 把一个落后的序列推上去；已经领先的不动。
///
/// 返回 `true` 表示确实调整过。
async fn advance_sequence(pool: &Pool, col: &SerialColumn) -> sqlx::Result<bool> {
    // 表里当前的最大 id（空表为 None）。
    // 注意两处：
    // 1. schema 限定的是**表**，列只能写列名 —— `MAX("public"."id")` 是非法
    //    SQL（postgres 会去找名为 public 的表）。
    // 2. postgres 里 `max(int4)` 的返回值仍是 **int4**（只有 `max(int8)` 才是
    //    int8），按 `Option<i64>` 解会报 "mismatched types"。必须显式
    //    `::bigint` 转一道 —— 这与本仓库其它地方的 INT4 列处理是同一个道理。
    let max_col: Option<i64> = crate::dispatch!(pool, |p| {
        sqlx::query_scalar(&format!(
            "SELECT MAX({})::bigint FROM {}.{}",
            quote_ident(&col.column),
            quote_ident(&col.schema),
            quote_ident(&col.table)
        ))
        .fetch_one(p)
    })?;

    // 序列当前状态：`last_value` 是最后发放的值，`is_called=false` 表示
    // 还没发放过 —— 此时下一次 nextval 返回的就是 `last_value` 本身。
    let (last_value, is_called): (i64, bool) = crate::dispatch!(pool, |p| {
        sqlx::query_as(&format!(
            "SELECT last_value, is_called FROM {}.{}",
            quote_ident(&col.schema),
            quote_ident(&col.sequence)
        ))
        .fetch_one(p)
    })?;

    let next_value = next_value_of(last_value, is_called);
    let max_value = max_col.unwrap_or(0);
    if !needs_advance(next_value, max_value) {
        // 序列已经领先（或刚好对齐）—— **不能往回拨**，否则会重复发放 id。
        return Ok(false);
    }

    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&format!(
            "SELECT setval({}, $1, true)",
            regclass_literal(&col.schema, &col.sequence)
        ))
        .bind(max_value)
        .execute(p)
    })?;
    Ok(true)
}

/// 序列的**下一个**将要发放的值。
///
/// `is_called = false` 表示还没发放过 —— 此时 `nextval` 返回的就是
/// `last_value` 本身（而不是 `last_value + 1`）。
fn next_value_of(last_value: i64, is_called: bool) -> i64 {
    if is_called {
        last_value + 1
    } else {
        last_value
    }
}

/// 序列是否**落后于**表内容、需要被推进。
///
/// 只有"下一个要发放的 id 还没超过表里的最大 id"时才推进。
/// 序列**领先**时（postgres 的 nextval 非事务性，回滚的 INSERT 或删掉最大 id
/// 的行都会让序列领先 —— 这是常态）必须**原样不动**：往回拨会让后面几次插入
/// **复用已经发放过的 id**，而本 schema 没有任何外键，残留引用会静默挂错行。
fn needs_advance(next_value: i64, max_value: i64) -> bool {
    next_value <= max_value
}

/// 把标识符安全地嵌进 SQL（postgres 的带引号标识符）。
///
/// 这些名字来自**系统目录**（`pg_class` / `pg_namespace`），不是运行时输入，
/// 注入面为零 —— 但拼进 SQL 仍必须正确转义：postgres 的带引号标识符里
/// 出现 `"` 要写成 `""`。
///
/// **不要**改回"只放行 `[a-z0-9_]+`"的白名单：那样会把带引号的大写表名
/// （`"GB_Mixed"`）和非 ASCII 表名（`"终端表"`）**静默跳过** —— 而这些表
/// 恰恰是最可能带显式种子 id 的表。跳过它们等于本模块要修的
/// "种子行主键冲突"原样复发，而且日志里一行提示都没有（静默失败最难查）。
#[allow(dead_code)]
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// 拼出 `setval()` 需要的 `regclass` 字面量（`'"schema"."name"'`）。
///
/// 字面量里的 `'` 要转义成 `''`。
#[allow(dead_code)]
fn regclass_literal(schema: &str, name: &str) -> String {
    let raw = format!("{}.{}", quote_ident(schema), quote_ident(name));
    format!("'{}'", raw.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 序列落后于表内容时**要**推进。
    ///
    /// 判据是"**下一个要发放的 id** 是不是已经被占用了"：
    /// `next_value <= max_value` 说明下一个会撞主键，必须推上去。
    #[test]
    fn advances_when_sequence_is_behind() {
        // 种子行的坑：last=1、还没发放过（is_called=false）→ nextval 会发 1，
        // 而 id=1 已被 admin 占用 → 必须推进（推进到 1，下一个就发 2）。
        assert!(needs_advance(next_value_of(1, false), 1));
        // last=3、已发放过 → nextval 会发 4，而 id=4 已被占用 → 必须推进。
        assert!(needs_advance(next_value_of(3, true), 4));
        // last=4、已发放过 → nextval 会发 5，没被占用 → 不用动。
        assert!(!needs_advance(next_value_of(4, true), 4));
    }

    /// 序列领先于表内容时**不能**推进 —— 往回拨会重复发放 id。
    #[test]
    fn never_rewinds_a_sequence_that_is_ahead() {
        // 表里最大 id=15，序列下一个要发 31 → 明显领先
        assert!(!needs_advance(next_value_of(30, true), 15));
        // 刚好对齐：nextval 会发 16 > 15 → 不动
        assert!(!needs_advance(next_value_of(15, true), 15));
        // 空表（max 视为 0），序列下一个要发 1 → 不动（1 是空着的）
        assert!(!needs_advance(next_value_of(1, false), 0));
    }

    /// `is_called = false` 时 nextval 返回 `last_value` 本身。
    #[test]
    fn next_value_accounts_for_is_called() {
        assert_eq!(next_value_of(7, true), 8);
        assert_eq!(next_value_of(7, false), 7);
    }

    /// 标识符转义：postgres 的带引号标识符里 `"` 要写成 `""`。
    ///
    /// 关键点：**不能**再用"只放行 `[a-z0-9_]+`"的白名单 —— 那会把
    /// `"GB_Mixed"`、`"终端表"` 这类表**静默跳过**，而它们恰恰是最可能带
    /// 显式种子 id 的表（跳过 = 本模块要修的主键冲突原样复发，且无日志）。
    #[test]
    fn quote_ident_escapes_quotes_and_keeps_exotic_names() {
        assert_eq!(quote_ident("gb_user"), "\"gb_user\"");
        assert_eq!(quote_ident("GB_Mixed"), "\"GB_Mixed\"");
        assert_eq!(quote_ident("终端表"), "\"终端表\"");
        // 内嵌的 `"` 必须翻倍，否则会提前闭合标识符
        assert_eq!(quote_ident("gb\"user"), "\"gb\"\"user\"");
        assert_eq!(
            quote_ident("a\"; drop table x; --"),
            "\"a\"\"; drop table x; --\""
        );
    }

    /// `setval()` 的 `regclass` 字面量：单引号要转义成两个。
    #[test]
    fn regclass_literal_escapes_single_quote() {
        assert_eq!(
            regclass_literal("public", "gb_user_id_seq"),
            "'\"public\".\"gb_user_id_seq\"'"
        );
        assert_eq!(regclass_literal("pub'lic", "seq"), "'\"pub''lic\".\"seq\"'");
    }
}
