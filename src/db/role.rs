//! 角色表 wvp_user_role

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

/// 角色信息结构体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: i32,
    pub name: Option<String>,
    pub authority: Option<String>,
    pub create_time: Option<String>,
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
    sqlx::query_as::<_, Role>(
        "SELECT id, name, authority, create_time, update_time FROM wvp_user_role ORDER BY id",
    )
    .fetch_all(pool)
    .await
}

/// 根据ID查询角色
pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<Role>> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query_as::<_, Role>(
            "SELECT id, name, authority, create_time, update_time FROM wvp_user_role WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query_as::<_, Role>(
            "SELECT id, name, authority, create_time, update_time FROM wvp_user_role WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }
}

/// 根据名称查询角色
pub async fn get_by_name(pool: &Pool, name: &str) -> sqlx::Result<Option<Role>> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query_as::<_, Role>(
            "SELECT id, name, authority, create_time, update_time FROM wvp_user_role WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query_as::<_, Role>(
            "SELECT id, name, authority, create_time, update_time FROM wvp_user_role WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }
}

/// 添加角色
pub async fn add(pool: &Pool, role: &RoleCreate) -> sqlx::Result<i32> {
    #[cfg(feature = "postgres")]
    {
        let result: (i32,) = sqlx::query_as(
            "INSERT INTO wvp_user_role (name, authority, create_time, update_time) \
             VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(&role.name)
        .bind(&role.authority)
        .bind(&role.create_time)
        .bind(&role.create_time)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "INSERT INTO wvp_user_role (name, authority, create_time, update_time) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(&role.name)
        .bind(&role.authority)
        .bind(&role.create_time)
        .bind(&role.create_time)
        .execute(pool)
        .await?;

        Ok(result.last_insert_id() as i32)
    }
}

/// 更新角色
pub async fn update(pool: &Pool, role: &RoleUpdate) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query(
            "UPDATE wvp_user_role SET \
             name = COALESCE($2, name), \
             authority = COALESCE($3, authority), \
             update_time = $4 \
             WHERE id = $1",
        )
        .bind(role.id)
        .bind(&role.name)
        .bind(&role.authority)
        .bind(&role.update_time)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "UPDATE wvp_user_role SET \
             name = COALESCE(?, name), \
             authority = COALESCE(?, authority), \
             update_time = ? \
             WHERE id = ?",
        )
        .bind(&role.name)
        .bind(&role.authority)
        .bind(&role.update_time)
        .bind(role.id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

/// 删除角色
pub async fn delete(pool: &Pool, id: i32) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query("DELETE FROM wvp_user_role WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("DELETE FROM wvp_user_role WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

/// 检查角色是否存在
pub async fn exists(pool: &Pool, id: i32) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wvp_user_role WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?;

        Ok(result.0 > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wvp_user_role WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;

        Ok(result.0 > 0)
    }
}
