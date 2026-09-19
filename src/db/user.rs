use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

use super::Pool;

/// gb_user + gb_user_role 联合查询结果（`role` 是嵌套对象，不是扁平的 roleId/roleName）
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

/// 登录后返回给前端的用户结构（含嵌套 `role`），字段名 camelCase 与前端一致
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginUserResponse {
    pub id: i32,
    pub username: Option<String>,
    pub role: RoleInfo,
    pub push_key: Option<String>,
    pub access_token: Option<String>,
    pub server_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleInfo {
    pub id: i32,
    pub name: Option<String>,
    pub authority: Option<String>,
}

/// 用户列表项：含嵌套 role、camelCase 字段，与前端表格 role.name / pushKey 一致
#[derive(Debug, Serialize, ToSchema)]
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
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id
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
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id
        WHERE u.username = $1 AND u.password = $2"#,
    )
    .bind(username)
    .bind(password_md5)
    .fetch_optional(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let u = sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id
        WHERE u.username = ? AND u.password = ?"#,
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
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.id = ?"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.id = ?"#,
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
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.username = ?"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.username = $1"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.username = ?"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await;
}

/// 分页查询用户，可按用户名模糊搜索。
///
/// 两个关键点：
///
/// 1. **必须 `LEFT JOIN`**（历史缺陷）：此前是 `JOIN gb_user_role`，于是
///    `role_id` 悬空（角色被删、用户还在）的行会被 SQL 直接丢掉 ——
///    用户管理页看不到这些用户，但 `count_users` 仍把他们算进 total，
///    表现为"共 N 条却只有 N-k 行"且翻页也找不回。改成 LEFT JOIN 后，
///    悬空角色的用户仍会列出，角色名显示为占位值。
/// 2. `query` 为 `Some(非空)` 时按 `username LIKE %q%` 过滤；`count_users`
///    必须用同一条件，否则总数与列表不一致。
pub async fn get_users_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
) -> sqlx::Result<Vec<User>> {
    let offset = (page - 1).max(0) * count;
    let q = query.map(str::trim).filter(|s| !s.is_empty());
    let like = q.map(|s| format!("%{}%", s));

    #[cfg(feature = "mysql")]
    {
        return match &like {
            Some(pat) => {
                sqlx::query_as::<_, User>(
                    r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id
        WHERE u.username LIKE ? ORDER BY u.id LIMIT ? OFFSET ?"#,
                )
                .bind(pat)
                .bind(count as i64)
                .bind(offset as i64)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, User>(
                    r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id ORDER BY u.id LIMIT ? OFFSET ?"#,
                )
                .bind(count as i64)
                .bind(offset as i64)
                .fetch_all(pool)
                .await
            }
        };
    }
    #[cfg(feature = "postgres")]
    {
        return match &like {
            Some(pat) => {
                sqlx::query_as::<_, User>(
                    r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id
        WHERE u.username LIKE $1 ORDER BY u.id LIMIT $2 OFFSET $3"#,
                )
                .bind(pat)
                .bind(count as i64)
                .bind(offset as i64)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, User>(
                    r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id ORDER BY u.id LIMIT $1 OFFSET $2"#,
                )
                .bind(count as i64)
                .bind(offset as i64)
                .fetch_all(pool)
                .await
            }
        };
    }
    #[cfg(feature = "sqlite")]
    {
        return match &like {
            Some(pat) => {
                sqlx::query_as::<_, User>(
                    r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id
        WHERE u.username LIKE ? ORDER BY u.id LIMIT ? OFFSET ?"#,
                )
                .bind(pat)
                .bind(count as i64)
                .bind(offset as i64)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, User>(
                    r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id ORDER BY u.id LIMIT ? OFFSET ?"#,
                )
                .bind(count as i64)
                .bind(offset as i64)
                .fetch_all(pool)
                .await
            }
        };
    }
}

pub async fn get_all_users(pool: &Pool) -> sqlx::Result<Vec<User>> {
    // LEFT JOIN：理由同 `get_users_paged`（悬空 role_id 不能被丢掉）。
    sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id ORDER BY u.id"#,
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
        r#"INSERT INTO gb_user (username, password, role_id, push_key, create_time, update_time)
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
        r#"INSERT INTO gb_user (username, password, role_id, push_key, create_time, update_time)
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
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"INSERT INTO gb_user (username, password, role_id, push_key, create_time, update_time)
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
    Ok(r.rows_affected())
}

pub async fn delete_user(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM gb_user WHERE id != 1 AND id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_user WHERE id != 1 AND id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("DELETE FROM gb_user WHERE id != 1 AND id = ?")
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
    let r = sqlx::query("UPDATE gb_user SET password = ?, update_time = ? WHERE id = ?")
        .bind(password_md5)
        .bind(&now)
        .bind(user_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_user SET password = $1, update_time = $2 WHERE id = $3")
        .bind(password_md5)
        .bind(&now)
        .bind(user_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_user SET password = ?, update_time = ? WHERE id = ?")
        .bind(password_md5)
        .bind(&now)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn change_push_key(pool: &Pool, user_id: i32, push_key: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_user SET push_key = ? WHERE id = ?")
        .bind(push_key)
        .bind(user_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_user SET push_key = $1 WHERE id = $2")
        .bind(push_key)
        .bind(user_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_user SET push_key = ? WHERE id = ?")
        .bind(push_key)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 统计用户数。`query` 必须与 `get_users_paged` 的过滤条件一致，
/// 否则列表与「共 N 条」会对不上。
pub async fn count_users(pool: &Pool, query: Option<&str>) -> sqlx::Result<i64> {
    let q = query.map(str::trim).filter(|s| !s.is_empty());
    match q {
        Some(s) => {
            let like = format!("%{}%", s);
            #[cfg(feature = "postgres")]
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username LIKE $1";
            #[cfg(not(feature = "postgres"))]
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username LIKE ?";
            sqlx::query_scalar::<_, i64>(sql)
                .bind(like)
                .fetch_one(pool)
                .await
        }
        None => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_user")
                .fetch_one(pool)
                .await
        }
    }
}

/// 统计仍引用某角色的用户数（删角色前的引用检查）。
pub async fn count_users_by_role(pool: &Pool, role_id: i32) -> sqlx::Result<i64> {
    #[cfg(feature = "postgres")]
    let sql = "SELECT COUNT(*) FROM gb_user WHERE role_id = $1";
    #[cfg(not(feature = "postgres"))]
    let sql = "SELECT COUNT(*) FROM gb_user WHERE role_id = ?";
    sqlx::query_scalar::<_, i64>(sql)
        .bind(role_id)
        .fetch_one(pool)
        .await
}

/// 用户名是否已被占用（`exclude_id` 用于「改用户名」时排除自己）。
pub async fn username_taken(pool: &Pool, username: &str, exclude_id: Option<i32>) -> sqlx::Result<bool> {
    match exclude_id {
        Some(id) => {
            #[cfg(feature = "postgres")]
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username = $1 AND id != $2";
            #[cfg(not(feature = "postgres"))]
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ? AND id != ?";
            let n: i64 = sqlx::query_scalar(sql)
                .bind(username)
                .bind(id)
                .fetch_one(pool)
                .await?;
            Ok(n > 0)
        }
        None => {
            #[cfg(feature = "postgres")]
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username = $1";
            #[cfg(not(feature = "postgres"))]
            let sql = "SELECT COUNT(*) FROM gb_user WHERE username = ?";
            let n: i64 = sqlx::query_scalar(sql)
                .bind(username)
                .fetch_one(pool)
                .await?;
            Ok(n > 0)
        }
    }
}

/// 更新用户资料（用户名 / 角色），只更新传入的字段。
///
/// 收敛了原先两个零调用死函数 `update_username` / `update_user_role`，
/// 现在由 `POST /api/user/update` 真实使用。
pub async fn update_user(
    pool: &Pool,
    user_id: i32,
    username: Option<&str>,
    role_id: Option<i32>,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_user SET username = COALESCE(?, username), role_id = COALESCE(?, role_id), \
         update_time = ? WHERE id = ?",
    )
    .bind(username)
    .bind(role_id)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_user SET username = COALESCE($1, username), role_id = COALESCE($2, role_id), \
         update_time = $3 WHERE id = $4",
    )
    .bind(username)
    .bind(role_id)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_user SET username = COALESCE(?, username), role_id = COALESCE(?, role_id), \
         update_time = ? WHERE id = ?",
    )
    .bind(username)
    .bind(role_id)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn role_exists(pool: &Pool, role_id: i32) -> sqlx::Result<bool> {
    #[cfg(feature = "mysql")]
    let row: (i64,) = sqlx::query_as("SELECT 1 FROM gb_user_role WHERE id = ?")
        .bind(role_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or((0,));
    #[cfg(feature = "postgres")]
    let row: (i64,) = sqlx::query_as("SELECT 1 FROM gb_user_role WHERE id = $1")
        .bind(role_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or((0,));
    #[cfg(feature = "sqlite")]
    let row: (i64,) = sqlx::query_as("SELECT 1 FROM gb_user_role WHERE id = ?")
        .bind(role_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or((0,));
    Ok(row.0 > 0)
}

