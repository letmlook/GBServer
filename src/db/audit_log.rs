use super::Pool;

pub async fn ensure_table(pool: &Pool) -> sqlx::Result<()> {
    #[cfg(feature = "postgres")]
    {
        if let Err(e) = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_audit_log (
                id BIGSERIAL PRIMARY KEY,
                username VARCHAR(100),
                action VARCHAR(200),
                resource VARCHAR(200),
                method VARCHAR(10),
                path VARCHAR(500),
                ip VARCHAR(50),
                status_code INT,
                request_body TEXT,
                elapsed_ms BIGINT,
                create_time TIMESTAMP DEFAULT NOW()
            )"#,
        )
        .execute(pool)
        .await
        {
            tracing::error!("创建 gb_audit_log 表失败（审计日志将不可用）: {}", e);
        }

        // 索引是可选优化：建不出来只告警
        if let Err(e) = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_create_time ON gb_audit_log(create_time)",
        )
        .execute(pool)
        .await
        {
            tracing::warn!("创建审计日志索引 idx_audit_log_create_time 失败: {}", e);
        }

        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_username ON gb_audit_log(username)",
        )
        .execute(pool)
        .await;

        // `CREATE TABLE IF NOT EXISTS` **不会**给已存在的表补列，而
        // `insert_with_metrics` 写的是带 `elapsed_ms` 的扩展列 —— 列不存在时
        // 每个被审计的请求都会先失败一次再回退（postgres 日志里每请求一条
        // ERROR），而且耗时根本没落库。这里补上。
        if let Err(e) =
            sqlx::query("ALTER TABLE gb_audit_log ADD COLUMN IF NOT EXISTS elapsed_ms BIGINT")
                .execute(pool)
                .await
        {
            tracing::warn!("为 gb_audit_log 补 elapsed_ms 列失败: {}", e);
        }
    }

    #[cfg(feature = "mysql")]
    {
        if let Err(e) = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_audit_log (
                id BIGINT AUTO_INCREMENT PRIMARY KEY,
                username VARCHAR(100),
                action VARCHAR(200),
                resource VARCHAR(200),
                method VARCHAR(10),
                path VARCHAR(500),
                ip VARCHAR(50),
                status_code INT,
                request_body TEXT,
                elapsed_ms BIGINT,
                create_time DATETIME DEFAULT CURRENT_TIMESTAMP
            )"#,
        )
        .execute(pool)
        .await
        {
            tracing::error!("创建 gb_audit_log 表失败（审计日志将不可用）: {}", e);
        }

        // **MySQL 的 `CREATE INDEX` 不支持 `IF NOT EXISTS`**（会报
        // `1064 ... near 'IF NOT EXISTS idx_...'` 语法错误 —— 只有 MariaDB 支持）。
        // 因此这里不带 `IF NOT EXISTS`，把重复键（1061）当成幂等重放的正常结果。
        // 实测：2026-09-12 首次在真实 MySQL 上跑后端时，两条索引都因语法错误
        //       没建出来（只有 WARN），审计日志表因此长期缺索引。
        let _ = sqlx::query("CREATE INDEX idx_audit_log_create_time ON gb_audit_log(create_time)")
            .execute(pool)
            .await;
        let _ = sqlx::query("CREATE INDEX idx_audit_log_username ON gb_audit_log(username)")
            .execute(pool)
            .await;

        // 给已存在的表补 `elapsed_ms`（MySQL 不支持 `ADD COLUMN IF NOT EXISTS`，
        // 重复列的 1060/1061 当成幂等重放的正常结果忽略掉）。
        if let Err(e) = sqlx::query("ALTER TABLE gb_audit_log ADD COLUMN elapsed_ms BIGINT")
            .execute(pool)
            .await
        {
            let msg = e.to_string();
            if !msg.contains("1060") && !msg.contains("1061") && !msg.contains("Duplicate") {
                tracing::warn!("为 gb_audit_log 补 elapsed_ms 列失败: {}", e);
            }
        }
    }

    #[cfg(feature = "sqlite")]
    {
        // SQLite 使用 DATETIME + CURRENT_TIMESTAMP；status_code 用 INTEGER
        if let Err(e) = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username VARCHAR(100),
                action VARCHAR(200),
                resource VARCHAR(200),
                method VARCHAR(10),
                path VARCHAR(500),
                ip VARCHAR(50),
                status_code INTEGER,
                request_body TEXT,
                elapsed_ms BIGINT,
                create_time DATETIME DEFAULT CURRENT_TIMESTAMP
            )"#,
        )
        .execute(pool)
        .await
        {
            tracing::error!("创建 gb_audit_log 表失败（审计日志将不可用）: {}", e);
        }

        // 索引属于可选优化：建不出来只告警
        if let Err(e) = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_create_time ON gb_audit_log(create_time)",
        )
        .execute(pool)
        .await
        {
            tracing::warn!("创建审计日志索引 idx_audit_log_create_time 失败: {}", e);
        }

        if let Err(e) = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_username ON gb_audit_log(username)",
        )
        .execute(pool)
        .await
        {
            tracing::warn!("创建审计日志索引 idx_audit_log_username 失败: {}", e);
        }
    }

    Ok(())
}

pub async fn insert(
    pool: &Pool,
    username: &str,
    action: &str,
    resource: &str,
    method: &str,
    path: &str,
    ip: &str,
    status_code: i32,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO gb_audit_log (username, action, resource, method, path, ip, status_code) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(username)
    .bind(action)
    .bind(resource)
    .bind(method)
    .bind(path)
    .bind(ip)
    .bind(status_code)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO gb_audit_log (username, action, resource, method, path, ip, status_code) VALUES ($1, $2, $3, $4, $5, $6, $7)"
    )
    .bind(username)
    .bind(action)
    .bind(resource)
    .bind(method)
    .bind(path)
    .bind(ip)
    .bind(status_code)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "INSERT INTO gb_audit_log (username, action, resource, method, path, ip, status_code) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(username)
    .bind(action)
    .bind(resource)
    .bind(method)
    .bind(path)
    .bind(ip)
    .bind(status_code)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Phase 7.4: extended insert with `elapsed_ms` column. Caller (audit_middleware)
/// already times the request; we just persist it.
pub async fn insert_with_metrics(
    pool: &Pool,
    username: &str,
    action: &str,
    resource: &str,
    method: &str,
    path: &str,
    ip: &str,
    status_code: i32,
    elapsed_ms: i64,
) -> sqlx::Result<()> {
    // Try extended schema first (gb_audit_log + elapsed_ms), fall back to basic schema.
    // We do this with INSERT IGNORE-style fallback: attempt to insert with elapsed_ms;
    // if the column doesn't exist, retry without it.
    let with_elapsed_result = sqlx::query(&crate::dyn_where::dialect_sql(
        "INSERT INTO gb_audit_log (username, action, resource, method, path, ip, status_code, elapsed_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    ))
    .bind(username).bind(action).bind(resource).bind(method).bind(path).bind(ip).bind(status_code).bind(elapsed_ms)
    .execute(pool).await;
    if with_elapsed_result.is_ok() {
        return Ok(());
    }
    // Fallback: basic schema (no elapsed_ms column)
    #[cfg(feature = "mysql")]
    {
        let _ = sqlx::query("INSERT INTO gb_audit_log (username, action, resource, method, path, ip, status_code) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(username).bind(action).bind(resource).bind(method).bind(path).bind(ip).bind(status_code)
        .execute(pool).await?;
    }
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query("INSERT INTO gb_audit_log (username, action, resource, method, path, ip, status_code) VALUES ($1, $2, $3, $4, $5, $6, $7)")
        .bind(username).bind(action).bind(resource).bind(method).bind(path).bind(ip).bind(status_code)
        .execute(pool).await?;
    }
    #[cfg(feature = "sqlite")]
    {
        let _ = sqlx::query("INSERT INTO gb_audit_log (username, action, resource, method, path, ip, status_code) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(username).bind(action).bind(resource).bind(method).bind(path).bind(ip).bind(status_code)
        .execute(pool).await?;
    }
    Ok(())
}

pub async fn list_paged(
    pool: &Pool,
    username: Option<&str>,
    action: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
    page: u32,
    count: u32,
) -> sqlx::Result<(i64, Vec<serde_json::Value>)> {
    let offset = crate::pagination::offset(page, count);

    #[derive(sqlx::FromRow)]
    struct AuditRow {
        id: i64,
        username: Option<String>,
        action: Option<String>,
        resource: Option<String>,
        method: Option<String>,
        path: Option<String>,
        ip: Option<String>,
        status_code: Option<i32>,
        create_time: Option<String>,
    }

    let mut conditions = String::new();
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(u) = username {
        if !u.is_empty() {
            let like = format!("%{}%", u);
            #[cfg(feature = "postgres")]
            conditions.push_str(&format!(" AND username ILIKE ${}", bind_values.len() + 1));
            #[cfg(feature = "mysql")]
            conditions.push_str(&format!(" AND username LIKE ?"));
            #[cfg(feature = "sqlite")]
            conditions.push_str(" AND username LIKE ? COLLATE NOCASE");
            bind_values.push(like);
        }
    }
    if let Some(a) = action {
        if !a.is_empty() {
            #[cfg(feature = "postgres")]
            conditions.push_str(&format!(" AND action = ${}", bind_values.len() + 1));
            #[cfg(feature = "mysql")]
            conditions.push_str(" AND action = ?");
            #[cfg(feature = "sqlite")]
            conditions.push_str(" AND action = ?");
            bind_values.push(a.to_string());
        }
    }
    if let Some(st) = start_time {
        if !st.is_empty() {
            #[cfg(feature = "postgres")]
            conditions.push_str(&format!(" AND create_time >= ${}", bind_values.len() + 1));
            #[cfg(feature = "mysql")]
            conditions.push_str(" AND create_time >= ?");
            #[cfg(feature = "sqlite")]
            conditions.push_str(" AND create_time >= ?");
            bind_values.push(st.to_string());
        }
    }
    if let Some(et) = end_time {
        if !et.is_empty() {
            #[cfg(feature = "postgres")]
            conditions.push_str(&format!(" AND create_time <= ${}", bind_values.len() + 1));
            #[cfg(feature = "mysql")]
            conditions.push_str(" AND create_time <= ?");
            #[cfg(feature = "sqlite")]
            conditions.push_str(" AND create_time <= ?");
            bind_values.push(et.to_string());
        }
    }

    let count_sql = format!("SELECT COUNT(*) FROM gb_audit_log WHERE 1=1{}", conditions);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &bind_values {
        count_q = count_q.bind(b);
    }
    let total: i64 = count_q.fetch_one(pool).await.unwrap_or(0);

    #[cfg(feature = "postgres")]
    let data_sql = format!(
        "SELECT id, username, action, resource, method, path, ip, status_code, create_time::text as create_time FROM gb_audit_log WHERE 1=1{} ORDER BY id DESC LIMIT ${} OFFSET ${}",
        conditions, bind_values.len() + 1, bind_values.len() + 2
    );
    #[cfg(feature = "mysql")]
    let data_sql = format!(
        "SELECT id, username, action, resource, method, path, ip, status_code, create_time FROM gb_audit_log WHERE 1=1{} ORDER BY id DESC LIMIT ? OFFSET ?",
        conditions
    );
    // SQLite：DATETIME 直接以文本存储，无需 ::text 转换
    #[cfg(feature = "sqlite")]
    let data_sql = format!(
        "SELECT id, username, action, resource, method, path, ip, status_code, create_time FROM gb_audit_log WHERE 1=1{} ORDER BY id DESC LIMIT ? OFFSET ?",
        conditions
    );

    let mut data_q = sqlx::query_as::<_, AuditRow>(&data_sql);
    for b in &bind_values {
        data_q = data_q.bind(b);
    }
    data_q = data_q.bind(count as i64).bind(offset);

    let rows: Vec<AuditRow> = data_q.fetch_all(pool).await.unwrap_or_default();

    let list: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "username": r.username,
                "action": r.action,
                "resource": r.resource,
                "method": r.method,
                "path": r.path,
                "ip": r.ip,
                "statusCode": r.status_code,
                "createTime": r.create_time
            })
        })
        .collect();

    Ok((total, list))
}
