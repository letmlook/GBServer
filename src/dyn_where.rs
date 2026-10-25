//! 动态 WHERE 构造器（多方言共用一份 SQL 文本）。
//!
//! 统一用 `?` 写条件，postgres 下按顺序把第 k 个 `?` 改写成 `$k`。
//!
//! 背景：alarm / 通道列表 / 云录像这些查询此前把"同一份带全部条件分支的 SQL"抄了
//! 6 遍（3 方言 × 行查询/计数），改一个筛选条件要改 6 处——漏改就是某个
//! 方言上静默失效（`startTime`/`endTime` 被接收却在 WHERE 里从未使用，
//! 就是这么来的）。
//!
//! 只放这一份：`stub.rs` / `alarm.rs` / `db::cloud_record` 共用。

/// `dialect_sql` 的可测版本：`postgres` 显式传入，便于在 sqlite 构建下
/// 也能验证改写逻辑（否则这段代码只在 postgres 构建里才跑到）。
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

/// 把一个整数列渲染成**64 位**表达式，让 Rust 侧的 `i64` 在三种方言下都能解出来。
///
/// 背景：三方言的**整数列宽度不一样** ——
/// postgres 的 `serial` 主键是 **INT4**，而 mysql 的 `bigint auto_increment`
/// 与 sqlite 的 `INTEGER` 都是 64 位。sqlx-postgres 对类型是**严格**的：用 `i64`
/// 去解 INT4 列直接报
/// `mismatched types; Rust type i64 (as SQL type INT8) is not compatible with SQL type INT4`；
/// sqlx-mysql / sqlx-sqlite 宽松，同一个洞**只在 postgres 上炸** ——
/// sqlite 的单测与 mysql 的冒烟全绿，问题一路留到 postgres 才暴露。
///
/// 与占位符（`?` vs `$1`）、布尔列（`bool` vs `tinyint(1)`）是同一类问题，
/// 处理方式也一样：**在 SQL 侧统一**，不要指望解码端兼容。
///
/// postgres 侧用 `::bigint` 显式转换；mysql/sqlite 本来就是 64 位，原样返回。
///
/// 2026-10-20 在 postgres 上实测命中 3 处：录像计划通道列表、报警列表/详情，
/// 以及录像计划调度器 —— 其中调度器那处会让**云录像计划每一轮都失败**。
/// 动态 WHERE 构造器（录像计划通道列表用）。
///
/// 统一用 `?` 写条件，postgres 下按顺序把第 k 个 `?` 改写成 `$k`——同一份 SQL
/// 文本即可服务三种方言。早期实现把整条 SQL（含全部条件分支）抄了 6 遍
/// （3 方言 × 行查询/计数），改一个条件要改 6 处，正是"改了这里忘了那里"的温床。
pub(crate) struct DynWhere {
    pub(crate) conds: Vec<String>,
    pub(crate) binds: Vec<BindValue>,
}

#[derive(Clone)]
pub(crate) enum BindValue {
    Text(String),
    Int(i32),
    /// 64 位整数（时间戳毫秒等，i32 会溢出截断）
    Big(i64),
    /// 布尔列（`pulling` / `pushing` / `enable` …）。
    ///
    /// 必须单独一个变体：postgres 里这些列是 `bool`，用 `Int(1)` 绑定会得到
    /// `operator does not exist: boolean = integer`，整个查询 500 ——
    /// 而 sqlite 上（`INTEGER` 列）却完全正常，所以这个洞只在 postgres 构建里炸。
    Bool(bool),
}

impl DynWhere {
    pub(crate) fn new() -> Self {
        Self {
            conds: Vec::new(),
            binds: Vec::new(),
        }
    }

    /// 追加一个条件；`?` 的个数必须与 `values` 个数一致。
    pub(crate) fn add(&mut self, cond: impl Into<String>, values: Vec<BindValue>) {
        let cond = cond.into();
        debug_assert_eq!(cond.matches('?').count(), values.len());
        self.conds.push(cond);
        self.binds.extend(values);
    }

    /// 按方言生成最终 SQL（postgres 把 `?` 换成 `$1..$n`）。
    ///
    /// **方言必须由调用方显式给出**（通常是 `pool.dialect()`）——
    /// 此前这里读的是**进程级全局** `dialect::current()`，它只在
    /// 「一个进程一个库」时与 pool 的方言一致；测试里直接构造
    /// `Pool::Postgres(..)` 时全局仍是默认 sqlite，于是 postgres 上
    /// 会收到没被改写的 `?` → `syntax error`。
    /// 把方言改成**必填参数**，等于让编译器强制每一个调用点把 pool 的方言
    /// 显式传进来 —— 这类「判据用错对象」的缺陷从此无法悄悄回归。
    pub(crate) fn sql(&self, base: &str, dialect: crate::db::Dialect) -> String {
        self.sql_for(base, dialect.is_postgres())
    }

    /// `sql` 的可测版本：`postgres` 显式传入，便于在 sqlite 构建下也能
    /// 验证占位符改写（否则这段逻辑只在 postgres 构建里才跑到）。
    pub(crate) fn sql_for(&self, base: &str, postgres: bool) -> String {
        let raw = if self.conds.is_empty() {
            base.to_string()
        } else {
            format!("{} WHERE {}", base, self.conds.join(" AND "))
        };
        rewrite_placeholders(&raw, postgres).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_placeholders_numbers_in_order() {
        let sql = "SELECT * FROM t WHERE a = ? AND b = ? OR c = ?";
        assert_eq!(
            rewrite_placeholders(sql, true),
            "SELECT * FROM t WHERE a = $1 AND b = $2 OR c = $3"
        );
        assert_eq!(rewrite_placeholders(sql, false), sql);
    }

    #[test]
    fn rewrite_placeholders_skips_quoted_regions() {
        // 字符串字面量里的 ? 不是占位符，不能被改写
        assert_eq!(
            rewrite_placeholders("SELECT * FROM t WHERE a LIKE '%?%' AND b = ?", true),
            "SELECT * FROM t WHERE a LIKE '%?%' AND b = $1"
        );
        // 带引号标识符里的 ? 同理
        assert_eq!(
            rewrite_placeholders("UPDATE t SET \"we?ird\" = ? WHERE id = ?", true),
            "UPDATE t SET \"we?ird\" = $1 WHERE id = $2"
        );
        // '' 转义
        assert_eq!(
            rewrite_placeholders("SELECT 'it''s ?' , ?", true),
            "SELECT 'it''s ?' , $1"
        );
        assert_eq!(rewrite_placeholders("SELECT 1", true), "SELECT 1");
    }

    #[test]
    fn dyn_where_sql_uses_same_rewriter() {
        let mut w = DynWhere::new();
        w.add(
            "a = ? AND b = ?",
            vec![BindValue::Int(1), BindValue::Bool(true)],
        );
        assert_eq!(
            w.sql_for("SELECT * FROM t", true),
            "SELECT * FROM t WHERE a = $1 AND b = $2"
        );
        assert_eq!(
            w.sql_for("SELECT * FROM t", false),
            "SELECT * FROM t WHERE a = ? AND b = ?"
        );
    }
}
