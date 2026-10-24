#!/usr/bin/env python3
"""已验证的**手工修复**集合（脚本化的手工改动，可重放）。

每条都对应一个具体的编译错误形态，注释里写清「根因 → 改法」。
"""
import re
import sys
import pathlib

APPLY = '--apply' in sys.argv
n = 0


def sub(path, old, new, label, count=1):
    global n
    p = pathlib.Path(path)
    t = p.read_text()
    if old not in t:
        print('  ! 未匹配：%s (%s)' % (path, label))
        return
    p.write_text(t.replace(old, new, count))
    n += 1
    print('  ✓ %s：%s' % (path, label))


# --- 1) SQLite 的 `?1/?2` 编号占位符统一成 `?` -----------------------------
# 根因：SQLite 特有的编号占位符在「三方言合一」时没被统一，
#       而合并后的分支里 `?1` 会被当成**字面量**绑定。
# 改法：三种方言都按顺序绑定 `?`，语义一致。
sub('src/db/user.rs', '''            let sql = if crate::db::dialect::current().is_() {
                let sql = "SELECT COUNT(*) FROM gb_user WHERE username LIKE ?1";
            } else {
                let sql = "SELECT COUNT(*) FROM gb_user WHERE username LIKE ?";
            }''',
    '''            // SQLite 的 `?1` 编号占位符统一成 `?` —— 三种方言按顺序绑定，语义一致。
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username LIKE ?";''',
    'username LIKE ?1 → ?')

sub('src/db/user.rs', '''    let sql = if crate::db::dialect::current().is_() {
        let sql = "SELECT COUNT(*) FROM gb_user WHERE role_id = ?1";
    } else {
        let sql = "SELECT COUNT(*) FROM gb_user WHERE role_id = ?";
    }''',
    '''    // SQLite 的 `?1` 编号占位符统一成 `?` —— 三种方言按顺序绑定，语义一致。
    let sql = "SELECT COUNT(*) FROM gb_user WHERE role_id = ?";''',
    'role_id = ?1 → ?')

sub('src/db/user.rs', '''            let sql = if crate::db::dialect::current().is_() {
                let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ?1 AND id != ?2";
            } else {
                let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ? AND id != ?";
            }''',
    '''            // SQLite 的 `?1/?2` 编号占位符统一成 `?` —— 三种方言按顺序绑定。
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ? AND id != ?";''',
    'username = ?1 AND id != ?2 → ?')

sub('src/db/user.rs', '''            let sql = if crate::db::dialect::current().is_() {
                let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ?1";
            } else {
                let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ?";
            }''',
    '''            // SQLite 的 `?1` 编号占位符统一成 `?` —— 三种方言按顺序绑定。
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ?";''',
    'username = ?1 → ?')

# --- 2) device.rs：count_expr 的方言分支 + 缺失的类型标注冒号 --------------
sub('src/db/device.rs', '''    let count_expr = if crate::db::dialect::current().is_() {
        let count_expr = "(SELECT COUNT(*)::INTEGER FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count";
    } else {
        let count_expr = "(SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count";
    }''',
    '''    let count_expr = if crate::db::dialect::current().is_postgres() {
        // PostgreSQL 需要显式 cast（COUNT(*) 是 bigint，Rust 侧按 i32 解）
        "(SELECT COUNT(*)::INTEGER FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count"
    } else {
        "(SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count"
    };''',
    'count_expr 方言分支')

# 根因：拆分 body 时类型标注的冒号被丢掉（`let rows: Vec<..>` → `let rows Vec<..>`）
p = pathlib.Path('src/db/device.rs')
t = p.read_text()
t2 = t.replace('let rows Vec<(String, i32)> = crate::dispatch!',
                'let rows: Vec<(String, i32)> = crate::dispatch!')
if t2 != t:
    p.write_text(t2)
    n += 1
    print('  ✓ src/db/device.rs：补回类型标注的冒号')

# --- 3) role.rs：残留的 cfg 属性 + add() 重写 -------------------------------
p = pathlib.Path('src/db/role.rs')
t = p.read_text()
t2 = re.sub(r'\n *#\[cfg\(feature = "(mysql|postgres|sqlite)"\)\]\n', '\n', t)
if t2 != t:
    p.write_text(t2)
    n += 1
    print('  ✓ src/db/role.rs：清掉语句位置的残留 cfg 属性')

ROLE_ADD = '''pub async fn add(pool: &Pool, role: &RoleCreate) -> sqlx::Result<i32> {
    // `RETURNING id` 只有 PostgreSQL / SQLite(≥3.35) 支持，**MySQL 不支持**
    // —— MySQL 走 `execute()` 的自增 id。
    //
    // 改造前这里是三个 `#[cfg(feature = ...)]` 分支（编译期三选一），
    // 现在按**运行期方言**分支。
    if crate::db::dialect::current().is_mysql() {
        let sql = crate::db::dialect::current_sql(
            "INSERT INTO gb_user_role (name, authority, create_time, update_time) \\
             VALUES (?, ?, ?, ?)",
        )
        .into_owned();
        let r = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(&role.name)
                .bind(&role.authority)
                .bind(&role.create_time)
                .bind(&role.create_time)
                .execute(p)
        })?;
        // MySQL 的自增 id 走 `last_insert_id()`（方言专有 API）
        return Ok(r.last_insert_id().unwrap_or(0) as i32);
    }

    let sql = crate::db::dialect::current_sql(
        "INSERT INTO gb_user_role (name, authority, create_time, update_time) \\
         VALUES (?, ?, ?, ?) RETURNING id",
    )
    .into_owned();
    let row: (i32,) = crate::dispatch!(pool, |p| {
        sqlx::query_as(&sql)
            .bind(&role.name)
            .bind(&role.authority)
            .bind(&role.create_time)
            .bind(&role.create_time)
            .fetch_one(p)
    })?;
    Ok(row.0)
}
'''
p = pathlib.Path('src/db/role.rs')
t = p.read_text()
if 'pub async fn add(pool: &Pool, role: &RoleCreate)' in t:
    start = t.index('pub async fn add(pool: &Pool, role: &RoleCreate)')
    end = t.index('\n}\n', start) + 3
    p.write_text(t[:start] + ROLE_ADD + t[end:])
    n += 1
    print('  ✓ src/db/role.rs：add() 重写为运行期方言分支')

# --- 4) audit_log.rs：`let _ = ...` 放在 dispatch 宏体内 ---------------------
# 根因：宏体做 `($q).await`，宏体必须是**一个表达式**；
#       `let _ = <expr>;` 的值是 `()` → 宏去 await 一个 `()`。
sub('src/db/audit_log.rs', '''    return crate::dispatch_exec!(pool, |p| {  
                let _ = sqlx::query(&sql)
                .bind(username).bind(action).bind(resource).bind(method).bind(path).bind(ip).bind(status_code)
                .execute(p) });
    Ok(())''',
    '''    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(username)
            .bind(action)
            .bind(resource)
            .bind(method)
            .bind(path)
            .bind(ip)
            .bind(status_code)
            .execute(p)
    })?;
    Ok(())''',
    '宏体内的 let _ = 改成表达式')

print('共应用 %d 处' % n)
