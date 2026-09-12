//! 动态 WHERE 构造器（多方言共用一份 SQL 文本）。
//!
//! 统一用 `?` 写条件，postgres 下按顺序把第 k 个 `?` 改写成 `$k`。
//!
//! 背景：alarm / 通道列表这些查询此前把"同一份带全部条件分支的 SQL"抄了
//! 6 遍（3 方言 × 行查询/计数），改一个筛选条件要改 6 处——漏改就是某个
//! 方言上静默失效（`startTime`/`endTime` 被接收却在 WHERE 里从未使用，
//! 就是这么来的）。
//!
//! 只放这一份，`stub.rs` 与 `alarm.rs` 共用。

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
    pub(crate) fn sql(&self, base: &str) -> String {
        self.sql_for(base, cfg!(feature = "postgres"))
    }

    /// `sql` 的可测版本：`postgres` 显式传入，便于在 sqlite 构建下也能
    /// 验证占位符改写（否则这段逻辑只在 postgres 构建里才跑到）。
    pub(crate) fn sql_for(&self, base: &str, postgres: bool) -> String {
        let raw = if self.conds.is_empty() {
            base.to_string()
        } else {
            format!("{} WHERE {}", base, self.conds.join(" AND "))
        };
        if postgres {
            let mut out = String::with_capacity(raw.len() + 16);
            let mut n = 0usize;
            for ch in raw.chars() {
                if ch == '?' {
                    n += 1;
                    out.push_str(&format!("${n}"));
                } else {
                    out.push(ch);
                }
            }
            out
        } else {
            raw
        }
    }
}

