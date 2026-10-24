#!/usr/bin/env python3
"""修正 ExecResult 的 last_insert_id 语义。

各方言的自增 id 是**方言专有 API**：
- MySQL：`MySqlQueryResult::last_insert_id()` → u64
- SQLite：`SqliteQueryResult::last_insert_rowid()` → i64
- PostgreSQL：**没有** → 用 `INSERT ... RETURNING id`

调用点若把 `ExecResult` 当成 sqlite 的 `QueryResult` 用（调
`last_insert_rowid()`），或把 `Option<i64>` 直接 `as i64`，都要修。
"""
import re
import pathlib

n = 0
for p in sorted(pathlib.Path('src').rglob('*.rs')):
    if str(p).endswith('src/db/mod.rs'):
        continue
    s = p.read_text()
    o = s
    s = s.replace('.last_insert_rowid() as i64', '.last_insert_id().unwrap_or(0) as i64')
    s = s.replace('.last_insert_rowid() as i32', '.last_insert_id().unwrap_or(0) as i32')
    s = re.sub(r'Ok\((r|result)\.last_insert_rowid\(\)\)', r'Ok(\1.last_insert_id())', s)
    if s != o:
        p.write_text(s)
        n += 1
print('改了 %d 个文件' % n)

# 宏里 sqlite 分支要用 last_insert_rowid（不是 last_insert_id）
p = pathlib.Path('src/db/mod.rs')
if p.exists():
    s = p.read_text()
    s = s.replace("                last_insert_id: Some(r.last_insert_id().unwrap_or(0) as i64),",
                  "                last_insert_id: Some(r.last_insert_id() as i64),")
    s = s.replace("""            $crate::db::Pool::Sqlite($p) => ($q).await.map(|r| $crate::db::ExecResult {
                rows_affected: r.rows_affected(),
                last_insert_id: Some(r.last_insert_id()),
            }),""",
"""            $crate::db::Pool::Sqlite($p) => ($q).await.map(|r| $crate::db::ExecResult {
                rows_affected: r.rows_affected(),
                last_insert_id: Some(r.last_insert_rowid()),
            }),""")
    p.write_text(s)
    print('  宏的 mysql/sqlite 分支已校正')
