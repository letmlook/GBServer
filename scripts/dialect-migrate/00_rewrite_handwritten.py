#!/usr/bin/env python3
"""把 src/lib.rs 的「编译期方言分支」改写成「运行期按 dialect 分支」。

基于 git HEAD 的原文重新生成（脚本化转换破坏了结构，这里做定向重写）。
"""
import re
import sys

SRC = '/tmp/lib_orig.rs'
DST = 'src/lib.rs'

src = open(SRC, encoding='utf-8').read()
lines = src.split('\n')


def find_block(lines, start_idx):
    """start_idx 处是 `{` 开头，返回配对 `}` 的行号。

    必须**逐字符**统计花括号：`if x {` 这类行尾开括号同样要计数，
    只认「整行只有 {」会把块提前截断（改造第一版就栽在这里）。
    字符串与行注释里的花括号要跳过。
    """
    depth = 0
    for i in range(start_idx, len(lines)):
        line = lines[i]
        in_str = None
        j = 0
        while j < len(line):
            c = line[j]
            if in_str:
                if c == '\\':
                    j += 2
                    continue
                if c == in_str:
                    in_str = None
                j += 1
                continue
            if c == '"':
                in_str = c
                j += 1
                continue
            if c == '/' and j + 1 < len(line) and line[j + 1] == '/':
                break
            if c == '{':
                depth += 1
            elif c == '}':
                depth -= 1
                if depth == 0:
                    return i
            j += 1
    raise RuntimeError('no matching brace')


def replace_once(text, old, new, label):
    if old not in text:
        print('  ✗ 未匹配:', label)
        sys.exit(1)
    return text.replace(old, new, 1)


def sub_once(text, old, new, label):
    return replace_once(text, old, new, label)


print('重写 src/lib.rs ...')

# ---------------------------------------------------------------- 1. SCHEMA_SQL
text = src
text = replace_once(text, '''#[cfg(feature = "postgres")]
const SCHEMA_SQL: &str = include_str!("../database/init-postgresql-2.7.4.sql");
#[cfg(feature = "mysql")]
const SCHEMA_SQL: &str = include_str!("../database/init-mysql-2.7.4.sql");
#[cfg(feature = "sqlite")]
const SCHEMA_SQL: &str = include_str!("../database/init-sqlite-2.7.4.sql");''',
'''/// 当前方言对应的 schema 脚本。
///
/// 改造前这里是三个 `#[cfg(feature = ...)]` 常量（编译期三选一）；
/// 现在三种方言**同时编进同一个二进制**，由运行期的 [`db::Dialect`] 决定用哪一份。
fn schema_sql(dialect: db::Dialect) -> &'static str {
    match dialect {
        db::Dialect::Postgres => include_str!("../database/init-postgresql-2.7.4.sql"),
        db::Dialect::MySql => include_str!("../database/init-mysql-2.7.4.sql"),
        db::Dialect::Sqlite => include_str!("../database/init-sqlite-2.7.4.sql"),
    }
}''', 'SCHEMA_SQL 常量')

text = replace_once(text, "for raw in SCHEMA_SQL.split(';') {",
                    "for raw in schema_sql(pool.dialect()).split(';') {", 'ensure_missing_tables 用 SCHEMA_SQL')

# ---------------------------------------------------------------- 2. ensure_missing_tables 的执行
text = replace_once(text, '''        if let Err(e) = sqlx::query(stmt).execute(pool).await {
            let msg = e.to_string();''',
'''        let res = crate::dispatch!(pool, |p| {
            sqlx::query(stmt).execute(p)
        })
        .await;
        if let Err(e) = res {
            let msg = e.to_string();''', 'ensure_missing_tables 执行')

# ---------------------------------------------------------------- 3. PgColumnFix
text = replace_once(text, '''#[cfg(feature = "postgres")]
struct PgColumnFix {''', '''struct PgColumnFix {''', 'PgColumnFix struct')

text = replace_once(text, '''#[cfg(feature = "postgres")]
async fn ensure_pg_column_types(pool: &db::Pool) -> anyhow::Result<()> {
    const FIXES''', '''async fn ensure_pg_column_types(pool: &db::Pool) -> anyhow::Result<()> {
    // 列宽纠正是 postgres 独有的（只有它严格校验列类型），其它方言直接跳过。
    if !pool.dialect().is_postgres() {
        return Ok(());
    }
    const FIXES''', 'ensure_pg_column_types 头部')

text = replace_once(text, '''        let data_type: Option<String> = sqlx::query_scalar(
            "SELECT data_type FROM information_schema.columns \\
             WHERE table_schema = current_schema() AND table_name = $1 AND column_name = $2",
        )
        .bind(fix.table)
        .bind(fix.column)
        .fetch_optional(pool)
        .await?;''',
'''        let data_type: Option<String> = crate::dispatch!(pool, |p| {
            sqlx::query_scalar(
                "SELECT data_type FROM information_schema.columns \\
                 WHERE table_schema = current_schema() AND table_name = ? AND column_name = ?",
            )
            .bind(fix.table)
            .bind(fix.column)
            .fetch_optional(p)
        })
        .await?;''', 'pg data_type 查询')

text = replace_once(text, '''        sqlx::query(&format!(
            "ALTER TABLE {} ALTER COLUMN {} TYPE {}{}",
            fix.table, fix.column, fix.target, using
        ))
        .execute(pool)
        .await?;''',
'''        let alter_sql = format!(
            "ALTER TABLE {} ALTER COLUMN {} TYPE {}{}",
            fix.table, fix.column, fix.target, using
        );
        crate::dispatch!(pool, |p| {
            sqlx::query(&alter_sql).execute(p)
        })
        .await?;''', 'pg ALTER TABLE')

text = replace_once(text, '''            if let Err(e) = sqlx::query(&format!("ALTER SEQUENCE IF EXISTS {seq} AS bigint"))
                .execute(pool)
                .await
            {
                tracing::warn!("[schema][pg] 序列 {seq} 提升为 bigint 失败（已忽略）: {e}");
            }''',
'''            let seq_sql = format!("ALTER SEQUENCE IF EXISTS {seq} AS bigint");
            let res = crate::dispatch!(pool, |p| {
                sqlx::query(&seq_sql).execute(p)
            })
            .await;
            if let Err(e) = res {
                tracing::warn!("[schema][pg] 序列 {seq} 提升为 bigint 失败（已忽略）: {e}");
            }''', 'pg ALTER SEQUENCE')

text = replace_once(text, '''
/// 非 postgres 构建下的空实现，保持 `init_db_tables` 单一调用点。
#[cfg(not(feature = "postgres"))]
async fn ensure_pg_column_types(_pool: &db::Pool) -> anyhow::Result<()> {
    Ok(())
}
''', '\n', '删除 pg 空实现')

# ---------------------------------------------------------------- 4. mysql 列类型纠正
text = replace_once(text, '''#[cfg(feature = "mysql")]
async fn ensure_mysql_column_types(pool: &db::Pool) -> anyhow::Result<()> {
    const FIXES''',
'''async fn ensure_mysql_column_types(pool: &db::Pool) -> anyhow::Result<()> {
    // 只有 mysql 需要把 `plate_color` 收敛成 int（其余方言该列本来就是整数）。
    if !pool.dialect().is_mysql() {
        return Ok(());
    }
    const FIXES''', 'ensure_mysql_column_types 头部')

text = replace_once(text, '''        let data_type: Option<String> = sqlx::query_scalar(
            "SELECT data_type FROM information_schema.columns \\
             WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
        )
        .bind(table)
        .bind(column)
        .fetch_optional(pool)
        .await?;''',
'''        let data_type: Option<String> = crate::dispatch!(pool, |p| {
            sqlx::query_scalar(
                "SELECT data_type FROM information_schema.columns \\
                 WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
            )
            .bind(table)
            .bind(column)
            .fetch_optional(p)
        })
        .await?;''', 'mysql data_type 查询')

text = replace_once(text, '''                sqlx::query(&format!("ALTER TABLE {table} MODIFY {column} {target}"))
                    .execute(pool)
                    .await?;''',
'''                let alter_sql = format!("ALTER TABLE {table} MODIFY {column} {target}");
                crate::dispatch_exec!(pool, |p| {
                    sqlx::query(&alter_sql).execute(p)
                })
                .await?;''', 'mysql ALTER TABLE')

text = replace_once(text, '''
/// 非 mysql 构建下的空实现。
#[cfg(not(feature = "mysql"))]
async fn ensure_mysql_column_types(_pool: &db::Pool) -> anyhow::Result<()> {
    Ok(())
}
''', '\n', '删除 mysql 空实现')

# ---------------------------------------------------------------- 5. init_db_tables 三方言块
text_lines = text.split('\n')
# 依次定位三个块
blocks = []
i = 0
while i < len(text_lines):
    if text_lines[i].strip() in ('#[cfg(feature = "postgres")]', '#[cfg(feature = "mysql")]',
                                '#[cfg(feature = "sqlite")]'):
        indent = len(text_lines[i]) - len(text_lines[i].lstrip())
        # 下一非空行必须是 `{`
        j = i + 1
        while j < len(text_lines) and text_lines[j].strip() == '':
            j += 1
        if text_lines[j].strip() == '{':
            end = find_block(text_lines, j)
            blocks.append((i, j, end, text_lines[i].strip()))
            i = end + 1
            continue
    i += 1

pg_block = next(b for b in blocks if 'postgres' in b[3])
my_block = next(b for b in blocks if 'mysql' in b[3])
sq_block = next(b for b in blocks if 'sqlite' in b[3])


def block_body(b):
    """块内容（去掉 cfg 行与最外层花括号）。"""
    return text_lines[b[1] + 1:b[2]]


def dedent(lines_):
    out = []
    for ln in lines_:
        if ln.startswith('    '):
            out.append(ln[4:])
        else:
            out.append(ln)
    return out


pg_body = dedent(block_body(pg_block))
my_body = dedent(block_body(my_block))
sq_body = dedent(block_body(sq_block))

# 三个块的 SQL 查询改成 dispatch
pg_body = [ln.replace('sqlx::query_scalar(\n', 'sqlx::query_scalar(\n') for ln in pg_body]

# pg 块：table_exists 查询 + 逐条执行
pg_body_txt = '\n'.join(pg_body)
pg_body_txt = pg_body_txt.replace('''let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'gb_device')",
        )
        .fetch_one(pool)
        .await?;''', '''let table_exists: bool = crate::dispatch!(pool, |p| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'gb_device')",
            )
            .fetch_one(p)
        })
        .await?;''')
pg_body_txt = pg_body_txt.replace('''let _ = sqlx::query(stmt).execute(pool).await;''',
'''let _ = crate::dispatch!(pool, |p| {
                    sqlx::query(stmt).execute(p)
                })
                .await;''')

my_body_txt = '\n'.join(my_body)
my_body_txt = my_body_txt.replace('''let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'gb_device')",
        )
        .fetch_one(pool)
        .await?;''', '''let table_exists: bool = crate::dispatch!(pool, |p| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'gb_device')",
            )
            .fetch_one(p)
        })
        .await?;''')
my_body_txt = my_body_txt.replace('''let _ = sqlx::query(stmt).execute(pool).await;''',
'''let _ = crate::dispatch!(pool, |p| {
                    sqlx::query(stmt).execute(p)
                })
                .await;''')

sq_body_txt = '\n'.join(sq_body)
sq_body_txt = sq_body_txt.replace('''let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
            )
            .bind(tbl)
            .fetch_one(pool)
            .await
            .unwrap_or(false);''', '''let exists: bool = crate::dispatch!(pool, |p| {
                sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                )
                .bind(tbl)
                .fetch_one(p)
                .await
                .unwrap_or(false)
            });''')
sq_body_txt = sq_body_txt.replace('''match sqlx::query(stmt).execute(pool).await {''',
'''match crate::dispatch!(pool, |p| {
                        sqlx::query(stmt).execute(p)
                    })
                    .await
                {''')

new_blocks = (
    ['    // Check if core tables exist; if not, run full schema init',
     '    // 三个方言的检测方式不同（information_schema / sqlite_master），',
     '    // 但「缺表就全量初始化」的语义一致 —— 运行期按实际连接的库分支。',
     '    match pool.dialect() {',
     '        db::Dialect::Postgres => {']
    + ['            ' + ln if ln.strip() else ln for ln in pg_body_txt.split('\n')]
    + ['        }', '        db::Dialect::MySql => {']
    + ['            ' + ln if ln.strip() else ln for ln in my_body_txt.split('\n')]
    + ['        }', '        db::Dialect::Sqlite => {']
    + ['            ' + ln if ln.strip() else ln for ln in sq_body_txt.split('\n')]
    + ['        }', '    }']
)

# 替换：从 pg 块起点到 sqlite 块终点
text_lines[new_blocks[0] and 0 or 0:0] = []
new_lines = (text_lines[:pg_block[0]] + new_blocks + text_lines[sq_block[2] + 1:])
text = '\n'.join(new_lines)

# ---------------------------------------------------------------- 6. sqlite 升级补建
text = replace_once(text, '''    #[cfg(feature = "sqlite")]
    ensure_sqlite_upgrade_tables(pool).await?;''',
'''    if pool.dialect().is_sqlite() {
        ensure_sqlite_upgrade_tables(pool).await?;
    }''', 'sqlite 升级调用')

text = replace_once(text, '''#[cfg(feature = "sqlite")]
async fn ensure_sqlite_upgrade_tables(pool: &db::Pool) -> anyhow::Result<()> {
    const STMTS''',
'''async fn ensure_sqlite_upgrade_tables(pool: &db::Pool) -> anyhow::Result<()> {
    if !pool.dialect().is_sqlite() {
        return Ok(());
    }
    const STMTS''', 'sqlite 升级定义')

text = replace_once(text, '''        if let Err(e) = sqlx::query(stmt).execute(pool).await {''',
'''        let res = crate::dispatch!(pool, |p| {
            sqlx::query(stmt).execute(p)
        })
        .await;
        if let Err(e) = res {''', 'sqlite 升级执行')

# ---------------------------------------------------------------- 7. 启动期设备数检查
text = replace_once(text, '''    #[cfg(feature = "sqlite")]
    if let Some(limit) = cfg.database.sqlite_max_devices {''',
'''    if let Some(limit) = cfg.database.sqlite_max_devices.filter(|_| pool.dialect().is_sqlite()) {''',
    'sqlite_max_devices 检查')


# ---------------------------------------------------------------------------
# handlers/stub.rs 的 ensure_record_collect_table：DDL 常量与迁移块改运行期
# ---------------------------------------------------------------------------

def replace_function(path, sig, new_text):
    """按函数签名定位并整体替换函数体（保留首行签名）。"""
    lines = open(path, encoding='utf-8').read().split('\n')
    start = None
    for i, l in enumerate(lines):
        if l.startswith(sig):
            start = i
            break
    if start is None:
        print('  ! 未找到函数:', sig, 'in', path)
        return
    end = None
    for j in range(start + 1, len(lines)):
        if lines[j] == '}':
            end = j
            break
    if end is None:
        print('  ! 未找到函数结尾:', sig)
        return
    lines[start:end + 1] = new_text.split('\n')
    open(path, 'w', encoding='utf-8').write('\n'.join(lines))
    print('  ✓ 已重写', sig)


AUDIT_LOG_ENSURE_TABLE = 'pub async fn ensure_table(pool: &Pool) -> sqlx::Result<()> {\n    // 建表 DDL 按**运行期方言**选（改造前是三个 `#[cfg(feature = ...)]` 块，\n    // 编译期三选一）。\n    let ddl = match pool.dialect() {\n        crate::db::Dialect::Postgres => r#"CREATE TABLE IF NOT EXISTS gb_audit_log (\n            id BIGSERIAL PRIMARY KEY,\n            username VARCHAR(100),\n            action VARCHAR(200),\n            resource VARCHAR(200),\n            method VARCHAR(10),\n            path VARCHAR(500),\n            ip VARCHAR(50),\n            status_code INT,\n            request_body TEXT,\n            elapsed_ms BIGINT,\n            create_time TIMESTAMP DEFAULT NOW()\n        )"#,\n        crate::db::Dialect::MySql => r#"CREATE TABLE IF NOT EXISTS gb_audit_log (\n            id BIGINT AUTO_INCREMENT PRIMARY KEY,\n            username VARCHAR(100),\n            action VARCHAR(200),\n            resource VARCHAR(200),\n            method VARCHAR(10),\n            path VARCHAR(500),\n            ip VARCHAR(50),\n            status_code INT,\n            request_body TEXT,\n            elapsed_ms BIGINT,\n            create_time DATETIME DEFAULT CURRENT_TIMESTAMP\n        )"#,\n        crate::db::Dialect::Sqlite => r#"CREATE TABLE IF NOT EXISTS gb_audit_log (\n            id INTEGER PRIMARY KEY AUTOINCREMENT,\n            username VARCHAR(100),\n            action VARCHAR(200),\n            resource VARCHAR(200),\n            method VARCHAR(10),\n            path VARCHAR(500),\n            ip VARCHAR(50),\n            status_code INTEGER,\n            request_body TEXT,\n            elapsed_ms BIGINT,\n            create_time DATETIME DEFAULT CURRENT_TIMESTAMP\n        )"#,\n    };\n\n    if let Err(e) = crate::dispatch_exec!(pool, |p| {\n        sqlx::query(ddl).execute(p)\n    }) {\n        tracing::error!("创建 gb_audit_log 表失败（审计日志将不可用）: {}", e);\n    }\n\n    // 索引是可选优化：建不出来只告警。\n    //\n    // **MySQL 的 `CREATE INDEX` 不支持 `IF NOT EXISTS`**（会报\n    // `1064 ... near \'IF NOT EXISTS idx_...\'` 语法错误 —— 只有 MariaDB 支持）。\n    // 因此这里不带 `IF NOT EXISTS`，把重复键（1061）当成幂等重放的正常结果。\n    // 实测：2026-09-12 首次在真实 MySQL 上跑后端时，两条索引都因语法错误\n    //       没建出来（只有 WARN），审计日志表因此长期缺索引。\n    let idx_create_time = if pool.dialect().is_mysql() {\n        "CREATE INDEX idx_audit_log_create_time ON gb_audit_log(create_time)"\n    } else {\n        "CREATE INDEX IF NOT EXISTS idx_audit_log_create_time ON gb_audit_log(create_time)"\n    };\n    if let Err(e) = crate::dispatch_exec!(pool, |p| {\n        sqlx::query(idx_create_time).execute(p)\n    }) {\n        tracing::warn!("创建审计日志索引 idx_audit_log_create_time 失败: {}", e);\n    }\n\n    let idx_username = if pool.dialect().is_mysql() {\n        "CREATE INDEX idx_audit_log_username ON gb_audit_log(username)"\n    } else {\n        "CREATE INDEX IF NOT EXISTS idx_audit_log_username ON gb_audit_log(username)"\n    };\n    let _ = crate::dispatch_exec!(pool, |p| {\n        sqlx::query(idx_username).execute(p)\n    });\n\n    // `CREATE TABLE IF NOT EXISTS` **不会**给已存在的表补列，而\n    // `insert_with_metrics` 写的是带 `elapsed_ms` 的扩展列 —— 列不存在时\n    // 每个被审计的请求都会先失败一次再回退（postgres 日志里每请求一条\n    // ERROR），而且耗时根本没落库。这里补上。\n    //\n    // MySQL 不支持 `ADD COLUMN IF NOT EXISTS`，重复列的 1060/1061 当成\n    // 幂等重放的正常结果忽略掉。\n    let add_elapsed = if pool.dialect().is_mysql() {\n        "ALTER TABLE gb_audit_log ADD COLUMN elapsed_ms BIGINT"\n    } else {\n        "ALTER TABLE gb_audit_log ADD COLUMN IF NOT EXISTS elapsed_ms BIGINT"\n    };\n    if let Err(e) = crate::dispatch_exec!(pool, |p| {\n        sqlx::query(add_elapsed).execute(p)\n    }) {\n        let msg = e.to_string();\n        if !msg.contains("1060") && !msg.contains("1061") && !msg.contains("Duplicate") {\n            tracing::warn!("为 gb_audit_log 补 elapsed_ms 列失败: {}", e);\n        }\n    }\n    Ok(())\n}'
COMMON_CHANNEL_UPDATE = 'pub async fn update(\n    pool: &Pool,\n    id: i64,\n    f: &ChannelWriteFields<\'_>,\n    now: &str,\n) -> sqlx::Result<u64> {\n    // 全部 COALESCE：None = 保持原值\n    //\n    // 占位符**统一用 `?`**，由 `pool.sql()` 在运行期按方言改写\n    // （postgres → `$1..$15`）。改造前这里是三份 `{pN}` 模板：\n    // mysql / sqlite 把 `{pN}` 替换成 `?`，postgres 替换成 `$N`。\n    let set = "name = COALESCE(?, name), \\\n               gb_device_id = COALESCE(?, gb_device_id), \\\n               civil_code = COALESCE(?, civil_code), \\\n               parent_id = COALESCE(?, parent_id), \\\n               business_group_id = COALESCE(?, business_group_id), \\\n               ptz_type = COALESCE(?, ptz_type), \\\n               custom_name = COALESCE(?, custom_name), \\\n               manufacturer = COALESCE(?, manufacturer), \\\n               model = COALESCE(?, model), \\\n               owner = COALESCE(?, owner), \\\n               address = COALESCE(?, address), \\\n               stream_identification = COALESCE(?, stream_identification), \\\n               channel_type = COALESCE(?, channel_type), \\\n               update_time = ? \\\n               WHERE id = ?";\n    let stmt = format!("UPDATE gb_device_channel SET {}", set);\n    let sql = pool.sql(&stmt).into_owned();\n    let r = crate::dispatch_exec!(pool, |p| {\n        sqlx::query(&sql)\n            .bind(f.name)\n            .bind(f.channel_id)\n            .bind(f.civil_code)\n            .bind(f.parent_id)\n            .bind(f.business_group)\n            .bind(f.ptz_type)\n            .bind(f.custom_name)\n            .bind(f.manufacturer)\n            .bind(f.model)\n            .bind(f.owner)\n            .bind(f.address)\n            .bind(f.stream_identification)\n            .bind(f.channel_type)\n            .bind(now)\n            .bind(id)\n            .execute(p)\n    })?;\n    Ok(r.rows_affected())\n}'
PLATFORM_CHANNEL_UPDATE = 'pub async fn update(\n    pool: &Pool,\n    id: i64,\n    custom_name: Option<&str>,\n    custom_info: Option<&str>,\n) -> sqlx::Result<u64> {\n    if custom_name.is_none() && custom_info.is_none() {\n        return Ok(0);\n    }\n\n    if crate::db::dialect::current().is_postgres() {\n    {\n        let r = match (custom_name, custom_info) {\n            (Some(name), Some(info)) => crate::dispatch_exec!(pool, |p| {\nsqlx::query(\n                "UPDATE gb_platform_channel SET custom_name = $1, custom_address = $2 WHERE id = $3",\n            )\n            .bind(name)\n            .bind(info)\n            .bind(id)\n            .execute(p)\n}'

def rewrite_stub(path='src/handlers/stub.rs'):
    txt = open(path, encoding='utf-8').read()
    if 'ensure_record_collect_table' not in txt:
        return
    lines = txt.split('\n')
    start = None
    for i, l in enumerate(lines):
        if l.startswith('pub(crate) async fn ensure_record_collect_table'):
            start = i
            break
    if start is None:
        return
    end = None
    for j in range(start + 1, len(lines)):
        if lines[j] == '}':
            end = j
            break
    if end is None:
        return
    new = 'pub(crate) async fn ensure_record_collect_table(pool: &crate::db::Pool) {\n    // 建表 DDL 按**运行期方言**选（改造前是三个 `#[cfg(feature = ...)]` 常量，\n    // 编译期三选一）。\n    let ddl = match pool.dialect() {\n        crate::db::Dialect::Postgres => r#"CREATE TABLE IF NOT EXISTS gb_record_collect (\n            id BIGSERIAL PRIMARY KEY,\n            record_id VARCHAR(255) NOT NULL UNIQUE,\n            device_id VARCHAR(64),\n            channel_id VARCHAR(64),\n            name VARCHAR(255),\n            create_time VARCHAR(50)\n        )"#,\n        crate::db::Dialect::MySql => r#"CREATE TABLE IF NOT EXISTS gb_record_collect (\n            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,\n            record_id VARCHAR(255) NOT NULL UNIQUE,\n            device_id VARCHAR(64),\n            channel_id VARCHAR(64),\n            name VARCHAR(255),\n            create_time VARCHAR(50)\n        )"#,\n        crate::db::Dialect::Sqlite => r#"CREATE TABLE IF NOT EXISTS gb_record_collect (\n            id INTEGER PRIMARY KEY AUTOINCREMENT,\n            record_id VARCHAR(255) NOT NULL UNIQUE,\n            device_id VARCHAR(64),\n            channel_id VARCHAR(64),\n            name VARCHAR(255),\n            create_time VARCHAR(50)\n        )"#,\n    };\n\n    // ---- 既有库的类型迁移 ----\n    if pool.dialect().is_sqlite() {\n        let sql = pool\n            .sql("SELECT type FROM pragma_table_info(\'gb_record_collect\') WHERE name = \'id\'")\n            .into_owned();\n        let id_type: Option<String> = crate::dispatch!(pool, |p| {\n            sqlx::query_scalar(&sql).fetch_optional(p)\n        })\n        .ok()\n        .flatten();\n        if let Some(t) = id_type.filter(|t| !t.eq_ignore_ascii_case("INTEGER")) {\n            tracing::warn!(\n                "[schema] gb_record_collect.id 当前类型为 {}（应为 INTEGER），迁移中（保留数据）",\n                t\n            );\n            let old = "gb_record_collect_legacy";\n            for stmt in [\n                format!("ALTER TABLE gb_record_collect RENAME TO {old}"),\n                ddl.to_string(),\n                format!(\n                    "INSERT INTO gb_record_collect (record_id, device_id, channel_id, name, create_time) \\\n                     SELECT record_id, device_id, channel_id, name, create_time FROM {old}"\n                ),\n                format!("DROP TABLE {old}"),\n            ] {\n                if let Err(e) = crate::dispatch_exec!(pool, |p| {\n                    sqlx::query(&stmt).execute(p)\n                }) {\n                    tracing::error!("[schema] gb_record_collect 迁移失败: {} | {}", e, stmt);\n                    return;\n                }\n            }\n        }\n    }\n\n    if pool.dialect().is_mysql() {\n        let sql = pool\n            .sql("SELECT data_type FROM information_schema.columns \\\n             WHERE table_schema = DATABASE() AND table_name = \'gb_record_collect\' AND column_name = \'id\'")\n            .into_owned();\n        if let Ok(Some(t)) = crate::dispatch!(pool, |p| {\n            sqlx::query_scalar::<_, String>(&sql).fetch_optional(p)\n        }) {\n            if t != "bigint" {\n                tracing::warn!("[schema] gb_record_collect.id 类型为 {}，迁移为有符号 BIGINT", t);\n                let alter = "ALTER TABLE gb_record_collect MODIFY id BIGINT NOT NULL AUTO_INCREMENT";\n                if let Err(e) = crate::dispatch_exec!(pool, |p| {\n                    sqlx::query(alter).execute(p)\n                }) {\n                    tracing::error!("[schema] gb_record_collect.id 迁移失败: {}", e);\n                }\n            }\n        }\n    }\n\n    if let Err(e) = crate::dispatch_exec!(pool, |p| {\n        sqlx::query(ddl).execute(p)\n    }) {\n        let msg = e.to_string();\n        if !msg.contains("already exists") {\n            tracing::error!("创建 gb_record_collect 失败（收藏功能不可用）: {}", msg);\n        }\n    }\n}'
    lines[start:end + 1] = new.split('\n')
    open(path, 'w', encoding='utf-8').write('\n'.join(lines))
    print('  ✓ 已重写 ensure_record_collect_table')

open(DST, 'w', encoding='utf-8').write(text)
rewrite_stub()
replace_function('src/db/audit_log.rs', 'pub async fn ensure_table(', AUDIT_LOG_ENSURE_TABLE)
replace_function('src/db/platform_channel.rs', 'pub async fn update(', PLATFORM_CHANNEL_UPDATE)
replace_function('src/db/common_channel.rs', 'pub async fn update(', COMMON_CHANNEL_UPDATE)
print('  ✓ 已写出', DST)
