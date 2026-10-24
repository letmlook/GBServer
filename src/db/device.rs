//! 国标设备与通道表 gb_device, gb_device_channel

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 国标设备完整信息
///
/// `rename_all = "camelCase"` 与前端 Vue 页面保持一致：
/// 前端表格列通过 `scope.row.deviceId`、`scope.row.streamMode`、
/// `scope.row.onLine`、`scope.row.subscribeCycleForCatalog` 等
/// camelCase 属性访问；后端返回 snake_case 会导致列表列空白，
/// 以及基于 `row.deviceId` 的删除请求 URL 退化为
/// `/api/device/query/devices/undefined/delete`（看似删除无效果）。
#[derive(Debug, Clone, Default, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: i32,
    pub device_id: String,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub transport: Option<String>,
    pub stream_mode: Option<String>,
    pub on_line: Option<bool>,
    pub register_time: Option<String>,
    pub keepalive_time: Option<String>,
    pub ip: Option<String>,
    pub port: Option<i32>,
    pub expires: Option<i32>,
    /// 心跳间隔/次数：列一直在库里，但结构体与 SELECT 列表都没有 →
    /// 前端编辑框里的心跳参数永远显示为空（改了也读不回来）。
    pub heart_beat_interval: Option<i32>,
    pub heart_beat_count: Option<i32>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub media_server_id: Option<String>,
    pub custom_name: Option<String>,
    pub charset: Option<String>,
    pub ssrc_check: Option<bool>,
    pub geo_coord_sys: Option<String>,
    pub sdp_ip: Option<String>,
    pub local_ip: Option<String>,
    pub password: Option<String>,
    pub subscribe_cycle_for_catalog: Option<i32>,
    pub subscribe_cycle_for_mobile_position: Option<i32>,
    pub mobile_position_submission_interval: Option<i32>,
    pub host_address: Option<String>,
    pub channel_count: Option<i32>,
    pub server_id: Option<String>,
}

/// Phase 7 audit fix: full column list matching the Device struct above.
/// Used by query_devices_paged and related endpoints to avoid
/// "no column found for name: firmware" 500 errors when callers
/// (e.g. the web frontend) request all device fields.
pub const DEVICE_SELECT_COLUMNS: &str = "id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, channel_count, server_id";

/// 设备通道完整列名集合,与 [`DeviceChannel`] 字段一一对应。
/// Phase 4.x: 所有 SELECT DeviceChannel 的查询必须使用该常量,避免
/// "no column found for name: <field>" 500 错误。
pub const DEVICE_CHANNEL_SELECT_COLUMNS: &str = "id, device_id, name, manufacturer, model, owner, civil_code, address, parental, parent_id, gb_device_id, status, longitude, latitude, ptz_type, create_time, update_time, sub_count, stream_id, has_audio, stream_identification, channel_type, map_level, gb_name, gb_manufacturer, gb_model, gb_civil_code, gb_address, gb_status, gb_longitude, gb_latitude, record_plan_id";

/// 设备通道完整信息
#[derive(Debug, Clone, Default, Serialize, FromRow)]
pub struct DeviceChannel {
    pub id: i32,
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub owner: Option<String>,
    pub civil_code: Option<String>,
    pub address: Option<String>,
    pub parental: Option<i32>,
    pub parent_id: Option<String>,
    #[serde(rename = "channelId")]
    pub gb_device_id: Option<String>,
    pub status: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub ptz_type: Option<i32>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub sub_count: Option<i32>,
    pub stream_id: Option<String>,
    pub has_audio: Option<bool>,
    pub stream_identification: Option<String>,
    pub channel_type: Option<i32>,
    pub map_level: Option<i32>,
    pub gb_name: Option<String>,
    pub gb_manufacturer: Option<String>,
    pub gb_model: Option<String>,
    pub gb_civil_code: Option<String>,
    pub gb_address: Option<String>,
    pub gb_status: Option<String>,
    pub gb_longitude: Option<f64>,
    pub gb_latitude: Option<f64>,
    pub record_plan_id: Option<i32>,
}

pub async fn update_channel_has_audio(
    pool: &Pool,
    channel_id: i64,
    has_audio: bool,
) -> sqlx::Result<u64> {
    let sql =
        crate::db::dialect::current_sql("UPDATE gb_device_channel SET has_audio = ? WHERE id = ?")
            .into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(has_audio)
            .bind(channel_id)
            .execute(p)
    })?;

    Ok(result.rows_affected())
}

pub async fn update_channel_stream_identification(
    pool: &Pool,
    channel_id: i64,
    stream_identification: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device_channel SET stream_identification = ? WHERE id = ?",
    )
    .into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(stream_identification)
            .bind(channel_id)
            .execute(p)
    })?;

    Ok(result.rows_affected())
}

/// 补全 `gb_device` 里**缺失**的 `transport` / `stream_mode`。
///
/// 为什么需要它：注册成功时写库的 `upsert_device` 曾经把这两列都传 `None`，
/// 于是**所有设备的「信令」和「流模式」列永远是空的**（前端显示 "-"）。
///
/// 语义要点：
/// * **已有值不覆盖** —— `COALESCE(NULLIF(col,''), ?)` 只在列为 NULL 或空串时
///   才写入。免得管理员在设备编辑里选了 `TCP-ACTIVE`，下一次注册/心跳就被
///   这里的默认值冲掉。
/// * 用 `NULLIF(col,'')` 而不是 `col IS NULL`，一并处理历史遗留的空串。
/// * 幂等：值已存在时是一条 no-op UPDATE，重复调用无副作用。
///
/// 调用时机：
/// * REGISTER 成功 —— 用 Via 头解析出的真实信令（UDP/TCP）
/// * 心跳 —— 让**修复前就已注册**的老设备在 1 个心跳周期内（默认 60s）
///   把历史 NULL 补上，不必等到下次重新注册（可能是一小时后）
pub async fn fill_device_transport_if_missing(
    pool: &Pool,
    device_id: &str,
    transport: Option<&str>,
    stream_mode: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_device SET \
         transport = COALESCE(NULLIF(transport, ''), ?), \
         stream_mode = COALESCE(NULLIF(stream_mode, ''), ?), \
         update_time = ? \
         WHERE device_id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(transport)
            .bind(stream_mode)
            .bind(now)
            .bind(device_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn update_device_stream_mode(
    pool: &Pool,
    device_id: &str,
    stream_mode: &str,
) -> sqlx::Result<u64> {
    if crate::db::dialect::current().is_mysql() {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device SET stream_mode = ?, update_time = NOW() WHERE device_id = ?",
        )
        .into_owned();

        let result = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(stream_mode)
                .bind(device_id)
                .execute(p)
        })?;
        Ok(result.rows_affected())
    } else if crate::db::dialect::current().is_postgres() {
        let sql = crate::db::dialect::current_sql(
            "UPDATE gb_device SET stream_mode = $1, update_time = NOW() WHERE device_id = $2",
        )
        .into_owned();

        let result = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(stream_mode)
                .bind(device_id)
                .execute(p)
        })?;
        Ok(result.rows_affected())
    } else {
        let sql = crate::db::dialect::current_sql("UPDATE gb_device SET stream_mode = ?, update_time = datetime('now') WHERE device_id = ?").into_owned();

        let result = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(stream_mode)
                .bind(device_id)
                .execute(p)
        })?;
        Ok(result.rows_affected())
    }
}

pub async fn update_device_catalog_subscription(
    pool: &Pool,
    device_id: &str,
    cycle: i32,
) -> sqlx::Result<u64> {
    let sql = match crate::db::dialect::current() {
        crate::db::Dialect::MySql => crate::db::dialect::current_sql("UPDATE gb_device SET subscribe_cycle_for_catalog = ?, update_time = NOW() WHERE device_id = ?").into_owned(),
        crate::db::Dialect::Sqlite => crate::db::dialect::current_sql("UPDATE gb_device SET subscribe_cycle_for_catalog = ?, update_time = datetime('now') WHERE device_id = ?").into_owned(),
        _ => crate::db::dialect::current_sql("UPDATE gb_device SET subscribe_cycle_for_catalog = $1, update_time = NOW() WHERE device_id = $2").into_owned(),
    };

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(cycle).bind(device_id).execute(p)
    })?;

    Ok(result.rows_affected())
}

pub async fn update_device_mobile_position_subscription(
    pool: &Pool,
    device_id: &str,
    cycle: i32,
    interval: i32,
) -> sqlx::Result<u64> {
    let sql = match crate::db::dialect::current() {
        crate::db::Dialect::MySql => crate::db::dialect::current_sql("UPDATE gb_device SET subscribe_cycle_for_mobile_position = ?, mobile_position_submission_interval = ?, update_time = NOW() WHERE device_id = ?").into_owned(),
        crate::db::Dialect::Sqlite => crate::db::dialect::current_sql("UPDATE gb_device SET subscribe_cycle_for_mobile_position = ?, mobile_position_submission_interval = ?, update_time = datetime('now') WHERE device_id = ?").into_owned(),
        _ => crate::db::dialect::current_sql("UPDATE gb_device SET subscribe_cycle_for_mobile_position = $1, mobile_position_submission_interval = $2, update_time = NOW() WHERE device_id = $3").into_owned(),
    };

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(cycle)
            .bind(interval)
            .bind(device_id)
            .execute(p)
    })?;

    Ok(result.rows_affected())
}

/// Count total device channels across all devices
pub async fn count_all_channels(pool: &Pool) -> sqlx::Result<i64> {
    let sql =
        crate::db::dialect::current_sql("SELECT COUNT(*) FROM gb_device_channel").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    });
}

/// Count channels for online devices only
pub async fn count_online_channels(pool: &Pool) -> sqlx::Result<i64> {
    let sql = crate::db::dialect::current_sql("SELECT COUNT(*) FROM gb_device_channel dc INNER JOIN gb_device d ON dc.device_id = d.device_id WHERE d.on_line = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).bind(true).fetch_one(p)
    });
}

pub async fn list_devices_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
    status: Option<bool>,
) -> sqlx::Result<Vec<Device>> {
    // 上限 1000：摄像机树一次要 1000 条。此前硬截到 100，第 101 台设备及其通道
    // 会静默消失（响应里的 count 还是请求值）。想限制页大小的接口自行 clamp
    // （`/api/device/query/devices` 就是 `.min(100)`）。
    let limit = count.min(1000) as i64;
    // OFFSET 的步长必须跟 LIMIT 的页长一致（都用 clamp 后的 count），
    // 否则 count 大于上限时翻页会跳过一整段数据。
    let offset = crate::pagination::offset(page, count.min(1000));
    let q = query.unwrap_or("").trim();
    let has_query = !q.is_empty();
    let like = format!("%{q}%");

    let rows = if crate::db::dialect::current().is_mysql() {
        if has_query && status.is_some() {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device WHERE (device_id LIKE ? OR name LIKE ?) AND on_line = ? ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(&like).bind(&like).bind(status.unwrap()).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else if has_query {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device WHERE (device_id LIKE ? OR name LIKE ?) ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(&like).bind(&like).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else if status.is_some() {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device WHERE on_line = ? ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(status.unwrap()).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(limit).bind(offset)
                .fetch_all(p)
            })?
        }
    } else if crate::db::dialect::current().is_postgres() {
        if has_query && status.is_some() {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    "SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, CAST((SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS INTEGER) AS channel_count, server_id FROM gb_device WHERE (device_id LIKE $1 OR name LIKE $2) AND on_line = $3 ORDER BY id LIMIT $4 OFFSET $5",
                )
                .bind(&like).bind(&like).bind(status.unwrap()).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else if has_query {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    "SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, CAST((SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS INTEGER) AS channel_count, server_id FROM gb_device WHERE (device_id LIKE $1 OR name LIKE $2) ORDER BY id LIMIT $3 OFFSET $4",
                )
                .bind(&like).bind(&like).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else if status.is_some() {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    "SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, CAST((SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS INTEGER) AS channel_count, server_id FROM gb_device WHERE on_line = $1 ORDER BY id LIMIT $2 OFFSET $3",
                )
                .bind(status.unwrap()).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    "SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, CAST((SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS INTEGER) AS channel_count, server_id FROM gb_device ORDER BY id LIMIT $1 OFFSET $2",
                )
                .bind(limit).bind(offset)
                .fetch_all(p)
            })?
        }
    } else {
        if has_query && status.is_some() {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device WHERE (device_id LIKE ? OR name LIKE ?) AND on_line = ? ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(&like).bind(&like).bind(status.unwrap()).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else if has_query {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device WHERE (device_id LIKE ? OR name LIKE ?) ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(&like).bind(&like).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else if status.is_some() {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device WHERE on_line = ? ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(status.unwrap()).bind(limit).bind(offset)
                .fetch_all(p)
            })?
        } else {
            crate::dispatch!(pool, |p| {
                sqlx::query_as::<_, Device>(
                    &crate::dyn_where::dialect_sql("SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position, mobile_position_submission_interval, host_address, (SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count, server_id FROM gb_device ORDER BY id LIMIT ? OFFSET ?"),
                )
                .bind(limit).bind(offset)
                .fetch_all(p)
            })?
        }
    };
    Ok(rows)
}

pub async fn count_devices(
    pool: &Pool,
    query: Option<&str>,
    status: Option<bool>,
) -> sqlx::Result<i64> {
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();
    let sql = crate::db::dialect::current_sql(
        "SELECT COUNT(*) FROM gb_device WHERE (device_id LIKE ? OR name LIKE ?) AND on_line = ?",
    )
    .into_owned();
    // 带 `?` 的 SQL 必须先在**宏外面**算好：`dialect_sql(...)` 返回的临时
    // `Cow` 在宏体块尾就会被析构，而查询 future 还借着它。
    let sql_like = crate::dyn_where::dialect_sql(
        "SELECT COUNT(*) FROM gb_device WHERE (device_id LIKE ? OR name LIKE ?)",
    );
    let sql_online =
        crate::dyn_where::dialect_sql("SELECT COUNT(*) FROM gb_device WHERE on_line = ?");
    // 宏体**不能**带 `.await`（`dispatch!` 自己 await），也不能用 `return`
    // （那会让分支块的值变成 `!`）—— 改成一条 if/else 链，尾部是表达式。
    crate::dispatch!(pool, |p| {
        if has_query && status.is_some() {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(status.unwrap())
                .fetch_one(p)
        } else if has_query {
            sqlx::query_scalar::<_, i64>(&sql_like)
                .bind(&like)
                .bind(&like)
                .fetch_one(p)
        } else if status.is_some() {
            sqlx::query_scalar::<_, i64>(&sql_online)
                .bind(status.unwrap())
                .fetch_one(p)
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device").fetch_one(p)
        }
    })
}

pub async fn get_device_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<Option<Device>> {
    // Phase 4.x: SELECT 必须与 Device struct 字段一一对应,否则 FromRow 报
    // ColumnNotFound。所有 db 列都要包含,新增字段(host_address、
    // channel_count、server_id)也要同步加上。
    //
    // channel_count 用 (SELECT COUNT(*) FROM gb_device_channel ...) 子查询
    // 而不是直接列引用,与 list_devices_paged 同模式。这样不依赖 gb_device
    // 表存在 channel_count 列——早期 schema 升级前的存量库缺这列,直接
    // SELECT 会触发 "no such column: channel_count",handler 把 Err 当
    // None 返回 null,前端 /api/device/query/devices/:id 拿不到 onLine,
    // 国标通道列表的"播放"按钮被 :disabled="!device" 锁死。
    let count_expr = if crate::db::dialect::current().is_postgres() {
        // PostgreSQL 需要显式 cast（COUNT(*) 是 bigint，Rust 侧按 i32 解）
        "(SELECT COUNT(*)::INTEGER FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count"
    } else {
        "(SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS channel_count"
    };
    let cols = DEVICE_SELECT_COLUMNS.replace("channel_count", count_expr);
    // SQL 文本只写一份 `?`，由 **pool 自己的方言** 在运行期改写成 `$1..$n`
    // （postgres 不认 `?`）。方言必须看 pool，不能看进程级全局
    // `dialect::current()` —— 后者在直接构造 `Pool::Postgres(..)` 的测试里
    // 仍是默认值（sqlite），会让 postgres 收到 `?` → `syntax error`。
    // `{}` 是运行期拼进去的列名，`?` 是占位符 —— 占位符形态由 **pool 自己的
    // 方言** 决定（postgres 只认 `$1`），所以先把模板交给 `pool.sql(..)` 改写，
    // 再把列名填进去（顺序不能反）。
    let sql = pool
        .sql("SELECT {} FROM gb_device WHERE device_id = ?")
        .replace("{}", &cols);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Device>(&sql)
            .bind(device_id)
            .fetch_optional(p)
    });
}

pub async fn list_channels_paged(
    pool: &Pool,
    device_id: &str,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = crate::pagination::offset(page, count);
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE device_id = ? ORDER BY id LIMIT ? OFFSET ?")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(device_id)
            .bind(count as i64)
            .bind(offset)
            .fetch_all(p)
    });
}

/// 某设备下的通道（可带关键字/在线/类型过滤）。
///
/// 与 `list_channels_paged` 的差别就是多了过滤条件：设备详情页的通道列表
/// 支持按名称/编号搜索、按在线状态与通道类型筛选，此前这三个参数后端压根没读。
pub async fn list_channels_filtered(
    pool: &Pool,
    device_id: &str,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
    page: u32,
    count: u32,
) -> sqlx::Result<(Vec<DeviceChannel>, i64)> {
    use crate::dyn_where::{BindValue, DynWhere};
    let offset = crate::pagination::offset(page, count);

    let mut w = DynWhere::new();
    w.add(
        "device_id = ?",
        vec![BindValue::Text(device_id.to_string())],
    );
    if let Some(kw) = query.map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{kw}%");
        w.add(
            "(name LIKE ? OR gb_device_id LIKE ?)",
            vec![BindValue::Text(like.clone()), BindValue::Text(like)],
        );
    }
    if let Some(on) = online {
        w.add(
            "status = ?",
            vec![BindValue::Text(if on { "ON" } else { "OFF" }.to_string())],
        );
    }
    if let Some(t) = channel_type {
        w.add("channel_type = ?", vec![BindValue::Int(t)]);
    }

    let base = format!(
        "SELECT {} FROM gb_device_channel",
        DEVICE_CHANNEL_SELECT_COLUMNS
    );
    let limit_ph = if crate::db::dialect::current().is_postgres() {
        format!(
            " LIMIT ${} OFFSET ${}",
            w.binds.len() + 1,
            w.binds.len() + 2
        )
    } else {
        " LIMIT ? OFFSET ?".to_string()
    };
    let sql_rows = format!("{}{} ORDER BY id{limit_ph}", w.sql(&base), "");
    let rows = crate::dispatch!(pool, |p| {
        let mut q = sqlx::query_as::<_, DeviceChannel>(&sql_rows);
        for b in &w.binds {
            q = match b {
                BindValue::Text(v) => q.bind(v.as_str()),
                BindValue::Int(v) => q.bind(*v),
                BindValue::Big(v) => q.bind(*v),
                BindValue::Bool(v) => q.bind(*v),
            };
        }
        q.bind(count as i64).bind(offset).fetch_all(p)
    })?;

    let count_sql = w.sql("SELECT COUNT(*) FROM gb_device_channel");
    let total = crate::dispatch!(pool, |p| {
        let mut cq = sqlx::query_scalar::<_, i64>(&count_sql);
        for b in &w.binds {
            cq = match b {
                BindValue::Text(v) => cq.bind(v.as_str()),
                BindValue::Int(v) => cq.bind(*v),
                BindValue::Big(v) => cq.bind(*v),
                BindValue::Bool(v) => cq.bind(*v),
            };
        }
        cq.fetch_one(p)
    })?;
    Ok((rows, total))
}

pub async fn count_channels(pool: &Pool, device_id: &str) -> sqlx::Result<i64> {
    let sql = crate::db::dialect::current_sql(
        "SELECT COUNT(*) FROM gb_device_channel WHERE device_id = ?",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql)
            .bind(device_id)
            .fetch_one(p)
    });
}

pub async fn get_channel_by_device_and_channel_id(
    pool: &Pool,
    device_id: &str,
    channel_id: &str,
) -> sqlx::Result<Option<DeviceChannel>> {
    let id_val = channel_id.parse::<i64>().unwrap_or(0);
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE device_id = ? AND (gb_device_id = ? OR id = ?) LIMIT 1")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(device_id)
            .bind(channel_id)
            .bind(id_val)
            .fetch_optional(p)
    });
}

/// 按**主键 id** 取通道行（`/api/device/query/channel/raw?id=` 的数据源）。
///
/// 通道编辑弹窗先按主键拉原始行做回显；此前本平台没有按主键取通道的
/// 查询，该端点在路由里根本不存在。
pub async fn get_channel_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<DeviceChannel>> {
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE id = ?")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(id)
            .fetch_optional(p)
    });
}

/// 按**通道国标 ID** 反查通道（不限设备）。
///
/// 级联场景下上级平台只给出本级通道编码，平台必须据此找到它挂在哪个
/// 国标设备下才能拉起媒体流。此前没有这个查询，`handle_invite` 的级联分支
/// 因而无法把通道映射到设备。
pub async fn get_channel_by_channel_id(
    pool: &Pool,
    channel_id: &str,
) -> sqlx::Result<Option<DeviceChannel>> {
    let id_val = channel_id.parse::<i64>().unwrap_or(0);
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE gb_device_id = ? OR id = ? LIMIT 1")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(channel_id)
            .bind(id_val)
            .fetch_optional(p)
    });
}

pub async fn list_channels_by_parent(
    pool: &Pool,
    device_id: &str,
    parent_channel_id: &str,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE device_id = ? AND (parent_id = ? OR gb_parent_id = ?) ORDER BY id")
        .replace("{}", DEVICE_CHANNEL_SELECT_COLUMNS);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(device_id)
            .bind(parent_channel_id)
            .bind(parent_channel_id)
            .fetch_all(p)
    });
}

pub async fn list_channels_for_device(
    pool: &Pool,
    device_id: &str,
) -> sqlx::Result<Vec<DeviceChannel>> {
    // Phase 4.x: SELECT 必须覆盖 DeviceChannel 全部字段,否则 FromRow 报
    // "no column found for name: <field>" 500。
    let cols = "id, device_id, name, manufacturer, model, owner, civil_code, address, parental, parent_id, gb_device_id, status, longitude, latitude, ptz_type, create_time, update_time, sub_count, stream_id, has_audio, stream_identification, channel_type, map_level, gb_name, gb_manufacturer, gb_model, gb_civil_code, gb_address, gb_status, gb_longitude, gb_latitude, record_plan_id";
    let sql = pool
        .sql("SELECT {} FROM gb_device_channel WHERE device_id = ? ORDER BY id")
        .replace("{}", &cols);
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql)
            .bind(device_id)
            .fetch_all(p)
    });
}

/// 通用通道列表：全量分页，支持 query(名称/通道号)、online(设备在线)、channel_type
pub async fn list_common_channels_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let limit = count.min(100) as i64;
    let offset = crate::pagination::offset(page, count.min(100));
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();

    // Phase 4.x: 改用 SELECT *,避免遗漏 DeviceChannel 字段导致
    // "no column found for name: <field>" 500;多读若干冗余字段可控。
    //
    // SQL 文本只写一份 `?`，由 **pool 自己的方言** 在运行期改写成 `$1..$n`
    // （postgres 不认 `?`）。方言必须看 pool，不能看进程级全局
    // `dialect::current()` —— 后者在直接构造 `Pool::Postgres(..)` 的测试里
    // 仍是默认值（sqlite），会让 postgres 收到 `?` → `syntax error`。
    let sql = if has_query && online.is_some() && channel_type.is_some() {
        pool.sql("SELECT c.* FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ? AND c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
            .into_owned()
    } else if has_query && online.is_some() {
        pool.sql("SELECT c.* FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ? ORDER BY c.id LIMIT ? OFFSET ?")
            .into_owned()
    } else if has_query && channel_type.is_some() {
        pool.sql("SELECT c.* FROM gb_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
            .into_owned()
    } else if has_query {
        pool.sql("SELECT c.* FROM gb_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) ORDER BY c.id LIMIT ? OFFSET ?")
            .into_owned()
    } else if online.is_some() && channel_type.is_some() {
        pool.sql("SELECT c.* FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE d.on_line = ? AND c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
            .into_owned()
    } else if online.is_some() {
        pool.sql("SELECT c.* FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE d.on_line = ? ORDER BY c.id LIMIT ? OFFSET ?")
            .into_owned()
    } else if channel_type.is_some() {
        pool.sql("SELECT c.* FROM gb_device_channel c WHERE c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
            .into_owned()
    } else {
        pool.sql("SELECT * FROM gb_device_channel ORDER BY id LIMIT ? OFFSET ?")
            .into_owned()
    };

    let rows = if has_query && online.is_some() && channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(online.unwrap())
                .bind(channel_type.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    } else if has_query && online.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    } else if has_query && channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(channel_type.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    } else if has_query {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    } else if online.is_some() && channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(online.unwrap())
                .bind(channel_type.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    } else if online.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    } else if channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(channel_type.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    } else {
        crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, DeviceChannel>(&sql)
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
        })?
    };
    Ok(rows)
}

pub async fn count_common_channels(
    pool: &Pool,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
) -> sqlx::Result<i64> {
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();

    // SQL 文本只写一份 `?`，由 **pool 自己的方言** 在运行期改写成 `$1..$n`
    // （postgres 不认 `?`）。方言必须看 pool，不能看进程级全局
    // `dialect::current()` —— 后者在直接构造 `Pool::Postgres(..)` 的测试里
    // 仍是默认值（sqlite），会让 postgres 收到 `?` → `syntax error`。
    let sql = if has_query && online.is_some() && channel_type.is_some() {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ? AND c.channel_type = ?")
            .into_owned()
    } else if has_query && online.is_some() {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ?")
            .into_owned()
    } else if has_query && channel_type.is_some() {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND c.channel_type = ?")
            .into_owned()
    } else if has_query {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?)")
            .into_owned()
    } else if online.is_some() && channel_type.is_some() {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE d.on_line = ? AND c.channel_type = ?")
            .into_owned()
    } else if online.is_some() {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel c INNER JOIN gb_device d ON c.device_id = d.device_id WHERE d.on_line = ?")
            .into_owned()
    } else if channel_type.is_some() {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel c WHERE c.channel_type = ?")
            .into_owned()
    } else {
        pool.sql("SELECT COUNT(*) FROM gb_device_channel")
            .into_owned()
    };

    let row = if has_query && online.is_some() && channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(online.unwrap())
                .bind(channel_type.unwrap())
                .fetch_one(p)
        })?
    } else if has_query && online.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(online.unwrap())
                .fetch_one(p)
        })?
    } else if has_query && channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(channel_type.unwrap())
                .fetch_one(p)
        })?
    } else if has_query {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(&like)
                .bind(&like)
                .fetch_one(p)
        })?
    } else if online.is_some() && channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(online.unwrap())
                .bind(channel_type.unwrap())
                .fetch_one(p)
        })?
    } else if online.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(online.unwrap())
                .fetch_one(p)
        })?
    } else if channel_type.is_some() {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(channel_type.unwrap())
                .fetch_one(p)
        })?
    } else {
        crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
        })?
    };
    Ok(row)
}

pub async fn delete_device_cascade(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("DELETE FROM gb_device_channel WHERE device_id = ?")
        .into_owned();
    // 两条语句要分两次 dispatch（宏体只能是一个 future）。
    let _ = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(device_id).execute(p) })?;
    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&crate::dyn_where::dialect_sql(
            "DELETE FROM gb_device WHERE device_id = ?",
        ))
        .bind(device_id)
        .execute(p)
    })
    .map(|r| r.rows_affected())
}

/// SQLite 模式专用：检查新增设备是否超过上限。
/// - 已存在的设备（更新）：始终允许
/// - 新增设备：当前数量 < max_devices 时允许；否则返回 Err
/// PG/MySQL 后端此函数始终返回 Ok（0），由调用方按 feature 决定是否启用。
pub async fn check_sqlite_device_limit(
    pool: &Pool,
    device_id: &str,
    max_devices: Option<usize>,
) -> Result<(), DeviceLimitError> {
    let Some(limit) = max_devices else {
        return Ok(()); // 未配置上限 = 无限制
    };

    // 已存在 → 允许（这是更新）
    if device_exists(pool, device_id).await.unwrap_or(false) {
        return Ok(());
    }

    // 新增 → 检查总数
    let count = count_devices(pool, None, None).await.unwrap_or(0);
    if (count as usize) >= limit {
        return Err(DeviceLimitError {
            current: count as usize,
            limit,
            device_id: device_id.to_string(),
        });
    }
    Ok(())
}

/// 设备存在性查询（用于限制检查）
pub async fn device_exists(pool: &Pool, device_id: &str) -> sqlx::Result<bool> {
    if crate::db::dialect::current().is_mysql() {
        {
            let row: (i64,) = crate::dispatch!(pool, |p| {
                sqlx::query_as(&crate::dyn_where::dialect_sql(
                    "SELECT EXISTS(SELECT 1 FROM gb_device WHERE device_id = ?)",
                ))
                .bind(device_id)
                .fetch_optional(p)
            })?
            .unwrap_or((0,));
            Ok(row.0 > 0)
        }
    } else if crate::db::dialect::current().is_postgres() {
        {
            let row: (bool,) = crate::dispatch!(pool, |p| {
                sqlx::query_as("SELECT EXISTS(SELECT 1 FROM gb_device WHERE device_id = $1)")
                    .bind(device_id)
                    .fetch_optional(p)
            })?
            .unwrap_or((false,));
            Ok(row.0)
        }
    } else {
        {
            let row: (i64,) = crate::dispatch!(pool, |p| {
                sqlx::query_as(&crate::dyn_where::dialect_sql(
                    "SELECT EXISTS(SELECT 1 FROM gb_device WHERE device_id = ?)",
                ))
                .bind(device_id)
                .fetch_optional(p)
            })?
            .unwrap_or((0,));
            Ok(row.0 > 0)
        }
    }
}

/// SQLite 设备上限错误
#[derive(Debug, Clone)]
pub struct DeviceLimitError {
    pub current: usize,
    pub limit: usize,
    pub device_id: String,
}

impl std::fmt::Display for DeviceLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SQLite 设备数量已达上限 ({}/{}); 请迁移到 PostgreSQL/MySQL 后端。新设备: {}",
            self.current, self.limit, self.device_id
        )
    }
}

impl std::error::Error for DeviceLimitError {}

/// 手工新增/编辑设备时的可写字段。
///
/// 用结构体而不是继续加形参：此前 `ip/port/password/expires/
/// heart_beat_interval/heart_beat_count` 六列**从来没被写进去过**
/// （前端填了、接口回成功、库里仍是空）。
#[derive(Debug, Default, Clone)]
pub struct DeviceWriteFields<'a> {
    pub name: Option<&'a str>,
    pub manufacturer: Option<&'a str>,
    pub model: Option<&'a str>,
    pub transport: Option<&'a str>,
    pub stream_mode: Option<&'a str>,
    pub media_server_id: Option<&'a str>,
    pub custom_name: Option<&'a str>,
    pub ip: Option<&'a str>,
    pub port: Option<i32>,
    /// 明文/哈希由调用方决定；空串视为"不修改"
    pub password: Option<&'a str>,
    pub expires: Option<i32>,
    pub heart_beat_interval: Option<i32>,
    pub heart_beat_count: Option<i32>,
}

pub async fn insert_device(
    pool: &Pool,
    device_id: &str,
    f: &DeviceWriteFields<'_>,
    now: &str,
) -> sqlx::Result<u64> {
    let mid = f.media_server_id.unwrap_or("auto");
    let cols = "device_id, name, manufacturer, model, ip, port, password, expires, \
                heart_beat_interval, heart_beat_count, transport, stream_mode, on_line, \
                create_time, update_time, media_server_id, custom_name";
    // `on_line` 在 SQL 文本里是**字面量**（不是占位符）：postgres 的 `on_line`
    // 是 BOOLEAN 列，只能写 `false`；mysql / sqlite 写 `0`。因此这条语句是
    // **方言专属**的 —— 用 `dispatch_dialect!` 每分支一份，
    // 占位符数量三方一致（都是 16 个 bind），bind 顺序也完全一致。
    //
    // 分支由 **pool 的类型**决定（不是进程级全局 `dialect::current()`）：
    // 直接构造 `Pool::Postgres(..)` 的测试里全局仍是默认值（sqlite），
    // 走错分支就会把 `?` 发给 postgres → `syntax error`。
    // `on_line` 在 SQL 文本里是**字面量**（不是占位符）：postgres 的 `on_line`
    // 是 BOOLEAN 列，只能写 `false`；mysql / sqlite 写 `0`。因此这条语句是
    // **方言专属**的 —— 用 `dispatch_dialect!` 每分支一份，
    // 占位符数量三方一致（都是 16 个 bind），bind 顺序也完全一致。
    //
    // 分支由 **pool 的类型**决定（不是进程级全局 `dialect::current()`）：
    // 直接构造 `Pool::Postgres(..)` 的测试里全局仍是默认值（sqlite），
    // 走错分支就会把 `?` 发给 postgres → `syntax error`。
    let r: u64 = crate::dispatch_dialect!(
        pool,
        |p| {
            // PostgreSQL
            let raw = format!(
                "INSERT INTO gb_device ({cols}) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,false,$13,$14,$15,$16)"
            );
            let sql = pool.sql(&raw).into_owned();
            sqlx::query(&sql)
                .bind(device_id)
                .bind(f.name)
                .bind(f.manufacturer)
                .bind(f.model)
                .bind(f.ip)
                .bind(f.port)
                .bind(f.password)
                .bind(f.expires)
                .bind(f.heart_beat_interval)
                .bind(f.heart_beat_count)
                .bind(f.transport)
                .bind(f.stream_mode)
                .bind(now)
                .bind(now)
                .bind(mid)
                .bind(f.custom_name)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        },
        |p| {
            // MySQL
            // `{}` = 运行期拼进去的列名；`?` = 占位符（由 **pool 自己的方言**
            // 改写成 `$1..$n`）。
            let sql = pool
                .sql("INSERT INTO gb_device ({cols}) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,0,?,?,?,?)")
                .replace("{cols}", &cols);
            sqlx::query(&sql)
                .bind(device_id)
                .bind(f.name)
                .bind(f.manufacturer)
                .bind(f.model)
                .bind(f.ip)
                .bind(f.port)
                .bind(f.password)
                .bind(f.expires)
                .bind(f.heart_beat_interval)
                .bind(f.heart_beat_count)
                .bind(f.transport)
                .bind(f.stream_mode)
                .bind(now)
                .bind(now)
                .bind(mid)
                .bind(f.custom_name)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        },
        |p| {
            // SQLite
            // `{}` = 运行期拼进去的列名；`?` = 占位符（由 **pool 自己的方言**
            // 改写成 `$1..$n`）。
            let sql = pool
                .sql("INSERT INTO gb_device ({cols}) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,0,?,?,?,?)")
                .replace("{cols}", &cols);
            sqlx::query(&sql)
                .bind(device_id)
                .bind(f.name)
                .bind(f.manufacturer)
                .bind(f.model)
                .bind(f.ip)
                .bind(f.port)
                .bind(f.password)
                .bind(f.expires)
                .bind(f.heart_beat_interval)
                .bind(f.heart_beat_count)
                .bind(f.transport)
                .bind(f.stream_mode)
                .bind(now)
                .bind(now)
                .bind(mid)
                .bind(f.custom_name)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        },
    )?;
    Ok(r)
}

pub async fn update_device(
    pool: &Pool,
    device_id: &str,
    f: &DeviceWriteFields<'_>,
    now: &str,
) -> sqlx::Result<u64> {
    let set = "name = COALESCE(?, name), manufacturer = COALESCE(?, manufacturer), \
               model = COALESCE(?, model), transport = COALESCE(?, transport), \
               stream_mode = COALESCE(?, stream_mode), \
               media_server_id = COALESCE(?, media_server_id), \
               custom_name = COALESCE(?, custom_name), ip = COALESCE(?, ip), \
               port = COALESCE(?, port), \
               password = COALESCE(NULLIF(?, ''), password), \
               expires = COALESCE(?, expires), \
               heart_beat_interval = COALESCE(?, heart_beat_interval), \
               heart_beat_count = COALESCE(?, heart_beat_count), \
               update_time = ? WHERE device_id = ?";
    let stmt = format!("UPDATE gb_device SET {}", set);
    let sql = pool.sql(&stmt).into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(f.name).bind(f.manufacturer).bind(f.model).bind(f.transport).bind(f.stream_mode).bind(f.media_server_id).bind(f.custom_name).bind(f.ip).bind(f.port).bind(f.password).bind(f.expires).bind(f.heart_beat_interval).bind(f.heart_beat_count).bind(now).bind(device_id).execute(p)
    })?;
    Ok(r.rows_affected())
}
pub async fn update_device_online(
    pool: &Pool,
    device_id: &str,
    online: bool,
    ip: Option<&str>,
    port: Option<i32>,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(r#"UPDATE gb_device SET on_line = ?, ip = COALESCE(?, ip), port = COALESCE(?, port), update_time = ?
           WHERE device_id = ?"#).into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(online)
            .bind(ip)
            .bind(port)
            .bind(now)
            .bind(device_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 把 SIP `DeviceInfo` 应答里的 `<DeviceName>/<Manufacturer>/<Model>/<Firmware>`
/// 写回 `gb_device`（COALESCE：None 视为"不修改"，不会被空字符串覆盖）。
///
/// 此前 `handle_message` 对设备的 DeviceInfo 应答**只回 200 OK**，从未把
/// 应答里的厂家/型号/固件写库 → 设备列表长期空白，运维只能手动逐条编辑。
pub async fn update_device_info_fields(
    pool: &Pool,
    device_id: &str,
    name: Option<&str>,
    manufacturer: Option<&str>,
    model: Option<&str>,
    firmware: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        r#"UPDATE gb_device
           SET name = COALESCE(NULLIF(?, ''), name),
               manufacturer = COALESCE(NULLIF(?, ''), manufacturer),
               model = COALESCE(NULLIF(?, ''), model),
               firmware = COALESCE(NULLIF(?, ''), firmware),
               update_time = ?
           WHERE device_id = ?"#,
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(name)
            .bind(manufacturer)
            .bind(model)
            .bind(firmware)
            .bind(now)
            .bind(device_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn upsert_device(
    pool: &Pool,
    device_id: &str,
    name: Option<&str>,
    manufacturer: Option<&str>,
    model: Option<&str>,
    firmware: Option<&str>,
    transport: Option<&str>,
    stream_mode: Option<&str>,
    ip: Option<&str>,
    port: Option<i32>,
    online: bool,
    media_server_id: Option<&str>,
    now: &str,
) -> sqlx::Result<()> {
    let mid = media_server_id.unwrap_or("auto");
    // 三个方言的 INSERT 语句**占位符数量不同**：
    // * MySQL 的 `ON DUPLICATE KEY UPDATE` 要把被更新的列**再绑定一遍**（24 个）；
    // * PostgreSQL / SQLite 的 `ON CONFLICT ... DO UPDATE` 用 `excluded.<列>` /
    //   `gb_device.<列>` 引用新行，**不用重复绑定**（13 个）。
    //
    // 所以这三个分支必须各带自己的 SQL 与 bind 链 —— 不能合成一条。
    // 分支值统一成 `Result<u64, Error>`（`rows_affected`），否则三个分支的
    // `QueryResult` 类型不同（`PgQueryResult` / `MySqlQueryResult` / `SqliteQueryResult`）
    // 无法放进同一个 `match`。
    crate::dispatch_dialect!(
        pool,
        |p| {
            sqlx::query(
                r#"INSERT INTO gb_device (device_id, name, manufacturer, model, firmware, transport, stream_mode, ip, port, on_line, create_time, update_time, media_server_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
               ON CONFLICT (device_id) DO UPDATE SET
               name = COALESCE($2, gb_device.name), manufacturer = COALESCE($3, gb_device.manufacturer),
               model = COALESCE($4, gb_device.model), firmware = COALESCE($5, gb_device.firmware),
               transport = COALESCE($6, gb_device.transport), stream_mode = COALESCE($7, gb_device.stream_mode),
                ip = COALESCE($8, gb_device.ip), port = COALESCE($9, gb_device.port),
                on_line = $10, update_time = $12, media_server_id = COALESCE($13, gb_device.media_server_id)"#,
            )
            .bind(device_id).bind(name).bind(manufacturer).bind(model).bind(firmware).bind(transport).bind(stream_mode)
            .bind(ip).bind(port).bind(online).bind(now).bind(now).bind(mid)
            .execute(p)
            .await
            .map(|r| r.rows_affected())
        },
        |p| {
            sqlx::query(
                &crate::dyn_where::dialect_sql(r#"INSERT INTO gb_device (device_id, name, manufacturer, model, firmware, transport, stream_mode, ip, port, on_line, create_time, update_time, media_server_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE
               name = COALESCE(?, name), manufacturer = COALESCE(?, manufacturer), model = COALESCE(?, model),
               firmware = COALESCE(?, firmware), transport = COALESCE(?, transport), stream_mode = COALESCE(?, stream_mode),
               ip = COALESCE(?, ip), port = COALESCE(?, port), on_line = ?, update_time = ?, media_server_id = COALESCE(?, media_server_id)"#),
            )
            .bind(device_id).bind(name).bind(manufacturer).bind(model).bind(firmware).bind(transport).bind(stream_mode)
            .bind(ip).bind(port).bind(online).bind(now).bind(now).bind(mid)
            .bind(name).bind(manufacturer).bind(model).bind(firmware).bind(transport).bind(stream_mode)
            .bind(ip).bind(port).bind(online).bind(now).bind(mid)
            .execute(p)
            .await
            .map(|r| r.rows_affected())
        },
        |p| {
            sqlx::query(
                &crate::dyn_where::dialect_sql(r#"INSERT INTO gb_device (device_id, name, manufacturer, model, firmware, transport, stream_mode, ip, port, on_line, create_time, update_time, media_server_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(device_id) DO UPDATE SET
               name = COALESCE(excluded.name, gb_device.name), manufacturer = COALESCE(excluded.manufacturer, gb_device.manufacturer),
               model = COALESCE(excluded.model, gb_device.model), firmware = COALESCE(excluded.firmware, gb_device.firmware),
               transport = COALESCE(excluded.transport, gb_device.transport), stream_mode = COALESCE(excluded.stream_mode, gb_device.stream_mode),
               ip = COALESCE(excluded.ip, gb_device.ip), port = COALESCE(excluded.port, gb_device.port),
               on_line = excluded.on_line, update_time = excluded.update_time, media_server_id = COALESCE(excluded.media_server_id, gb_device.media_server_id)"#),
            )
            .bind(device_id).bind(name).bind(manufacturer).bind(model).bind(firmware).bind(transport).bind(stream_mode)
            .bind(ip).bind(port).bind(online).bind(now).bind(now).bind(mid)
            .execute(p)
            .await
            .map(|r| r.rows_affected())
        },
    )
    .map(|_| ())
}

pub async fn list_all_channels(pool: &Pool) -> sqlx::Result<Vec<DeviceChannel>> {
    let sql = crate::db::dialect::current_sql("SELECT * FROM gb_device_channel").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, DeviceChannel>(&sql).fetch_all(p)
    });
}

/// 获取所有需要目录订阅续期的在线设备
/// 返回 (device_id, subscribe_cycle_for_catalog) 列表
pub async fn get_devices_for_catalog_renewal(pool: &Pool) -> sqlx::Result<Vec<(String, i32)>> {
    let sql = pool.sql(
        "SELECT device_id, subscribe_cycle_for_catalog FROM gb_device \
         WHERE on_line = ? AND subscribe_cycle_for_catalog > 0",
    );
    let rows: Vec<(String, i32)> = crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, (String, i32)>(&sql)
            .bind(true)
            .fetch_all(p)
    })?;
    Ok(rows)
}

/// 获取所有需要移动位置订阅续期的在线设备
/// 返回 (device_id, subscribe_cycle_for_mobile_position) 列表
pub async fn get_devices_for_mobile_position_renewal(
    pool: &Pool,
) -> sqlx::Result<Vec<(String, i32)>> {
    let sql = pool.sql(
        "SELECT device_id, subscribe_cycle_for_mobile_position FROM gb_device \
         WHERE on_line = ? AND subscribe_cycle_for_mobile_position > 0",
    );
    let rows: Vec<(String, i32)> = crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, (String, i32)>(&sql)
            .bind(true)
            .fetch_all(p)
    })?;
    Ok(rows)
}

/// 批量更新通道在线状态
pub async fn batch_update_channel_status(
    pool: &Pool,
    device_id: &str,
    channel_ids: &[&str],
    status: &str,
) -> sqlx::Result<u64> {
    if channel_ids.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut total = 0u64;
    for channel_id in channel_ids {
        let sql = crate::db::dialect::current_sql("UPDATE gb_device_channel SET status = ?, update_time = ? WHERE device_id = ? AND gb_device_id = ?").into_owned();
        let r = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(status)
                .bind(&now)
                .bind(device_id)
                .bind(channel_id)
                .execute(p)
        })?;
        total += r.rows_affected();
    }
    Ok(total)
}

/// 删除设备的所有通道
pub async fn delete_channels_by_device(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("DELETE FROM gb_device_channel WHERE device_id = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(device_id).execute(p) })?;
    Ok(r.rows_affected())
}

/// 删除单个通道
pub async fn delete_channel_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let sql =
        crate::db::dialect::current_sql("DELETE FROM gb_device_channel WHERE id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

/// 删除指定设备的指定通道
pub async fn delete_channel(pool: &Pool, device_id: &str, channel_id: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "DELETE FROM gb_device_channel WHERE device_id = ? AND gb_device_id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(device_id)
            .bind(channel_id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 批量插入通道
pub async fn batch_insert_channels(
    pool: &Pool,
    device_id: &str,
    channels: &[ChannelInsertData],
) -> sqlx::Result<u64> {
    if channels.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut total = 0u64;
    // 三个方言的 SQL 只差占位符，统一用 `?`。
    // `id` 一律按 i32 读（MySQL 的 bigint / PG 的 serial / SQLite 的 INTEGER
    // 都落在这条口径上，见 `docs/DB_DIALECT_NOTES.md` 规则 2）。
    let parent_db_id: i32 = crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i32>(&crate::dyn_where::dialect_sql(
            "SELECT id FROM gb_device WHERE device_id = ?",
        ))
        .bind(device_id)
        .fetch_one(p)
    })?;

    for ch in channels {
        let sql = crate::db::dialect::current_sql("INSERT INTO gb_device_channel (device_id, name, gb_device_id, status, longitude, latitude, has_audio, channel_type, create_time, update_time, data_type, data_device_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)").into_owned();
        let r = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(device_id)
                .bind(&ch.name)
                .bind(&ch.gb_device_id)
                .bind(&ch.status)
                .bind(ch.longitude)
                .bind(ch.latitude)
                .bind(ch.has_audio)
                .bind(ch.channel_type.unwrap_or(0))
                .bind(&now)
                .bind(&now)
                .bind(parent_db_id)
                .execute(p)
        })?;
        total += r.rows_affected();
    }
    Ok(total)
}

/// 通道插入数据结构
#[derive(Debug, Clone)]
pub struct ChannelInsertData {
    pub name: String,
    pub gb_device_id: String,
    pub status: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub has_audio: Option<bool>,
    pub channel_type: Option<i32>,
}

/// 统计在线设备数量
pub async fn count_online_devices(pool: &Pool) -> sqlx::Result<i64> {
    // `on_line` 是布尔列：PG 是 BOOL、MySQL 是 TINYINT(1)、SQLite 是 INTEGER。
    // 统一用**参数绑定**传 Rust `bool`（pg 走 BOOLEAN、mysql 走 TINYINT、
    // sqlite 走 INTEGER），不必在 SQL 文本里分叉 `true` / `1`。
    let sql = pool.sql("SELECT COUNT(*) FROM gb_device WHERE on_line = ?");
    crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).bind(true).fetch_one(p)
    })
}

/// 获取所有设备列表（不分页）
pub async fn list_all_devices(pool: &Pool) -> sqlx::Result<Vec<Device>> {
    // `channel_count` 在 Rust 侧是 `Option<i32>`：PG 的 `COUNT(*)` 返回 int8，
    // 必须显式 `CAST(... AS INTEGER)`；MySQL / SQLite 直接取（宽进严出 / 亲和性）。
    // `CAST(x AS INTEGER)` 在 MySQL 上不合法（要用 SIGNED），所以只有这一段分方言。
    let channel_count_expr = match pool.dialect() {
        crate::db::Dialect::Postgres => {
            "CAST((SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id) AS INTEGER)"
        }
        _ => "(SELECT COUNT(*) FROM gb_device_channel WHERE device_id = gb_device.device_id)",
    };
    let sql = pool
        .sql(&format!(
            "SELECT id, device_id, name, manufacturer, model, firmware, transport, stream_mode, \
             on_line, register_time, keepalive_time, ip, port, expires, heart_beat_interval, \
             heart_beat_count, create_time, update_time, media_server_id, custom_name, charset, \
             ssrc_check, geo_coord_sys, sdp_ip, local_ip, password, subscribe_cycle_for_catalog, \
             subscribe_cycle_for_mobile_position, mobile_position_submission_interval, \
             host_address, {channel_count_expr} AS channel_count, server_id FROM gb_device ORDER BY id"
        ))
        .into_owned();
    crate::dispatch!(pool, |p| { sqlx::query_as::<_, Device>(&sql).fetch_all(p) })
}

pub async fn upsert_channel_from_catalog(
    pool: &Pool,
    device_id: &str,
    channel_device_id: &str,
    name: &str,
    manufacturer: Option<&str>,
    model: Option<&str>,
    owner: Option<&str>,
    civil_code: Option<&str>,
    address: Option<&str>,
    parent_id: Option<&str>,
    online: bool,
    longitude: Option<f64>,
    latitude: Option<f64>,
    ptz_type: Option<i32>,
    has_audio: Option<bool>,
    sub_count: Option<i32>,
) -> sqlx::Result<bool> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    crate::dispatch_dialect!(
        pool,
        // ---- PostgreSQL：先看有没有，有就 UPDATE，没有才 INSERT ----
        |p| {
            let existing: i64 = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM gb_device_channel WHERE device_id = $1 AND gb_device_id = $2",
            )
            .bind(device_id)
            .bind(channel_device_id)
            .fetch_one(p)
            .await
            .unwrap_or(0);

            if existing > 0 {
                sqlx::query(
                    r#"UPDATE gb_device_channel SET
                       name = COALESCE($3, name),
                       manufacturer = COALESCE($4, manufacturer),
                       model = COALESCE($5, model),
                       owner = COALESCE($6, owner),
                       civil_code = COALESCE($7, civil_code),
                       address = COALESCE($8, address),
                       parent_id = COALESCE($9, parent_id),
                       status = CASE WHEN $10 THEN 'ON' ELSE 'OFF' END,
                       longitude = COALESCE($11, longitude),
                       latitude = COALESCE($12, latitude),
                       ptz_type = COALESCE($13, ptz_type),
                       has_audio = COALESCE($14, has_audio),
                       sub_count = COALESCE($15, sub_count),
                       update_time = $16
                       WHERE device_id = $1 AND gb_device_id = $2"#,
                )
                .bind(device_id)
                .bind(channel_device_id)
                .bind(if name.is_empty() {
                    None as Option<&str>
                } else {
                    Some(name)
                })
                .bind(manufacturer)
                .bind(model)
                .bind(owner)
                .bind(civil_code)
                .bind(address)
                .bind(parent_id)
                .bind(online)
                .bind(longitude)
                .bind(latitude)
                .bind(ptz_type)
                .bind(has_audio)
                .bind(sub_count)
                .bind(&now)
                .execute(p)
                .await?;
                Ok(false)
            } else {
                let parent_db_id: i32 =
                    sqlx::query_scalar::<_, i32>("SELECT id FROM gb_device WHERE device_id = $1")
                        .bind(device_id)
                        .fetch_one(p)
                        .await?;
                sqlx::query(
                    r#"INSERT INTO gb_device_channel
                       (device_id, gb_device_id, name, manufacturer, model, owner, civil_code, address,
                        parent_id, status, longitude, latitude, ptz_type, has_audio, sub_count,
                        create_time, update_time, data_type, data_device_id)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
                        CASE WHEN $10 THEN 'ON' ELSE 'OFF' END,
                        $11, $12, $13, $14, $15, $16, $17, 0, $18)"#,
                )
                .bind(device_id)
                .bind(channel_device_id)
                .bind(if name.is_empty() { "Unknown" } else { name })
                .bind(manufacturer)
                .bind(model)
                .bind(owner)
                .bind(civil_code)
                .bind(address)
                .bind(parent_id)
                .bind(online)
                .bind(longitude)
                .bind(latitude)
                .bind(ptz_type)
                .bind(has_audio)
                .bind(sub_count)
                .bind(&now)
                .bind(&now)
                .bind(parent_db_id)
                .execute(p)
                .await?;
                Ok(true)
            }
        },
        // ---- MySQL：ON DUPLICATE KEY UPDATE ----
        |p| {
            let parent_db_id: i32 = sqlx::query_scalar::<_, i32>(&crate::dyn_where::dialect_sql(
                "SELECT id FROM gb_device WHERE device_id = ?",
            ))
            .bind(device_id)
            .fetch_one(p)
            .await?;
            sqlx::query(&crate::dyn_where::dialect_sql(
                r#"INSERT INTO gb_device_channel
                   (device_id, gb_device_id, name, manufacturer, model, owner, civil_code, address,
                    parent_id, status, longitude, latitude, ptz_type, has_audio, sub_count,
                    create_time, update_time, data_type, data_device_id)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?,
                    CASE WHEN ? THEN 'ON' ELSE 'OFF' END,
                    ?, ?, ?, ?, ?, ?, ?, 0, ?)
                   ON DUPLICATE KEY UPDATE
                   name = COALESCE(VALUES(name), name),
                   manufacturer = COALESCE(VALUES(manufacturer), manufacturer),
                   parent_id = COALESCE(VALUES(parent_id), parent_id),
                   status = VALUES(status),
                   longitude = COALESCE(VALUES(longitude), longitude),
                   latitude = COALESCE(VALUES(latitude), latitude),
                   has_audio = COALESCE(VALUES(has_audio), has_audio),
                   sub_count = COALESCE(VALUES(sub_count), sub_count),
                   update_time = VALUES(update_time)"#,
            ))
            .bind(device_id)
            .bind(channel_device_id)
            .bind(if name.is_empty() { "Unknown" } else { name })
            .bind(manufacturer)
            .bind(model)
            .bind(owner)
            .bind(civil_code)
            .bind(address)
            .bind(parent_id)
            .bind(online)
            .bind(longitude)
            .bind(latitude)
            .bind(ptz_type)
            .bind(has_audio)
            .bind(sub_count)
            .bind(&now)
            .bind(&now)
            .bind(parent_db_id)
            .execute(p)
            .await?;
            Ok(true)
        },
        // ---- SQLite：ON CONFLICT ... DO UPDATE ----
        |p| {
            // Phase 4.x: 查询父设备 db id,作为 data_device_id;data_type = 0 (channel)
            // 否则受 NOT NULL 约束直接 INSERT 失败。
            let parent_db_id: i64 = sqlx::query_scalar::<_, i64>(&crate::dyn_where::dialect_sql(
                "SELECT COALESCE(id, 0) FROM gb_device WHERE device_id = ?",
            ))
            .bind(device_id)
            .fetch_optional(p)
            .await
            .unwrap_or(None)
            .unwrap_or(0);

            sqlx::query(&crate::dyn_where::dialect_sql(
                r#"INSERT INTO gb_device_channel
                   (device_id, gb_device_id, name, manufacturer, model, owner, civil_code, address,
                    parent_id, status, longitude, latitude, ptz_type, has_audio, sub_count,
                    create_time, update_time, data_type, data_device_id)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?,
                    CASE WHEN ? THEN 'ON' ELSE 'OFF' END,
                    ?, ?, ?, ?, ?, ?, ?, 0, ?)
                   ON CONFLICT(gb_device_id) DO UPDATE SET
                   name = COALESCE(excluded.name, gb_device_channel.name),
                   manufacturer = COALESCE(excluded.manufacturer, gb_device_channel.manufacturer),
                   parent_id = COALESCE(excluded.parent_id, gb_device_channel.parent_id),
                   status = excluded.status,
                   longitude = COALESCE(excluded.longitude, gb_device_channel.longitude),
                   latitude = COALESCE(excluded.latitude, gb_device_channel.latitude),
                   has_audio = COALESCE(excluded.has_audio, gb_device_channel.has_audio),
                   sub_count = COALESCE(excluded.sub_count, gb_device_channel.sub_count),
                   update_time = excluded.update_time"#,
            ))
            .bind(device_id)
            .bind(channel_device_id)
            .bind(if name.is_empty() { "Unknown" } else { name })
            .bind(manufacturer)
            .bind(model)
            .bind(owner)
            .bind(civil_code)
            .bind(address)
            .bind(parent_id)
            .bind(online)
            .bind(longitude)
            .bind(latitude)
            .bind(ptz_type)
            .bind(has_audio)
            .bind(sub_count)
            .bind(&now)
            .bind(&now)
            .bind(parent_db_id)
            .execute(p)
            .await?;
            Ok(true)
        },
    )
}

/// 在线注册设备数（与 on_line 字段一致）
pub async fn count_registered_devices(pool: &Pool) -> sqlx::Result<i64> {
    let n: i64 = crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device").fetch_one(p)
    })?;
    Ok(n)
}

/// 在线设备数（最近 60s 内收到过 keepalive 的设备视为在线）
pub async fn count_alive_devices(pool: &Pool, alive_window_secs: i64) -> sqlx::Result<i64> {
    // "最近 N 秒内收到过心跳" —— 时间差算子三种方言各不相同，属于**方言专属语法**：
    // * PostgreSQL：`EXTRACT(EPOCH FROM (NOW() - keepalive_time))`
    // * MySQL：`TIMESTAMPDIFF(SECOND, keepalive_time, NOW())`
    // * SQLite：`(julianday('now') - julianday(keepalive_time)) * 86400`
    // 三个分支各带自己的 SQL（MySQL 分支此前被错配成了 pg 的 `$1` 占位符）。
    let n: i64 = crate::dispatch_dialect!(
        pool,
        |p| {
            sqlx::query_scalar::<_, i64>(
                &crate::dyn_where::dialect_sql("SELECT COUNT(*) FROM gb_device                  WHERE on_line = ? AND keepalive_time IS NOT NULL                  AND EXTRACT(EPOCH FROM (NOW() - keepalive_time)) <= $1"),
            )
            .bind(true)
            .bind(alive_window_secs)
            .fetch_one(p)
            .await
        },
        |p| {
            sqlx::query_scalar::<_, i64>(
                &crate::dyn_where::dialect_sql("SELECT COUNT(*) FROM gb_device                  WHERE on_line = ? AND keepalive_time IS NOT NULL                  AND TIMESTAMPDIFF(SECOND, keepalive_time, NOW()) <= ?"),
            )
            .bind(true)
            .bind(alive_window_secs)
            .fetch_one(p)
            .await
        },
        |p| {
            sqlx::query_scalar::<_, i64>(
                &crate::dyn_where::dialect_sql("SELECT COUNT(*) FROM gb_device                  WHERE on_line = ? AND keepalive_time IS NOT NULL                  AND (julianday('now') - julianday(keepalive_time)) * 86400 <= ?"),
            )
            .bind(true)
            .bind(alive_window_secs as f64)
            .fetch_one(p)
            .await
        },
    )?;
    Ok(n)
}

/// Phase 2.2: 批量 upsert 通道（Catalog NOTIFY 用）
///
/// 把 per-channel `upsert_channel_from_catalog` 的 N 次 round-trip 优化为单条
/// `INSERT ... ON CONFLICT/ON DUPLICATE KEY UPDATE`。
///
/// 每个 channel 一个 ChannelInsertData 元素，函数返回成功插入/更新的行数。
pub async fn batch_upsert_channels(
    pool: &Pool,
    parent_device_id: &str,
    channels: &[ChannelInsertData],
) -> sqlx::Result<u64> {
    if channels.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    crate::dispatch_dialect!(
        pool,
        // ---- PostgreSQL：单条 INSERT ... ON CONFLICT DO UPDATE（UNNEST 避免 N 个参数）----
        |p| {
            let gb_ids: Vec<&str> = channels.iter().map(|c| c.gb_device_id.as_str()).collect();
            let names: Vec<&str> = channels.iter().map(|c| c.name.as_str()).collect();
            let statuses: Vec<&str> = channels.iter().map(|c| c.status.as_str()).collect();
            let longitudes: Vec<Option<f64>> = channels.iter().map(|c| c.longitude).collect();
            let latitudes: Vec<Option<f64>> = channels.iter().map(|c| c.latitude).collect();
            let has_audios: Vec<Option<bool>> = channels.iter().map(|c| c.has_audio).collect();
            let channel_types: Vec<Option<i32>> = channels.iter().map(|c| c.channel_type).collect();

            let result = sqlx::query(
                r#"INSERT INTO gb_device_channel
                   (device_id, gb_device_id, name, status, longitude, latitude, has_audio, channel_type, create_time, update_time, data_type, data_device_id)
                   SELECT $1, gb_id, name, status, longitude, latitude, has_audio, COALESCE(channel_type, 0), $2, $2, 0,
                          (SELECT id FROM gb_device WHERE device_id = $1)
                   FROM UNNEST($3::text[], $4::text[], $5::text[], $6::float8[], $7::float8[], $8::bool[], $9::int4[])
                   AS t(gb_id, name, status, longitude, latitude, has_audio, channel_type)
                   ON CONFLICT (gb_device_id) DO UPDATE SET
                       name = EXCLUDED.name,
                       status = EXCLUDED.status,
                       longitude = EXCLUDED.longitude,
                       latitude = EXCLUDED.latitude,
                       has_audio = EXCLUDED.has_audio,
                       channel_type = EXCLUDED.channel_type,
                       update_time = EXCLUDED.update_time"#,
            )
            .bind(parent_device_id)
            .bind(&now)
            .bind(&gb_ids)
            .bind(&names)
            .bind(&statuses)
            .bind(&longitudes)
            .bind(&latitudes)
            .bind(&has_audios)
            .bind(&channel_types)
            .execute(p)
            .await?;
            Ok(result.rows_affected())
        },
        // ---- MySQL：多次单条 INSERT ... ON DUPLICATE KEY UPDATE（不支持 UNNEST）----
        |p| {
            let parent_db_id: i32 = sqlx::query_scalar::<_, i32>(&crate::dyn_where::dialect_sql(
                "SELECT id FROM gb_device WHERE device_id = ?",
            ))
            .bind(parent_device_id)
            .fetch_one(p)
            .await?;
            let mut total: u64 = 0;
            for ch in channels {
                let res = sqlx::query(
                    &crate::dyn_where::dialect_sql(r#"INSERT INTO gb_device_channel
                       (device_id, gb_device_id, name, status, longitude, latitude, has_audio, channel_type, create_time, update_time, data_type, data_device_id)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)
                       ON DUPLICATE KEY UPDATE
                           name = VALUES(name),
                           status = VALUES(status),
                           longitude = VALUES(longitude),
                           latitude = VALUES(latitude),
                           has_audio = VALUES(has_audio),
                           channel_type = VALUES(channel_type),
                           update_time = VALUES(update_time)"#),
                )
                .bind(parent_device_id)
                .bind(&ch.gb_device_id)
                .bind(&ch.name)
                .bind(&ch.status)
                .bind(ch.longitude)
                .bind(ch.latitude)
                .bind(ch.has_audio)
                .bind(ch.channel_type.unwrap_or(0))
                .bind(&now)
                .bind(&now)
                .bind(parent_db_id)
                .execute(p)
                .await?;
                total += res.rows_affected();
            }
            Ok(total)
        },
        // ---- SQLite：多次单条 INSERT ... ON CONFLICT DO UPDATE（复用 MySQL 风格）----
        |p| {
            let parent_db_id: i64 = sqlx::query_scalar::<_, i64>(&crate::dyn_where::dialect_sql(
                "SELECT id FROM gb_device WHERE device_id = ?",
            ))
            .bind(parent_device_id)
            .fetch_one(p)
            .await?;
            let mut total: u64 = 0;
            for ch in channels {
                let res = sqlx::query(
                    &crate::dyn_where::dialect_sql(r#"INSERT INTO gb_device_channel
                       (device_id, gb_device_id, name, status, longitude, latitude, has_audio, channel_type, create_time, update_time, data_type, data_device_id)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)
                       ON CONFLICT(gb_device_id) DO UPDATE SET
                           name = excluded.name,
                           status = excluded.status,
                           longitude = excluded.longitude,
                           latitude = excluded.latitude,
                           has_audio = excluded.has_audio,
                           channel_type = excluded.channel_type,
                           update_time = excluded.update_time"#),
                )
                .bind(parent_device_id)
                .bind(&ch.gb_device_id)
                .bind(&ch.name)
                .bind(&ch.status)
                .bind(ch.longitude)
                .bind(ch.latitude)
                .bind(ch.has_audio)
                .bind(ch.channel_type.unwrap_or(0))
                .bind(&now)
                .bind(&now)
                .bind(parent_db_id)
                .execute(p)
                .await?;
                total += res.rows_affected();
            }
            Ok(total)
        },
    )
}
