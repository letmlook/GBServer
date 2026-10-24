#!/usr/bin/env python3
"""已验证有效的**收尾修复**集合（可重放）。

每条都对应一个具体的编译错误形态，注释里写清「根因 → 改法」。
"""
import re
import sys
import pathlib

APPLY = '--apply' in sys.argv
n = 0


def edit(path, pairs, label):
    """pairs: [(old, new)] —— 逐条替换。"""
    global n
    p = pathlib.Path(path)
    t = p.read_text()
    hit = 0
    for old, new in pairs:
        if old in t:
            t = t.replace(old, new, 1)
            hit += 1
    if hit:
        p.write_text(t)
        n += hit
        print('  ✓ %s：%s（%d 处）' % (path, label, hit))


# --- 1) `is_False()`：cfg(not(feature = "X")) 被转成了一个不存在的方言方法 -----
# 根因：not(feature) 的翻译把方法名留空了 → `current().is_False()`。
# 改法：jt1078 的 insert_area_* 系列是「非 MySQL」（MySQL 没有 RETURNING），
#       common_channel 的 data_device_id 是「非 PostgreSQL」（pg 需要 $1::text）。
edit('src/db/jt1078.rs', [
    ('crate::db::dialect::current().is_False()',
     '!crate::db::dialect::current().is_mysql()'),
], 'is_False() → !is_mysql()')

edit('src/handlers/common_channel.rs', [
    ('crate::db::dialect::current().is_False()',
     '!crate::db::dialect::current().is_postgres()'),
], 'is_False() → !is_postgres()')

# --- 2) `let X (TYPE) =` 缺类型标注的冒号 ------------------------------------
# 根因：拆分 body 时把 `let result: (i64,) = ...` 的冒号丢了。
for f in ('src/db/alarm.rs', 'src/db/cloud_record.rs', 'src/db/mobile_position.rs',
          'src/db/role.rs'):
    p = pathlib.Path(f)
    if not p.exists():
        continue
    t = p.read_text()
    t2 = re.sub(r'\blet\s+([A-Za-z_][A-Za-z0-9_]*)\s+(\([^=;]*?\))\s*=\s*(crate::dispatch)',
                 r'let \1: \2 = \3', t)
    if t2 != t:
        p.write_text(t2)
        n += 1
        print('  ✓ %s：补回 let 类型标注的冒号' % f)

# --- 3) `last_insert_id()` 返回 Option<i64>，不能直接 `as i64` -----------------
for f in ('src/db/role.rs', 'src/db/cloud_record.rs', 'src/db/common_channel.rs',
          'src/db/alarm.rs', 'src/db/mobile_position.rs', 'src/db/jt1078.rs'):
    p = pathlib.Path(f)
    if not p.exists():
        continue
    t = p.read_text()
    t2 = re.sub(r'\.(last_insert_id\(\)) as (i32|i64)', r'.\1.unwrap_or(0) as \2', t)
    t2 = t2.replace('Ok(result.last_insert_id())', 'Ok(result.last_insert_id().unwrap_or(0))')
    t2 = t2.replace('Ok(r.last_insert_id())', 'Ok(r.last_insert_id().unwrap_or(0))')
    if t2 != t:
        p.write_text(t2)
        n += 1
        print('  ✓ %s：last_insert_id 解包' % f)

# --- 4) seq_sync.rs：pg 专属的纯函数被错转成 #[cfg(test)] --------------------
# 根因：`#[cfg(any(feature = "postgres", test))]` 转成了 `#[cfg(test)]`，
#       生产构建里 quote_ident / regclass_literal 不存在 → 编译不过。
edit('src/db/seq_sync.rs', [
    ('#[cfg(test)]\nfn quote_ident(', '#[allow(dead_code)]\nfn quote_ident('),
    ('#[cfg(test)]\nfn regclass_literal(', '#[allow(dead_code)]\nfn regclass_literal('),
], 'pg 专属纯函数去掉 test 门控')

# --- 5) handlers/platform.rs：模块别名 ---------------------------------------
# 原文 `use crate::db::platform as platform_db;` —— 用别名，别写全路径。
edit('src/handlers/platform.rs', [
    ('crate::db::platform::update(', 'platform_db::update('),
], 'platform_db 别名')

# --- 6) scheduler/record_plan.rs：方法内的 pool 是 &self.pool ----------------
p = pathlib.Path('src/scheduler/record_plan.rs')
t = p.read_text()
t2 = t.replace('.fetch_all(&self.p)', '.fetch_all(p)')
t2 = t2.replace('crate::dispatch!(pool, |p| {', 'crate::dispatch!(&self.pool, |p| {')
if t2 != t:
    p.write_text(t2)
    n += 1
    print('  ✓ src/scheduler/record_plan.rs：pool 引用改为 &self.pool')

# --- 7) handlers/alarm.rs：查询构造必须与执行同在 dispatch 内 ----------------
edit('src/handlers/alarm.rs', [
    ('''    let mut query = sqlx::query_as::<_, AlarmRow>(&sql_rows);
    for b in &w.binds {
        query = match b {
            BindValue::Text(v) => query.bind(v.as_str()),
            BindValue::Int(v) => query.bind(*v),
            BindValue::Big(v) => query.bind(*v),
            BindValue::Bool(v) => query.bind(*v),
        };
    }
    let rows: Vec<AlarmRow> = crate::dispatch!(&state.pool, |p| { 
        query
            .bind(count as i64)
            .bind(offset)
            .fetch_all(p)
     })''',
     '''    let rows: Vec<AlarmRow> = crate::dispatch!(&state.pool, |p| {
        // 查询构造必须在 dispatch **内部**：`sqlx::query_as` 的类型随执行器变化。
        let mut query = sqlx::query_as::<_, AlarmRow>(&sql_rows);
        for b in &w.binds {
            query = match b {
                BindValue::Text(v) => query.bind(v.as_str()),
                BindValue::Int(v) => query.bind(*v),
                BindValue::Big(v) => query.bind(*v),
                BindValue::Bool(v) => query.bind(*v),
            };
        }
        query
            .bind(count as i64)
            .bind(offset)
            .fetch_all(p)
    })'''),
], 'query 构造搬进 dispatch')

print('共应用 %d 处' % n)
