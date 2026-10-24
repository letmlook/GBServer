//! 角色表 gb_user_role

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::Pool;

/// 角色信息结构体。
///
/// 仅**序列化**方向改为 camelCase（前端 `Role` 接口用 `createTime`/`updateTime`，
/// `/api/role/all` 此前返回 `create_time`，列表里的创建/更新时间列是空的）。
/// 反序列化方向保持 snake_case 主名 + camelCase alias，不影响请求体兼容性。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Role {
    pub id: i32,
    pub name: Option<String>,
    pub authority: Option<String>,
    #[serde(alias = "createTime")]
    pub create_time: Option<String>,
    #[serde(alias = "updateTime")]
    pub update_time: Option<String>,
}

/// 创建角色参数
pub struct RoleCreate {
    pub name: String,
    pub authority: String,
    pub create_time: String,
}

/// 更新角色参数
pub struct RoleUpdate {
    pub id: i32,
    pub name: Option<String>,
    pub authority: Option<String>,
    pub update_time: String,
}

/// 查询所有角色列表
pub async fn list_all(pool: &Pool) -> sqlx::Result<Vec<Role>> {
    let sql = crate::db::dialect::current_sql(
        "SELECT id, name, authority, create_time, update_time FROM gb_user_role ORDER BY id",
    )
    .into_owned();

    crate::dispatch!(pool, |p| { sqlx::query_as::<_, Role>(&sql,).fetch_all(p) })
}

/// 根据ID查询角色
pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<Role>> {
    let sql = crate::db::dialect::current_sql(
        "SELECT id, name, authority, create_time, update_time FROM gb_user_role WHERE id = ?",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Role>(&sql).bind(id).fetch_optional(p)
    });
}

/// 根据名称查询角色
pub async fn get_by_name(pool: &Pool, name: &str) -> sqlx::Result<Option<Role>> {
    let sql = crate::db::dialect::current_sql(
        "SELECT id, name, authority, create_time, update_time FROM gb_user_role WHERE name = ?",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Role>(&sql).bind(name).fetch_optional(p)
    });
}

/// 添加角色
pub async fn add(pool: &Pool, role: &RoleCreate) -> sqlx::Result<i32> {
    // `RETURNING id` 只有 PostgreSQL / SQLite(≥3.35) 支持，**MySQL 不支持**
    // —— MySQL 走 `execute()` 的自增 id。
    //
    // 改造前这里是三个 `#[cfg(feature = ...)]` 分支（编译期三选一），
    // 现在按**运行期方言**分支。
    if crate::db::dialect::current().is_mysql() {
        let sql = crate::db::dialect::current_sql(
            "INSERT INTO gb_user_role (name, authority, create_time, update_time) \
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
        "INSERT INTO gb_user_role (name, authority, create_time, update_time) \
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

/// 更新角色
pub async fn update(pool: &Pool, role: &RoleUpdate) -> sqlx::Result<bool> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_user_role SET \
             name = COALESCE(?, name), \
             authority = COALESCE(?, authority), \
             update_time = ? \
             WHERE id = ?",
    )
    .into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(&role.name)
            .bind(&role.authority)
            .bind(&role.update_time)
            .bind(role.id)
            .execute(p)
    })?;

    Ok(result.rows_affected() > 0)
}

/// 删除角色
pub async fn delete(pool: &Pool, id: i32) -> sqlx::Result<bool> {
    let sql = crate::db::dialect::current_sql("DELETE FROM gb_user_role WHERE id = ?").into_owned();

    let result = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;

    Ok(result.rows_affected() > 0)
}

/// 检查角色是否存在
pub async fn exists(pool: &Pool, id: i32) -> sqlx::Result<bool> {
    let sql = crate::db::dialect::current_sql("SELECT COUNT(*) FROM gb_user_role WHERE id = ?")
        .into_owned();

    let result: (i64,) =
        crate::dispatch!(pool, |p| { sqlx::query_as(&sql).bind(id).fetch_one(p) })?;

    Ok(result.0 > 0)
}
