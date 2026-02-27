//! 角色表 wvp_user_role

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Role {
    pub id: i64,
    pub name: Option<String>,
    pub authority: Option<String>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

pub async fn list_all(pool: &Pool) -> sqlx::Result<Vec<Role>> {
    sqlx::query_as::<_, Role>(
        "SELECT id, name, authority, create_time, update_time FROM wvp_user_role ORDER BY id",
    )
    .fetch_all(pool)
    .await
}
