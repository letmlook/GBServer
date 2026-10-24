#!/usr/bin/env python3
"""重写「`RETURNING id` 三方言差异」的函数。

`RETURNING id` 只有 PostgreSQL / SQLite(≥3.35) 支持，**MySQL 不支持** ——
MySQL 走 `execute()` 的自增 id（`last_insert_id()`）。

改造前这三个方言各写一份（编译期三选一），这里合并成
「MySQL 一支 + 其余两支共用 RETURNING」。
"""
import re
import subprocess
import sys

TPL = '''    // `RETURNING id` 只有 PostgreSQL / SQLite(≥3.35) 支持，**MySQL 不支持**
    // —— MySQL 走 `execute()` 的自增 id。
    //
    // 改造前这里是三个 `#[cfg(feature = ...)]` 分支（编译期三选一），
    // 现在按**运行期方言**分支。
    if crate::db::dialect::current().is_mysql() {
        let sql = crate::db::dialect::current_sql(
            "@MYSQL_SQL@",
        )
        .into_owned();
        let r = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
@MYSQL_BINDS@
                .execute(p)
        })?;
        // MySQL 的自增 id 走 `last_insert_id()`（方言专有 API）
        return Ok(r.last_insert_id().unwrap_or(0) as @RT@);
    }

    let sql = crate::db::dialect::current_sql(
        "@PG_SQL@",
    )
    .into_owned();
    let row: (@RT@,) = crate::dispatch!(pool, |p| {
        sqlx::query_as(&sql)
@PG_BINDS@
            .fetch_one(p)
    })?;
    Ok(row.0)
}
'''


def orig(path):
    return subprocess.run(['git', 'show', 'HEAD~1:' + path],
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
    print('  OK %s %s' % (path, sig))


def sig_of(fn_text):
    out = []
    for l in fn_text.split('\n'):
        out.append(l)
        if l.rstrip().endswith('{'):
            break
    return out


def branch(fn_text, dialect):
    """取某个方言分支里的 SQL 字面量与 bind 链。"""
    m = re.search(r'#\[cfg\(feature = "%s"\)\]' % dialect, fn_text)
    if not m:
        return None, None
    seg = fn_text[m.end():]
    nxt = re.search(r'#\[cfg\(', seg)
    if nxt:
        seg = seg[:nxt.start()]
    lits = re.findall(r'"((?:[^"\\]|\\.)*)"', seg)
    sql = lits[0] if lits else None
    binds = re.findall(r'\.bind\(([^)]*)\)', seg)
    return sql, binds


def build(path, sig, fn_pat):
    t = orig(path)
    m = re.search(fn_pat, t, re.S)
    assert m, (path, fn_pat[:60])
    start = m.start()
    rest = t[start:]
    fm = re.match(r'(?:pub\s+)?(?:async\s+)?fn\s+\w+\s*\([^)]*\)[^{]*\{', rest, re.S)
    assert fm, (path, '函数签名')
    depth = 0
    end = None
    for k in range(fm.end() - 1, len(rest)):
        c = rest[k]
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0:
                end = k + 1
                break
    assert end, (path, '函数结束')
    fn = rest[:end]

    my_sql, my_binds = branch(fn, 'mysql')
    pg_sql, pg_binds = branch(fn, 'postgres')
    assert my_sql and pg_sql, (path, 'SQL 未取到')
    pg_sql = re.sub(r'\$\d+', '?', pg_sql)

    rt = 'i64'
    mret = re.search(r'->\s*sqlx::Result<(\w+)>', fn)
    if mret:
        rt = mret.group(1)

    head = sig_of(fn)
    body = (TPL
            .replace('@MYSQL_SQL@', my_sql)
            .replace('@MYSQL_BINDS@', '\n'.join('            .bind(%s)' % b for b in my_binds))
            .replace('@PG_SQL@', pg_sql)
            .replace('@PG_BINDS@', '\n'.join('            .bind(%s)' % b for b in pg_binds))
            .replace('@RT@', rt))
    replace_fn(path, sig, '\n'.join(head) + '\n' + body)


def main():
    build('src/db/platform_region.rs', 'pub async fn add(',
          r'pub async fn add\(.*?\n\}\n')
    build('src/db/platform_group.rs', 'pub async fn add(',
          r'pub async fn add\(.*?\n\}\n')
    build('src/db/role.rs', 'pub async fn add(',
          r'pub async fn add\(.*?\n\}\n')
    return 0


if __name__ == '__main__':
    sys.exit(main())
