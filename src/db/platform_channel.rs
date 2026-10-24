//! 平台通道表 gb_platform_channel

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 平台通道结构体
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlatformChannel {
    pub id: i32,
    // `gb_platform_channel.platform_id` / `device_channel_id` 在 postgres 里是
    // `integer`（对应 `gb_platform.id` / `gb_device_channel.id`，两者 Rust 侧都是
    // i32）。这里曾经写成 `Option<i64>`，sqlite（动态类型）和 mysql（宽进严出）
    // 都不报错，只有 postgres 会在解码时抛
    // `Rust type i64 is not compatible with SQL type INT4`。
    pub platform_id: Option<i32>,
    pub device_channel_id: Option<i32>,
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
    let limit = count.min(100);
    let offset = crate::pagination::offset(page, limit);

    crate::dispatch_dialect!(
        pool,
        |p| {
            sqlx::query_as::<_, PlatformChannel>(
                    "SELECT * FROM gb_platform_channel WHERE platform_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
                )
                .bind(platform_id)
                .bind(limit as i64)
                .bind(offset)
                .fetch_all(p)
                .await
        },
        |p| {
            sqlx::query_as::<_, PlatformChannel>(
                    &crate::dyn_where::dialect_sql("SELECT * FROM gb_platform_channel WHERE platform_id = ? ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(platform_id)
                .bind(limit as i64)
                .bind(offset)
                .fetch_all(p)
                .await
        },
        |p| {
            sqlx::query_as::<_, PlatformChannel>(
                    &crate::dyn_where::dialect_sql("SELECT * FROM gb_platform_channel WHERE platform_id = ? ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(platform_id)
                .bind(limit as i64)
                .bind(offset)
                .fetch_all(p)
                .await
        },
    )
}

/// 统计平台通道数量
pub async fn count_by_platform_id(pool: &Pool, platform_id: i64) -> sqlx::Result<i64> {
    crate::dispatch_dialect!(
        pool,
        |p| {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM gb_platform_channel WHERE platform_id = $1",
            )
            .bind(platform_id)
            .fetch_one(p)
            .await
        },
        |p| {
            sqlx::query_scalar::<_, i64>(&crate::dyn_where::dialect_sql(
                "SELECT COUNT(*) FROM gb_platform_channel WHERE platform_id = ?",
            ))
            .bind(platform_id)
            .fetch_one(p)
            .await
        },
        |p| {
            sqlx::query_scalar::<_, i64>(&crate::dyn_where::dialect_sql(
                "SELECT COUNT(*) FROM gb_platform_channel WHERE platform_id = ?",
            ))
            .bind(platform_id)
            .fetch_one(p)
            .await
        },
    )
}

/// 添加平台通道
pub async fn add(pool: &Pool, platform_id: i64, device_channel_id: i64) -> sqlx::Result<u64> {
    let r = crate::dispatch_exec_dialect!(
        pool,
        |p| {
            sqlx::query(
                "INSERT INTO gb_platform_channel (platform_id, device_channel_id) VALUES ($1, $2)",
            )
            .bind(platform_id)
            .bind(device_channel_id)
            .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "INSERT INTO gb_platform_channel (platform_id, device_channel_id) VALUES (?, ?)",
            ))
            .bind(platform_id)
            .bind(device_channel_id)
            .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "INSERT INTO gb_platform_channel (platform_id, device_channel_id) VALUES (?, ?)",
            ))
            .bind(platform_id)
            .bind(device_channel_id)
            .execute(p)
        },
    )?;

    Ok(r.rows_affected())
}

/// 删除平台通道
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let r = crate::dispatch_exec_dialect!(
        pool,
        |p| {
            sqlx::query("DELETE FROM gb_platform_channel WHERE id = $1")
                .bind(id)
                .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "DELETE FROM gb_platform_channel WHERE id = ?",
            ))
            .bind(id)
            .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "DELETE FROM gb_platform_channel WHERE id = ?",
            ))
            .bind(id)
            .execute(p)
        },
    )?;

    Ok(r.rows_affected())
}

/// 批量删除平台通道
pub async fn batch_delete_by_platform(pool: &Pool, platform_id: i64) -> sqlx::Result<u64> {
    let r = crate::dispatch_exec_dialect!(
        pool,
        |p| {
            sqlx::query("DELETE FROM gb_platform_channel WHERE platform_id = $1")
                .bind(platform_id)
                .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "DELETE FROM gb_platform_channel WHERE platform_id = ?",
            ))
            .bind(platform_id)
            .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "DELETE FROM gb_platform_channel WHERE platform_id = ?",
            ))
            .bind(platform_id)
            .execute(p)
        },
    )?;

    Ok(r.rows_affected())
}

/// 根据ID获取平台通道
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<PlatformChannel>> {
    let row = crate::dispatch_dialect!(
        pool,
        |p| {
            sqlx::query_as::<_, PlatformChannel>("SELECT * FROM gb_platform_channel WHERE id = $1")
                .bind(id)
                .fetch_optional(p)
                .await
        },
        |p| {
            sqlx::query_as::<_, PlatformChannel>(&crate::dyn_where::dialect_sql(
                "SELECT * FROM gb_platform_channel WHERE id = ?",
            ))
            .bind(id)
            .fetch_optional(p)
            .await
        },
        |p| {
            sqlx::query_as::<_, PlatformChannel>(&crate::dyn_where::dialect_sql(
                "SELECT * FROM gb_platform_channel WHERE id = ?",
            ))
            .bind(id)
            .fetch_optional(p)
            .await
        },
    );

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

    Ok(crate::dispatch_dialect!(
        pool,
        |p| {
            let r = match (custom_name, custom_info) {
                        (Some(name), Some(info)) => sqlx::query(
                            "UPDATE gb_platform_channel SET custom_name = $1, custom_address = $2 WHERE id = $3",
                        )
                        .bind(name)
                        .bind(info)
                        .bind(id)
                        .execute(p)
                        .await?,
                        (Some(name), None) => sqlx::query(
                            "UPDATE gb_platform_channel SET custom_name = $1 WHERE id = $2",
                        )
                        .bind(name)
                        .bind(id)
                        .execute(p)
                        .await?,
                        (None, Some(info)) => sqlx::query(
                            "UPDATE gb_platform_channel SET custom_address = $1 WHERE id = $2",
                        )
                        .bind(info)
                        .bind(id)
                        .execute(p)
                        .await?,
                        (None, None) => unreachable!(),
                    };
            Ok::<_, sqlx::Error>(r.rows_affected())
        },
        |p| {
            // SQLite 与 MySQL 共用 ? 占位符；列名来自固定白名单，值只通过 bind 传递。
            let mut updates = Vec::new();
            if custom_name.is_some() {
                updates.push("custom_name = ?");
            }
            if custom_info.is_some() {
                updates.push("custom_address = ?");
            }

            if updates.is_empty() {
                return Ok(0);
            }

            // `{}` = 运行期拼进去的 SET 子句（列名来自固定白名单）；`?` = 占位符
            // （由 **pool 自己的方言** 改写成 `$1..$n`）。
            let sql = pool
                .sql("UPDATE gb_platform_channel SET {} WHERE id = ?")
                .replace("{}", &updates.join(", "));
            let mut query = sqlx::query(&sql);
            if let Some(name) = custom_name {
                query = query.bind(name);
            }
            if let Some(info) = custom_info {
                query = query.bind(info);
            }
            query = query.bind(id);
            let r = query.execute(p).await?;
            Ok::<_, sqlx::Error>(r.rows_affected())
        },
        |p| {
            // SQLite 与 MySQL 共用 ? 占位符；列名来自固定白名单，值只通过 bind 传递。
            let mut updates = Vec::new();
            if custom_name.is_some() {
                updates.push("custom_name = ?");
            }
            if custom_info.is_some() {
                updates.push("custom_address = ?");
            }

            if updates.is_empty() {
                return Ok(0);
            }

            // `{}` = 运行期拼进去的 SET 子句（列名来自固定白名单）；`?` = 占位符
            // （由 **pool 自己的方言** 改写成 `$1..$n`）。
            let sql = pool
                .sql("UPDATE gb_platform_channel SET {} WHERE id = ?")
                .replace("{}", &updates.join(", "));
            let mut query = sqlx::query(&sql);
            if let Some(name) = custom_name {
                query = query.bind(name);
            }
            if let Some(info) = custom_info {
                query = query.bind(info);
            }
            query = query.bind(id);
            let r = query.execute(p).await?;
            Ok::<_, sqlx::Error>(r.rows_affected())
        },
    )?)
}

#[cfg(all(test, feature = "sqlite"))]
mod update_tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_custom_fields_roundtrip() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        let id = crate::dispatch_exec!(pool, |p| {
            sqlx::query(
                "INSERT INTO gb_platform_channel (platform_id, device_channel_id) VALUES (NULL, NULL)",
            )
            .execute(p)
        })
        .unwrap()
        .last_insert_id()
        .expect("sqlite 自增 id");
        assert_eq!(
            update(&pool, id, Some("O'Brien"), Some("north gate"))
                .await
                .unwrap(),
            1
        );
        let sql =
            pool.sql("SELECT custom_name, custom_address FROM gb_platform_channel WHERE id = ?");
        let fields: (Option<String>, Option<String>) = crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, (Option<String>, Option<String>)>(&sql)
                .bind(id)
                .fetch_one(p)
        })
        .unwrap();
        assert_eq!(fields.0.as_deref(), Some("O'Brien"));
        assert_eq!(fields.1.as_deref(), Some("north gate"));
    }
}

/// 根据设备通道ID删除平台通道
pub async fn delete_by_device_channel_id(
    pool: &Pool,
    platform_id: i64,
    device_channel_id: i64,
) -> sqlx::Result<u64> {
    let r = crate::dispatch_exec_dialect!(
        pool,
        |p| {
            sqlx::query(
                "DELETE FROM gb_platform_channel WHERE platform_id = $1 AND device_channel_id = $2",
            )
            .bind(platform_id)
            .bind(device_channel_id)
            .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "DELETE FROM gb_platform_channel WHERE platform_id = ? AND device_channel_id = ?",
            ))
            .bind(platform_id)
            .bind(device_channel_id)
            .execute(p)
        },
        |p| {
            sqlx::query(&crate::dyn_where::dialect_sql(
                "DELETE FROM gb_platform_channel WHERE platform_id = ? AND device_channel_id = ?",
            ))
            .bind(platform_id)
            .bind(device_channel_id)
            .execute(p)
        },
    )?;

    Ok(r.rows_affected())
}

/// 根据平台ID和通道ID查询
pub async fn get_by_platform_and_channel(
    pool: &Pool,
    platform_id: i64,
    channel_id: &str,
) -> sqlx::Result<Option<PlatformChannel>> {
    crate::dispatch_dialect!(
        pool,
        |p| {
            sqlx::query_as::<_, PlatformChannel>(
                "SELECT * FROM gb_platform_channel WHERE platform_id = $1 AND channel_id = $2",
            )
            .bind(platform_id)
            .bind(channel_id)
            .fetch_optional(p)
            .await
        },
        |p| {
            sqlx::query_as::<_, PlatformChannel>(&crate::dyn_where::dialect_sql(
                "SELECT * FROM gb_platform_channel WHERE platform_id = ? AND channel_id = ?",
            ))
            .bind(platform_id)
            .bind(channel_id)
            .fetch_optional(p)
            .await
        },
        |p| {
            sqlx::query_as::<_, PlatformChannel>(&crate::dyn_where::dialect_sql(
                "SELECT * FROM gb_platform_channel WHERE platform_id = ? AND channel_id = ?",
            ))
            .bind(platform_id)
            .bind(channel_id)
            .fetch_optional(p)
            .await
        },
    )
}

/// 批量添加平台通道
pub async fn batch_add_channels(
    pool: &Pool,
    platform_id: i64,
    channel_ids: &[String],
) -> sqlx::Result<u64> {
    if channel_ids.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut total = 0u64;

    for channel_id in channel_ids {
        let r = crate::dispatch_exec_dialect!(
            pool,
            |p| {
                sqlx::query(
                    "INSERT INTO gb_platform_channel (platform_id, channel_id, create_time, update_time) VALUES ($1, $2, $3, $4)",
                )
                .bind(platform_id)
                .bind(channel_id)
                .bind(&now)
                .bind(&now)
                .execute(p)
            },
            |p| {
                sqlx::query(
                    &crate::dyn_where::dialect_sql("INSERT INTO gb_platform_channel (platform_id, channel_id, create_time, update_time) VALUES (?, ?, ?, ?)"),
                )
                .bind(platform_id)
                .bind(channel_id)
                .bind(&now)
                .bind(&now)
                .execute(p)
            },
            |p| {
                sqlx::query(
                    &crate::dyn_where::dialect_sql("INSERT INTO gb_platform_channel (platform_id, channel_id, create_time, update_time) VALUES (?, ?, ?, ?)"),
                )
                .bind(platform_id)
                .bind(channel_id)
                .bind(&now)
                .bind(&now)
                .execute(p)
            },
        )?;
        total += r.rows_affected();
    }
    Ok(total)
}

/// 批量删除平台通道
pub async fn batch_delete_channels(
    pool: &Pool,
    platform_id: i64,
    channel_ids: &[String],
) -> sqlx::Result<u64> {
    if channel_ids.is_empty() {
        return Ok(0);
    }
    let mut total = 0u64;

    for channel_id in channel_ids {
        let r = crate::dispatch_exec_dialect!(
            pool,
            |p| {
                sqlx::query(
                    "DELETE FROM gb_platform_channel WHERE platform_id = $1 AND channel_id = $2",
                )
                .bind(platform_id)
                .bind(channel_id)
                .execute(p)
            },
            |p| {
                sqlx::query(&crate::dyn_where::dialect_sql(
                    "DELETE FROM gb_platform_channel WHERE platform_id = ? AND channel_id = ?",
                ))
                .bind(platform_id)
                .bind(channel_id)
                .execute(p)
            },
            |p| {
                sqlx::query(&crate::dyn_where::dialect_sql(
                    "DELETE FROM gb_platform_channel WHERE platform_id = ? AND channel_id = ?",
                ))
                .bind(platform_id)
                .bind(channel_id)
                .execute(p)
            },
        )?;
        total += r.rows_affected();
    }
    Ok(total)
}
