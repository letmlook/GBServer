//! 平台通道表 wvp_platform_channel

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 平台通道结构体
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlatformChannel {
    pub id: i32,
    pub platform_id: Option<i64>,
    pub device_channel_id: Option<i64>,
    pub custom_device_id: Option<String>,
    pub custom_name: Option<String>,
    pub custom_manufacturer: Option<String>,
    pub custom_model: Option<String>,
    pub custom_owner: Option<String>,
    pub custom_civil_code: Option<String>,
    pub custom_block: Option<String>,
    pub custom_address: Option<String>,
    pub custom_parental: Option<i32>,
    pub custom_parent_id: Option<String>,
    pub custom_safety_way: Option<i32>,
    pub custom_register_way: Option<i32>,
    pub custom_cert_num: Option<String>,
    pub custom_certifiable: Option<i32>,
    pub custom_err_code: Option<i32>,
    pub custom_end_time: Option<String>,
    pub custom_secrecy: Option<i32>,
    pub custom_ip_address: Option<String>,
    pub custom_port: Option<i32>,
    pub custom_password: Option<String>,
    pub custom_status: Option<String>,
    pub custom_longitude: Option<f64>,
    pub custom_latitude: Option<f64>,
    pub custom_ptz_type: Option<i32>,
    pub custom_position_type: Option<i32>,
    pub custom_room_type: Option<i32>,
    pub custom_use_type: Option<i32>,
    pub custom_supply_light_type: Option<i32>,
    pub custom_direction_type: Option<i32>,
    pub custom_resolution: Option<String>,
    pub custom_business_group_id: Option<String>,
    pub custom_download_speed: Option<String>,
    pub custom_svc_space_support_mod: Option<i32>,
    pub custom_svc_time_support_mode: Option<i32>,
}

/// 根据平台ID获取通道列表
pub async fn list_by_platform_id(
    pool: &Pool,
    platform_id: i64,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<PlatformChannel>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, PlatformChannel>(
        "SELECT * FROM wvp_platform_channel WHERE platform_id = ? ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(platform_id)
    .bind(limit)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, PlatformChannel>(
        "SELECT * FROM wvp_platform_channel WHERE platform_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
    )
    .bind(platform_id)
    .bind(limit)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
}

/// 统计平台通道数量
pub async fn count_by_platform_id(pool: &Pool, platform_id: i64) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_platform_channel WHERE platform_id = ?"
    )
    .bind(platform_id)
    .fetch_one(pool)
    .await;
    
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_platform_channel WHERE platform_id = $1"
    )
    .bind(platform_id)
    .fetch_one(pool)
    .await;
}

/// 添加平台通道
pub async fn add(
    pool: &Pool,
    platform_id: i64,
    device_channel_id: i64,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO wvp_platform_channel (platform_id, device_channel_id) VALUES (?, ?)",
    )
    .bind(platform_id)
    .bind(device_channel_id)
    .execute(pool)
    .await?;
    
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO wvp_platform_channel (platform_id, device_channel_id) VALUES ($1, $2)",
    )
    .bind(platform_id)
    .bind(device_channel_id)
    .execute(pool)
    .await?;
    
    Ok(r.rows_affected())
}

/// 删除平台通道
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_platform_channel WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_platform_channel WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    
    Ok(r.rows_affected())
}

/// 批量删除平台通道
pub async fn batch_delete_by_platform(pool: &Pool, platform_id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_platform_channel WHERE platform_id = ?")
        .bind(platform_id)
        .execute(pool)
        .await?;
    
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_platform_channel WHERE platform_id = $1")
        .bind(platform_id)
        .execute(pool)
        .await?;
    
    Ok(r.rows_affected())
}


/// 根据ID获取平台通道
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<PlatformChannel>> {
    #[cfg(feature = "mysql")]
    let row = sqlx::query_as::<_, PlatformChannel>(
        "SELECT * FROM wvp_platform_channel WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    
    #[cfg(feature = "postgres")]
    let row = sqlx::query_as::<_, PlatformChannel>(
        "SELECT * FROM wvp_platform_channel WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    
    row
}

/// 更新平台通道自定义信息
pub async fn update(
    pool: &Pool,
    id: i64,
    custom_name: Option<&str>,
    custom_info: Option<&str>,
) -> sqlx::Result<u64> {
    if custom_name.is_none() && custom_info.is_none() {
        return Ok(0);
    }
    
    #[cfg(feature = "mysql")]
    {
        let mut updates = Vec::new();
        let mut query = sqlx::query("UPDATE wvp_platform_channel SET ");
        
        if let Some(name) = custom_name {
            updates.push("custom_name = ?");
            query = query.bind(name);
        }
        if let Some(info) = custom_info {
            updates.push("custom_address = ?");
            query = query.bind(info);
        }
        
        if updates.is_empty() {
            return Ok(0);
        }
        
        let sql = format!("{} WHERE id = ?", updates.join(", "));
        let mut query = sqlx::query(&sql);
        if let Some(name) = custom_name {
            query = query.bind(name);
        }
        if let Some(info) = custom_info {
            query = query.bind(info);
        }
        query = query.bind(id);
        let r = query.execute(pool).await?;
        return Ok(r.rows_affected());
    }
    
    #[cfg(feature = "postgres")]
    {
        let mut updates = Vec::new();
        
        if let Some(name) = custom_name {
            updates.push(format!("custom_name = '{}'", name.replace("'", "''")));
        }
        if let Some(info) = custom_info {
            updates.push(format!("custom_address = '{}'", info.replace("'", "''")));
        }
        
        if updates.is_empty() {
            return Ok(0);
        }
        
        let sql = format!("{} WHERE id = $1", updates.join(", "));
        let r = sqlx::query(&sql)
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(r.rows_affected());
    }
}

/// 根据设备通道ID删除平台通道
pub async fn delete_by_device_channel_id(pool: &Pool, platform_id: i64, device_channel_id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "DELETE FROM wvp_platform_channel WHERE platform_id = ? AND device_channel_id = ?"
    )
    .bind(platform_id)
    .bind(device_channel_id)
    .execute(pool)
    .await?;
    
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "DELETE FROM wvp_platform_channel WHERE platform_id = $1 AND device_channel_id = $2"
    )
    .bind(platform_id)
    .bind(device_channel_id)
    .execute(pool)
    .await?;
    
    Ok(r.rows_affected())
}