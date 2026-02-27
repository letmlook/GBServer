use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// wvp_user + wvp_user_role 联合查询结果（与 Java User 含 Role 一致）
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct User {
    pub id: i32,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password: Option<String>,
    pub role_id: Option<i32>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub push_key: Option<String>,
    #[serde(rename = "role_name")]
    pub role_name: Option<String>,
    #[serde(rename = "role_authority")]
    pub role_authority: Option<String>,
}

impl User {
    /// 用于登录返回：不暴露 password
    pub fn for_login(&mut self) {
        self.password = None;
    }
}

/// 前端需要的 LoginUser 结构（与 Java LoginUser 兼容），字段名 camelCase 与前端一致
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginUserResponse {
    pub id: i32,
    pub username: Option<String>,
    pub role: RoleInfo,
    pub push_key: Option<String>,
    pub access_token: Option<String>,
    pub server_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RoleInfo {
    pub id: i32,
    pub name: Option<String>,
    pub authority: Option<String>,
}

/// 用户列表项：含嵌套 role、camelCase 字段，与前端表格 role.name / pushKey 一致
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserListRow {
    pub id: i32,
    pub username: Option<String>,
    pub push_key: Option<String>,
    pub role: RoleInfo,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

pub async fn find_by_username_password(
    pool: &Pool,
    username: &str,
    password_md5: &str,
) -> sqlx::Result<Option<User>> {
    #[cfg(feature = "mysql")]
    let u = sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id
        WHERE u.username = ? AND u.password = ?"#,
    )
    .bind(username)
    .bind(password_md5)
    .fetch_optional(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let u = sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id
        WHERE u.username = $1 AND u.password = $2"#,
    )
    .bind(username)
    .bind(password_md5)
    .fetch_optional(pool)
    .await?;
    Ok(u)
}

pub async fn find_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<User>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id WHERE u.id = ?"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id WHERE u.id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

pub async fn find_by_username(pool: &Pool, username: &str) -> sqlx::Result<Option<User>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id WHERE u.username = ?"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id WHERE u.username = $1"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await;
}

pub async fn get_users_paged(pool: &Pool, page: u32, count: u32) -> sqlx::Result<Vec<User>> {
    let offset = (page - 1).max(0) * count;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id ORDER BY u.id LIMIT ? OFFSET ?"#,
    )
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id ORDER BY u.id LIMIT $1 OFFSET $2"#,
    )
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn get_all_users(pool: &Pool) -> sqlx::Result<Vec<User>> {
    sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM wvp_user u JOIN wvp_user_role r ON u.role_id = r.id ORDER BY u.id"#,
    )
    .fetch_all(pool)
    .await
}

pub async fn add_user(
    pool: &Pool,
    username: &str,
    password_md5: &str,
    role_id: i32,
    push_key: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_user (username, password, role_id, push_key, create_time, update_time)
        VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(username)
    .bind(password_md5)
    .bind(role_id)
    .bind(push_key)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_user (username, password, role_id, push_key, create_time, update_time)
        VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(username)
    .bind(password_md5)
    .bind(role_id)
    .bind(push_key)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn delete_user(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_user WHERE id != 1 AND id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_user WHERE id != 1 AND id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn change_password(
    pool: &Pool,
    user_id: i32,
    password_md5: &str,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_user SET password = ?, update_time = ? WHERE id = ?")
        .bind(password_md5)
        .bind(&now)
        .bind(user_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_user SET password = $1, update_time = $2 WHERE id = $3")
        .bind(password_md5)
        .bind(&now)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn change_push_key(pool: &Pool, user_id: i32, push_key: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_user SET push_key = ? WHERE id = ?")
        .bind(push_key)
        .bind(user_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_user SET push_key = $1 WHERE id = $2")
        .bind(push_key)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn count_users(pool: &Pool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_user")
        .fetch_one(pool)
        .await
}

pub async fn role_exists(pool: &Pool, role_id: i32) -> sqlx::Result<bool> {
    #[cfg(feature = "mysql")]
    let row: (i64,) = sqlx::query_as("SELECT 1 FROM wvp_user_role WHERE id = ?")
        .bind(role_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or((0,));
    #[cfg(feature = "postgres")]
    let row: (i64,) = sqlx::query_as("SELECT 1 FROM wvp_user_role WHERE id = $1")
        .bind(role_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or((0,));
    Ok(row.0 > 0)
}
