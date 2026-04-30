use super::Pool;

pub async fn ensure_table(pool: &Pool) -> sqlx::Result<()> {
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS wvp_audit_log (
                id BIGSERIAL PRIMARY KEY,
                username VARCHAR(100),
                action VARCHAR(200),
                resource VARCHAR(200),
                method VARCHAR(10),
                path VARCHAR(500),
                ip VARCHAR(50),
                status_code INT,
                request_body TEXT,
                create_time TIMESTAMP DEFAULT NOW()
            )"#
        )
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_create_time ON wvp_audit_log(create_time)"
        )
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_username ON wvp_audit_log(username)"
        )
        .execute(pool)
        .await;
    }

    #[cfg(feature = "mysql")]
    {
        let _ = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS wvp_audit_log (
                id BIGINT AUTO_INCREMENT PRIMARY KEY,
                username VARCHAR(100),
                action VARCHAR(200),
                resource VARCHAR(200),
                method VARCHAR(10),
                path VARCHAR(500),
                ip VARCHAR(50),
                status_code INT,
                request_body TEXT,
                create_time DATETIME DEFAULT CURRENT_TIMESTAMP
            )"#
        )
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_create_time ON wvp_audit_log(create_time)"
        )
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_username ON wvp_audit_log(username)"
        )
        .execute(pool)
        .await;
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
        "INSERT INTO wvp_audit_log (username, action, resource, method, path, ip, status_code) VALUES (?, ?, ?, ?, ?, ?, ?)"
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
        "INSERT INTO wvp_audit_log (username, action, resource, method, path, ip, status_code) VALUES ($1, $2, $3, $4, $5, $6, $7)"
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

pub async fn list_paged(
    pool: &Pool,
    username: Option<&str>,
    action: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
    page: u32,
    count: u32,
) -> sqlx::Result<(i64, Vec<serde_json::Value>)> {
    let offset = (page.saturating_sub(1)) * count;
    
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
            bind_values.push(like);
        }
    }
    if let Some(a) = action {
        if !a.is_empty() {
            #[cfg(feature = "postgres")]
            conditions.push_str(&format!(" AND action = ${}", bind_values.len() + 1));
            #[cfg(feature = "mysql")]
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
            bind_values.push(st.to_string());
        }
    }
    if let Some(et) = end_time {
        if !et.is_empty() {
            #[cfg(feature = "postgres")]
            conditions.push_str(&format!(" AND create_time <= ${}", bind_values.len() + 1));
            #[cfg(feature = "mysql")]
            conditions.push_str(" AND create_time <= ?");
            bind_values.push(et.to_string());
        }
    }

    let count_sql = format!("SELECT COUNT(*) FROM wvp_audit_log WHERE 1=1{}", conditions);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &bind_values { count_q = count_q.bind(b); }
    let total: i64 = count_q.fetch_one(pool).await.unwrap_or(0);

    #[cfg(feature = "postgres")]
    let data_sql = format!(
        "SELECT id, username, action, resource, method, path, ip, status_code, create_time::text as create_time FROM wvp_audit_log WHERE 1=1{} ORDER BY id DESC LIMIT ${} OFFSET ${}",
        conditions, bind_values.len() + 1, bind_values.len() + 2
    );
    #[cfg(feature = "mysql")]
    let data_sql = format!(
        "SELECT id, username, action, resource, method, path, ip, status_code, create_time FROM wvp_audit_log WHERE 1=1{} ORDER BY id DESC LIMIT ? OFFSET ?",
        conditions
    );

    let mut data_q = sqlx::query_as::<_, AuditRow>(&data_sql);
    for b in &bind_values { data_q = data_q.bind(b); }
    data_q = data_q.bind(count as i64).bind(offset as i64);

    let rows: Vec<AuditRow> = data_q.fetch_all(pool).await.unwrap_or_default();

    let list: Vec<serde_json::Value> = rows.iter().map(|r| {
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
    }).collect();

    Ok((total, list))
}
