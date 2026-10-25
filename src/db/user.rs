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
    let sql = pool.sql(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id
        WHERE u.username = ? AND u.password = ?"#,
    )
    .into_owned();
    let u = crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, User>(&sql)
            .bind(username)
            .bind(password_md5)
            .fetch_optional(p)
    })?;
    Ok(u)
}

pub async fn find_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<User>> {
    let sql = pool.sql(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.id = ?"#,
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, User>(&sql).bind(id).fetch_optional(p)
    });
}

pub async fn find_by_username(pool: &Pool, username: &str) -> sqlx::Result<Option<User>> {
    let sql = pool.sql(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u JOIN gb_user_role r ON u.role_id = r.id WHERE u.username = ?"#,
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, User>(&sql)
            .bind(username)
            .fetch_optional(p)
    });
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
    let offset = crate::pagination::offset(page, count);
    let q = query.map(str::trim).filter(|s| !s.is_empty());
    let like = q.map(|s| format!("%{}%", s));

    let sql = pool.sql(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id
        WHERE u.username LIKE ? ORDER BY u.id LIMIT ? OFFSET ?"#,
    )
    .into_owned();
    // 宏体**不能**带 `.await`（`dispatch!` 自己 await），也不能用 `return`。
    // 带 `?` 的 SQL 必须先在**宏外面**算好：`dialect_sql(...)` 返回的临时
    // `Cow` 在宏体块尾就会被析构，而查询 future 还借着它。
    let sql_no_like = pool.sql(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id ORDER BY u.id LIMIT ? OFFSET ?"#,
    );
    crate::dispatch!(pool, |p| {
        match &like {
            Some(pat) => sqlx::query_as::<_, User>(&sql)
                .bind(pat)
                .bind(count as i64)
                .bind(offset)
                .fetch_all(p),
            None => sqlx::query_as::<_, User>(&sql_no_like)
                .bind(count as i64)
                .bind(offset)
                .fetch_all(p),
        }
    })
}

pub async fn get_all_users(pool: &Pool) -> sqlx::Result<Vec<User>> {
    // LEFT JOIN：理由同 `get_users_paged`（悬空 role_id 不能被丢掉）。
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, User>(
        r#"SELECT u.id, u.username, u.password, u.role_id, u.create_time, u.update_time, u.push_key,
               r.name AS role_name, r.authority AS role_authority
        FROM gb_user u LEFT JOIN gb_user_role r ON u.role_id = r.id ORDER BY u.id"#,
    )
    .fetch_all(p)
    })
}

pub async fn add_user(
    pool: &Pool,
    username: &str,
    password_md5: &str,
    role_id: i32,
    push_key: &str,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = pool
        .sql(
            r#"INSERT INTO gb_user (username, password, role_id, push_key, create_time, update_time)
        VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(username)
            .bind(password_md5)
            .bind(role_id)
            .bind(push_key)
            .bind(now)
            .bind(now)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn delete_user(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    let sql = pool
        .sql("DELETE FROM gb_user WHERE id != 1 AND id = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

pub async fn change_password(pool: &Pool, user_id: i32, password_md5: &str) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sql = pool
        .sql("UPDATE gb_user SET password = ?, update_time = ? WHERE id = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(password_md5)
            .bind(&now)
            .bind(user_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn change_push_key(pool: &Pool, user_id: i32, push_key: &str) -> sqlx::Result<u64> {
    let sql = pool
        .sql("UPDATE gb_user SET push_key = ? WHERE id = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(push_key).bind(user_id).execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 统计用户数。`query` 必须与 `get_users_paged` 的过滤条件一致，
/// 否则列表与「共 N 条」会对不上。
pub async fn count_users(pool: &Pool, query: Option<&str>) -> sqlx::Result<i64> {
    let q = query.map(str::trim).filter(|s| !s.is_empty());
    match q {
        Some(s) => {
            let like = format!("%{}%", s);
            // SQLite 的 `?1` 编号占位符统一成 `?` —— 三种方言按顺序绑定，语义一致。
            let sql = pool.sql("SELECT COUNT(*) FROM gb_user WHERE username LIKE ?");
            crate::dispatch!(pool, |p| {
                sqlx::query_scalar::<_, i64>(&sql).bind(like).fetch_one(p)
            })
        }
        None => {
            crate::dispatch!(pool, |p| {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_user").fetch_one(p)
            })
        }
    }
}

/// 统计仍引用某角色的用户数（删角色前的引用检查）。
pub async fn count_users_by_role(pool: &Pool, role_id: i32) -> sqlx::Result<i64> {
    // SQLite 的 `?1` 编号占位符统一成 `?` —— 三种方言按顺序绑定，语义一致。
    let sql = pool.sql("SELECT COUNT(*) FROM gb_user WHERE role_id = ?");
    crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql)
            .bind(role_id)
            .fetch_one(p)
    })
}

/// 用户名是否已被占用（`exclude_id` 用于「改用户名」时排除自己）。
pub async fn username_taken(
    pool: &Pool,
    username: &str,
    exclude_id: Option<i32>,
) -> sqlx::Result<bool> {
    match exclude_id {
        Some(id) => {
            // SQLite 的 `?1/?2` 编号占位符统一成 `?` —— 三种方言按顺序绑定。
            let sql = pool.sql("SELECT COUNT(*) FROM gb_user WHERE username = ? AND id != ?");
            let n: i64 = crate::dispatch!(pool, |p| {
                sqlx::query_scalar::<_, i64>(&sql)
                    .bind(username)
                    .bind(id)
                    .fetch_one(p)
            })?;
            Ok(n > 0)
        }
        None => {
            // SQLite 的 `?1` 编号占位符统一成 `?` —— 三种方言按顺序绑定。
            let sql = pool.sql("SELECT COUNT(*) FROM gb_user WHERE username = ?");
            let n: i64 = crate::dispatch!(pool, |p| {
                sqlx::query_scalar::<_, i64>(&sql)
                    .bind(username)
                    .fetch_one(p)
            })?;
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
    let sql = pool
        .sql(
            "UPDATE gb_user SET username = COALESCE(?, username), role_id = COALESCE(?, role_id), \
         update_time = ? WHERE id = ?",
        )
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(username)
            .bind(role_id)
            .bind(&now)
            .bind(user_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn role_exists(pool: &Pool, role_id: i32) -> sqlx::Result<bool> {
    // 用 `EXISTS` 而不是 `SELECT 1`：postgres 里 `SELECT 1` 的字面量类型是
    // **INT4**，按 `i64` 解会报 "decoding column 0: mismatched types"，
    // 而 `EXISTS` 在 postgres 返回 `bool`、mysql/sqlite 返回整数 —— 与
    // `db::device::device_exists` 用的是同一套按方言分支的写法。
    if pool.dialect().is_mysql() {
        {
            let row: (i64,) = crate::dispatch!(pool, |p| {
                sqlx::query_as(&pool.sql("SELECT EXISTS(SELECT 1 FROM gb_user_role WHERE id = ?)"))
                    .bind(role_id)
                    .fetch_optional(p)
            })?
            .unwrap_or((0,));
            Ok(row.0 > 0)
        }
    } else if pool.dialect().is_postgres() {
        {
            let row: (bool,) = crate::dispatch!(pool, |p| {
                sqlx::query_as("SELECT EXISTS(SELECT 1 FROM gb_user_role WHERE id = $1)")
                    .bind(role_id)
                    .fetch_optional(p)
            })?
            .unwrap_or((false,));
            Ok(row.0)
        }
    } else {
        {
            let row: (i64,) = crate::dispatch!(pool, |p| {
                sqlx::query_as(&pool.sql("SELECT EXISTS(SELECT 1 FROM gb_user_role WHERE id = ?)"))
                    .bind(role_id)
                    .fetch_optional(p)
            })?
            .unwrap_or((0,));
            Ok(row.0 > 0)
        }
    }
}
