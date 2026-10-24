//! 中亿视图（SY）对接契约的**剩余别名端点**。
//!
//! `/api/sy/*` 是第三方地图视图的定制对接面。本文件补齐 `docs/OPEN_ISSUES.md`
//! A1 中登记但尚未实现的 5 条端点；每条都**转调已有实现**，不重写业务逻辑：
//!
//! | SY 端点 | 复用实现 |
//! |---|---|
//! | `GET /api/sy/camera/one` | `common_channel::channel_one` |
//! | `GET /api/sy/camera/update` | `common_channel::channel_update`（同一份 DTO 与落库逻辑） |
//! | `GET /api/sy/push/play` | `play::play_start`（先做平台侧前置校验） |
//! | `GET /api/sy/push/play-without-check` | `play::play_start`（跳过平台侧前置校验） |
//! | `GET /api/sy/record/collect/add` | `stub::cloud_record_collect_add` |
//! | `GET /api/sy/record/collect/delete` | `cloud_record_extra::collect_delete` |
//! | `GET /api/sy/record/list-url` | `cloud_record_extra::list_url` |
//! | `GET /api/sy/record/zip` | `cloud_record_extra::zip` |
//! | `GET /api/sy/forceClose` | `stream::push_force_close` |
//! | `GET /api/sy/test` | 平台自检：DB / 设备 / 媒体节点 / SIP 就绪度 |
//!
//! 这些端点全部挂在**受保护**路由表（`src/openapi/routes_sy.rs`），与既有
//! `/api/sy/*` 一致：未登录返回 401，而不是 404。

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::AppError;
use crate::handlers::common_channel::{ChannelIdQuery, ChannelUpdateBody};
use crate::response::ApiResult;
use crate::AppState;

/// SY 推送播放的查询参数。
///
/// 与 `/api/sy/camera/control/*` 一致使用**国标 ID**（`deviceId` + `channelId`），
/// 而不是平台内部的通道主键。
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
pub struct PushPlayQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
}

/// SY 自检结果。
///
/// 只回报**就绪度**（数量、布尔、版本），不含节点地址、密钥等部署细节，
/// 避免对接方把诊断接口当成配置读取口。
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SyTestReport {
    /// 平台版本号。
    pub version: String,
    /// 整体是否就绪：DB 可读且 SIP 已初始化且至少有一个可用媒体节点。
    pub ok: bool,
    /// 数据库连通性。
    pub database: bool,
    /// SIP 服务是否已初始化。
    pub sip: bool,
    /// 已登记的 GB28181 设备数。
    pub device_count: i64,
    /// 在线设备数。
    pub online_device_count: i64,
    /// 已登记的通道数。
    pub channel_count: i64,
    /// 媒体节点总数。
    pub media_server_count: i64,
    /// 已启用且在线的媒体节点数。
    pub media_server_online_count: i64,
    /// 自检过程中的问题列表（成功时为空）。
    pub issues: Vec<String>,
}

/// GET /api/sy/camera/one?id=
///
/// 单通道详情：`/api/common/channel/one` 的 SY 前缀别名。
///
/// `id` / `channelId` / `channel_id` 三个名字都接受（复用平台既有 DTO）。
#[utoipa::path(
    get,
    path = "/api/sy/camera/one",
    tag = "channel",
    operation_id = "sy_camera_one",
    summary = "中亿视图：单通道详情（转调 common_channel/one）",
    params(SyChannelIdQuery),
    responses(
        (status = 200, description = "单个通道详情；id 缺失或为 0 时 data=null",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"id":1,"deviceId":"34020000001320000001","channelId":"34020000001310000001","name":"前门"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn camera_one(
    State(state): State<AppState>,
    Query(q): Query<SyChannelIdQuery>,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    let pk = resolve_channel_pk(&state, q.id.as_deref()).await?;
    crate::handlers::common_channel::channel_one(
        State(state),
        Query(ChannelIdQuery {
            channel_id: Some(pk),
        }),
    )
    .await
}

/// SY 侧的通道标识：**数字主键**与 **20 位国标通道号**都接受。
///
/// 平台版的 `ChannelIdQuery.id` 是 `i64`，20 位国标号装不下 —— 传进去会
/// 被 axum 的 QueryRejection 拒掉，返回**裸 HTTP 400 文本**（不是
/// `{code,msg,data}` 信封），对接方拿到的不是契约里约定的格式。
/// 而同一 SY 模块的 `push/play` 用的恰恰是国标 `deviceId`/`channelId`，
/// 两套 ID 口径并存很容易踩坑，所以这里统一成"两种都收"。
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
pub struct SyChannelIdQuery {
    /// 通道主键（数字）或国标通道号，都接受。
    #[serde(alias = "channelId", alias = "channel_id", alias = "id")]
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub id: Option<String>,
}

/// 把"数字主键或国标通道号"解析成通道主键。
async fn resolve_channel_pk(state: &AppState, raw: Option<&str>) -> Result<i64, AppError> {
    let raw = raw.map(str::trim).unwrap_or_default();
    if raw.is_empty() {
        // 平台版在 id 缺失时回 data=null，这里保持一致。
        return Ok(0);
    }
    if let Ok(pk) = raw.parse::<i64>() {
        return Ok(pk);
    }
    match crate::db::record_plan::channel_id_by_gb_id(&state.pool, raw).await {
        Ok(Some(pk)) => Ok(pk),
        Ok(None) => Ok(0),
        Err(e) => Err(AppError::business(
            crate::error::ErrorCode::Error500,
            format!("解析通道号失败: {e}"),
        )),
    }
}

/// GET /api/sy/camera/update?id=&name=&...
///
/// 更新通道：`/api/common/channel/update` 的 SY 前缀别名。
///
/// SY 契约把更新也放在 GET 上（与本模块其它端点一致），因此这里用**查询参数**
/// 承载同一份 `ChannelUpdateBody`，落库仍走 `common_channel::update`，
/// 字段语义与 POST 版本完全一致：未传的字段保持原值。
#[utoipa::path(
    get,
    path = "/api/sy/camera/update",
    tag = "channel",
    operation_id = "sy_camera_update",
    summary = "中亿视图：更新通道（转调 common_channel/update）",
    params(SyChannelUpdateQuery),
    responses(
        (status = 200, description = "更新成功",
         body = ApiResult<utoipa::TupleUnit>),
        (status = 400, description = "缺少 id"),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn camera_update(
    State(state): State<AppState>,
    Query(q): Query<SyChannelUpdateQuery>,
) -> Result<Json<ApiResult<()>>, AppError> {
    // id 可能是数字主键，也可能是国标通道号 —— 先解析成主键再走平台的更新逻辑。
    let id = match q.id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => {
            let pk = resolve_channel_pk(&state, Some(raw)).await?;
            if pk <= 0 {
                return Err(AppError::business(
                    crate::error::ErrorCode::Error400,
                    format!("通道不存在: {raw}"),
                ));
            }
            Some(pk)
        }
        None => None,
    };
    let body = ChannelUpdateBody {
        id,
        name: q.name,
        channel_id: q.channel_id,
        civil_code: q.civil_code,
        parent_id: q.parent_id,
        business_group: q.business_group,
        ptz_type: q.ptz_type,
        custom_name: q.custom_name,
        manufacturer: q.manufacturer,
        model: q.model,
        owner: q.owner,
        address: q.address,
        stream_identification: q.stream_identification,
        channel_type: q.channel_type,
    };
    crate::handlers::common_channel::channel_update(State(state), Json(body)).await
}

/// SY 更新通道的查询参数。
///
/// 与 `ChannelUpdateBody` 字段一一对应，只是把 `id` 收进来（`ChannelUpdateBody`
/// 本身的 `id` 就是必填项，语义一致）。
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
pub struct SyChannelUpdateQuery {
    /// 通道主键（数字）或国标通道号，都接受（必填）。
    #[serde(alias = "id")]
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub id: Option<String>,
    /// 通道名称。
    pub name: Option<String>,
    /// 通道国标 ID。
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    /// 行政区划编码。
    #[serde(alias = "civilCode")]
    pub civil_code: Option<String>,
    /// 父通道标识（国标父通道编号，**字符串**）。
    ///
    /// `gb_device_channel.parent_id` 是 VARCHAR 列，不是整型。
    #[serde(alias = "parentId")]
    pub parent_id: Option<String>,
    /// 业务分组。
    #[serde(alias = "businessGroup")]
    pub business_group: Option<String>,
    /// 云台类型。
    #[serde(alias = "ptzType")]
    pub ptz_type: Option<i32>,
    /// 自定义名称。
    #[serde(alias = "customName")]
    pub custom_name: Option<String>,
    /// 厂商。
    pub manufacturer: Option<String>,
    /// 型号。
    pub model: Option<String>,
    /// 负责人。
    pub owner: Option<String>,
    /// 地址。
    pub address: Option<String>,
    /// 码流标识。
    #[serde(alias = "streamIdentification")]
    pub stream_identification: Option<String>,
    /// 通道类型。
    #[serde(alias = "channelType")]
    pub channel_type: Option<i32>,
}

/// GET /api/sy/push/play?deviceId=&channelId=
///
/// 推送播放（**带平台侧前置校验**）。
///
/// 先确认目标设备已登记、通道属于该设备、设备在线，不满足就直接返回业务错误。
///
/// 与 `/api/sy/push/play-without-check` 的**实际**区别要说准：
/// 点播链路（`play::play_start`）自己也会做设备/通道/在线三项检查，而且都在
/// 发起 INVITE **之前** —— 所以两个端点**都不会**对着未知目标发 SIP。
/// 真正的差别只有两处：
///   1. **错误文案**：本端点用 SY 口径（"设备不存在: X" / "设备离线: X" /
///      "通道不存在或不属于该设备: X/Y"），另一个端点透出平台口径
///      （"Device not found" / "Device is offline" / "Channel not found"）；
///   2. **检查顺序**：本端点先看在线、再看通道归属；另一个端点是先看通道归属。
///
/// 校验通过后与平台其它点播入口走同一条链路（SIP INVITE + ZLM 收流）。
#[utoipa::path(
    get,
    path = "/api/sy/push/play",
    tag = "live",
    operation_id = "sy_push_play",
    summary = "中亿视图：推送播放（含平台侧前置校验）",
    params(PushPlayQuery),
    responses(
        (status = 200, description = "点播结果",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"app":"rtp","stream":"...","playUrl":"rtsp://...","flvUrl":"http://.../live.flv"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn push_play(
    State(state): State<AppState>,
    Query(q): Query<PushPlayQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    if device_id.is_empty() || channel_id.is_empty() {
        return Json(ApiResult::error("deviceId and channelId required"));
    }

    // 前置校验：目标必须是平台已登记、且在线的国标通道。
    let device = match db::device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => d,
        Ok(None) => return Json(ApiResult::error(format!("设备不存在: {device_id}"))),
        Err(e) => {
            tracing::error!("sy/push/play 查询设备失败: {e}");
            return Json(ApiResult::error("数据库错误"));
        }
    };
    if !device.on_line.unwrap_or(false) {
        return Json(ApiResult::error(format!("设备离线: {device_id}")));
    }
    match db::device::get_channel_by_device_and_channel_id(&state.pool, &device_id, &channel_id)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Json(ApiResult::error(format!(
                "通道不存在或不属于该设备: {device_id}/{channel_id}"
            )))
        }
        Err(e) => {
            tracing::error!("sy/push/play 查询通道失败: {e}");
            return Json(ApiResult::error("数据库错误"));
        }
    }

    tracing::info!("sy/push/play → play_start {device_id}/{channel_id}（含校验）");
    crate::handlers::play::play_start(State(state), axum::extract::Path((device_id, channel_id)))
        .await
}

/// GET /api/sy/push/play-without-check?deviceId=&channelId=
///
/// 推送播放（**不做平台侧前置校验**）。
///
/// 直接进入点播链路，由点播链路自身做设备/通道/在线检查并回报失败
/// （错误文案是平台口径）。省掉了 SY 侧那一遍重复的库查询。
///
/// 注意：点播链路的检查同样在发起 INVITE **之前**，所以这个端点也**不会**
/// 对着未知目标发 SIP —— 与 `push/play` 的差别只在错误文案与检查顺序。
#[utoipa::path(
    get,
    path = "/api/sy/push/play-without-check",
    tag = "live",
    operation_id = "sy_push_play_without_check",
    summary = "中亿视图：推送播放（跳过平台侧前置校验）",
    params(PushPlayQuery),
    responses(
        (status = 200, description = "点播结果",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"app":"rtp","stream":"...","playUrl":"rtsp://...","flvUrl":"http://.../live.flv"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn push_play_without_check(
    State(state): State<AppState>,
    Query(q): Query<PushPlayQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    if device_id.is_empty() || channel_id.is_empty() {
        return Json(ApiResult::error("deviceId and channelId required"));
    }
    tracing::info!("sy/push/play-without-check → play_start {device_id}/{channel_id}（跳过校验）");
    crate::handlers::play::play_start(State(state), axum::extract::Path((device_id, channel_id)))
        .await
}

/// GET /api/sy/test
///
/// 中亿视图对接自检：回报平台是否具备对接条件。
///
/// 只回报就绪度（计数、布尔、版本），**不**回报节点地址、密钥或完整 Hook 载荷；
/// 失败原因固定文案，便于对接方按码判断。
#[utoipa::path(
    get,
    path = "/api/sy/test",
    tag = "system",
    operation_id = "sy_test",
    summary = "中亿视图：对接自检",
    responses(
        (status = 200, description = "自检结果",
         body = ApiResult<SyTestReport>,
         example = json!({"code":0,"msg":"成功","data":{"version":"2.0.0","ok":true,"database":true,"sip":true,"device_count":1,"online_device_count":1,"channel_count":2,"media_server_count":1,"media_server_online_count":1,"issues":[]}})),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "非管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn sy_test(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResult<SyTestReport>>, AppError> {
    // 这是**新增的诊断端点**（不是别名），会回报部署拓扑（设备/通道/媒体节点
    // 数量）。按仓库规约（AGENTS.md「新增任何管理类端点都必须调用
    // authz::require_admin」）与平台同类诊断口（`/api/test/hook/list`、
    // `/api/test/redis`）保持一致 —— 否则普通登录用户也能读到这些数字。
    crate::handlers::authz::require_admin(&state, &headers).await?;

    let mut issues: Vec<String> = Vec::new();

    // 计数**不能吞错**：某个查询失败时若还报 0，报告里就会出现
    // `{"database":true,"device_count":0,"ok":true}` 这种自相矛盾的结论，
    // 对接方据此判定"平台就绪"就错了。失败一律记进 issues 并让 ok=false。
    let device_count = match db::device::count_devices(&state.pool, None, None).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!("自检：统计设备数失败: {e}");
            issues.push("设备数统计失败".to_string());
            0
        }
    };
    let online_device_count = match db::device::count_devices(&state.pool, None, Some(true)).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!("自检：统计在线设备数失败: {e}");
            issues.push("在线设备数统计失败".to_string());
            0
        }
    };
    let channel_count = match db::device::count_all_channels(&state.pool).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!("自检：统计通道数失败: {e}");
            issues.push("通道数统计失败".to_string());
            0
        }
    };
    let media_server_count = match db::media_server::count_all(&state.pool).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!("自检：统计媒体节点数失败: {e}");
            issues.push("媒体节点数统计失败".to_string());
            0
        }
    };
    let media_server_online_count = match db::media_server::list_online_servers(&state.pool).await {
        Ok(v) => v.len() as i64,
        Err(e) => {
            tracing::warn!("自检：统计在线媒体节点数失败: {e}");
            issues.push("在线媒体节点数统计失败".to_string());
            0
        }
    };

    // 数据库连通性：上面的通道数查询已经探过库了。
    let database = !issues.iter().any(|i| i.contains("统计失败"));
    if !database {
        issues.push("数据库不可读".to_string());
    }

    let sip = state.sip_server.is_some();
    if !sip {
        issues.push("SIP 服务未初始化".to_string());
    }

    if media_server_count == 0 {
        issues.push("未配置媒体节点".to_string());
    } else if media_server_online_count == 0 {
        issues.push("没有在线的媒体节点".to_string());
    }

    let ok = database && sip && media_server_online_count > 0;

    Ok(Json(ApiResult::success(SyTestReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        ok,
        database,
        sip,
        device_count,
        online_device_count,
        channel_count,
        media_server_count,
        media_server_online_count,
        issues,
    })))
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::db::Pool;

    const DEVICE_ID: &str = "34020000001320000991";
    const CHANNEL_ID: &str = "34020000001310000991";

    /// 插入一台在线设备 + 一条属于它的通道。
    async fn seed_device_and_channel(pool: &Pool, on_line: bool) -> i64 {
        sqlx::query(
            "INSERT INTO gb_device (device_id, name, on_line, create_time, update_time) \
             VALUES (?, ?, ?, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .bind(DEVICE_ID)
        .bind("SY 契约测试设备")
        .bind(if on_line { 1 } else { 0 })
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO gb_device_channel (device_id, name, gb_device_id, status, has_audio, \
             create_time, update_time, data_type, data_device_id, channel_type) \
             VALUES (?, ?, ?, 'ON', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00', 0, 0, 0)",
        )
        .bind(DEVICE_ID)
        .bind("SY 契约测试通道")
        .bind(CHANNEL_ID)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query_scalar::<_, i64>("SELECT id FROM gb_device_channel ORDER BY id DESC LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn query(pairs: &[(&str, &str)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// `/api/sy/camera/one` 必须与 `/api/common/channel/one` 返回同一条通道。
    #[tokio::test]
    async fn camera_one_returns_same_channel_as_common_alias() {
        let state = crate::test_support::app_state().await;
        let channel_pk = seed_device_and_channel(&state.pool, true).await;

        let sy = super::camera_one(
            State(state.clone()),
            Query(SyChannelIdQuery {
                id: Some(channel_pk.to_string()),
            }),
        )
        .await
        .unwrap()
        .0;
        // 平台实现用的是自己的 DTO（数字主键）。
        let common = crate::handlers::common_channel::channel_one(
            State(state),
            Query(ChannelIdQuery {
                channel_id: Some(channel_pk),
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(sy.code, 0, "{sy:?}");
        assert_eq!(sy.data, common.data, "SY 别名与平台实现返回不一致");
        assert_eq!(
            sy.data.as_ref().unwrap()["deviceId"],
            serde_json::json!(DEVICE_ID)
        );
        assert_eq!(
            sy.data.as_ref().unwrap()["channelId"],
            serde_json::json!(CHANNEL_ID)
        );
    }

    /// `/api/sy/camera/one` 缺 id 时返回 data=null，而不是报错。
    #[tokio::test]
    async fn camera_one_without_id_returns_null_data() {
        let state = crate::test_support::app_state().await;
        let out = super::camera_one(State(state), Query(SyChannelIdQuery { id: None }))
            .await
            .unwrap()
            .0;
        assert_eq!(out.code, 0, "{out:?}");
        // 信封里的 data 是 JSON null（而不是缺失字段）。
        assert_eq!(out.data, Some(serde_json::Value::Null), "{out:?}");
        let serialized = serde_json::to_string(&out).unwrap();
        assert!(serialized.contains("\"data\":null"), "{serialized}");
    }

    /// `/api/sy/camera/update` 走 GET 查询参数，但落库效果与 POST 版本一致。
    #[tokio::test]
    async fn camera_update_applies_query_parameters() {
        let state = crate::test_support::app_state().await;
        let channel_pk = seed_device_and_channel(&state.pool, true).await;

        let out = super::camera_update(
            State(state.clone()),
            Query(SyChannelUpdateQuery {
                id: Some(channel_pk.to_string()),
                name: Some("SY 改名后的通道".to_string()),
                owner: Some("SY 对接方".to_string()),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(out.code, 0, "{out:?}");

        let saved: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT name, owner FROM gb_device_channel WHERE id = ?")
                .bind(channel_pk)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(saved.0.as_deref(), Some("SY 改名后的通道"));
        assert_eq!(saved.1.as_deref(), Some("SY 对接方"));
    }

    /// `/api/sy/camera/update` 缺 id 必须报错，且不落库。
    #[tokio::test]
    async fn camera_update_without_id_is_rejected() {
        let state = crate::test_support::app_state().await;
        let channel_pk = seed_device_and_channel(&state.pool, true).await;

        let err = super::camera_update(
            State(state.clone()),
            Query(SyChannelUpdateQuery {
                id: None,
                name: Some("不应写入".to_string()),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("id"), "{err}");

        let saved: Option<String> =
            sqlx::query_scalar("SELECT name FROM gb_device_channel WHERE id = ?")
                .bind(channel_pk)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(saved.as_deref(), Some("SY 契约测试通道"));
    }

    /// `push/play`（含校验）对着**未登记**的设备直接返回业务错误，不发起点播。
    #[tokio::test]
    async fn push_play_rejects_unknown_device_without_playing() {
        let state = crate::test_support::app_state().await;
        let out = super::push_play(
            State(state),
            Query(PushPlayQuery {
                device_id: Some("34020000001320000000".to_string()),
                channel_id: Some(CHANNEL_ID.to_string()),
            }),
        )
        .await
        .0;
        assert_ne!(out.code, 0, "{out:?}");
        assert!(out.msg.contains("设备不存在"), "{out:?}");
    }

    /// `push/play`（含校验）对着**离线**设备返回业务错误。
    #[tokio::test]
    async fn push_play_rejects_offline_device() {
        let state = crate::test_support::app_state().await;
        seed_device_and_channel(&state.pool, false).await;
        let out = super::push_play(
            State(state),
            Query(PushPlayQuery {
                device_id: Some(DEVICE_ID.to_string()),
                channel_id: Some(CHANNEL_ID.to_string()),
            }),
        )
        .await
        .0;
        assert_ne!(out.code, 0, "{out:?}");
        assert!(out.msg.contains("设备离线"), "{out:?}");
    }

    /// 通道不属于该设备时，含校验版本必须拦下。
    #[tokio::test]
    async fn push_play_rejects_channel_of_another_device() {
        let state = crate::test_support::app_state().await;
        seed_device_and_channel(&state.pool, true).await;
        let out = super::push_play(
            State(state),
            Query(PushPlayQuery {
                device_id: Some(DEVICE_ID.to_string()),
                channel_id: Some("34020000001310000000".to_string()),
            }),
        )
        .await
        .0;
        assert_ne!(out.code, 0, "{out:?}");
        assert!(out.msg.contains("通道不存在"), "{out:?}");
    }

    /// 缺 deviceId / channelId 时两个推流播放端点都必须拒绝。
    #[tokio::test]
    async fn push_play_requires_device_and_channel_id() {
        let state = crate::test_support::app_state().await;
        for handler_is_checked in [true, false] {
            let q = PushPlayQuery {
                device_id: None,
                channel_id: Some(CHANNEL_ID.to_string()),
            };
            let out = if handler_is_checked {
                super::push_play(State(state.clone()), Query(q)).await.0
            } else {
                super::push_play_without_check(State(state.clone()), Query(q))
                    .await
                    .0
            };
            assert_ne!(out.code, 0, "{out:?}");
            assert!(out.msg.contains("deviceId"), "{out:?}");
        }
    }

    /// 自检报告必须给出可判定的字段，且不泄露部署细节。
    #[tokio::test]
    async fn sy_test_reports_readiness_without_secrets() {
        let state = crate::test_support::app_state().await;
        seed_device_and_channel(&state.pool, true).await;

        // 自检是管理类端点，必须带管理员身份。
        let token = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes())
            .create_token("admin", 60)
            .unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("access-token", token.parse().unwrap());
        let out = super::sy_test(State(state), headers).await.unwrap().0;
        assert_eq!(out.code, 0, "{out:?}");
        let report = out.data.unwrap();
        assert_eq!(report.version, env!("CARGO_PKG_VERSION"));
        assert!(report.database, "数据库应可读");
        assert_eq!(report.device_count, 1, "{report:?}");
        assert_eq!(report.channel_count, 1, "{report:?}");
        // 没有配置媒体节点时必须报出来，而不是谎报"就绪"。
        assert!(!report.ok, "{report:?}");
        assert!(
            report.issues.iter().any(|i| i.contains("媒体节点")),
            "{report:?}"
        );
    }

    /// 自检是管理类端点：非管理员必须被拒。
    ///
    /// 它会回报部署拓扑（设备/通道/媒体节点数量），口径要与平台同类诊断口
    /// （`/api/test/hook/list`、`/api/test/redis`）一致 —— 都要求管理员。
    #[tokio::test]
    async fn sy_test_requires_admin() {
        let state = crate::test_support::app_state().await;
        let token = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes())
            .create_token("nobody", 60)
            .unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("access-token", token.parse().unwrap());
        // 用户存在但不是管理员 → 403（而不是放行）。
        // 注意要同时建一个**非管理员角色**：`find_by_username` 是
        // `gb_user JOIN gb_user_role`，role_id 指向不存在的角色时整行查不出来，
        // 会走到 401 而不是 403 —— 那样就测不到"管理员粒度"这一层了。
        sqlx::query(
            "INSERT INTO gb_user_role (id, name, authority, create_time, update_time) \
             VALUES (2, 'operator', '9', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO gb_user (username, password, role_id, push_key, create_time, update_time) \
             VALUES ('nobody', 'x', 2, 'k', '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        let err = super::sy_test(State(state), headers).await.unwrap_err();
        assert!(
            format!("{err:?}").contains("403") || format!("{err:?}").contains("权限"),
            "非管理员不应被放行: {err:?}"
        );
    }

    /// `camera/one` 要能按**国标通道号**查到（20 位数字）。
    ///
    /// 平台版只收数字主键，20 位国标号会 i64 溢出 → axum 直接回裸 HTTP 400
    /// 文本（不是 `{code,msg,data}` 信封）；而同一 SY 模块的 `push/play` 用的
    /// 就是国标号，两套口径并存会让对接方踩坑。
    #[tokio::test]
    async fn camera_one_accepts_gb_channel_id() {
        let state = crate::test_support::app_state().await;
        seed_device_and_channel(&state.pool, true).await;

        let by_pk = super::camera_one(
            State(state.clone()),
            Query(SyChannelIdQuery {
                id: Some("1".to_string()),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(by_pk.code, 0, "{by_pk:?}");

        let by_gb = super::camera_one(
            State(state.clone()),
            Query(SyChannelIdQuery {
                id: Some(CHANNEL_ID.to_string()),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(by_gb.code, 0, "{by_gb:?}");
        let data = by_gb.data.unwrap();
        assert_eq!(data["channelId"], serde_json::json!(CHANNEL_ID), "{data}");
        assert_eq!(data["deviceId"], serde_json::json!(DEVICE_ID), "{data}");
    }

    /// 查不到的通道号返回 `data=null`（而不是报错）。
    #[tokio::test]
    async fn camera_one_with_unknown_id_returns_null_data() {
        let state = crate::test_support::app_state().await;
        let out = super::camera_one(
            State(state),
            Query(SyChannelIdQuery {
                id: Some("34020000001319999997".to_string()),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(out.code, 0, "{out:?}");
        // 与平台版一致：查不到时 data 是 JSON null（不是缺字段）。
        assert_eq!(out.data, Some(serde_json::Value::Null), "{out:?}");
    }

    /// 查询串拼装辅助（保持测试可读）。
    #[test]
    fn query_builder_joins_pairs() {
        assert_eq!(query(&[("a", "1"), ("b", "2")]), "a=1&b=2");
    }
}

/// GET /api/sy/record/collect/add?recordId=
///
/// 录像收藏：`/api/cloud/record/collect/add` 的 SY 前缀别名。
#[utoipa::path(
    get,
    path = "/api/sy/record/collect/add",
    tag = "cloud",
    operation_id = "sy_record_collect_add",
    summary = "中亿视图：收藏录像（转调 cloud/record/collect/add）",
    params(crate::handlers::stub::CloudRecordCollectQuery),
    responses(
        (status = 200, description = "收藏结果",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"status":"collected"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn record_collect_add(
    State(state): State<AppState>,
    Query(q): Query<crate::handlers::stub::CloudRecordCollectQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    crate::handlers::stub::cloud_record_collect_add(State(state), Query(q)).await
}

/// 取消收藏的入参：数字主键与组合串 `recordId` 都接受。
///
/// 平台的 DELETE 版走 `CloudRecordCollectQuery`（string），GET 版走
/// `CollectQuery`（i64）—— 两侧口径不一致，对接方很容易照着另一边写然后
/// 收到"missing id"。SY 别名统一成**字符串**：两种 id 都收。
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
pub struct CollectDeleteQuery {
    /// 云端录像主键（数字）或组合串 recordId，都接受。
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub id: Option<String>,
}

/// GET /api/sy/record/collect/delete?id=
///
/// 取消收藏：`/api/cloud/record/collect/delete` 的 SY 前缀别名。
///
/// 与平台版本一致：GET 与 DELETE 两种方法都注册，避免只支持一种方法的对接方
/// 收到 405。两个方法都走同一套 `clear_collect` 逻辑，返回同一形状
/// `{recordId, status}`。
#[utoipa::path(
    get,
    path = "/api/sy/record/collect/delete",
    tag = "cloud",
    operation_id = "sy_record_collect_delete",
    summary = "中亿视图：取消收藏（转调 cloud/record/collect/delete）",
    params(CollectDeleteQuery),
    responses(
        (status = 200, description = "取消收藏结果",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"recordId":"zlm-1::rtp::ch::a.mp4","status":"deleted"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn record_collect_delete(
    State(state): State<AppState>,
    Query(q): Query<CollectDeleteQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let raw_id = q.id.unwrap_or_default();
    if raw_id.is_empty() {
        return Json(ApiResult::error("record_id is required"));
    }
    match crate::handlers::cloud_record_extra::clear_collect(&state, &raw_id).await {
        Ok(record_id) => Json(ApiResult::success(serde_json::json!({
            "recordId": record_id,
            "status": "deleted",
        }))),
        Err(e) => Json(ApiResult::error(e)),
    }
}

/// DELETE /api/sy/record/collect/delete?id=
///
/// 与 GET 版本同一条逻辑：平台侧的收藏删除同时支持 GET 与 DELETE，
/// SY 别名也保持一致。
#[utoipa::path(
    delete,
    path = "/api/sy/record/collect/delete",
    tag = "cloud",
    operation_id = "sy_record_collect_delete_method",
    summary = "中亿视图：取消收藏（DELETE 方法，转调 cloud/record/collect/delete）",
    params(crate::handlers::cloud_record_extra::CollectQuery),
    responses(
        (status = 200, description = "取消收藏结果",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"status":"removed"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn record_collect_delete_method(
    State(state): State<AppState>,
    Json(body): Json<crate::handlers::stub::CloudRecordCollectQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    // 入参与平台 DELETE 版**完全一致**（JSON body），返回同一形状。
    // 此前这里转调的是 GET 版实现（只认 query 的数字主键），对接方照平台
    // DELETE 的写法发 JSON body 会被 axum 忽略 → "missing id"，收藏根本没删掉。
    let raw_id = body
        .id
        .or(body.record_id)
        .or(body.cloud_record_id)
        .unwrap_or_default();
    if raw_id.is_empty() {
        return Json(ApiResult::error("record_id is required"));
    }
    match crate::handlers::cloud_record_extra::clear_collect(&state, &raw_id).await {
        Ok(record_id) => Json(ApiResult::success(serde_json::json!({
            "recordId": record_id,
            "status": "deleted",
        }))),
        Err(e) => Json(ApiResult::error(e)),
    }
}

/// GET /api/sy/record/list-url
///
/// 录像列表（仅 URL 形态）：`/api/cloud/record/list-url` 的 SY 前缀别名。
#[utoipa::path(
    get,
    path = "/api/sy/record/list-url",
    tag = "cloud",
    operation_id = "sy_record_list_url",
    summary = "中亿视图：录像列表（仅 URL 形态，转调 cloud/record/list-url）",
    params(crate::handlers::cloud_record_extra::ListUrlQuery),
    responses(
        (status = 200, description = "录像列表",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"total":1,"list":[]}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn record_list_url(
    State(state): State<AppState>,
    Query(q): Query<crate::handlers::cloud_record_extra::ListUrlQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    crate::handlers::cloud_record_extra::list_url(State(state), Query(q)).await
}

/// GET /api/sy/record/zip
///
/// 录像打包下载（ZIP）：`/api/cloud/record/zip` 的 SY 前缀别名。
#[utoipa::path(
    get,
    path = "/api/sy/record/zip",
    tag = "cloud",
    operation_id = "sy_record_zip",
    summary = "中亿视图：录像打包下载（转调 cloud/record/zip）",
    params(crate::handlers::cloud_record_extra::ZipQuery),
    responses(
        (status = 200, description = "ZIP 下载",
         body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn record_zip(
    State(state): State<AppState>,
    Query(q): Query<crate::handlers::cloud_record_extra::ZipQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    crate::handlers::cloud_record_extra::zip(State(state), Query(q)).await
}

/// GET /api/sy/forceClose?id=
///
/// 强制关闭流：`/api/push/forceClose` 的 SY 前缀别名。
#[utoipa::path(
    get,
    path = "/api/sy/forceClose",
    tag = "stream",
    operation_id = "sy_force_close",
    summary = "中亿视图：强制关闭流（转调 push/forceClose）",
    params(crate::handlers::stream::PushForceCloseQuery),
    responses(
        (status = 200, description = "已关闭",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"id":1,"stream":"live1","closed":true}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn force_close(
    State(state): State<AppState>,
    Query(q): Query<crate::handlers::stream::PushForceCloseQuery>,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    crate::handlers::stream::push_force_close(State(state), Query(q)).await
}
