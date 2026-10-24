//! **方言占位符守卫**（只跑在测试里；生产构建不含本模块）。
//!
//! ## 为什么需要它
//!
//! sqlx 的 `?` 占位符只对 **mysql / sqlite** 合法；**postgres** 的预编译语句
//! 协议只认 `$1`。把 `?` 原样发给 pg，得到的是
//! `ERROR: syntax error at end of input`（`?` 在语句末尾时）或
//! `syntax error at or near ","`（后面还有内容时）。
//!
//! 这个洞**特别难发现**，因为：
//! - `cargo test` 默认跑 sqlite，`?` 完全合法 → **单测全绿**；
//! - sqlite / mysql 的方言冒烟也全绿；
//! - 只有 **postgres 运行时**才暴露 —— 而 postgres 是 `config/application.toml`
//!   里写明的「生产主力」数据库。
//!
//! 2026-09-12 的 postgres 冒烟一次性抓到 **23 处**同类问题
//! （见 [`crate::dyn_where`] 的说明）；2026-10-24 第九轮又抓到 **1 处新引入**的：
//! `db::jt1078::mark_terminal_offline_if_stale` 里的
//! `SELECT id, update_time FROM gb_jt_terminal WHERE phone_number = ?` ——
//! 它让 postgres 上的「会话超时 ⇒ 终端置离线 ⇒ 需重新注册 + 重新鉴权」
//! **整条链路失效**：错误被 `jt1078::manager::cleanup_once` 吞成一条 warn 日志，
//! 终端于是永远显示在线，平台照样给它下发命令。
//!
//! ## 这条测试守什么
//!
//! 扫描 `src/` 下所有 `.rs`，找出**在 postgres 构建里会被编译**、且 SQL 里带
//! **裸 `?`** 的语句。它们必须满足其一：
//! - 被 [`crate::dyn_where::dialect_sql`] 包裹（自动改写成 `$1..$n`），**或**
//! - 位于 `#[cfg(feature = "mysql" / "sqlite")]` 分支里（postgres 构建根本不编译）。
//!
//! 以后任何新写的裸 `?`（忘了写 postgres 分支、或忘了包 `dialect_sql`）
//! 都会在这里变红 —— 不用再等某次 postgres 冒烟才想起来。

use std::path::{Path, PathBuf};

/// 一条字符串字面量。
struct Lit {
    /// 字面量在源码中的起始偏移
    pos: usize,
    /// 字面量内容（未做转义还原，够用）
    text: String,
}

/// 递归收集目录下的所有 `.rs` 文件。
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "target" || name.starts_with('.') {
                continue;
            }
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// 解析 `pos` 处的字符串字面量，返回 `(内容, 字面量结束后的位置)`。
///
/// 支持普通字符串（`"..."`，带 `\` 转义）与 raw string（`r"..."` / `r#"..."#`）。
fn parse_string_literal(src: &str, pos: usize) -> Option<(String, usize)> {
    let b = src.as_bytes();
    if pos >= b.len() {
        return None;
    }
    if b[pos] == b'r' {
        let mut i = pos + 1;
        let mut hashes = 0usize;
        while i < b.len() && b[i] == b'#' {
            hashes += 1;
            i += 1;
        }
        if i >= b.len() || b[i] != b'"' {
            return None;
        }
        let closing = format!("\"{}", "#".repeat(hashes));
        let end = src[i + 1..].find(&closing)? + i + 1;
        return Some((src[i + 1..end].to_string(), end + closing.len()));
    }
    if b[pos] != b'"' {
        return None;
    }
    let mut i = pos + 1;
    let mut text = String::new();
    while i < b.len() {
        match b[i] {
            b'\\' => {
                if i + 1 < b.len() {
                    text.push_str(&src[i..i + 2]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'"' => return Some((text, i + 1)),
            _ => {
                let ch = src[i..].chars().next()?;
                text.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

/// 找出源码里所有字符串字面量（跳过注释与字符字面量）。
fn find_string_literals(src: &str) -> Vec<Lit> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            let mut depth = 1usize;
            while i < b.len() && depth > 0 {
                if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                    continue;
                }
                i += 1;
            }
            continue;
        }
        let is_raw = b[i] == b'r' && i + 1 < b.len() && (b[i + 1] == b'"' || b[i + 1] == b'#');
        if is_raw || b[i] == b'"' {
            if let Some((text, end)) = parse_string_literal(src, i) {
                out.push(Lit { pos: i, text });
                i = end;
                continue;
            }
        }
        // 字符字面量：b'x' / 'x' / '\n'（跳过，避免把里面的引号当字符串起点）
        if b[i] == b'\'' {
            if i + 2 < b.len() && b[i + 1] == b'\\' && b[i + 2] == b'\'' {
                i += 4;
                continue;
            }
            if i + 2 < b.len() && b[i + 2] == b'\'' {
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 从 `start`（`{` 的位置）起找配对的 `}`，字符串/注释感知。
fn matching_brace(src: &str, open: usize) -> usize {
    let b = src.as_bytes();
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                if let Some((_, end)) = parse_string_literal(src, i) {
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    b.len()
}

/// 找到 `start` 之后**字符串/注释之外**的第一个 `{`（括号深度为 0）。
fn find_brace_outside_strings(src: &str, start: usize) -> Option<usize> {
    let b = src.as_bytes();
    let mut i = start;
    let mut pdepth = 0i32;
    while i < b.len() {
        match b[i] {
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                if let Some((_, end)) = parse_string_literal(src, i) {
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'(' => {
                pdepth += 1;
                i += 1;
            }
            b')' => {
                pdepth -= 1;
                i += 1;
            }
            b'{' if pdepth == 0 => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// 语句结尾：括号深度与花括号深度都为 0 时的 `;`。
fn statement_end(src: &str, start: usize) -> usize {
    let b = src.as_bytes();
    let mut pdepth = 0i32;
    let mut bdepth = 0i32;
    let mut i = start;
    while i < b.len() {
        match b[i] {
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                if let Some((_, end)) = parse_string_literal(src, i) {
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'(' => {
                pdepth += 1;
                i += 1;
            }
            b')' => {
                pdepth -= 1;
                i += 1;
            }
            b'{' => {
                bdepth += 1;
                i += 1;
            }
            b'}' => {
                bdepth -= 1;
                if bdepth < 0 {
                    return i;
                }
                i += 1;
            }
            b';' if pdepth == 0 && bdepth == 0 => return i,
            _ => i += 1,
        }
    }
    b.len()
}

/// `#[cfg(...)]` 属性支配的范围：`(条件文本, 属性起始, 支配范围结束)`。
///
/// 支配范围的形态有三种，都得认：
/// - `#[cfg(x)] { ... }` → 块
/// - `#[cfg(x)] let r = ...;` → 语句
/// - `#[cfg(x)] mod tests { ... }` → **项**（没有分号，按花括号配对）
fn cfg_spans(src: &str) -> Vec<(String, usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0usize;
    while let Some(idx) = src[from..].find("#[cfg") {
        let attr_start = from + idx;
        let after = attr_start + 5; // "#[cfg"
        let mut j = after;
        while j < src.len() && src.as_bytes()[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= src.len() || src.as_bytes()[j] != b'(' {
            from = attr_start + 5;
            continue;
        }
        let cond_start = j + 1;
        let mut depth = 1i32;
        let mut i = cond_start;
        while i < src.len() && depth > 0 {
            match src.as_bytes()[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        let cond = src[cond_start..i - 1].to_string();
        // 支配范围：从属性之后开始
        let mut k = i;
        while k < src.len() && src.as_bytes()[k].is_ascii_whitespace() {
            k += 1;
        }
        let end = if k < src.len() && src.as_bytes()[k] == b'{' {
            matching_brace(src, k)
        } else {
            let rest = &src[k..];
            let is_item = [
                "mod ",
                "fn ",
                "impl ",
                "struct ",
                "enum ",
                "trait ",
                "union ",
                "macro_rules!",
            ]
            .iter()
            .any(|kw| rest.starts_with(kw));
            if is_item {
                match find_brace_outside_strings(src, k) {
                    Some(ob) => matching_brace(src, ob),
                    None => statement_end(src, k),
                }
            } else {
                statement_end(src, k)
            }
        };
        spans.push((cond, attr_start, end));
        from = i;
    }
    spans
}

/// 最内层（起始位置最大且覆盖 `pos`）的 `#[cfg]` 条件。
fn governing<'a>(spans: &'a [(String, usize, usize)], pos: usize) -> Option<&'a str> {
    let mut best: Option<&(String, usize, usize)> = None;
    for s in spans {
        if s.1 <= pos && pos <= s.2 {
            if best.is_none() || s.1 > best.unwrap().1 {
                best = Some(s);
            }
        }
    }
    best.map(|s| s.0.as_str())
}

/// 该 `#[cfg]` 条件下，这段代码在 **postgres 构建**里会不会被编译。
fn compiled_in_postgres(cond: &str) -> bool {
    let c = cond.to_ascii_lowercase();
    let negated = c.trim_start().starts_with("not(");
    let core = if negated {
        // `not(feature = "mysql")` —— 在 postgres 下为真，语义相反
        c.trim_start()
            .trim_start_matches("not(")
            .trim_end_matches(')')
            .to_string()
    } else {
        c
    };
    if core.contains("postgres") {
        return !negated;
    }
    if core.contains("test") {
        return negated;
    }
    if core.contains("mysql") || core.contains("sqlite") {
        return negated;
    }
    !negated
}

/// 这段文字看起来像一条 SQL 语句吗（用来把注释里的 `?` 排除掉）。
fn looks_like_sql(text: &str) -> bool {
    let up = text.to_ascii_uppercase();
    up.contains("SELECT ")
        || up.contains("INSERT ")
        || up.contains("UPDATE ")
        || up.contains("DELETE ")
        || up.contains(" FROM ")
}

/// 这个字面量是不是被 `dialect_sql(...)` 包裹的？
fn wrapped_by_dialect_sql(src: &str, lit_pos: usize) -> bool {
    let head = &src[..lit_pos];
    let Some(idx) = head.rfind("dialect_sql(") else {
        return false;
    };
    let between = src[idx + "dialect_sql(".len()..lit_pos].trim();
    // `dialect_sql(` 与字面量之间只允许空白和 `&`
    between.is_empty() || between == "&"
}

#[test]
fn no_raw_question_mark_sql_in_postgres_build() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_root = manifest.join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_root, &mut files);
    assert!(
        !files.is_empty(),
        "没找到任何 .rs 源文件（CARGO_MANIFEST_DIR={}）—— 守卫形同虚设",
        manifest.display()
    );

    let mut violations: Vec<String> = Vec::new();
    for path in &files {
        // 守卫自身跳过（它满是含 `?` 的示例字面量）
        if path.file_name().and_then(|s| s.to_str()) == Some("dialect_guard.rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        // 整个文件都是测试专用（`#![cfg(test)]` / `#![cfg(all(test, ...))]`）
        if src.contains("#![cfg(test)]") || src.contains("#![cfg(all(test") {
            continue;
        }
        let spans = cfg_spans(&src);
        for lit in find_string_literals(&src) {
            if !lit.text.contains('?') || !looks_like_sql(&lit.text) {
                continue;
            }
            if wrapped_by_dialect_sql(&src, lit.pos) {
                continue;
            }
            let cond = governing(&spans, lit.pos);
            if let Some(c) = cond {
                if !compiled_in_postgres(c) {
                    continue; // 这段代码在 postgres 构建里不存在
                }
            }
            let line = src[..lit.pos].matches('\n').count() + 1;
            let rel = path.strip_prefix(manifest).unwrap_or(path);
            let head = lit.text.lines().next().unwrap_or("").trim();
            violations.push(format!("{}:{}  {}", rel.display(), line, head));
        }
    }

    assert!(
        violations.is_empty(),
        "发现 {} 处**在 postgres 构建下会被编译**、却仍用裸 `?` 占位符的 SQL —— \
         postgres 只认 `$1`，直接发 `?` 会得到 `syntax error`，该语句必然执行失败。\n\
         修法二选一：\n  1) 用 `crate::dyn_where::dialect_sql(...)` 包起来（自动改写成 `$1..$n`）；\n\
         2) 写成 `#[cfg(feature = \"postgres\")]` 与 `#[cfg(any(feature = \"mysql\", feature = \"sqlite\"))]` \
         两份，postgres 那份用 `$1..$n`。\n\
         命中清单：\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
}
