//! 通用通道 API /api/common/channel，与前端 commonChannel.js 对应

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::common_channel;
use crate::db::{count_common_channels, delete_channel_by_id, list_common_channels_paged, DeviceChannel};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

pub(crate) async fn lookup_channel_and_send(
    state: &AppState,
    channel_id: i64,
    cmd_builder: impl FnOnce(&DeviceChannel) -> (String, String, String),
) -> Json<serde_json::Value> {
    match common_channel::get_by_id(&state.pool, channel_id).await {
        Ok(Some(ch)) => {
            let device_id = match &ch.device_id {
                Some(id) => id.clone(),
                None => return Json(serde_json::json!({ "code": 1, "msg": "通道无设备ID" })),
            };
            let gb_channel_id = ch.gb_device_id.clone().unwrap_or_default();
            let (cmd_type, body, success_msg) = cmd_builder(&ch);

            if let Some(ref sip_server) = state.sip_server {
                let server = &*sip_server;
                if let Some(device) = server.device_manager().get(&device_id).await {
                    if device.online && device.addr.is_some() {
                        match server.send_device_control(&device_id, &gb_channel_id, &cmd_type, &body).await {
                            Ok(_) => return Json(serde_json::json!({ "code": 0, "msg": success_msg })),
                            Err(e) => return Json(serde_json::json!({ "code": 1, "msg": format!("SIP发送失败: {}", e) })),
                        }
                    }
                }
            }
            Json(serde_json::json!({ "code": 1, "msg": "设备不在线或SIP未初始化" }))
        }
        Ok(None) => Json(serde_json::json!({ "code": 1, "msg": "通道不存在" })),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": format!("数据库错误: {}", e) })),
    }
}

fn build_ptz_xml(command: &str, h_speed: u8, v_speed: u8, z_speed: u8) -> String {
    let ptz_cmd = match command.to_ascii_uppercase().as_str() {
        "UP" => format!("0501000000{:02X}FF", h_speed),
        "DOWN" => format!("0501000001{:02X}FF", v_speed),
        "LEFT" => format!("0501000002{:02X}FF", h_speed),
        "RIGHT" => format!("0501000003{:02X}FF", h_speed),
        "ZOOM_IN" => format!("0501010000{:02X}FF", z_speed),
        "ZOOM_OUT" => format!("0501010001{:02X}FF", z_speed),
        "FOCUS_IN" => format!("0501020000{:02X}FF", z_speed),
        "FOCUS_OUT" => format!("0501020001{:02X}FF", z_speed),
        "IRIS_IN" => format!("0501030000{:02X}FF", z_speed),
        "IRIS_OUT" => format!("0501030001{:02X}FF", z_speed),
        "STOP" => "05010000000000FF".to_string(),
        _ => format!("050100000000{:02X}FF", h_speed),
    };
    format!(r#"<PTZCmd>{}</PTZCmd>"#, ptz_cmd)
}

fn build_preset_xml(command: &str, preset_index: u32) -> String {
    let preset_cmd = match command.to_ascii_uppercase().as_str() {
        "GOTO_PRESET" => format!("07000100000000{:02X}FF", preset_index),
        "SET_PRESET" => format!("07000100010000{:02X}FF", preset_index),
        "CLEAR_PRESET" => format!("07000100020000{:02X}FF", preset_index),
        _ => format!("07000100000000{:02X}FF", preset_index),
    };
    format!(r#"<PTZCmd>{}</PTZCmd>"#, preset_cmd)
}

fn build_fi_xml(cmd_type: &str, command: &str, speed: u8) -> String {
    let fi_cmd = match (cmd_type, command.to_lowercase().as_str()) {
        ("iris", "on" | "open") => format!("0501030000{:02X}FF", speed),
        ("iris", _) => format!("0501030001{:02X}FF", speed),
        ("focus", "on" | "open") => format!("0501020000{:02X}FF", speed),
        ("focus", _) => format!("0501020001{:02X}FF", speed),
        _ => format!("0501030000{:02X}FF", speed),
    };
    format!(r#"<PTZCmd>{}</PTZCmd>"#, fi_cmd)
}

/// 通用前端指令（`/api/front-end/common/:cmd/:ch` 兼容路径）支持的指令名。
///
/// 只覆盖本模块已有明确 GB28181 控制语义与 XML 构造函数的子集；更细的控制
/// （预置位、巡航、扫描、雨刷速度等）请走 `/api/common/channel/front-end/*` 专用端点。
pub const SUPPORTED_FRONT_END_COMMANDS: &[&str] = &[
    "UP",
    "DOWN",
    "LEFT",
    "RIGHT",
    "ZOOM_IN",
    "ZOOM_OUT",
    "STOP",
    "FOCUS_IN",
    "FOCUS_OUT",
    "IRIS_IN",
    "IRIS_OUT",
    "WIPER_ON",
    "WIPER_OFF",
    "GUARD_SET",
    "GUARD_RESET",
];

/// 把通用前端指令名映射为 `(SIP 控制消息类型, XML 体)`；未知指令返回 `None`。
///
/// 抽出为纯函数便于单测覆盖全部受支持指令。
pub fn front_end_command_body(cmd_upper: &str) -> Option<(String, String)> {
    let body = match cmd_upper {
        // PTZ 平移/缩放/停止：复用既有 PTZCmd 构造（默认中等速度 0x40）
        "UP" | "DOWN" | "LEFT" | "RIGHT" | "ZOOM_IN" | "ZOOM_OUT" | "STOP" => {
            build_ptz_xml(cmd_upper, 0x40, 0x40, 0x40)
        }
        // 聚焦 / 光圈
        "FOCUS_IN" => build_fi_xml("focus", "on", 0x40),
        "FOCUS_OUT" => build_fi_xml("focus", "off", 0x40),
        "IRIS_IN" => build_fi_xml("iris", "on", 0x40),
        "IRIS_OUT" => build_fi_xml("iris", "off", 0x40),
        // 雨刷
        "WIPER_ON" => "<WiperCmd>Open</WiperCmd>".to_string(),
        "WIPER_OFF" => "<WiperCmd>Close</WiperCmd>".to_string(),
        // 布防 / 撤防
        "GUARD_SET" => "<GuardCmd>SetGuard</GuardCmd>".to_string(),
        "GUARD_RESET" => "<GuardCmd>ResetGuard</GuardCmd>".to_string(),
        _ => return None,
    };
    Some(("DeviceControl".to_string(), body))
}

// ========== 查询参数 ==========
#[derive(Debug, Deserialize)]
pub struct CommonChannelQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub online: Option<String>,
    #[serde(alias = "channelType")]
    pub channel_type: Option<String>,
    #[serde(alias = "hasRecordPlan")]
    pub has_record_plan: Option<String>,
    #[serde(alias = "civilCode")]
    pub civil_code: Option<String>,
    #[serde(alias = "parentDeviceId")]
    pub parent_device_id: Option<String>,
    #[serde(alias = "groupDeviceId")]
    pub group_device_id: Option<String>,
    pub id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelIdQuery {
    #[serde(alias = "channelId")]
    pub channel_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ClearChannelBody {
    pub all: Option<bool>,
    pub channel_ids: Option<Vec<i64>>,
}

// ========== 通用通道列表 ==========
/// GET /api/common/channel/list
pub async fn channel_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);
    let online = match q.online.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let channel_type = q
        .channel_type
        .as_deref()
        .and_then(|s| s.parse::<i32>().ok());

    let list: Vec<DeviceChannel> = list_common_channels_paged(
        &state.pool,
        page,
        count,
        q.query.as_deref(),
        online,
        channel_type,
    )
    .await?;
    let total = count_common_channels(&state.pool, q.query.as_deref(), online, channel_type).await?;

    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|c| channel_to_json(c))
        .collect();

    let data = serde_json::json!({
        "list": rows,
        "total": total,
    });
    Ok(Json(WVPResult::success(data)))
}

pub(crate) fn channel_to_json(c: &DeviceChannel) -> serde_json::Value {
    // 把 DeviceChannel 字段同时以 camelCase 和 gb_* 前缀输出,兼容
    // 前端 /channel、/device/channel、/commonChannel 等页面读取 `gbName` /
    // `gbDeviceId` / `gbStatus` / `gbLongitude` / `gbLatitude` 等历史命名。
    // Phase 5: 修复前端通道列表 / 地图信息窗显示空白的问题。
    serde_json::json!({
        "id": c.id,
        "deviceId": c.device_id,
        "name": c.name,
        "channelId": c.gb_device_id,
        "gbId": c.gb_device_id.clone().unwrap_or_default(),
        "status": c.status,
        "longitude": c.longitude,
        "latitude": c.latitude,
        "createTime": c.create_time,
        "updateTime": c.update_time,
        "subCount": c.sub_count,
        "hasAudio": c.has_audio,
        "channelType": c.channel_type,
        "ptzType": c.ptz_type.map(|t| t.to_string()).unwrap_or_default(),
        // —— gb_* 兼容字段 ——
        "gbName": c.name,
        "gbDeviceId": c.gb_device_id,
        "gbManufacturer": c.manufacturer,
        "gbModel": c.model,
        "gbOwner": c.owner,
        "gbCivilCode": c.civil_code,
        "gbAddress": c.address,
        "gbParental": c.parental,
        "gbParentId": c.parent_id,
        "gbStatus": c.status,
        "gbLongitude": c.longitude,
        "gbLatitude": c.latitude,
        "streamId": c.stream_id,
        "streamIdentification": c.stream_identification,
        "manufacturer": c.manufacturer,
        "model": c.model,
        "owner": c.owner,
        "civilCode": c.civil_code,
        "address": c.address,
        "parental": c.parental,
        "parentId": c.parent_id,
        "ptzTypeText": ptz_type_text(c.ptz_type),
    })
}

/// 把数字 ptz_type 翻译为前端表头用的中文标签。
/// 与 web/src/views/device/channel/index.vue data().ptzTypes 保持一致。
fn ptz_type_text(ptz: Option<i32>) -> Option<String> {
    ptz.map(|v| match v {
        1 => "球机".to_string(),
        2 => "半球".to_string(),
        3 => "固定枪机".to_string(),
        4 => "遥控枪机".to_string(),
        _ => "未知".to_string(),
    })
}

/// GET /api/common/channel/one?id=
pub async fn channel_one(
    State(state): State<AppState>,
    Query(q): Query<ChannelIdQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.channel_id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::Value::Null)));
    }
    let ch = common_channel::get_by_id(&state.pool, id).await?;
    let out = match ch {
        Some(c) => channel_to_json(&c),
        None => serde_json::Value::Null,
    };
    Ok(Json(WVPResult::success(out)))
}

/// GET /api/common/channel/industry/list
pub async fn industry_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    let industries = vec![
        serde_json::json!({"value": "01", "label": "危险化学品"}),
        serde_json::json!({"value": "02", "label": "煤矿"}),
        serde_json::json!({"value": "03", "label": "非煤矿山"}),
        serde_json::json!({"value": "04", "label": "烟花爆竹"}),
        serde_json::json!({"value": "05", "label": "工贸"}),
        serde_json::json!({"value": "99", "label": "其他"}),
    ];
    Json(WVPResult::success(industries))
}

/// GET /api/common/channel/type/list
pub async fn type_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    let types = vec![
        serde_json::json!({"value": 1, "label": "摄像机"}),
        serde_json::json!({"value": 2, "label": "半球"}),
        serde_json::json!({"value": 3, "label": "快球"}),
        serde_json::json!({"value": 4, "label": "云台"}),
        serde_json::json!({"value": 5, "label": "红外枪机"}),
        serde_json::json!({"value": 6, "label": "广播"}),
        serde_json::json!({"value": 7, "label": "报警"}),
        serde_json::json!({"value": 8, "label": "存储设备"}),
        serde_json::json!({"value": 9, "label": "移动设备"}),
        serde_json::json!({"value": 10, "label": "门禁"}),
        serde_json::json!({"value": 11, "label": "智能检测"}),
        serde_json::json!({"value": 12, "label": "安全监测"}),
    ];
    Json(WVPResult::success(types))
}

/// GET /api/common/channel/network/identification/list
pub async fn network_identification_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    let list = vec![
        serde_json::json!({"value": "IP", "label": "IP"}),
        serde_json::json!({"value": "MAC", "label": "MAC"}),
        serde_json::json!({"value": "E1", "label": "E1"}),
        serde_json::json!({"value": "ADSL", "label": "ADSL"}),
    ];
    Json(WVPResult::success(list))
}

/// POST /api/common/channel/update
#[derive(Debug, Deserialize)]
pub struct ChannelUpdateBody {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub channel_id: Option<String>,
    pub civil_code: Option<String>,
    pub parent_id: Option<i64>,
    pub business_group: Option<String>,
    pub ptz_type: Option<i32>,
    pub custom_name: Option<String>,
}

pub async fn channel_update(
    State(state): State<AppState>,
    Json(body): Json<ChannelUpdateBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    common_channel::update(
        &state.pool,
        id,
        body.name.as_deref(),
        body.channel_id.as_deref(),
        body.civil_code.as_deref(),
        body.parent_id,
        body.business_group.as_deref(),
        body.ptz_type,
        body.custom_name.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/reset
#[derive(Debug, Deserialize)]
pub struct ChannelResetBody {
    pub id: Option<i64>,
}

pub async fn channel_reset(
    State(state): State<AppState>,
    Json(body): Json<ChannelResetBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    common_channel::reset(&state.pool, id, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/add
#[derive(Debug, Deserialize)]
pub struct ChannelAddBody {
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub channel_id: Option<String>,
    pub civil_code: Option<String>,
    pub parent_id: Option<i64>,
    pub business_group: Option<String>,
    pub ptz_type: Option<i32>,
    pub custom_name: Option<String>,
}

pub async fn channel_add(
    State(state): State<AppState>,
    Json(body): Json<ChannelAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_id = body.device_id.as_deref().unwrap_or("").trim();
    let channel_id = body.channel_id.as_deref().unwrap_or("").trim();
    let name = body.name.as_deref().unwrap_or("").trim();

    if device_id.is_empty() || channel_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "deviceId 和 channelId 必填"));
    }

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let id = common_channel::add(
        &state.pool,
        device_id,
        name,
        channel_id,
        body.civil_code.as_deref(),
        body.parent_id,
        body.business_group.as_deref(),
        body.ptz_type,
        body.custom_name.as_deref(),
        &now,
    )
    .await?;

    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "通道添加成功"
    }))))
}

/// GET /api/common/channel/civilcode/list
pub async fn civilcode_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);
    let online = match q.online.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let channel_type = q
        .channel_type
        .as_deref()
        .and_then(|s| s.parse::<i32>().ok());

    let list: Vec<DeviceChannel> = list_common_channels_paged(
        &state.pool,
        page,
        count,
        q.query.as_deref(),
        online,
        channel_type,
    )
    .await?;
    let total = count_common_channels(&state.pool, q.query.as_deref(), online, channel_type).await?;

    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|c| channel_to_json(c))
        .collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
    }))))
}

/// GET /api/common/channel/civilCode/unusual/list
pub async fn unusual_civilcode_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    // 返回 civiCode 为空或异常的通道
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);

    let list: Vec<DeviceChannel> = common_channel::get_unusual_civilcode(&state.pool, page, count).await?;
    let total = common_channel::count_unusual_civilcode(&state.pool).await?;

    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|c| channel_to_json(c))
        .collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
    }))))
}

/// GET /api/common/channel/parent/unusual/list
pub async fn unusual_parent_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);

    let list: Vec<DeviceChannel> = common_channel::get_unusual_parent(&state.pool, page, count).await?;
    let total = common_channel::count_unusual_parent(&state.pool).await?;

    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|c| channel_to_json(c))
        .collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
    }))))
}

/// POST /api/common/channel/civilCode/unusual/clear
pub async fn clear_unusual_civilcode(
    State(state): State<AppState>,
    Json(body): Json<ClearChannelBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let channel_ids = body.channel_ids.unwrap_or_default();
    for id in channel_ids {
        common_channel::clear_unusual_civilcode(&state.pool, id).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/parent/unusual/clear
pub async fn clear_unusual_parent(
    State(state): State<AppState>,
    Json(body): Json<ClearChannelBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let channel_ids = body.channel_ids.unwrap_or_default();
    for id in channel_ids {
        common_channel::clear_unusual_parent(&state.pool, id).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/common/channel/parent/list
pub async fn parent_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);
    let online = match q.online.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let channel_type = q
        .channel_type
        .as_deref()
        .and_then(|s| s.parse::<i32>().ok());

    let list: Vec<DeviceChannel> = common_channel::get_parent_channels(&state.pool, page, count, q.query.as_deref(), online, channel_type).await?;
    let total = common_channel::count_parent_channels(&state.pool, q.query.as_deref(), online, channel_type).await?;

    // Phase 6: 同步输出 camelCase + gb_* + ptzTypeText,前端
    // /channel/group、/channel/region 都读取这些字段。
    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|c| channel_to_json(c))
        .collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
    }))))
}

// ========== 通道与区域/分组关联 ==========
/// POST /api/common/channel/region/add
#[derive(Debug, Deserialize)]
pub struct ChannelRegionBody {
    pub civil_code: Option<String>,
    pub channel_ids: Option<Vec<i64>>,
}

pub async fn channel_region_add(
    State(state): State<AppState>,
    Json(body): Json<ChannelRegionBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let civil_code = body.civil_code.as_deref().unwrap_or("").trim();
    if civil_code.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 civilCode"));
    }
    let channel_ids = body.channel_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for id in channel_ids {
        common_channel::update_civil_code(&state.pool, id, civil_code, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/region/delete
#[derive(Debug, Deserialize)]
pub struct ChannelRegionDeleteBody {
    pub channel_ids: Option<Vec<i64>>,
}

pub async fn channel_region_delete(
    State(state): State<AppState>,
    Json(body): Json<ChannelRegionDeleteBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let channel_ids = body.channel_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for id in channel_ids {
        common_channel::clear_civil_code(&state.pool, id, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/region/device/add
#[derive(Debug, Deserialize)]
pub struct DeviceRegionBody {
    pub civil_code: Option<String>,
    pub device_ids: Option<Vec<String>>,
}

pub async fn device_region_add(
    State(state): State<AppState>,
    Json(body): Json<DeviceRegionBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let civil_code = body.civil_code.as_deref().unwrap_or("").trim();
    if civil_code.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 civilCode"));
    }
    let device_ids = body.device_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for device_id in device_ids {
        common_channel::update_device_civil_code(&state.pool, &device_id, civil_code, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/region/device/delete
#[derive(Debug, Deserialize)]
pub struct DeviceRegionDeleteBody {
    pub device_ids: Option<Vec<String>>,
}

pub async fn device_region_delete(
    State(state): State<AppState>,
    Json(body): Json<DeviceRegionDeleteBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_ids = body.device_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for device_id in device_ids {
        common_channel::clear_device_civil_code(&state.pool, &device_id, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/group/add
#[derive(Debug, Deserialize)]
pub struct ChannelGroupBody {
    pub parent_id: Option<i64>,
    pub business_group: Option<String>,
    pub channel_ids: Option<Vec<i64>>,
}

pub async fn channel_group_add(
    State(state): State<AppState>,
    Json(body): Json<ChannelGroupBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let parent_id = body.parent_id.unwrap_or(0);
    let business_group = body.business_group.as_deref().unwrap_or("0");
    let channel_ids = body.channel_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for id in channel_ids {
        common_channel::update_group(&state.pool, id, parent_id, business_group, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/group/delete
#[derive(Debug, Deserialize)]
pub struct ChannelGroupDeleteBody {
    pub channel_ids: Option<Vec<i64>>,
}

pub async fn channel_group_delete(
    State(state): State<AppState>,
    Json(body): Json<ChannelGroupDeleteBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let channel_ids = body.channel_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for id in channel_ids {
        common_channel::clear_group(&state.pool, id, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/group/device/add
#[derive(Debug, Deserialize)]
pub struct DeviceGroupBody {
    pub parent_id: Option<i64>,
    pub business_group: Option<String>,
    pub device_ids: Option<Vec<String>>,
}

pub async fn device_group_add(
    State(state): State<AppState>,
    Json(body): Json<DeviceGroupBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let parent_id = body.parent_id.unwrap_or(0);
    let business_group = body.business_group.as_deref().unwrap_or("0");
    let device_ids = body.device_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for device_id in device_ids {
        common_channel::update_device_group(&state.pool, &device_id, parent_id, business_group, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/group/device/delete
#[derive(Debug, Deserialize)]
pub struct DeviceGroupDeleteBody {
    pub device_ids: Option<Vec<String>>,
}

pub async fn device_group_delete(
    State(state): State<AppState>,
    Json(body): Json<DeviceGroupDeleteBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_ids = body.device_ids.unwrap_or_default();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for device_id in device_ids {
        common_channel::clear_device_group(&state.pool, &device_id, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

// ========== 通道播放控制 ==========
pub async fn channel_play(
    State(state): State<AppState>,
    Query(q): Query<ChannelIdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let channel_id = match q.channel_id {
        Some(id) => id,
        None => return Json(WVPResult::error("缺少 channelId")),
    };
    
    match common_channel::get_by_id(&state.pool, channel_id).await {
        Ok(Some(ch)) => {
            let device_id = match &ch.device_id {
                Some(id) => id.clone(),
                None => return Json(WVPResult::error("通道无设备ID")),
            };
            let gb_channel_id = match &ch.gb_device_id {
                Some(id) => id.clone(),
                None => return Json(WVPResult::error("通道无国标ID")),
            };
            
            if let Some(ref zlm_client) = state.zlm_client {
                let rtsp_url = format!("rtsp://127.0.0.1:{}/{}", 554u16, channel_id);
                // Use similar approach as play_start to proxy the stream
                let request = crate::zlm::AddStreamProxyRequest {
                    secret: zlm_client.secret.clone(),
                    vhost: "__defaultVhost__".to_string(),
                    app: "gb".to_string(),
                    stream: format!("{}${}", device_id, channel_id),
                    url: rtsp_url.clone(),
                    rtp_type: Some(0),
                    timeout_sec: Some(30.0),
                    enable_hls: Some(false),
                    enable_mp4: Some(false),
                    enable_rtsp: Some(true),
                    enable_rtmp: Some(true),
                    enable_fmp4: Some(true),
                    enable_ts: Some(false),
                    enableAAC: Some(false),
                };
                match zlm_client.add_stream_proxy(&request).await {
                    Ok(key) => {
                        let stream_url = format!("gb/{}${}", device_id, channel_id);
                        // Phase 3.6: 用真实 ZLM 节点 IP（不再 hardcode 127.0.0.1）
                        let media_ip = zlm_client.ip.clone();
                        let http_port = zlm_client.http_port;
                        let play_url = format!("rtsp://{}:554/{}", media_ip, stream_url);
                        let flv_url = format!("http://{}:{}/{}.flv", media_ip, http_port, stream_url);
                        let data = serde_json::json!({
                            "app": "gb",
                            "stream": key,
                            "playUrl": play_url,
                            "flvUrl": flv_url,
                            "wsUrl": format!("ws://{}:{}/{}.flv", media_ip, http_port, stream_url),
                            "deviceId": device_id,
                            "channelId": gb_channel_id,
                            "hasAudio": ch.has_audio.unwrap_or(false),
                            "rtspUrl": rtsp_url,
                        });
                        return Json(WVPResult::success(data));
                    }
                    Err(e) => {
                        return Json(WVPResult::error(format!("ZLM error: {}", e)));
                    }
                }
            }
            Json(WVPResult::success(serde_json::json!({
                "app": "",
                "stream": "",
                "tracks": [],
                "msg": "ZLM not configured or unavailable"
            })))
        }
        Ok(None) => Json(WVPResult::error("通道不存在".to_string() )),
        Err(_) => Json(WVPResult::error("数据库错误")),
    }
}

pub async fn channel_play_stop(
    State(state): State<AppState>,
    Query(q): Query<ChannelIdQuery>,
) -> Json<serde_json::Value> {
    let channel_id = match q.channel_id {
        Some(id) => id,
        None => return Json(serde_json::json!({"code": 1, "msg": "缺少 channelId"})),
    };
    
    match common_channel::get_by_id(&state.pool, channel_id).await {
        Ok(Some(_ch)) => {
            if let Some(ref zlm_client) = state.zlm_client {
                // Stop the specific stream for this channel
                let stream_key = format!("__defaultVhost__/gb/{}@{}", _ch.device_id.clone().unwrap_or_default(), channel_id);
                let _ = zlm_client.close_streams(Some("rtsp"), Some("gb"), Some(&format!("{}@{}", _ch.device_id.clone().unwrap_or_default(), channel_id)), true).await;
                let resp = serde_json::json!({
                    "code": 0,
                    "msg": "停止播放请求已发送",
                    "data": {"stream": stream_key}
                });
                return Json(resp);
            }
            Json(serde_json::json!({"code": 0, "msg": "停止播放请求已发送"}))
        }
        Ok(None) => Json(serde_json::json!({"code": 1, "msg": "通道不存在"})),
        Err(_) => Json(serde_json::json!({"code": 1, "msg": "数据库错误"})),
    }
}

// ========== 地图相关 ==========
/// GET /api/common/channel/map/list
#[derive(Debug, Deserialize)]
pub struct MapChannelQuery {
    pub query: Option<String>,
    pub online: Option<String>,
    pub has_record_plan: Option<String>,
    pub channel_type: Option<String>,
}

pub async fn map_channel_list(
    State(state): State<AppState>,
    Query(q): Query<MapChannelQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let online = match q.online.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let channel_type = q
        .channel_type
        .as_deref()
        .and_then(|s| s.parse::<i32>().ok());

    let list: Vec<DeviceChannel> = common_channel::get_channels_for_map(&state.pool, q.query.as_deref(), online, channel_type).await?;

    let rows: Vec<serde_json::Value> = list
        .into_iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "deviceId": c.device_id,
                "name": c.name,
                "channelId": c.gb_device_id,
                "longitude": c.longitude,
                "latitude": c.latitude,
                "channelType": c.channel_type,
                "status": c.status,
            })
        })
        .collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": rows.len(),
    }))))
}

/// POST /api/common/channel/map/save-level
#[derive(Debug, Deserialize)]
pub struct MapLevelBody {
    pub level: Option<i32>,
    pub channels: Option<Vec<i64>>,
}

pub async fn map_save_level(
    State(state): State<AppState>,
    Json(body): Json<MapLevelBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let level = body.level.unwrap_or(0);
    let channels = body.channels.unwrap_or_default();
    
    if channels.is_empty() {
        return Ok(Json(WVPResult::<()>::success_empty()));
    }
    
    let result: sqlx::Result<u64> = common_channel::update_map_level(&state.pool, &channels, level).await;
    result.map_err(|e| AppError::business(ErrorCode::Error500, format!("更新地图级别失败: {}", e)))?;
    
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/map/reset-level
pub async fn map_reset_level(
    State(state): State<AppState>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let result: sqlx::Result<u64> = common_channel::reset_map_level(&state.pool).await;
    result.map_err(|e| AppError::business(ErrorCode::Error500, format!("重置地图级别失败: {}", e)))?;
    
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/common/channel/map/thin/clear?id=
/// 内部工具 — 按 feature 分发不同 SQL；sqlite 路径下部分参数仅在 cfg(postgres/mysql) 中使用
#[allow(unused_variables)]
pub async fn map_thin_clear(
    State(state): State<AppState>,
    Query(q): Query<ChannelIdQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let channel_id = q.channel_id.unwrap_or(0);
    if channel_id > 0 {
        #[cfg(feature = "postgres")]
        sqlx::query("UPDATE gb_device_channel SET geojson = NULL WHERE id = $1")
            .bind(channel_id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("清除稀化数据失败: {}", e)))?;
        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        sqlx::query("UPDATE gb_device_channel SET geojson = NULL WHERE id = ?")
            .bind(channel_id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("清除稀化数据失败: {}", e)))?;
        tracing::info!("Cleared thinned geojson for channel {}", channel_id);
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/common/channel/map/thin/progress?id=
pub async fn map_thin_progress(
    State(state): State<AppState>,
    Query(q): Query<ChannelIdQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let channel_id = q.channel_id.unwrap_or(0);
    if channel_id > 0 {
        #[cfg(feature = "postgres")]
        let has_geojson: bool = sqlx::query_scalar(
            "SELECT (geojson IS NOT NULL) FROM gb_device_channel WHERE id = $1"
        )
        .bind(channel_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);

        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        let has_geojson: bool = sqlx::query_scalar(
            "SELECT (geojson IS NOT NULL) FROM gb_device_channel WHERE id = ?"
        )
        .bind(channel_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);

        let progress = if has_geojson { 100 } else { 0 };
        return Ok(Json(WVPResult::success(serde_json::json!({
            "progress": progress
        }))));
    }
    Ok(Json(WVPResult::success(serde_json::json!({
        "progress": 0
    }))))
}

/// GET /api/common/channel/map/thin/save?id=
/// Performs Douglas-Peucker thinning on the channel's position history and saves result
pub async fn map_thin_save(
    State(state): State<AppState>,
    Query(q): Query<ChannelIdQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let channel_id = q.channel_id.unwrap_or(0);
    if channel_id <= 0 {
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    // Get channel's device_id and gb_device_id to look up position history
    let channel = common_channel::get_by_id(&state.pool, channel_id).await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("查询通道失败: {}", e)))?;
    
    let channel = match channel {
        Some(c) => c,
        None => return Ok(Json(WVPResult::<()>::success_empty())),
    };

    let device_id = match &channel.device_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => return Ok(Json(WVPResult::<()>::success_empty())),
    };

    // Get position history points
    #[derive(sqlx::FromRow)]
    struct PositionPoint {
        longitude: Option<f64>,
        latitude: Option<f64>,
    }

    #[cfg(feature = "postgres")]
    let points: Vec<PositionPoint> = sqlx::query_as(
        "SELECT longitude, latitude FROM gb_position_history WHERE device_id = $1 ORDER BY time",
    )
    .bind(&device_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let points: Vec<PositionPoint> = sqlx::query_as(
        "SELECT longitude, latitude FROM gb_position_history WHERE device_id = ? ORDER BY time",
    )
    .bind(&device_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    if points.len() < 2 {
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    // Douglas-Peucker simplification with epsilon = 0.0001 degrees (~11m)
    let coords: Vec<(f64, f64)> = points.iter()
        .filter_map(|p| {
            match (p.longitude, p.latitude) {
                (Some(lng), Some(lat)) if lng != 0.0 && lat != 0.0 => Some((lng, lat)),
                _ => None,
            }
        })
        .collect();

    let simplified = douglas_peucker(&coords, 0.0001);

    // Build GeoJSON LineString
    let coord_arrays: Vec<Vec<f64>> = simplified.iter()
        .map(|(lng, lat)| vec![*lng, *lat])
        .collect();
    let geojson = serde_json::json!({
        "type": "Feature",
        "geometry": {
            "type": "LineString",
            "coordinates": coord_arrays
        },
        "properties": {
            "channelId": channel_id,
            "originalPoints": coords.len(),
            "simplifiedPoints": simplified.len()
        }
    });
    let geojson_str = serde_json::to_string(&geojson).unwrap_or_default();

    // Save to channel's geojson field
    // 修正：此前两条分支都用 `let _ =` 吞掉错误，稀化算了半天却可能根本没落库，
    // 接口仍返回成功。现在如实传播。
    #[cfg(feature = "postgres")]
    let affected = sqlx::query("UPDATE gb_device_channel SET geojson = $1 WHERE id = $2")
        .bind(&geojson_str)
        .bind(channel_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("保存稀化数据失败: {}", e)))?
        .rows_affected();

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let affected = sqlx::query("UPDATE gb_device_channel SET geojson = ? WHERE id = ?")
        .bind(&geojson_str)
        .bind(channel_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("保存稀化数据失败: {}", e)))?
        .rows_affected();

    if affected == 0 {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("通道不存在: id={}", channel_id),
        ));
    }

    tracing::info!("Map thin saved for channel {}: {} -> {} points", channel_id, coords.len(), simplified.len());

    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/common/channel/map/thin/draw
#[derive(Debug, Deserialize)]
pub struct MapThinDrawBody {
    pub id: Option<i64>,
    pub geojson: Option<serde_json::Value>,
}

pub async fn map_thin_draw(
    State(state): State<AppState>,
    Json(body): Json<MapThinDrawBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let channel_id = body.id.unwrap_or(0);
    if channel_id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "type": "Feature",
            "geometry": { "type": "LineString", "coordinates": [] },
            "properties": {}
        }))));
    }

    // If geojson provided in request, save it and return
    if let Some(ref geojson) = body.geojson {
        let geojson_str = serde_json::to_string(geojson).unwrap_or_default();
        // 修正：此前写入失败会被静默忽略，但仍把入参回显为“已保存”
        #[cfg(feature = "postgres")]
        let affected = sqlx::query("UPDATE gb_device_channel SET geojson = $1 WHERE id = $2")
            .bind(&geojson_str)
            .bind(channel_id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("保存绘制数据失败: {}", e)))?
            .rows_affected();
        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        let affected = sqlx::query("UPDATE gb_device_channel SET geojson = ? WHERE id = ?")
            .bind(&geojson_str)
            .bind(channel_id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("保存绘制数据失败: {}", e)))?
            .rows_affected();
        if affected == 0 {
            return Err(AppError::business(
                ErrorCode::Error404,
                format!("通道不存在: id={}", channel_id),
            ));
        }
        return Ok(Json(WVPResult::success(geojson.clone())));
    }

    // Otherwise return stored thinned geojson
    #[derive(sqlx::FromRow)]
    struct GeojsonRow {
        geojson: Option<String>,
    }

    #[cfg(feature = "postgres")]
    let row: Option<GeojsonRow> = sqlx::query_as(
        "SELECT geojson FROM gb_device_channel WHERE id = $1"
    )
    .bind(channel_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let row: Option<GeojsonRow> = sqlx::query_as(
        "SELECT geojson FROM gb_device_channel WHERE id = ?"
    )
    .bind(channel_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();

    let geojson = row.and_then(|r| r.geojson)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({
            "type": "Feature",
            "geometry": { "type": "LineString", "coordinates": [] },
            "properties": {}
        }));

    Ok(Json(WVPResult::success(geojson)))
}

/// Douglas-Peucker line simplification algorithm
fn douglas_peucker(points: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let first = points[0];
    let last = points[points.len() - 1];

    let mut max_dist = 0.0;
    let mut max_idx = 0;

    for (i, point) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let dist = perpendicular_distance(*point, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        let mut left = douglas_peucker(&points[..=max_idx], epsilon);
        let right = douglas_peucker(&points[max_idx..], epsilon);
        left.pop();
        left.extend_from_slice(&right);
        left
    } else {
        vec![first, last]
    }
}

/// Calculate perpendicular distance from point to line defined by two endpoints
fn perpendicular_distance(point: (f64, f64), line_start: (f64, f64), line_end: (f64, f64)) -> f64 {
    let (px, py) = point;
    let (x1, y1) = line_start;
    let (x2, y2) = line_end;

    let dx = x2 - x1;
    let dy = y2 - y1;

    let line_len_sq = dx * dx + dy * dy;
    if line_len_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    let t = ((px - x1) * dx + (py - y1) * dy) / line_len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;

    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

/// GET /api/sy/camera/list/ids (测试接口)
#[derive(Debug, Deserialize)]
pub struct CameraListQuery {
    pub device_ids: Option<String>,
    pub geo_coord_sys: Option<String>,
    pub traditional: Option<bool>,
}

pub async fn camera_list_ids(
    Query(q): Query<CameraListQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_ids: Vec<String> = q.device_ids
        .as_ref()
        .map(|s| s.split(',').map(|x| x.to_string()).collect())
        .unwrap_or_default();

    let mut result = Vec::new();
    for device_id in device_ids {
        result.push(serde_json::json!({
            "deviceId": device_id,
            "latitude": 39.9042,
            "longitude": 116.4074,
            "name": format!("Camera-{}", device_id)
        }));
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": result,
        "total": result.len()
    }))))
}

// ========== 前端控制 front-end (commonChannel.js 使用的 channelId 版本) ==========
/// GET /api/common/channel/front-end/ptz
#[derive(Debug, Deserialize)]
pub struct CommonChannelPtzQuery {
    pub channel_id: Option<i64>,
    pub command: Option<String>,
    pub pan_speed: Option<i32>,
    pub tilt_speed: Option<i32>,
    pub zoom_speed: Option<i32>,
}

pub async fn front_end_ptz(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelPtzQuery>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let command = q.command.clone().unwrap_or_default();
    let h_speed = q.pan_speed.unwrap_or(1) as u8;
    let v_speed = q.tilt_speed.unwrap_or(1) as u8;
    let z_speed = q.zoom_speed.unwrap_or(1) as u8;

    tracing::info!("commonChannel PTZ: channel_id={}, cmd={}", channel_id, command);

    let body = build_ptz_xml(&command, h_speed, v_speed, z_speed);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "PTZ控制命令已发送".to_string())
    }).await
}

/// GET /api/common/channel/front-end/auxiliary
#[derive(Debug, Deserialize)]
pub struct AuxiliaryQuery {
    pub channel_id: Option<i64>,
    pub command: Option<String>,
    pub auxiliary_id: Option<i32>,
}

pub async fn front_end_auxiliary(
    State(state): State<AppState>,
    Query(q): Query<AuxiliaryQuery>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let command = q.command.clone().unwrap_or_default();
    let aux_id = q.auxiliary_id.unwrap_or(0) as u32;
    let body = format!(r#"<AuxiliaryCmd><cmd>{}</cmd><index>{}</index></AuxiliaryCmd>"#, 
        if command.to_lowercase() == "on" { "Set" } else { "Reset" }, aux_id);
    tracing::info!("commonChannel auxiliary: channel_id={}, cmd={}", channel_id, command);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "辅助开关控制命令已发送".to_string())
    }).await
}

/// GET /api/common/channel/front-end/wiper
#[derive(Debug, Deserialize)]
pub struct CommonWiperQuery {
    pub channel_id: Option<i64>,
    pub command: Option<String>,
}

pub async fn front_end_wiper(
    State(state): State<AppState>,
    Query(q): Query<CommonWiperQuery>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let command = q.command.clone().unwrap_or_default();
    let body = format!(r#"<WiperCmd>{}</WiperCmd>"#, if command.to_lowercase() == "on" { "Open" } else { "Close" });
    tracing::info!("commonChannel wiper: channel_id={}, cmd={}", channel_id, command);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "雨刷控制命令已发送".to_string())
    }).await
}

/// GET /api/common/channel/front-end/fi/iris
#[derive(Debug, Deserialize)]
pub struct IrisQuery {
    pub channel_id: Option<i64>,
    pub command: Option<String>,
    pub speed: Option<i32>,
}

pub async fn front_end_iris(
    State(state): State<AppState>,
    Query(q): Query<IrisQuery>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let command = q.command.clone().unwrap_or_default();
    let speed = q.speed.unwrap_or(1) as u8;
    let body = build_fi_xml("iris", &command, speed);
    tracing::info!("commonChannel iris: channel_id={}, cmd={}", channel_id, command);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "光圈控制命令已发送".to_string())
    }).await
}

/// GET /api/common/channel/front-end/fi/focus
#[derive(Debug, Deserialize)]
pub struct FocusQuery {
    pub channel_id: Option<i64>,
    pub command: Option<String>,
    pub speed: Option<i32>,
}

pub async fn front_end_focus(
    State(state): State<AppState>,
    Query(q): Query<FocusQuery>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let command = q.command.clone().unwrap_or_default();
    let speed = q.speed.unwrap_or(1) as u8;
    let body = build_fi_xml("focus", &command, speed);
    tracing::info!("commonChannel focus: channel_id={}, cmd={}", channel_id, command);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "聚焦控制命令已发送".to_string())
    }).await
}

// ========== 预置位 ==========
/// GET /api/common/channel/front-end/preset/query
#[derive(Debug, Deserialize)]
pub struct PresetQueryQ {
    pub channel_id: Option<i64>,
}

pub async fn front_end_preset_query(
    State(state): State<AppState>,
    Query(q): Query<PresetQueryQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    tracing::info!("commonChannel preset query: channel_id={}", channel_id);
    let body = r#"<PTZCmd Query="PresetList"/>"#.to_string();
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "预置位查询命令已发送".to_string())
    }).await
}

/// GET /api/common/channel/front-end/preset/add
#[derive(Debug, Deserialize)]
pub struct PresetAddQ {
    pub channel_id: Option<i64>,
    pub preset_id: Option<i32>,
    pub preset_name: Option<String>,
}

pub async fn front_end_preset_add(
    State(state): State<AppState>,
    Query(q): Query<PresetAddQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let preset_id = q.preset_id.unwrap_or(0);
    let body = build_preset_xml("SET_PRESET", preset_id as u32);
    tracing::info!("commonChannel preset add: channel_id={}, preset_id={}", channel_id, preset_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "预置位添加成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/preset/call
#[derive(Debug, Deserialize)]
pub struct PresetCallQ {
    pub channel_id: Option<i64>,
    pub preset_id: Option<i32>,
}

pub async fn front_end_preset_call(
    State(state): State<AppState>,
    Query(q): Query<PresetCallQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let preset_id = q.preset_id.unwrap_or(0);
    let body = build_preset_xml("GOTO_PRESET", preset_id as u32);
    tracing::info!("commonChannel preset call: channel_id={}, preset_id={}", channel_id, preset_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "预置位调用成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/preset/delete
#[derive(Debug, Deserialize)]
pub struct PresetDeleteQ {
    pub channel_id: Option<i64>,
    pub preset_id: Option<i32>,
}

pub async fn front_end_preset_delete(
    State(state): State<AppState>,
    Query(q): Query<PresetDeleteQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let preset_id = q.preset_id.unwrap_or(0);
    let body = build_preset_xml("CLEAR_PRESET", preset_id as u32);
    tracing::info!("commonChannel preset delete: channel_id={}, preset_id={}", channel_id, preset_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "预置位删除成功".to_string())
    }).await
}

// ========== 巡航 ==========
/// GET /api/common/channel/front-end/tour/point/add
#[derive(Debug, Deserialize)]
pub struct TourPointAddQ {
    pub channel_id: Option<i64>,
    pub tour_id: Option<i32>,
    pub preset_id: Option<i32>,
}

pub async fn front_end_tour_point_add(
    State(state): State<AppState>,
    Query(q): Query<TourPointAddQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let tour_id = q.tour_id.unwrap_or(0);
    let preset_id = q.preset_id.unwrap_or(0);
    let body = format!(r#"<CruiseCmd id="{}" preset="{}" action="add" />"#, tour_id, preset_id);
    tracing::info!("commonChannel tour point add: channel_id={}, tour_id={}", channel_id, tour_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "巡航点添加成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/tour/point/delete
#[derive(Debug, Deserialize)]
pub struct TourPointDeleteQ {
    pub channel_id: Option<i64>,
    pub tour_id: Option<i32>,
    pub preset_id: Option<i32>,
}

pub async fn front_end_tour_point_delete(
    State(state): State<AppState>,
    Query(q): Query<TourPointDeleteQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let tour_id = q.tour_id.unwrap_or(0);
    let preset_id = q.preset_id.unwrap_or(0);
    let body = format!(r#"<CruiseCmd id="{}" preset="{}" action="delete" />"#, tour_id, preset_id);
    tracing::info!("commonChannel tour point delete: channel_id={}, tour_id={}", channel_id, tour_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "巡航点删除成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/tour/speed
#[derive(Debug, Deserialize)]
pub struct TourSpeedQ {
    pub channel_id: Option<i64>,
    pub tour_id: Option<i32>,
    pub preset_id: Option<i32>,
    pub speed: Option<i32>,
}

pub async fn front_end_tour_speed(
    State(state): State<AppState>,
    Query(q): Query<TourSpeedQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let tour_id = q.tour_id.unwrap_or(0);
    let speed = q.speed.unwrap_or(1);
    let body = format!(r#"<CruiseSpeed id="{}" speed="{}" />"#, tour_id, speed);
    tracing::info!("commonChannel tour speed: channel_id={}, tour_id={}", channel_id, tour_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "巡航速度设置成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/tour/time
#[derive(Debug, Deserialize)]
pub struct TourTimeQ {
    pub channel_id: Option<i64>,
    pub tour_id: Option<i32>,
    pub preset_id: Option<i32>,
    pub time: Option<i32>,
}

pub async fn front_end_tour_time(
    State(state): State<AppState>,
    Query(q): Query<TourTimeQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let tour_id = q.tour_id.unwrap_or(0);
    let time = q.time.unwrap_or(10);
    let body = format!(r#"<CruiseTime id="{}" time="{}" />"#, tour_id, time);
    tracing::info!("commonChannel tour time: channel_id={}, tour_id={}", channel_id, tour_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "巡航停留时间设置成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/tour/start
#[derive(Debug, Deserialize)]
pub struct TourStartQ {
    pub channel_id: Option<i64>,
    pub tour_id: Option<i32>,
}

pub async fn front_end_tour_start(
    State(state): State<AppState>,
    Query(q): Query<TourStartQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let tour_id = q.tour_id.unwrap_or(0);
    let body = format!(r#"<CruiseCmd id="{}" action="start" />"#, tour_id);
    tracing::info!("commonChannel tour start: channel_id={}, tour_id={}", channel_id, tour_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "巡航启动成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/tour/stop
#[derive(Debug, Deserialize)]
pub struct TourStopQ {
    pub channel_id: Option<i64>,
    pub tour_id: Option<i32>,
}

pub async fn front_end_tour_stop(
    State(state): State<AppState>,
    Query(q): Query<TourStopQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let tour_id = q.tour_id.unwrap_or(0);
    let body = format!(r#"<CruiseCmd id="{}" action="stop" />"#, tour_id);
    tracing::info!("commonChannel tour stop: channel_id={}, tour_id={}", channel_id, tour_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "巡航停止成功".to_string())
    }).await
}

// ========== 扫描 ==========
/// GET /api/common/channel/front-end/scan/set/speed
#[derive(Debug, Deserialize)]
pub struct ScanSpeedQ {
    pub channel_id: Option<i64>,
    pub scan_id: Option<i32>,
    pub speed: Option<i32>,
}

pub async fn front_end_scan_set_speed(
    State(state): State<AppState>,
    Query(q): Query<ScanSpeedQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let scan_id = q.scan_id.unwrap_or(0);
    let speed = q.speed.unwrap_or(1);
    let body = format!(r#"<ScanSpeed id="{}" speed="{}" />"#, scan_id, speed);
    tracing::info!("commonChannel scan speed: channel_id={}, scan_id={}", channel_id, scan_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "扫描速度设置成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/scan/set/left
#[derive(Debug, Deserialize)]
pub struct ScanLeftQ {
    pub channel_id: Option<i64>,
    pub scan_id: Option<i32>,
}

pub async fn front_end_scan_set_left(
    State(state): State<AppState>,
    Query(q): Query<ScanLeftQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let scan_id = q.scan_id.unwrap_or(0);
    let body = format!(r#"<ScanSet id="{}" type="left" />"#, scan_id);
    tracing::info!("commonChannel scan left: channel_id={}, scan_id={}", channel_id, scan_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "扫描左边界设置成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/scan/set/right
#[derive(Debug, Deserialize)]
pub struct ScanRightQ {
    pub channel_id: Option<i64>,
    pub scan_id: Option<i32>,
}

pub async fn front_end_scan_set_right(
    State(state): State<AppState>,
    Query(q): Query<ScanRightQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let scan_id = q.scan_id.unwrap_or(0);
    let body = format!(r#"<ScanSet id="{}" type="right" />"#, scan_id);
    tracing::info!("commonChannel scan right: channel_id={}, scan_id={}", channel_id, scan_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "扫描右边界设置成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/scan/start
#[derive(Debug, Deserialize)]
pub struct ScanStartQ {
    pub channel_id: Option<i64>,
    pub scan_id: Option<i32>,
}

pub async fn front_end_scan_start(
    State(state): State<AppState>,
    Query(q): Query<ScanStartQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let scan_id = q.scan_id.unwrap_or(0);
    let body = format!(r#"<ScanCmd id="{}" action="start" />"#, scan_id);
    tracing::info!("commonChannel scan start: channel_id={}, scan_id={}", channel_id, scan_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "扫描启动成功".to_string())
    }).await
}

/// GET /api/common/channel/front-end/scan/stop
#[derive(Debug, Deserialize)]
pub struct ScanStopQ {
    pub channel_id: Option<i64>,
    pub scan_id: Option<i32>,
}

pub async fn front_end_scan_stop(
    State(state): State<AppState>,
    Query(q): Query<ScanStopQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let scan_id = q.scan_id.unwrap_or(0);
    let body = format!(r#"<ScanCmd id="{}" action="stop" />"#, scan_id);
    tracing::info!("commonChannel scan stop: channel_id={}, scan_id={}", channel_id, scan_id);
    lookup_channel_and_send(&state, channel_id, |_| {
        ("DeviceControl".to_string(), body.clone(), "扫描停止成功".to_string())
    }).await
}

// ========== 通道回放 (commonChannel.js 使用的 channelId 版本) ==========
/// GET /api/common/channel/playback/query
#[derive(Debug, Deserialize)]
pub struct ChannelPlaybackQueryQ {
    pub channel_id: Option<i64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

pub async fn channel_playback_query(
    State(state): State<AppState>,
    Query(q): Query<ChannelPlaybackQueryQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();
    tracing::info!("commonChannel playback query: channel_id={}, {}-{}", channel_id, start_time, end_time);

    match common_channel::get_by_id(&state.pool, channel_id).await {
        Ok(Some(ch)) => {
            let device_id = ch.device_id.clone().unwrap_or_default();
            let gb_channel_id = ch.gb_device_id.clone().unwrap_or_default();
            tracing::info!("Playback query for device={}, channel={}", device_id, gb_channel_id);

            if let Some(ref zlm_client) = state.zlm_client {
                match zlm_client.get_mp4_record_file("rtp", &gb_channel_id, None, None, None).await {
                    Ok(list) => {
                        let filtered: Vec<serde_json::Value> = list.into_iter().map(|r| {
                            serde_json::json!({
                                "fileName": r.name,
                                "filePath": r.file_path,
                                "fileSize": r.size,
                                "startTime": r.create_time,
                                "duration": r.duration,
                            })
                        }).collect();
                        return Json(serde_json::json!({
                            "code": 0,
                            "msg": "查询成功",
                            "data": filtered
                        }));
                    }
                    Err(e) => tracing::warn!("ZLM record query failed: {}", e),
                }
            }
            Json(serde_json::json!({ "code": 0, "msg": "查询成功", "data": [] }))
        }
        _ => Json(serde_json::json!({ "code": 1, "msg": "通道不存在" })),
    }
}

/// GET /api/common/channel/playback
#[derive(Debug, Deserialize)]
pub struct ChannelPlaybackStartQ {
    pub channel_id: Option<i64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

pub async fn channel_playback_start(
    State(state): State<AppState>,
    Query(q): Query<ChannelPlaybackStartQ>,
) -> Json<serde_json::Value> {
    let channel_id = q.channel_id.unwrap_or(0);
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();
    tracing::info!("commonChannel playback start: channel_id={}, {}-{}", channel_id, start_time, end_time);

    match common_channel::get_by_id(&state.pool, channel_id).await {
        Ok(Some(ch)) => {
            let device_id = match &ch.device_id {
                Some(id) => id.clone(),
                None => return Json(serde_json::json!({ "code": 1, "msg": "通道无设备ID" })),
            };
            let gb_channel_id = ch.gb_device_id.clone().unwrap_or_default();

            if let Some(ref sip_server) = state.sip_server {
                let server = &*sip_server;
                if let Some(device) = server.device_manager().get(&device_id).await {
                    if device.online {
                        // 先让 ZLM 分配收流端口，再把它写进 INVITE 的 m=video。
                        // 此前这里直接发 INVITE，SDP 里 m=video 端口是 0
                        // （SDP 中 0 表示媒体流被禁用），设备无从推流。
                        let stream_id = format!("playback_{}_{}", device_id, channel_id);
                        let media_port = match state.zlm_client.as_ref() {
                            Some(zlm) => {
                                match zlm
                                    .open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                                        secret: zlm.secret.clone(),
                                        stream_id: stream_id.clone(),
                                        port: Some(0),
                                        use_tcp: Some(false),
                                        rtp_type: Some(0),
                                        recv_port: None,
                                    })
                                    .await
                                {
                                    Ok(info) => info.port,
                                    Err(e) => {
                                        tracing::error!(
                                            "openRtpServer for playback {}/{} failed: {}",
                                            device_id, channel_id, e
                                        );
                                        return Json(serde_json::json!({
                                            "code": 1,
                                            "msg": format!("ZLM 收流端口分配失败: {}", e)
                                        }));
                                    }
                                }
                            }
                            None => {
                                return Json(serde_json::json!({
                                    "code": 1,
                                    "msg": "ZLM 未配置，无法建立回放收流"
                                }));
                            }
                        };
                        match server.send_playback_invite(&device_id, &gb_channel_id, &start_time, &end_time, media_port).await {
                            Ok(_) => {
                                let stream_id = format!("playback_{}_{}", device_id, channel_id);
                                return Json(serde_json::json!({
                                    "code": 0,
                                    "msg": "回放启动成功",
                                    "data": {
                                        "streamId": stream_id,
                                        "deviceId": device_id,
                                        "channelId": gb_channel_id,
                                        "startTime": start_time,
                                        "endTime": end_time,
                                    }
                                }));
                            }
                            Err(e) => {
                                tracing::error!("Playback invite failed: {}", e);
                                return Json(serde_json::json!({ "code": 1, "msg": format!("回放邀请失败: {}", e) }));
                            }
                        }
                    }
                }
            }
            Json(serde_json::json!({ "code": 1, "msg": "设备不在线或SIP未初始化" }))
        }
        Ok(None) => Json(serde_json::json!({ "code": 1, "msg": "通道不存在" })),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": format!("数据库错误: {}", e) })),
    }
}

// ─────────────────────── 回放控制（GB28181 PlayBackCtrl） ───────────────────────
//
// 2026-09-11：以下端点是 WVP `/api/common/channel/playback/*` 兼容路径。
// 此前 pause / resume / seek / speed 四个端点只 `tracing::info!` 便返回成功
// （形参写成 `State(_state)`，故意不接收 state），属于「编造成功」；
// 现按 GB28181 PlayBackCtrl 规范真正下发 SIP，并同步本地回放会话状态。

/// 纯函数：从 `{prefix}_{deviceId}_{channelId}_{ts}` 解析设备/通道。
///
/// GB28181 回放流名形如 `playback_34020000001320000001_34020000001310000001_1700000000`。
fn parse_playback_stream_id(stream: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = stream.split('_').collect();
    if parts.len() >= 3 && !parts[1].is_empty() && !parts[2].is_empty() {
        Some((parts[1].to_string(), parts[2].to_string()))
    } else {
        None
    }
}

/// 解析回放控制的目标设备/通道。
///
/// 优先级：
/// 1. `playback_manager` 中登记的会话（权威 —— 含真实 `device_id` / `channel_id`）
/// 2. 从 stream 解析 `{prefix}_{deviceId}_{channelId}_{ts}` 形式
async fn resolve_playback_target(state: &AppState, stream: &str) -> Option<(String, String)> {
    if stream.is_empty() {
        return None;
    }
    if let Some(ref pm) = state.playback_manager {
        if let Some(session) = pm.get(stream).await {
            if !session.device_id.is_empty() && !session.channel_id.is_empty() {
                return Some((session.device_id, session.channel_id));
            }
        }
    }
    parse_playback_stream_id(stream)
}

/// 解析目标并下发 SIP `PlayBackCtrl`；成功返回 `(device_id, channel_id)`。
async fn dispatch_playback_control(
    state: &AppState,
    stream: &str,
    cmd: crate::sip::PlaybackControlCmd,
) -> Result<(String, String), String> {
    let (device_id, channel_id) = resolve_playback_target(state, stream)
        .await
        .ok_or_else(|| {
            format!(
                "无法解析 stream '{}' 的设备/通道：既无已登记的回放会话，也不符合 prefix_deviceId_channelId 格式",
                stream
            )
        })?;

    let sip_server = state
        .sip_server
        .as_ref()
        .ok_or_else(|| "SIP 服务未启用，回放控制未能下发".to_string())?;

    sip_server
        .send_playback_control(&device_id, &channel_id, cmd)
        .await
        .map_err(|e| format!("SIP 下发失败: {}", e))?;

    Ok((device_id, channel_id))
}

/// 统一成功响应体（保持 commonChannel 既有的 `code` / `msg` 契约）
fn playback_control_ok(
    stream: &str,
    device_id: &str,
    channel_id: &str,
    msg: &str,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "code": 0,
        "msg": msg,
        "stream": stream,
        "deviceId": device_id,
        "channelId": channel_id,
    }))
}

/// GET /api/common/channel/playback/stop
#[derive(Debug, Deserialize)]
pub struct ChannelPlaybackStopQ {
    pub channel_id: Option<i64>,
    pub stream: Option<String>,
}

/// 停止回放：解析目标 → 关闭 ZLM 流 → 摘除本地会话 → 下发 SIP BYE。
pub async fn channel_playback_stop(
    State(state): State<AppState>,
    Query(q): Query<ChannelPlaybackStopQ>,
) -> Json<serde_json::Value> {
    let stream = q.stream.clone().unwrap_or_default();
    tracing::info!(
        "commonChannel playback stop: channel_id={}, stream={}",
        q.channel_id.unwrap_or(0),
        stream
    );
    if stream.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 stream 参数" }));
    }

    // 1) 先解析目标（此时会话仍在，可拿到权威 device/channel）
    let target = resolve_playback_target(&state, &stream).await;

    // 2) 关闭 ZLM 流：优先按登记的会话（含 schema/app/stream 与 media_server_id）
    if let Some(ref pm) = state.playback_manager {
        if let Some(session) = pm.get(&stream).await {
            if let Some(zlm_client) = state
                .get_zlm_client(session.media_server_id.as_deref())
                .or_else(|| state.zlm_client.clone())
            {
                let _ = zlm_client
                    .close_streams(
                        Some(&session.schema),
                        Some(&session.app),
                        Some(&session.stream),
                        true,
                    )
                    .await;
            }
            pm.remove(&stream).await;
        }
    }
    // 兜底：无登记会话时按 stream 名关闭
    if let Some(ref zlm_client) = state.zlm_client {
        let _ = zlm_client
            .close_streams(None, None, Some(&stream), true)
            .await;
    }

    // 3) 下发 SIP BYE，真正让设备停止回放推流
    let (device_id, channel_id) = match target {
        Some(v) => v,
        None => {
            return Json(serde_json::json!({
                "code": 1,
                "msg": format!("无法解析 stream '{}' 的设备/通道：ZLM 流已清理，但未能下发 SIP BYE", stream)
            }))
        }
    };
    let Some(ref sip_server) = state.sip_server else {
        return Json(serde_json::json!({
            "code": 1,
            "msg": "SIP 服务未启用：ZLM 流已清理，但未能下发 SIP BYE"
        }));
    };
    if let Err(e) = sip_server.send_session_bye(&device_id, &channel_id).await {
        return Json(serde_json::json!({ "code": 1, "msg": format!("SIP BYE 下发失败: {}", e) }));
    }

    playback_control_ok(&stream, &device_id, &channel_id, "回放停止成功")
}

/// GET /api/common/channel/playback/pause
#[derive(Debug, Deserialize)]
pub struct ChannelPlaybackPauseQ {
    pub channel_id: Option<i64>,
    pub stream: Option<String>,
}

pub async fn channel_playback_pause(
    State(state): State<AppState>,
    Query(q): Query<ChannelPlaybackPauseQ>,
) -> Json<serde_json::Value> {
    let stream = q.stream.clone().unwrap_or_default();
    tracing::info!(
        "commonChannel playback pause: channel_id={}, stream={}",
        q.channel_id.unwrap_or(0),
        stream
    );
    if stream.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 stream 参数" }));
    }

    if let Some(ref pm) = state.playback_manager {
        pm.pause(&stream).await;
    }
    match dispatch_playback_control(&state, &stream, crate::sip::PlaybackControlCmd::Pause).await {
        Ok((device_id, channel_id)) => {
            playback_control_ok(&stream, &device_id, &channel_id, "回放暂停成功")
        }
        Err(msg) => Json(serde_json::json!({ "code": 1, "msg": msg })),
    }
}

/// GET /api/common/channel/playback/resume
#[derive(Debug, Deserialize)]
pub struct ChannelPlaybackResumeQ {
    pub channel_id: Option<i64>,
    pub stream: Option<String>,
}

pub async fn channel_playback_resume(
    State(state): State<AppState>,
    Query(q): Query<ChannelPlaybackResumeQ>,
) -> Json<serde_json::Value> {
    let stream = q.stream.clone().unwrap_or_default();
    tracing::info!(
        "commonChannel playback resume: channel_id={}, stream={}",
        q.channel_id.unwrap_or(0),
        stream
    );
    if stream.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 stream 参数" }));
    }

    if let Some(ref pm) = state.playback_manager {
        pm.resume(&stream).await;
    }
    match dispatch_playback_control(&state, &stream, crate::sip::PlaybackControlCmd::Resume).await {
        Ok((device_id, channel_id)) => {
            playback_control_ok(&stream, &device_id, &channel_id, "回放恢复成功")
        }
        Err(msg) => Json(serde_json::json!({ "code": 1, "msg": msg })),
    }
}

/// GET /api/common/channel/playback/seek
#[derive(Debug, Deserialize)]
pub struct ChannelPlaybackSeekQ {
    pub channel_id: Option<i64>,
    pub stream: Option<String>,
    pub seek_time: Option<String>,
}

pub async fn channel_playback_seek(
    State(state): State<AppState>,
    Query(q): Query<ChannelPlaybackSeekQ>,
) -> Json<serde_json::Value> {
    let stream = q.stream.clone().unwrap_or_default();
    let seek_time = q.seek_time.clone().unwrap_or_default();
    tracing::info!(
        "commonChannel playback seek: channel_id={}, stream={}, seek={}",
        q.channel_id.unwrap_or(0),
        stream,
        seek_time
    );
    if stream.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 stream 参数" }));
    }
    if seek_time.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 seek_time 参数" }));
    }

    if let Some(ref pm) = state.playback_manager {
        pm.update_current_time(&stream, seek_time.clone()).await;
    }
    let cmd = crate::sip::PlaybackControlCmd::Seek {
        seek_time: seek_time.clone(),
    };
    match dispatch_playback_control(&state, &stream, cmd).await {
        Ok((device_id, channel_id)) => {
            let mut body =
                playback_control_ok(&stream, &device_id, &channel_id, "回放跳转成功");
            if let Some(obj) = body.0.as_object_mut() {
                obj.insert(
                    "currentTime".to_string(),
                    serde_json::Value::String(seek_time),
                );
            }
            body
        }
        Err(msg) => Json(serde_json::json!({ "code": 1, "msg": msg })),
    }
}

/// 纯函数：解析回放倍速，要求为正数。
fn parse_playback_speed(raw: &str) -> Option<f64> {
    match raw.trim().parse::<f64>() {
        Ok(v) if v > 0.0 && v.is_finite() => Some(v),
        _ => None,
    }
}

/// GET /api/common/channel/playback/speed
#[derive(Debug, Deserialize)]
pub struct ChannelPlaybackSpeedQ {
    pub channel_id: Option<i64>,
    pub stream: Option<String>,
    pub speed: Option<String>,
}

pub async fn channel_playback_speed(
    State(state): State<AppState>,
    Query(q): Query<ChannelPlaybackSpeedQ>,
) -> Json<serde_json::Value> {
    let stream = q.stream.clone().unwrap_or_default();
    let speed_raw = q.speed.clone().unwrap_or_default();
    tracing::info!(
        "commonChannel playback speed: channel_id={}, stream={}, speed={}",
        q.channel_id.unwrap_or(0),
        stream,
        speed_raw
    );
    if stream.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 stream 参数" }));
    }
    let speed: f64 = match parse_playback_speed(&speed_raw) {
        Some(v) => v,
        None => {
            return Json(serde_json::json!({
                "code": 1,
                "msg": format!("无效的 speed 参数: '{}'（应为正数，如 0.5 / 1 / 2 / 4）", speed_raw)
            }))
        }
    };

    if let Some(ref pm) = state.playback_manager {
        pm.update_speed(&stream, speed).await;
    }
    let cmd = crate::sip::PlaybackControlCmd::Scale { speed };
    match dispatch_playback_control(&state, &stream, cmd).await {
        Ok((device_id, channel_id)) => {
            let mut body =
                playback_control_ok(&stream, &device_id, &channel_id, "回放倍速设置成功");
            if let Some(obj) = body.0.as_object_mut() {
                obj.insert("speed".to_string(), serde_json::json!(speed));
            }
            body
        }
        Err(msg) => Json(serde_json::json!({ "code": 1, "msg": msg })),
    }
}

/// DELETE /api/common/channel/delete?id=<i64>
/// 单条通用通道删除（与 device_id 解耦，仅按内部 id 删）
pub async fn channel_delete(
    State(state): State<AppState>,
    Query(q): Query<ChannelDeleteQ>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = q.id.unwrap_or(0);
    if id <= 0 {
        return Err(AppError::business(ErrorCode::Error400, "缺少通道 id"));
    }
    let n = delete_channel_by_id(&state.pool, id).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error400, "通道不存在"));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize)]
pub struct ChannelDeleteQ {
    pub id: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============ 回放控制参数解析（2026-09-11 真实实现后补测） ============

    #[test]
    fn test_parse_playback_stream_id_standard_form() {
        // 形如 playback_{deviceId}_{channelId}_{ts}
        let got = parse_playback_stream_id("playback_34020000001320000001_34020000001310000001_1700000000");
        assert_eq!(
            got,
            Some((
                "34020000001320000001".to_string(),
                "34020000001310000001".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_playback_stream_id_without_timestamp() {
        // 只有 3 段也应可解析（部分客户端不带时间戳）
        let got = parse_playback_stream_id("playback_34020000001320000001_34020000001310000001");
        assert_eq!(
            got,
            Some((
                "34020000001320000001".to_string(),
                "34020000001310000001".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_playback_stream_id_rejects_malformed() {
        assert_eq!(parse_playback_stream_id(""), None);
        assert_eq!(parse_playback_stream_id("juststream"), None);
        assert_eq!(parse_playback_stream_id("a_b"), None);
        // 空段必须被拒绝，否则会下发到空 deviceId 的 SIP 消息
        assert_eq!(parse_playback_stream_id("playback__34020000001310000001"), None);
        assert_eq!(parse_playback_stream_id("playback_34020000001320000001_"), None);
    }

    #[test]
    fn test_parse_playback_speed_accepts_valid() {
        assert_eq!(parse_playback_speed("1"), Some(1.0));
        assert_eq!(parse_playback_speed("0.5"), Some(0.5));
        assert_eq!(parse_playback_speed(" 4 "), Some(4.0));
        assert_eq!(parse_playback_speed("2.5"), Some(2.5));
    }

    // ============ 通用前端指令映射（/api/front-end/common/:cmd/:ch） ============

    #[test]
    fn test_front_end_command_body_covers_every_supported_command() {
        for cmd in SUPPORTED_FRONT_END_COMMANDS {
            let got = front_end_command_body(cmd);
            let (ty, body) = got.unwrap_or_else(|| panic!("{} 在受支持列表里却无法映射", cmd));
            assert_eq!(ty, "DeviceControl", "{} 的控制类型应为 DeviceControl", cmd);
            assert!(
                body.starts_with('<') && body.ends_with('>'),
                "{} 的 XML 体异常: {}",
                cmd,
                body
            );
        }
    }

    #[test]
    fn test_front_end_command_body_spot_checks() {
        assert!(front_end_command_body("UP").unwrap().1.contains("PTZCmd"));
        assert!(front_end_command_body("STOP").unwrap().1.contains("PTZCmd"));
        assert_eq!(
            front_end_command_body("WIPER_ON").unwrap().1,
            "<WiperCmd>Open</WiperCmd>"
        );
        assert_eq!(
            front_end_command_body("WIPER_OFF").unwrap().1,
            "<WiperCmd>Close</WiperCmd>"
        );
        assert_eq!(
            front_end_command_body("GUARD_SET").unwrap().1,
            "<GuardCmd>SetGuard</GuardCmd>"
        );
        assert_eq!(
            front_end_command_body("GUARD_RESET").unwrap().1,
            "<GuardCmd>ResetGuard</GuardCmd>"
        );
    }

    #[test]
    fn test_front_end_command_body_is_case_insensitive_via_caller() {
        // 调用方会先 to_ascii_uppercase；小写直达应返回 None，保证契约清晰
        assert!(front_end_command_body("up").is_none());
        assert!(front_end_command_body("NOT_A_COMMAND").is_none());
    }

    #[test]
    fn test_parse_playback_speed_rejects_invalid() {
        // 负数 / 0 / 非数字 / 空 / 非有限值都必须拒绝 —— 否则会下发非法 Scale 指令
        assert_eq!(parse_playback_speed("0"), None);
        assert_eq!(parse_playback_speed("-1"), None);
        assert_eq!(parse_playback_speed("abc"), None);
        assert_eq!(parse_playback_speed(""), None);
        assert_eq!(parse_playback_speed("inf"), None);
        assert_eq!(parse_playback_speed("NaN"), None);
    }
}
