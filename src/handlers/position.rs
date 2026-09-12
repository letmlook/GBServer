//! 移动位置（GB28181 `MobilePosition`）HTTP 接口 —— 对齐 WVP 的
//! `MobilePositionController`。
//!
//! ## 为什么单独一个模块
//!
//! WVP 提供 4 个端点（`.../gb28181/controller/MobilePositionController.java`）：
//!
//! | WVP | 本平台此前 |
//! |-----|-----------|
//! | `GET /api/position/history/{deviceId}?channelId=&start=&end=` | ⚠️ 有路由，但读的是**另一张表** `gb_position_history` |
//! | `GET /api/position/latest?channelId=` | ❌ 缺失 |
//! | `GET /api/position/realtime/{deviceId}` | ❌ 缺失 |
//! | `GET /api/position/subscribe/{deviceId}?expires=&interval=` | ❌ 缺失 |
//!
//! 平台里其实**一直在写**移动位置：设备上报的 NOTIFY 会落进
//! `gb_device_mobile_position`（`sip/gb28181/subscription_lifecycle.rs`），
//! DB 层也早已有 `list_paged` / `get_latest_position` / `delete_by_device`，
//! 但**没有任何 HTTP 端点把它读出来** —— 数据进了库却拿不到，等于功能缺失。
//!
//! ## 两张位置表的关系（不是重复）
//!
//! * `gb_device_mobile_position`：**WVP 对齐**的移动位置表，本模块对外暴露的
//!   就是它（字段与 WVP `MobilePosition` 一致）。
//! * `gb_position_history`：本平台自己的宽表，供「电子地图打点/轨迹抽稀」
//!   （`handlers/common_channel.rs::map_thin_save`）与 JT1078 `position-info` 使用。
//!
//! 两条写入路径（设备 SUBSCRIBE 后的 NOTIFY、以及 MESSAGE 查询的响应）现在
//! **都**会写 `gb_device_mobile_position`，因此对外接口在任何设备行为下都有数据。

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db;
use crate::db::mobile_position as pos_db;
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::sip::gb28181::XmlParser;
use crate::AppState;

/// 查询参数里的可选整数：**空串按"未提供"处理**，同时接受数字与数字字符串。
///
/// 前端把未填写的筛选条件发成 `channelId=`（空串），serde 默认会直接
/// 422 `cannot parse integer from empty string` —— 一个"没填"的筛选条件
/// 不该让整个请求失败。
fn de_opt_i64<'de, D>(de: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Num(i64),
        Str(String),
    }
    match Option::<Raw>::deserialize(de)? {
        None => Ok(None),
        Some(Raw::Num(n)) => Ok(Some(n)),
        Some(Raw::Str(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                t.parse::<i64>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
    }
}

/// `/api/position/latest` 查询参数。
///
/// WVP 只接受 `channelId`（**通道的数据库主键**）。为了便于直接按国标编号调试，
/// 这里额外接受 `deviceId`（设备国标编号）与 `gbChannelId`（通道国标编号）。
#[derive(Debug, Deserialize, Default)]
pub struct LatestQuery {
    #[serde(alias = "channelId", default, deserialize_with = "de_opt_i64")]
    pub channel_id: Option<i64>,
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "gbChannelId")]
    pub gb_channel_id: Option<String>,
}

/// `/api/position/history/:device_id` 查询参数。
#[derive(Debug, Deserialize, Default)]
pub struct HistoryQuery {
    /// 通道数据库主键（WVP 口径）。给了它就按通道查 `gb_device_mobile_position`。
    #[serde(alias = "channelId", default, deserialize_with = "de_opt_i64")]
    pub channel_id: Option<i64>,
    /// 通道国标编号（可选，配合 deviceId 使用）。
    #[serde(alias = "gbChannelId")]
    pub gb_channel_id: Option<String>,
    #[serde(alias = "startTime")]
    pub start: Option<String>,
    #[serde(alias = "endTime")]
    pub end: Option<String>,
    pub page: Option<i64>,
    pub count: Option<i64>,
}

/// `/api/position/subscribe/:device_id` 查询参数。
#[derive(Debug, Deserialize, Default)]
pub struct SubscribeQuery {
    /// 订阅有效期（秒）。WVP 的 `expires` 参数。
    pub expires: Option<i32>,
    /// 位置上报间隔（秒）。WVP 的 `interval` 参数。
    pub interval: Option<i32>,
}

/// 空串按「未提供」处理。
fn opt_trimmed(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

/// 把「通道数据库主键」解析成 `(设备国标编号, 通道国标编号)`。
///
/// WVP 的 `channelId` 是 `gb_device_channel.id`（不是国标编号），因此必须先
/// 查库换算出 `device_id` / `gb_device_id` —— 移动位置表里存的是这两个国标字段。
async fn resolve_channel(
    state: &AppState,
    channel_db_id: i64,
) -> Result<(String, String), AppError> {
    let ch = db::common_channel::get_by_id(&state.pool, channel_db_id)
        .await?
        .ok_or_else(|| {
            AppError::business(ErrorCode::Error400, format!("通道不存在: {channel_db_id}"))
        })?;
    let device_id = ch
        .device_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            AppError::business(
                ErrorCode::Error400,
                format!("通道 {channel_db_id} 没有关联设备编号"),
            )
        })?;
    let gb_channel = ch
        .gb_device_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| device_id.clone());
    Ok((device_id, gb_channel))
}

/// GET /api/position/latest
///
/// 与 WVP 一致：返回该通道**最新一条**移动位置。三种入参（优先级从高到低）：
/// `channelId`（数据库主键）/ `deviceId`+`gbChannelId` / 仅 `deviceId`。
pub async fn position_latest(
    State(state): State<AppState>,
    Query(q): Query<LatestQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let (device_id, channel_id) = if let Some(cid) = q.channel_id.filter(|v| *v > 0) {
        let (dev, ch) = resolve_channel(&state, cid).await?;
        (dev, Some(ch))
    } else {
        let dev = opt_trimmed(q.device_id.as_deref())
            .ok_or_else(|| {
                AppError::business(ErrorCode::Error400, "缺少 channelId 或 deviceId")
            })?
            .to_string();
        (dev, opt_trimmed(q.gb_channel_id.as_deref()).map(str::to_string))
    };

    let latest = pos_db::get_latest_position(&state.pool, &device_id, channel_id.as_deref()).await?;
    // `data` 直接就是位置对象（与 WVP 的 `latestPosition` 一致），
    // 查不到时为 null。设备/通道编号本来就在对象里。
    Ok(Json(WVPResult::success(
        serde_json::to_value(latest).unwrap_or(serde_json::Value::Null),
    )))
}

/// GET /api/position/history/:device_id
///
/// * 传了 `channelId`（通道数据库主键）→ 按 WVP 口径查
///   `gb_device_mobile_position`（这是 WVP `MobilePositionController.history` 的语义）。
/// * 未传 → 保持本平台原有行为：按设备国标编号查 `gb_position_history`
///   （电子地图轨迹用的宽表），避免破坏既有调用方。
pub async fn position_history(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let start = opt_trimmed(q.start.as_deref());
    let end = opt_trimmed(q.end.as_deref());

    if let Some(cid) = q.channel_id.filter(|v| *v > 0) {
        let (dev, gb_channel) = resolve_channel(&state, cid).await?;
        let page = q.page.unwrap_or(1).max(1);
        let count = q.count.unwrap_or(100).clamp(1, 1000);
        let total =
            pos_db::count(&state.pool, &dev, Some(&gb_channel), start, end).await?;
        let list = pos_db::list_paged(
            &state.pool,
            &dev,
            Some(&gb_channel),
            start,
            end,
            page,
            count,
        )
        .await?;
        return Ok(Json(WVPResult::success(serde_json::json!({
            "deviceId": dev,
            "channelId": gb_channel,
            "total": total,
            "page": page,
            "count": count,
            "list": list,
        }))));
    }

    // 未指定通道：兼容旧行为（gb_position_history 宽表）
    let list = db::position_history::list_by_device_and_time(&state.pool, &device_id, start, end)
        .await?;
    Ok(Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "source": "position_history",
        "total": list.len(),
        "list": list,
    }))))
}

/// 从设备响应的 XML 里解析移动位置（`<Response>` / `<Notify>` 两种形态都能读，
/// 因为 `parse_fields` 只按标签名取值）。
fn parse_position_xml(xml: &str) -> Option<pos_db::MobilePositionInsert> {
    let parsed = XmlParser::parse_fields(xml);
    let latitude: Option<f64> = parsed.get("Latitude").and_then(|s| s.parse().ok());
    let longitude: Option<f64> = parsed.get("Longitude").and_then(|s| s.parse().ok());
    let (latitude, longitude) = (latitude?, longitude?);
    let device_id = XmlParser::get_device_id(xml).unwrap_or_default();
    let time = parsed
        .get("Time")
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
    Some(pos_db::MobilePositionInsert {
        device_id: device_id.clone(),
        // 设备级位置上报没有通道号：与 NOTIFY 路径保持一致，用设备编号占位。
        channel_id: device_id.clone(),
        device_name: None,
        time: Some(time),
        longitude: Some(longitude),
        latitude: Some(latitude),
        altitude: parsed.get("Altitude").and_then(|s| s.parse().ok()),
        speed: parsed.get("Speed").and_then(|s| s.parse().ok()),
        direction: parsed.get("Direction").and_then(|s| s.parse().ok()),
        report_source: Some("realtime".to_string()),
        create_time: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

/// GET /api/position/realtime/:device_id
///
/// 实时向设备要一次位置（SIP MESSAGE `<Query><CmdType>MobilePosition</CmdType>`），
/// 等待响应并**落库**；设备离线或超时时回退到库里最新一条（附 `source` 说明），
/// 而不是返回一个空响应让调用方猜。
pub async fn position_realtime(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_id = device_id.trim().to_string();
    if device_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 deviceId"));
    }

    // 所有分支都会赋值（成功分支直接 return），因此无需初始值
    let note: Option<String>;
    if let Some(sip_server) = state.sip_server.as_ref() {
        let server = &**sip_server;
        if server.is_device_online(&device_id).await {
            let sn = chrono::Utc::now().timestamp_millis() as u32;
            let commander = server.device_commander();
            let (req, rx) = commander.register_mobile_position_with_receiver(&device_id, sn);
            let send = server.send_mobile_position_query(&device_id, sn).await;
            match send {
                Ok(()) => match commander.await_response(req, rx, 5).await {
                    Ok(xml) => {
                        if let Some(insert) = parse_position_xml(&xml) {
                            if let Err(e) = pos_db::insert(&state.pool, &insert).await {
                                tracing::warn!("实时位置落库失败（仍返回给调用方）: {}", e);
                            }
                            return Ok(Json(WVPResult::success(serde_json::json!({
                                "deviceId": device_id,
                                "source": "live",
                                "position": {
                                    "deviceId": insert.device_id,
                                    "channelId": insert.channel_id,
                                    "time": insert.time,
                                    "longitude": insert.longitude,
                                    "latitude": insert.latitude,
                                    "altitude": insert.altitude,
                                    "speed": insert.speed,
                                    "direction": insert.direction,
                                },
                            }))));
                        }
                        note = Some("设备响应里没有经纬度".to_string());
                    }
                    Err(_) => note = Some("设备在 5 秒内未返回位置".to_string()),
                },
                Err(e) => note = Some(format!("位置查询下发失败: {e}")),
            }
        } else {
            note = Some("设备离线".to_string());
        }
    } else {
        note = Some("SIP 服务未启动".to_string());
    }

    // 回退：库里最新一条
    let latest = pos_db::get_latest_position(&state.pool, &device_id, None).await?;
    Ok(Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "source": "cache",
        "message": note,
        "position": latest,
    }))))
}

/// GET /api/position/subscribe/:device_id?expires=&interval=
///
/// WVP 的语义：把 `subscribeCycleForMobilePosition` / `mobilePositionSubmissionInterval`
/// 写进设备表，由订阅循环周期下发 SUBSCRIBE。这里额外**立即下发一次** SUBSCRIBE，
/// 让用户点完马上生效（否则要等到下一个订阅周期）。
pub async fn position_subscribe(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Query(q): Query<SubscribeQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_id = device_id.trim().to_string();
    if device_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 deviceId"));
    }
    let expires = q.expires.unwrap_or(3600);
    let interval = q.interval.unwrap_or(5);
    if expires <= 0 || interval <= 0 {
        return Err(AppError::business(
            ErrorCode::Error400,
            "expires / interval 必须为正整数",
        ));
    }

    if db::device::get_device_by_device_id(&state.pool, &device_id)
        .await?
        .is_none()
    {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("设备不存在: {device_id}"),
        ));
    }

    db::device::update_device_mobile_position_subscription(
        &state.pool,
        &device_id,
        expires,
        interval,
    )
    .await?;

    // 立即下发一次 SUBSCRIBE；失败不吞掉，如实回报给调用方判断。
    let mut sip_message: Option<String> = None;
    let mut sent = false;
    match state.sip_server.as_ref() {
        Some(sip_server) => match sip_server.send_subscribe(&device_id, "MobilePosition", expires as u32).await {
            Ok(()) => sent = true,
            Err(e) => sip_message = Some(e.to_string()),
        },
        None => sip_message = Some("SIP 服务未启动".to_string()),
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "expires": expires,
        "interval": interval,
        "subscribeSent": sent,
        "message": sip_message,
    }))))
}

#[cfg(all(test, feature = "sqlite"))]
mod position_contract_tests {
    use super::*;
    use crate::test_support::app_state;

    async fn seed(state: &AppState) {
        sqlx::query(
            "INSERT INTO gb_device (device_id, name, on_line, create_time, update_time) \
             VALUES ('34020000001320000001', 'cam-1', 1, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .execute(&state.pool)
        .await
        .expect("seed device");
        sqlx::query(
            "INSERT INTO gb_device_channel \
             (device_id, name, gb_device_id, status, data_type, data_device_id, channel_type, \
              create_time, update_time) \
             VALUES ('34020000001320000001', 'ch-1', '34020000001310000001', 'ON', 0, 0, 0, \
                     '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .execute(&state.pool)
        .await
        .expect("seed channel");
        for (t, lon, lat) in [
            ("2026-01-01 10:00:00", 120.1, 30.1),
            ("2026-01-01 11:00:00", 120.2, 30.2),
        ] {
            sqlx::query(
                "INSERT INTO gb_device_mobile_position \
                 (device_id, channel_id, time, longitude, latitude, speed, direction, create_time) \
                 VALUES ('34020000001320000001', '34020000001310000001', ?, ?, ?, 5.0, 90.0, ?)",
            )
            .bind(t)
            .bind(lon)
            .bind(lat)
            .bind(t)
            .execute(&state.pool)
            .await
            .expect("seed position");
        }
    }

    /// `/position/latest`：WVP 用通道数据库主键，必须能换算成国标编号并取到最新一条。
    #[tokio::test]
    async fn test_position_latest_resolves_channel_db_id() {
        let state = app_state().await;
        seed(&state).await;
        let ch_id: i64 = sqlx::query_scalar("SELECT id FROM gb_device_channel LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();

        let resp = position_latest(
            State(state.clone()),
            Query(LatestQuery {
                channel_id: Some(ch_id),
                ..Default::default()
            }),
        )
        .await
        .expect("latest 应成功");
        let data = resp.0.data.unwrap();
        assert_eq!(data["deviceId"], "34020000001320000001");
        // 取的是时间最大的那条
        assert_eq!(data["longitude"], 120.2);
        assert_eq!(data["latitude"], 30.2);

        // 按国标编号（deviceId）也能取
        let resp = position_latest(
            State(state.clone()),
            Query(LatestQuery {
                device_id: Some("34020000001320000001".into()),
                ..Default::default()
            }),
        )
        .await
        .unwrap();
        assert_eq!(resp.0.data.unwrap()["latitude"], 30.2);
    }

    #[tokio::test]
    async fn test_position_latest_rejects_missing_params() {
        let state = app_state().await;
        let e = position_latest(
            State(state.clone()),
            Query(LatestQuery::default()),
        )
        .await
        .expect_err("缺参数应报错");
        assert!(matches!(e, AppError::Business(_, _)));

        let e = position_latest(
            State(state.clone()),
            Query(LatestQuery {
                channel_id: Some(99999),
                ..Default::default()
            }),
        )
        .await
        .expect_err("通道不存在应报错");
        assert!(matches!(e, AppError::Business(_, _)));
    }

    /// history 带 `channelId` → WVP 口径（gb_device_mobile_position + 时间过滤）。
    #[tokio::test]
    async fn test_position_history_by_channel_id_filters_time() {
        let state = app_state().await;
        seed(&state).await;
        let ch_id: i64 = sqlx::query_scalar("SELECT id FROM gb_device_channel LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();

        let resp = position_history(
            State(state.clone()),
            Path("34020000001320000001".into()),
            Query(HistoryQuery {
                channel_id: Some(ch_id),
                start: Some("2026-01-01 10:30:00".into()),
                end: Some("2026-01-01 12:00:00".into()),
                ..Default::default()
            }),
        )
        .await
        .unwrap();
        let data = resp.0.data.unwrap();
        assert_eq!(data["total"], 1);
        assert_eq!(data["list"][0]["latitude"], 30.2);
    }

    /// 不带 `channelId` 时保持旧行为（gb_position_history 宽表）。
    #[tokio::test]
    async fn test_position_history_without_channel_uses_legacy_table() {
        let state = app_state().await;
        // gb_position_history 由 ensure_table 建（不在 init-*.sql 里）
        db::position_history::ensure_table(&state.pool)
            .await
            .expect("建 gb_position_history");
        sqlx::query(
            "INSERT INTO gb_position_history \
             (device_id, timestamp, longitude, latitude, altitude, speed, direction) \
             VALUES ('dev-1', '2026-01-01 10:00:00', 1.0, 2.0, 3.0, 4.0, 5.0)",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        let resp = position_history(
            State(state.clone()),
            Path("dev-1".into()),
            Query(HistoryQuery::default()),
        )
        .await
        .unwrap();
        let data = resp.0.data.unwrap();
        assert_eq!(data["source"], "position_history");
        assert_eq!(data["total"], 1);
    }

    /// subscribe：设备必须存在；成功时把订阅周期写进 gb_device。
    #[tokio::test]
    async fn test_position_subscribe_persists_cycle() {
        let state = app_state().await;
        seed(&state).await;

        let resp = position_subscribe(
            State(state.clone()),
            Path("34020000001320000001".into()),
            Query(SubscribeQuery {
                expires: Some(600),
                interval: Some(15),
            }),
        )
        .await
        .expect("订阅应成功");
        let data = resp.0.data.unwrap();
        assert_eq!(data["expires"], 600);
        assert_eq!(data["interval"], 15);
        // 测试 AppState 没有 SIP server，必须如实告知"未下发"，而不是假成功
        assert_eq!(data["subscribeSent"], false);

        let row: (Option<i32>, Option<i32>) = sqlx::query_as(
            "SELECT subscribe_cycle_for_mobile_position, mobile_position_submission_interval \
             FROM gb_device WHERE device_id = '34020000001320000001'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(row.0, Some(600));
        assert_eq!(row.1, Some(15));
    }

    #[tokio::test]
    async fn test_position_subscribe_rejects_unknown_device() {
        let state = app_state().await;
        let e = position_subscribe(
            State(state.clone()),
            Path("no-such-device".into()),
            Query(SubscribeQuery::default()),
        )
        .await
        .expect_err("设备不存在应报错");
        assert!(matches!(e, AppError::Business(_, _)));
    }

    /// `channelId=`（空串）必须被当成"未提供"，而不是 422。
    /// 前端未填筛选条件时就是这么发的。
    #[test]
    fn test_query_params_tolerate_empty_and_string_numbers() {
        let q: LatestQuery =
            serde_json::from_value(serde_json::json!({"channelId": ""})).expect("空串应可解析");
        assert_eq!(q.channel_id, None);

        let q: LatestQuery =
            serde_json::from_value(serde_json::json!({"channelId": "7"})).expect("数字字符串应可解析");
        assert_eq!(q.channel_id, Some(7));

        let q: HistoryQuery = serde_json::from_value(serde_json::json!({
            "channelId": "",
            "start": "",
            "end": "",
            "page": 1,
            "count": 10
        }))
        .expect("空筛选条件应可解析");
        assert_eq!(q.channel_id, None);

        // 非法值仍要报错，不能静默当成"没有筛选条件"
        assert!(serde_json::from_value::<LatestQuery>(serde_json::json!({"channelId": "abc"})).is_err());
    }

    /// 实时查询的设备响应解析（`<Response>` 形态）。
    #[test]
    fn test_parse_position_xml_response_shape() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>MobilePosition</CmdType>
<SN>123</SN>
<DeviceID>34020000001320000001</DeviceID>
<Result>OK</Result>
<Time>2026-01-01 12:00:00</Time>
<Longitude>120.5</Longitude>
<Latitude>30.5</Latitude>
<Speed>12.5</Speed>
<Direction>180.0</Direction>
<Altitude>15.0</Altitude>
</Response>"#;
        let p = parse_position_xml(xml).expect("应能解析");
        assert_eq!(p.device_id, "34020000001320000001");
        assert_eq!(p.longitude, Some(120.5));
        assert_eq!(p.latitude, Some(30.5));
        assert_eq!(p.speed, Some(12.5));
        assert_eq!(p.altitude, Some(15.0));
        assert_eq!(p.time.as_deref(), Some("2026-01-01 12:00:00"));

        // 没有经纬度的响应不能被当成位置
        assert!(parse_position_xml("<Response><DeviceID>d</DeviceID></Response>").is_none());
    }
}
