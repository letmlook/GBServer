#!/usr/bin/env python3
"""手工重写：**脚本无法自动合并**的方言分支函数。

统一策略：占位符统一用 `?`，由 `pool.sql()` 在运行期按方言改写（postgres → `$N`）。
"""
import re
import subprocess
import sys


def orig(path):
    return subprocess.run(['git', 'show', 'HEAD:' + path],
                          capture_output=True, text=True, check=True).stdout


def replace_fn(path, sig, new_fn_text):
    lines = open(path, encoding='utf-8').read().split('\n')
    start = None
    for i, l in enumerate(lines):
        if l.startswith(sig):
            start = i
            break
    assert start is not None, (path, sig)
    end = None
    for j in range(start + 1, len(lines)):
        s = lines[j]
        if s and not s[0].isspace() and re.match(
                r'(pub\s+)?(async\s+)?(fn|struct|enum|impl|const|mod|type)\b', s):
            end = j
            break
    assert end is not None, (path, sig)
    lines[start:end] = new_fn_text.split('\n')
    open(path, 'w', encoding='utf-8').write('\n'.join(lines))
    print('  ✓ %s %s' % (path, sig))


def sig_of(fn_text):
    """函数签名部分（含 `{`）。"""
    out = []
    for l in fn_text.split('\n'):
        out.append(l)
        if l.rstrip().endswith('{'):
            break
    return out


def build_update(tmpl, binds, table, prefix='UPDATE '):
    new_tmpl = re.sub(r'\{p\d+\}', '?', tmpl)
    bind_lines = '\n'.join('            .bind(%s)' % b for b in binds)
    return '''    let set = "%s";
    let stmt = format!("%s%s {}", set);
    let sql = pool.sql(&stmt).into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
%s
            .execute(p)
    })?;
    Ok(r.rows_affected())
}''' % (new_tmpl, prefix, table, bind_lines)


def gen_from_orig(path, fn_pat, table, prefix='UPDATE '):
    t = orig(path)
    m = re.search(fn_pat, t, re.S)
    assert m, (path, fn_pat[:50])
    fn = m.group(0)
    tm = re.search(r'let set =\s*"(.*?)";', fn, re.S)
    assert tm, (path, 'set 模板')
    binds = re.findall(r'\.bind\(([^)]*)\)', fn)
    head = sig_of(fn)
    return '\n'.join(head) + '\n' + build_update(tm.group(1), binds, table, prefix)


def main():
    # 1) device.rs::update_device
    new = gen_from_orig(
        'src/db/device.rs',
        r"pub async fn update_device\(\s*pool: &Pool,\s*device_id: &str,\s*f: &DeviceWriteFields<'_>,\s*now: &str,\s*\) -> sqlx::Result<u64> \{.*?\n\}\n",
        'gb_device')
    replace_fn('src/db/device.rs', 'pub async fn update_device(', new)

    # 2) jt1078.rs::update_terminal
    new = gen_from_orig(
        'src/db/jt1078.rs',
        r"pub async fn update_terminal\(\s*pool: &Pool,.*?\n\}\n",
        'gb_jt_terminal')
    replace_fn('src/db/jt1078.rs', 'pub async fn update_terminal(', new)

    # 3) common_channel.rs::update
    new = gen_from_orig(
        'src/db/common_channel.rs',
        r"pub async fn update\(\s*pool: &Pool,\s*id: i64,\s*f: &ChannelWriteFields<'_>,\s*now: &str,\s*\) -> sqlx::Result<u64> \{.*?\n\}\n",
        'gb_device_channel')
    replace_fn('src/db/common_channel.rs', 'pub async fn update(', new)

    # 4) platform_channel.rs::update（动态 SET 子句）
    pc_new = '''pub async fn update(
    pool: &Pool,
    id: i64,
    custom_name: Option<&str>,
    custom_info: Option<&str>,
) -> sqlx::Result<u64> {
    if custom_name.is_none() && custom_info.is_none() {
        return Ok(0);
    }

    // 动态拼 SET 子句：三种方言的占位符统一用 `?`，由 `pool.sql()` 在运行期
    // 改写（postgres → `$N`）。
    //
    // 改造前 postgres 走的是「固定 3 个参数 + `$1..$3`」的 match 分支，
    // sqlite / mysql 走动态 SET；两条路的**语义相同**（只更新传入的字段），
    // 这里统一成动态 SET。
    let mut updates: Vec<&str> = Vec::new();
    if custom_name.is_some() {
        updates.push("custom_name = ?");
    }
    if custom_info.is_some() {
        updates.push("custom_address = ?");
    }
    if updates.is_empty() {
        return Ok(0);
    }
    let stmt = format!(
        "UPDATE gb_platform_channel SET {} WHERE id = ?",
        updates.join(", ")
    );
    let sql = pool.sql(&stmt).into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        let mut query = sqlx::query(&sql);
        if let Some(name) = custom_name {
            query = query.bind(name);
        }
        if let Some(info) = custom_info {
            query = query.bind(info);
        }
        query = query.bind(id);
        query.execute(p)
    })?;
    Ok(r.rows_affected())
}
'''
    replace_fn('src/db/platform_channel.rs', 'pub async fn update(', pc_new)

    # 5) audit_log.rs::ensure_table（DDL 按运行期方言选）
    t = orig('src/db/audit_log.rs')
    m = re.search(r'pub async fn ensure_table\(pool: &Pool\) -> sqlx::Result<\(\)> \{.*?\n\}\n', t, re.S)
    assert m
    fn = m.group(0)
    ddls = re.findall(r'r#"CREATE TABLE IF NOT EXISTS gb_audit_log \(.*?\)"#', fn, re.S)
    assert len(ddls) == 3, ddls
    # 原文顺序：postgres / mysql / sqlite
    pg, my, sq = ddls
    new = '''pub async fn ensure_table(pool: &Pool) -> sqlx::Result<()> {
    // 建表 DDL 按**运行期方言**选（改造前是三个 `#[cfg(feature = ...)]` 块，
    // 编译期三选一）。
    let ddl = match pool.dialect() {
        crate::db::Dialect::Postgres => %s,
        crate::db::Dialect::MySql => %s,
        crate::db::Dialect::Sqlite => %s,
    };

    if let Err(e) = crate::dispatch_exec!(pool, |p| {
        sqlx::query(ddl).execute(p)
    }) {
        tracing::error!("创建 gb_audit_log 表失败（审计日志将不可用）: {}", e);
    }

    // 索引是可选优化：建不出来只告警。
    //
    // **MySQL 的 `CREATE INDEX` 不支持 `IF NOT EXISTS`**（会报
    // `1064 ... near 'IF NOT EXISTS idx_...'` 语法错误 —— 只有 MariaDB 支持）。
    // 因此这里不带 `IF NOT EXISTS`，把重复键（1061）当成幂等重放的正常结果。
    // 实测：2026-09-12 首次在真实 MySQL 上跑后端时，两条索引都因语法错误
    //       没建出来（只有 WARN），审计日志表因此长期缺索引。
    let idx_create_time = if pool.dialect().is_mysql() {
        "CREATE INDEX idx_audit_log_create_time ON gb_audit_log(create_time)"
    } else {
        "CREATE INDEX IF NOT EXISTS idx_audit_log_create_time ON gb_audit_log(create_time)"
    };
    if let Err(e) = crate::dispatch_exec!(pool, |p| {
        sqlx::query(idx_create_time).execute(p)
    }) {
        tracing::warn!("创建审计日志索引 idx_audit_log_create_time 失败: {}", e);
    }

    let idx_username = if pool.dialect().is_mysql() {
        "CREATE INDEX idx_audit_log_username ON gb_audit_log(username)"
    } else {
        "CREATE INDEX IF NOT EXISTS idx_audit_log_username ON gb_audit_log(username)"
    };
    let _ = crate::dispatch_exec!(pool, |p| {
        sqlx::query(idx_username).execute(p)
    });

    // `CREATE TABLE IF NOT EXISTS` **不会**给已存在的表补列，而
    // `insert_with_metrics` 写的是带 `elapsed_ms` 的扩展列 —— 列不存在时
    // 每个被审计的请求都会先失败一次再回退（postgres 日志里每请求一条
    // ERROR），而且耗时根本没落库。这里补上。
    //
    // MySQL 不支持 `ADD COLUMN IF NOT EXISTS`，重复列的 1060/1061 当成
    // 幂等重放的正常结果忽略掉。
    let add_elapsed = if pool.dialect().is_mysql() {
        "ALTER TABLE gb_audit_log ADD COLUMN elapsed_ms BIGINT"
    } else {
        "ALTER TABLE gb_audit_log ADD COLUMN IF NOT EXISTS elapsed_ms BIGINT"
    };
    if let Err(e) = crate::dispatch_exec!(pool, |p| {
        sqlx::query(add_elapsed).execute(p)
    }) {
        let msg = e.to_string();
        if !msg.contains("1060") && !msg.contains("1061") && !msg.contains("Duplicate") {
            tracing::warn!("为 gb_audit_log 补 elapsed_ms 列失败: {}", e);
        }
    }
    Ok(())
}
''' % (pg, my, sq)
    replace_fn('src/db/audit_log.rs', 'pub async fn ensure_table(', new)
    return 0


if __name__ == '__main__':
    sys.exit(main())
