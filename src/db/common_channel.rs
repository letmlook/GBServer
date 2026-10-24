use crate::db::device::{DeviceChannel, DEVICE_CHANNEL_SELECT_COLUMNS};
use crate::db::Pool;

pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<DeviceChannel>> {
    // `{}` = 运行期拼进去的列名；`?` = 占位符 —— 占位符形态由 **pool 自己的
    // 方言** 决定（postgres 只认 `$1`）。方言必须看 pool，不能看进程级全局
    // `dialect::current()` —— 后者在直接构造 `Pool::Postgres(..)` 的测试里仍是
    // 默认值（sqlite），走错分支就会把 `?` 发给 postgres → `syntax error`。
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE id = ?")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(id)
            .fetch_optional(p)
    });
}

/// 通道可写字段（表单里出现的全部列）。
///
/// 用一个结构体而不是十几个形参：此前每加一个字段都要改 3 个方言分支 + handler
/// 调用点，于是前端表单里的 `manufacturer`/`channelType`/`address`/
/// `streamIdentification` 四列**从来没有被写过**（改了静默丢失）。
#[derive(Debug, Default, Clone)]
pub struct ChannelWriteFields<'a> {
    /// 仅新增用（必填）
    pub device_id: &'a str,
    /// 国标通道编号；`None` 表示更新时不改动（COALESCE）
    pub channel_id: Option<&'a str>,
    pub name: Option<&'a str>,
    /// 新增时写入 `data_device_id`（设备表的自增主键）
    pub data_device_id: Option<i32>,
    // 可选/可更新
    pub civil_code: Option<&'a str>,
    /// 父通道标识。
    ///
    /// `gb_device_channel.parent_id` 是 **VARCHAR**（国标父通道编号是字符串），
    /// 不是整型 —— 早先这里声明成 `Option<i64>`，于是 postgres 上
    /// `COALESCE(bigint, character varying)` 直接报错，**通道编辑在 postgres
    /// 上整个 500**（sqlite/mysql 宽松，能存下但类型也不对）。
    pub parent_id: Option<&'a str>,
    pub business_group: Option<&'a str>,
    pub ptz_type: Option<i32>,
    pub custom_name: Option<&'a str>,
    pub manufacturer: Option<&'a str>,
    pub model: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub address: Option<&'a str>,
    pub stream_identification: Option<&'a str>,
    pub channel_type: Option<i32>,
}

pub async fn update(
    pool: &Pool,
    id: i64,
    f: &ChannelWriteFields<'_>,
    now: &str,
) -> sqlx::Result<u64> {
    let set = "name = COALESCE(?, name), \
               gb_device_id = COALESCE(?, gb_device_id), \
               civil_code = COALESCE(?, civil_code), \
               parent_id = COALESCE(?, parent_id), \
               business_group_id = COALESCE(?, business_group_id), \
               ptz_type = COALESCE(?, ptz_type), \
               custom_name = COALESCE(?, custom_name), \
               manufacturer = COALESCE(?, manufacturer), \
               model = COALESCE(?, model), \
               owner = COALESCE(?, owner), \
               address = COALESCE(?, address), \
               stream_identification = COALESCE(?, stream_identification), \
               channel_type = COALESCE(?, channel_type), \
               update_time = ? \
               WHERE id = ?";
    let stmt = format!("UPDATE gb_device_channel SET {}", set);
    let sql = pool.sql(&stmt).into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(f.name).bind(f.channel_id).bind(f.civil_code).bind(f.parent_id).bind(f.business_group).bind(f.ptz_type).bind(f.custom_name).bind(f.manufacturer).bind(f.model).bind(f.owner).bind(f.address).bind(f.stream_identification).bind(f.channel_type).bind(now).bind(id).execute(p)
    })?;
    Ok(r.rows_affected())
}
pub async fn reset(pool: &Pool, id: i64, now: &str) -> sqlx::Result<u64> {
    let r = if crate::db::dialect::current().is_mysql() {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device_channel SET sub_count = 0, update_time = ? WHERE id = ?",
        )
        .into_owned();

        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql).bind(now).bind(id).execute(p)
        })?
    } else if crate::db::dialect::current().is_postgres() {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device_channel SET sub_count = 0, update_time = $1 WHERE id = $2",
        )
        .into_owned();

        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql).bind(now).bind(id).execute(p)
        })?
    } else {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device_channel SET sub_count = 0, update_time = ? WHERE id = ?",
        )
        .into_owned();

        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql).bind(now).bind(id).execute(p)
        })?
    };
    Ok(r.rows_affected())
}

pub async fn add(pool: &Pool, f: &ChannelWriteFields<'_>, now: &str) -> sqlx::Result<i64> {
    // channel_type 是 NOT NULL DEFAULT 0：显式绑 NULL 会直接违反约束
    let channel_type = f.channel_type.unwrap_or(0);
    let cols = "device_id, name, gb_device_id, civil_code, parent_id, business_group_id, \
                ptz_type, custom_name, manufacturer, model, owner, address, \
                stream_identification, channel_type, create_time, update_time, data_type, data_device_id";
    // 三个方言的 INSERT 语句**方言专属**：
    // * postgres 用 `RETURNING id` 取自增主键（它没有 `last_insert_id()`）；
    // * mysql 用 `last_insert_id()`，sqlite 用 `last_insert_rowid()`。
    //
    // 所以用 `dispatch_dialect!` 分派 —— 它按 **pool 的类型** 选分支，不是按
    // 进程级全局 `dialect::current()`（后者在直接构造 `Pool::Postgres(..)` 的
    // 测试里仍是默认值 sqlite，走错分支就会把 `?` 发给 postgres → `syntax error`）。
    let id: i64 = crate::dispatch_dialect!(
        pool,
        |p| {
            // PostgreSQL
            let sql = format!(
                "INSERT INTO gb_device_channel ({cols}) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,0,$17) RETURNING id"
            );
            let row = sqlx::query_scalar::<_, i32>(&sql)
                .bind(f.device_id)
                .bind(f.name.unwrap_or(""))
                .bind(f.channel_id.unwrap_or(""))
                .bind(f.civil_code)
                .bind(f.parent_id)
                .bind(f.business_group)
                .bind(f.ptz_type)
                .bind(f.custom_name)
                .bind(f.manufacturer)
                .bind(f.model)
                .bind(f.owner)
                .bind(f.address)
                .bind(f.stream_identification)
                .bind(channel_type)
                .bind(now)
                .bind(now)
                .bind(f.data_device_id.unwrap_or(0))
                .fetch_one(p)
                .await?;
            Ok::<i64, sqlx::Error>(row as i64)
        },
        |p| {
            // MySQL
            // `{}` = 运行期拼进去的列名；`?` = 占位符（由 **pool 自己的方言**
            // 改写成 `$1..$n`）。
            let sql = pool
                .sql("INSERT INTO gb_device_channel ({cols}) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,0,?)")
                .replace("{cols}", &cols);
            let result = sqlx::query(&sql)
                .bind(f.device_id)
                .bind(f.name.unwrap_or(""))
                .bind(f.channel_id.unwrap_or(""))
                .bind(f.civil_code)
                .bind(f.parent_id)
                .bind(f.business_group)
                .bind(f.ptz_type)
                .bind(f.custom_name)
                .bind(f.manufacturer)
                .bind(f.model)
                .bind(f.owner)
                .bind(f.address)
                .bind(f.stream_identification)
                .bind(channel_type)
                .bind(now)
                .bind(now)
                .bind(f.data_device_id.unwrap_or(0))
                .execute(p)
                .await?;
            Ok(result.last_insert_id() as i64)
        },
        |p| {
            // SQLite
            let sql = pool
                .sql("INSERT INTO gb_device_channel ({cols}) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,0,?)")
                .replace("{cols}", &cols);
            let result = sqlx::query(&sql)
                .bind(f.device_id)
                .bind(f.name.unwrap_or(""))
                .bind(f.channel_id.unwrap_or(""))
                .bind(f.civil_code)
                .bind(f.parent_id)
                .bind(f.business_group)
                .bind(f.ptz_type)
                .bind(f.custom_name)
                .bind(f.manufacturer)
                .bind(f.model)
                .bind(f.owner)
                .bind(f.address)
                .bind(f.stream_identification)
                .bind(channel_type)
                .bind(now)
                .bind(now)
                .bind(f.data_device_id.unwrap_or(0))
                .execute(p)
                .await?;
            Ok(result.last_insert_rowid())
        },
    )?;
    Ok(id)
}

pub async fn get_unusual_civilcode(
    pool: &Pool,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = crate::pagination::offset(page, count);
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE civil_code IS NULL OR civil_code = '' ORDER BY id LIMIT ? OFFSET ?")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(count as i64)
            .bind(offset)
            .fetch_all(p)
    });
}

pub async fn count_unusual_civilcode(pool: &Pool) -> sqlx::Result<i64> {
    let sql = crate::db::dialect::current_sql(
        "SELECT COUNT(*) FROM gb_device_channel WHERE civil_code IS NULL OR civil_code = ''",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    });
}

/// 取通道表中所有非空且去重的行政区划编码（civil_code），按编码升序。
///
/// 供 `/api/region/sync` 按行政区划补齐区域表使用。
pub async fn list_distinct_civil_codes(pool: &Pool) -> sqlx::Result<Vec<String>> {
    let sql = crate::db::dialect::current_sql(
        "SELECT DISTINCT civil_code FROM gb_device_channel \
         WHERE civil_code IS NOT NULL AND civil_code <> '' ORDER BY civil_code",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, String>(&sql).fetch_all(p)
    });
}

pub async fn get_unusual_parent(
    pool: &Pool,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = crate::pagination::offset(page, count);
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE parent_id IS NULL OR parent_id = '0' ORDER BY id LIMIT ? OFFSET ?")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(count as i64)
            .bind(offset)
            .fetch_all(p)
    });
}

pub async fn count_unusual_parent(pool: &Pool) -> sqlx::Result<i64> {
    let sql = crate::db::dialect::current_sql(
        "SELECT COUNT(*) FROM gb_device_channel WHERE parent_id IS NULL OR parent_id = '0'",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    });
}

pub async fn clear_unusual_civilcode(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device_channel SET civil_code = NULL WHERE id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

pub async fn clear_unusual_parent(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device_channel SET parent_id = '0' WHERE id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

pub async fn get_parent_channels(
    pool: &Pool,
    page: u32,
    count: u32,
    _query: Option<&str>,
    _online: Option<bool>,
    _channel_type: Option<i32>,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = crate::pagination::offset(page, count);
    let cols = DEVICE_CHANNEL_SELECT_COLUMNS;
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE parent_id IS NOT NULL AND parent_id != '0' ORDER BY id LIMIT ? OFFSET ?")
        .replace("{}", cols);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(count as i64)
            .bind(offset)
            .fetch_all(p)
    });
}

pub async fn count_parent_channels(
    pool: &Pool,
    _query: Option<&str>,
    _online: Option<bool>,
    _channel_type: Option<i32>,
) -> sqlx::Result<i64> {
    let sql = crate::db::dialect::current_sql(
        "SELECT COUNT(*) FROM gb_device_channel WHERE parent_id IS NOT NULL AND parent_id != '0'",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    });
}

pub async fn update_civil_code(
    pool: &Pool,
    id: i64,
    civil_code: &str,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device_channel SET civil_code = ?, update_time = ? WHERE id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(civil_code)
            .bind(now)
            .bind(id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn clear_civil_code(pool: &Pool, id: i64, now: &str) -> sqlx::Result<u64> {
    let r = if crate::db::dialect::current().is_mysql() {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device_channel SET civil_code = NULL, update_time = ? WHERE id = ?",
        )
        .into_owned();

        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql).bind(now).bind(id).execute(p)
        })?
    } else if crate::db::dialect::current().is_postgres() {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device_channel SET civil_code = NULL, update_time = $1 WHERE id = $2",
        )
        .into_owned();

        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql).bind(now).bind(id).execute(p)
        })?
    } else {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device_channel SET civil_code = NULL, update_time = ? WHERE id = ?",
        )
        .into_owned();

        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql).bind(now).bind(id).execute(p)
        })?
    };
    Ok(r.rows_affected())
}

pub async fn update_device_civil_code(
    pool: &Pool,
    device_id: &str,
    civil_code: &str,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device_channel SET civil_code = ?, update_time = ? WHERE device_id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(civil_code)
            .bind(now)
            .bind(device_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn clear_device_civil_code(pool: &Pool, device_id: &str, now: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device_channel SET civil_code = NULL, update_time = ? WHERE device_id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(now).bind(device_id).execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn update_group(
    pool: &Pool,
    id: i64,
    parent_id: i64,
    business_group: &str,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("UPDATE gb_device_channel SET parent_id = ?, business_group_id = ?, update_time = ? WHERE id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(parent_id)
            .bind(business_group)
            .bind(now)
            .bind(id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn clear_group(pool: &Pool, id: i64, now: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("UPDATE gb_device_channel SET parent_id = '0', business_group_id = NULL, update_time = ? WHERE id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(now).bind(id).execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn update_device_group(
    pool: &Pool,
    device_id: &str,
    parent_id: i64,
    business_group: &str,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("UPDATE gb_device_channel SET parent_id = ?, business_group_id = ?, update_time = ? WHERE device_id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(parent_id)
            .bind(business_group)
            .bind(now)
            .bind(device_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn clear_device_group(pool: &Pool, device_id: &str, now: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("UPDATE gb_device_channel SET parent_id = '0', business_group_id = NULL, update_time = ? WHERE device_id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(now).bind(device_id).execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn get_channels_for_map(
    pool: &Pool,
    _query: Option<&str>,
    _online: Option<bool>,
    _channel_type: Option<i32>,
) -> sqlx::Result<Vec<DeviceChannel>> {
    if crate::db::dialect::current().is_mysql() {
        return crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(
                &format!("SELECT {} FROM gb_device_channel WHERE longitude IS NOT NULL AND latitude IS NOT NULL ORDER BY id LIMIT 1000", DEVICE_CHANNEL_SELECT_COLUMNS),
            )
            .fetch_all(p)
        });
    } else if crate::db::dialect::current().is_postgres() {
        return crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(
                &format!("SELECT {} FROM gb_device_channel WHERE longitude IS NOT NULL AND latitude IS NOT NULL ORDER BY id LIMIT 1000", DEVICE_CHANNEL_SELECT_COLUMNS),
            )
            .fetch_all(p)
        });
    } else {
        return crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(
                &format!("SELECT {} FROM gb_device_channel WHERE longitude IS NOT NULL AND latitude IS NOT NULL ORDER BY id LIMIT 1000", DEVICE_CHANNEL_SELECT_COLUMNS),
            )
            .fetch_all(p)
        });
    }
}

pub async fn update_map_level(pool: &Pool, channel_ids: &[i64], level: i32) -> sqlx::Result<u64> {
    if channel_ids.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // IN 列表的占位符编号：mysql / sqlite 用 `?`（按顺序绑定），
    // postgres 用 `$3` 起（前面已经有 map_level / update_time 两个 bind）。
    // 整条语句的**前两个**占位符由 `pool.sql(..)` 改写，IN 列表按 pool 方言
    // 自己编号 —— bind 的个数与顺序三方完全一致。
    let placeholders: Vec<String> = channel_ids
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if pool.dialect().is_postgres() {
                format!("${}", i + 3)
            } else {
                "?".to_string()
            }
        })
        .collect();
    let sql = pool
        .sql("UPDATE gb_device_channel SET map_level = ?, update_time = ? WHERE id IN ({})")
        .replace("{}", &placeholders.join(","));
    let r = crate::dispatch_exec!(pool, |p| {
        let mut q = sqlx::query(&sql).bind(level).bind(&now);
        for id in channel_ids {
            q = q.bind(id);
        }
        q.execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn reset_map_level(pool: &Pool) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device_channel SET map_level = 0, update_time = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(&now).execute(p) })?;
    Ok(r.rows_affected())
}

/// Migration: add custom_name column to gb_device_channel if missing.
/// The channel_add / channel_update functions reference `custom_name` but
/// the original SQLite schema doesn't include it. Adding via ALTER TABLE
/// (idempotent: swallows "duplicate column" errors).
pub async fn ensure_columns(pool: &Pool) -> sqlx::Result<()> {
    let sql = crate::db::dialect::current_sql(
        "ALTER TABLE gb_device_channel ADD COLUMN custom_name VARCHAR(255)",
    )
    .into_owned();

    let _ = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).execute(p) });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sqlite_pool_with_schema;

    async fn seed_channel(pool: &Pool, device_id: &str, gb_id: &str, civil: Option<&str>) {
        let sql = crate::db::dialect::current_sql("INSERT INTO gb_device_channel \
             (device_id, name, gb_device_id, civil_code, status, data_type, data_device_id, \
              longitude, latitude, parent_id, create_time, update_time) \
             VALUES (?, ?, ?, ?, 'ON', 0, 0, 118.7, 32.0, '0', '2026-01-01 00:00:00', '2026-01-01 00:00:00')").into_owned();

        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(device_id)
                .bind(format!("ch-{}", gb_id))
                .bind(gb_id)
                .bind(civil)
                .execute(p)
        })
        .expect("insert channel");
    }

    /// 回归保护：SELECT 覆盖 DeviceChannel **全部**列。
    ///
    /// 2026-09-11：`get_by_id` / `get_unusual_parent` / `get_unusual_civilcode` /
    /// `get_channels_for_map` 曾使用只有 12 列的残缺清单，运行时一律报
    /// `no column found for name: manufacturer` —— 导致所有走
    /// `lookup_channel_and_send` 的 PTZ/预置位/雨刷/光圈/巡航端点以及
    /// `/api/common/channel/map/list` 全部 500。
    /// 该缺陷不会被编译期发现，只能靠真实的查询执行来兜住。
    #[tokio::test]
    async fn test_device_channel_queries_return_full_row() {
        let pool = sqlite_pool_with_schema().await;
        seed_channel(&pool, "dev1", "34020000001310000001", Some("340200")).await;

        let by_id = get_by_id(&pool, 1)
            .await
            .expect("get_by_id 不得因缺列而失败");
        assert!(by_id.is_some());

        let unusual_parent = get_unusual_parent(&pool, 1, 10)
            .await
            .expect("get_unusual_parent 不得因缺列而失败");
        assert_eq!(unusual_parent.len(), 1);

        let unusual_civil = get_unusual_civilcode(&pool, 1, 10)
            .await
            .expect("get_unusual_civilcode 不得因缺列而失败");
        assert!(unusual_civil.is_empty(), "civil_code 非空，不应命中");

        let map_rows = get_channels_for_map(&pool, None, None, None)
            .await
            .expect("get_channels_for_map 不得因缺列而失败");
        assert_eq!(map_rows.len(), 1);
        // 经纬度必须真实带出（地图相关端点的前提）
        assert_eq!(map_rows[0].longitude, Some(118.7));
        assert_eq!(map_rows[0].latitude, Some(32.0));
    }
}
