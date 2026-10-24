//! 分页偏移的统一算法。
//!
//! 仓库里到处是 `(page - 1) * count`。`page` / `count` 都是 `u32`（或从查询串
//! 反序列化成 `u32`），两者相乘在 `page` 很大时会**溢出 `u32`**：
//!   * debug 构建直接 panic（`attempt to multiply with overflow`），而 HTTP
//!     handler 链路上没有 `CatchPanic` 层，panic 会打断整条连接；
//!   * release 构建则**回绕**成一个随意的页码 —— 请求第 N 页却拿到了别的页。
//!
//! 实测：`page = u32::MAX`（`?page=4294967295`）、`count = 100` 时，
//! `4294967294 * 100` 已经超出 `u32` 上限。
//!
//! `page` 直接来自查询串、没有上限，所以偏移一律用 `i64` / `usize` 算。

/// SQL `OFFSET`：`(page - 1) * count`，用 `i64` 算，溢出饱和到 `i64::MAX`。
///
/// **必须传"实际生效的每页条数"**（已经 clamp 过的那个值），否则 `OFFSET` 的
/// 步长会与 `LIMIT` 的页长不一致，翻页会漏行或重复。
///
/// `page = 0` 按第 1 页处理（`saturating_sub`）。
pub fn offset(page: u32, count: u32) -> i64 {
    (page.saturating_sub(1) as i64).saturating_mul(count as i64)
}

/// 内存分页用的偏移：语义同上，结果是 `usize`。
///
/// 用于 `rows.iter().skip(offset)` 这类**先在内存里取完再切页**的路径。
pub fn offset_usize(page: u32, count: u32) -> usize {
    (page.saturating_sub(1) as usize).saturating_mul(count as usize)
}

/// `offset` 的 `i64` 版：给 `page` / `count` 本来就是 `i64` 的那几条老路径用。
///
/// 与 `offset` 的差别只有两点：`page` 小于 1 一律按第 1 页算（而不是算出负
/// 的 `OFFSET` —— 负偏移在 PostgreSQL / MySQL 上直接报错，在 SQLite 上却被
/// 当成 0，三种方言行为不一致）；乘法溢出饱和而不是回绕。
pub fn offset_i64(page: i64, count: i64) -> i64 {
    (page.max(1) - 1).saturating_mul(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归：超大 `page` 不能溢出（旧写法 `(page - 1) * count` 在 debug 下 panic）。
    #[test]
    fn offset_does_not_overflow_on_huge_page() {
        // 旧写法：4294967294u32 * 100u32 → debug panic / release 回绕成 4294967096。
        assert_eq!(offset(u32::MAX, 100), 429_496_729_400);
        assert_eq!(offset(u32::MAX, 100), 429_496_729_400i64);
        // 回绕值必须不再出现。
        assert_ne!(offset(u32::MAX, 100), 4_294_967_096);
    }

    #[test]
    fn offset_keeps_normal_paging() {
        assert_eq!(offset(1, 20), 0);
        assert_eq!(offset(2, 20), 20);
        assert_eq!(offset(3, 20), 40);
        // page = 0 按第 1 页处理。
        assert_eq!(offset(0, 20), 0);
    }

    #[test]
    fn offset_usize_does_not_overflow_on_huge_page() {
        assert_eq!(offset_usize(u32::MAX, 1000), 4_294_967_294_000);
        assert_eq!(offset_usize(1, 20), 0);
        assert_eq!(offset_usize(2, 20), 20);
        assert_eq!(offset_usize(0, 20), 0);
    }

    /// 回归：`offset_i64` 不能算出**负的 OFFSET**。
    ///
    /// 负偏移在 PostgreSQL / MySQL 上直接报错（`OFFSET must not be negative`），
    /// 在 SQLite 上却被当成 0 —— 同一条请求在三种方言上行为不一致。
    #[test]
    fn offset_i64_never_goes_negative() {
        assert_eq!(offset_i64(1, 20), 0);
        assert_eq!(offset_i64(2, 20), 20);
        // page = 0 与 page 为负都按第 1 页算。
        assert_eq!(offset_i64(0, 20), 0);
        assert_eq!(offset_i64(-5, 20), 0);
        assert_eq!(offset_i64(i64::MIN, 20), 0);
        // 极端 page 也不能溢出。
        assert_eq!(offset_i64(i64::MAX, 20), i64::MAX);
    }
}
