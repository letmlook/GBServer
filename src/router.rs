use axum::{
    extract::State,
    middleware,
    routing::{delete, get, post},
    Json,
    Router,
};
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::auth_middleware;
use crate::handlers::{
    alarm, common_channel, device, device_control, device_stub, front_end, jt1078, platform, play,
    playback, server, stream, stub, talk, user, websocket, webrtc, device_batch,
};
use crate::zlm::hook as zlm_hook;
use crate::AppState;

async fn health_check(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db_status = match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => "ok",
        Err(_) => "error",
    };
    let sip_status = if state.sip_server.is_some() { "ok" } else { "disabled" };
    let zlm_status = if state.zlm_client.is_some() { "ok" } else { "disabled" };
    let redis_status = if state.redis.is_some() { "ok" } else { "disabled" };
    let all_ok = db_status == "ok";
    let status_code = if all_ok { 200 } else { 503 };
    axum::Json(serde_json::json!({
        "status": if all_ok { "healthy" } else { "unhealthy" },
        "code": status_code,
        "components": {
            "database": db_status,
            "sip": sip_status,
            "zlm": zlm_status,
            "redis": redis_status,
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

pub fn app(state: AppState) -> Router<AppState> {
    let state_clone = state.clone();
    let api_protected = Router::new()
        .route(
            "/api/user/userInfo",
            get(user::user_info).post(user::user_info),
        )
        .route("/api/user/users", get(user::users))
        .route("/api/user/add", post(user::add_user))
        .route("/api/user/delete", delete(user::delete_user))
        .route("/api/user/changePassword", post(user::change_password))
        .route(
            "/api/user/changePasswordForAdmin",
            post(user::change_password_for_admin),
        )
        .route("/api/user/changePushKey", post(user::change_push_key))
        .route("/api/device/query/devices", get(device::query_devices))
        .route(
            "/api/device/query/devices/:device_id/channels",
            get(device::query_channels),
        )
        .route(
            "/api/device/query/sync_status",
            get(device_stub::sync_status),
        )
        .route(
            "/api/device/query/devices/:device_id/delete",
            delete(device_stub::device_delete),
        )
        .route(
            "/api/device/query/devices/:device_id/sync",
            get(device_stub::device_sync),
        )
        .route(
            "/api/device/query/transport/:device_id/:stream_mode",
            post(device_stub::device_transport),
        )
        .route(
            "/api/device/control/guard",
            get(device_control::device_guard),
        )
        .route("/api/device/control/ptz", get(device_control::device_ptz))
        .route(
            "/api/device/control/preset",
            get(device_control::device_preset),
        )
        .route(
            "/api/device/control/reboot",
            get(device_control::device_reboot),
        )
        .route(
            "/api/device/config/query",
            get(device_control::device_config_query),
        )
        .route(
            "/api/device/config/update",
            post(device_control::device_config_update),
        )
        .route(
            "/api/device/query/subscribe/catalog",
            get(device_control::subscribe_catalog),
        )
        .route(
            "/api/device/query/subscribe/mobile-position",
            get(device_stub::subscribe_mobile_position),
        )
        .route(
            "/api/device/config/query/:device_id/BasicParam",
            get(device_stub::config_basic_param),
        )
        .route(
            "/api/device/query/channel/one",
            get(device_stub::channel_one),
        )
        .route("/api/device/query/streams", get(device_stub::query_streams))
        .route(
            "/api/device/control/record",
            get(device_stub::control_record),
        )
        .route(
            "/api/device/query/sub_channels/:device_id/:parent_channel_id/channels",
            get(device_stub::sub_channels),
        )
        .route(
            "/api/device/query/tree/channel/:device_id",
            get(device_stub::tree_channel),
        )
        .route(
            "/api/device/query/channel/audio",
            post(device_stub::channel_audio),
        )
        .route(
            "/api/device/query/channel/stream/identification/update/",
            post(device_stub::channel_stream_identification_update),
        )
        .route(
            "/api/device/query/device/update",
            post(device_stub::device_update),
        )
        .route(
            "/api/device/query/device/add",
            post(device_stub::device_add),
        )
        .route(
            "/api/device/query/devices/:device_id",
            get(device_stub::device_one),
        )
        .route(
            "/api/device/query/tree/:device_id",
            get(device_stub::device_tree),
        )
        .route("/api/common/channel/list", get(stub::common_channel_list))
        .route("/api/role/all", get(stub::role_all))
        .route(
            "/api/server/media_server/online/list",
            get(server::media_server_online_list),
        )
        .route(
            "/api/server/media_server/list",
            get(server::media_server_list),
        )
        .route(
            "/api/server/media_server/one/:id",
            get(server::media_server_one),
        )
        .route(
            "/api/server/media_server/check",
            get(server::media_server_check),
        )
        .route(
            "/api/server/media_server/record/check",
            get(server::media_server_record_check),
        )
        .route(
            "/api/server/media_server/save",
            post(server::media_server_save),
        )
        .route(
            "/api/server/media_server/delete",
            delete(server::media_server_delete),
        )
        .route(
            "/api/server/media_server/media_info",
            get(server::media_server_media_info),
        )
        .route(
            "/api/server/media_server/load",
            get(server::media_server_load),
        )
        .route(
            "/api/server/system/configInfo",
            get(server::system_config_info),
        )
        .route("/api/server/system/info", get(server::system_info))
        .route("/api/server/map/config", get(server::map_config))
        .route(
            "/api/server/map/model-icon/list",
            get(server::map_model_icon_list),
        )
        .route("/api/server/info", get(server::server_info))
        .route("/api/server/resource/info", get(server::resource_info))
        .route("/api/push/list", get(stream::push_list))
        .route("/api/push/add", post(stream::push_add))
        .route("/api/push/update", post(stream::push_update))
        .route("/api/push/start", get(stream::push_start))
        .route("/api/push/remove", post(stream::push_remove))
        .route("/api/push/upload", post(stream::push_upload))
        .route("/api/push/batchRemove", delete(stream::push_batch_remove))
        .route("/api/push/save_to_gb", post(stream::push_save_to_gb))
        .route(
            "/api/push/remove_form_gb",
            delete(stream::push_remove_form_gb),
        )
        .route("/api/proxy/list", get(stream::proxy_list))
        .route(
            "/api/proxy/ffmpeg_cmd/list",
            get(stream::proxy_ffmpeg_cmd_list),
        )
        .route("/api/proxy/add", post(stream::proxy_add))
        .route("/api/proxy/update", post(stream::proxy_update))
        .route("/api/proxy/save", post(stream::proxy_save))
        .route("/api/proxy/start", get(stream::proxy_start))
        .route("/api/proxy/stop", get(stream::proxy_stop))
        .route("/api/proxy/delete", delete(stream::proxy_delete))
        .route("/api/platform/query", get(platform::platform_query))
        .route(
            "/api/platform/server_config",
            get(platform::platform_server_config),
        )
        .route(
            "/api/platform/channel/list",
            get(platform::platform_channel_list),
        )
        .route(
            "/api/platform/channel/push",
            get(platform::platform_channel_push),
        )
        .route(
            "/api/platform/channel/add",
            post(platform::platform_channel_add),
        )
        .route(
            "/api/platform/channel/device/add",
            post(platform::platform_channel_device_add),
        )
        .route(
            "/api/platform/channel/device/remove",
            post(platform::platform_channel_device_remove),
        )
        .route(
            "/api/platform/channel/remove",
            delete(platform::platform_channel_remove),
        )
        .route(
            "/api/platform/channel/custom/update",
            post(platform::platform_channel_custom_update),
        )
        .route("/api/platform/add", post(platform::platform_add))
        .route("/api/platform/update", post(platform::platform_update))
        .route("/api/platform/delete", delete(platform::platform_delete))
        .route(
            "/api/platform/exit/:device_gb_id",
            get(platform::platform_exit),
        )
        .route("/api/platform/catalog/add", post(platform::catalog_add))
        .route("/api/platform/catalog/edit", post(platform::catalog_edit))
        .route(
            "/api/play/start/:device_id/:channel_id",
            get(play::play_start),
        )
        .route(
            "/api/play/stop/:device_id/:channel_id",
            get(play::play_stop),
        )
        .route(
            "/api/play/broadcast/:device_id/:channel_id",
            get(play::broadcast_start),
        )
        .route(
            "/api/play/broadcast/stop/:device_id/:channel_id",
            get(play::broadcast_stop),
        )
        .route(
            "/api/play/webrtc",
            post(webrtc::webrtc_play),
        )
        .route(
            "/api/device/batch/control",
            post(device_batch::batch_control),
        )
        .route("/api/region/tree/list", get(stub::region_tree_list))
        .route("/api/region/delete", delete(stub::region_delete))
        .route("/api/region/description", get(stub::region_description))
        .route(
            "/api/region/addByCivilCode",
            get(stub::region_add_by_civil_code),
        )
        .route(
            "/api/region/queryChildListInBase",
            get(stub::region_query_child),
        )
        .route(
            "/api/region/base/child/list",
            get(stub::region_base_child_list),
        )
        .route("/api/region/update", post(stub::region_update))
        .route("/api/region/add", post(stub::region_add))
        .route("/api/region/path", get(stub::region_path))
        .route("/api/region/tree/query", get(stub::region_tree_query))
        .route("/api/group/tree/list", get(stub::group_tree_list))
        .route("/api/group/add", post(stub::group_add))
        .route("/api/group/update", post(stub::group_update))
        .route("/api/group/delete", delete(stub::group_delete))
        .route("/api/group/path", get(stub::group_path))
        .route("/api/group/tree/query", get(stub::group_tree_query))
        .route("/api/log/list", get(stub::log_list))
        .route("/api/log/file/:file_name", get(stub::log_file_download))
        .route("/api/userApiKey/remark", post(stub::user_api_key_remark))
        .route("/api/userApiKey/userApiKeys", get(stub::user_api_key_list))
        .route("/api/userApiKey/enable", post(stub::user_api_key_enable))
        .route("/api/userApiKey/disable", post(stub::user_api_key_disable))
        .route("/api/userApiKey/reset", post(stub::user_api_key_reset))
        .route("/api/userApiKey/delete", delete(stub::user_api_key_delete))
        .route("/api/userApiKey/add", post(stub::user_api_key_add))
        .route(
            "/api/playback/start/:device_id/:channel_id",
            get(playback::playback_start),
        )
        .route(
            "/api/playback/resume/:stream_id",
            get(playback::playback_resume),
        )
        .route(
            "/api/playback/pause/:stream_id",
            get(playback::playback_pause),
        )
        .route(
            "/api/playback/speed/:stream_id/:speed",
            get(playback::playback_speed),
        )
        .route(
            "/api/playback/seek/:stream_id/:seek_time",
            get(playback::playback_seek),
        )
        .route(
            "/api/playback/stop/:device_id/:channel_id/:stream_id",
            get(playback::playback_stop),
        )
        .route(
            "/api/gb_record/query/:device_id/:channel_id",
            get(playback::gb_record_query),
        )
        .route(
            "/api/gb_record/download/start/:device_id/:channel_id",
            get(playback::gb_record_download_start),
        )
        .route(
            "/api/gb_record/download/stop/:device_id/:channel_id/:stream_id",
            get(playback::gb_record_download_stop),
        )
        .route(
            "/api/gb_record/download/progress/:device_id/:channel_id/:stream_id",
            get(playback::gb_record_download_progress),
        )
        .route(
            "/api/cloud/record/play/path",
            get(stub::cloud_record_play_path),
        )
        .route(
            "/api/cloud/record/date/list",
            get(stub::cloud_record_date_list),
        )
        .route("/api/cloud/record/loadRecord", get(stub::cloud_record_load))
        .route("/api/cloud/record/seek", get(stub::cloud_record_seek))
        .route("/api/cloud/record/speed", get(stub::cloud_record_speed))
        .route(
            "/api/cloud/record/task/add",
            get(stub::cloud_record_task_add),
        )
        .route(
            "/api/cloud/record/task/list",
            get(stub::cloud_record_task_list),
        )
        .route(
            "/api/cloud/record/delete",
            delete(stub::cloud_record_delete),
        )
        .route("/api/cloud/record/list", get(stub::cloud_record_list))
        .route(
            "/api/talk/start/:device_id/:channel_id",
            get(talk::talk_start),
        )
        .route(
            "/api/talk/stop/:device_id/:channel_id",
            get(talk::talk_stop),
        )
        .route(
            "/api/talk/invite/:device_id/:channel_id",
            get(talk::talk_invite),
        )
        .route("/api/talk/ack", post(talk::talk_ack))
        .route("/api/talk/bye", post(talk::talk_bye))
        .route(
            "/api/talk/status/:device_id/:channel_id",
            get(talk::talk_status),
        )
        .route("/api/talk/list", get(talk::talk_list))
        .route("/api/record/plan/get", get(stub::record_plan_get))
        .route("/api/record/plan/add", post(stub::record_plan_add))
        .route("/api/record/plan/update", post(stub::record_plan_update))
        .route("/api/record/plan/query", get(stub::record_plan_query))
        .route("/api/record/plan/delete", delete(stub::record_plan_delete))
        .route(
            "/api/record/plan/channel/list",
            get(stub::record_plan_channel_list),
        )
        .route("/api/record/plan/link", post(stub::record_plan_link))
        .route(
            "/api/position/history/:device_id",
            get(stub::position_history),
        )
        // ========== 通用通道 common_channel ==========
        .route("/api/common/channel/one", get(common_channel::channel_one))
        .route(
            "/api/common/channel/industry/list",
            get(common_channel::industry_list),
        )
        .route(
            "/api/common/channel/type/list",
            get(common_channel::type_list),
        )
        .route(
            "/api/common/channel/network/identification/list",
            get(common_channel::network_identification_list),
        )
        .route(
            "/api/common/channel/update",
            post(common_channel::channel_update),
        )
        .route(
            "/api/common/channel/reset",
            post(common_channel::channel_reset),
        )
        .route("/api/common/channel/add", post(common_channel::channel_add))
        .route(
            "/api/common/channel/civilcode/list",
            get(common_channel::civilcode_list),
        )
        .route(
            "/api/common/channel/civilCode/unusual/list",
            get(common_channel::unusual_civilcode_list),
        )
        .route(
            "/api/common/channel/parent/unusual/list",
            get(common_channel::unusual_parent_list),
        )
        .route(
            "/api/common/channel/civilCode/unusual/clear",
            post(common_channel::clear_unusual_civilcode),
        )
        .route(
            "/api/common/channel/parent/unusual/clear",
            post(common_channel::clear_unusual_parent),
        )
        .route(
            "/api/common/channel/parent/list",
            get(common_channel::parent_list),
        )
        .route(
            "/api/common/channel/region/add",
            post(common_channel::channel_region_add),
        )
        .route(
            "/api/common/channel/region/delete",
            post(common_channel::channel_region_delete),
        )
        .route(
            "/api/common/channel/region/device/add",
            post(common_channel::device_region_add),
        )
        .route(
            "/api/common/channel/region/device/delete",
            post(common_channel::device_region_delete),
        )
        .route(
            "/api/common/channel/group/add",
            post(common_channel::channel_group_add),
        )
        .route(
            "/api/common/channel/group/delete",
            post(common_channel::channel_group_delete),
        )
        .route(
            "/api/common/channel/group/device/add",
            post(common_channel::device_group_add),
        )
        .route(
            "/api/common/channel/group/device/delete",
            post(common_channel::device_group_delete),
        )
        .route(
            "/api/common/channel/play",
            get(common_channel::channel_play),
        )
        .route(
            "/api/common/channel/play/stop",
            get(common_channel::channel_play_stop),
        )
        .route(
            "/api/common/channel/map/list",
            get(common_channel::map_channel_list),
        )
        .route(
            "/api/common/channel/map/save-level",
            post(common_channel::map_save_level),
        )
        .route(
            "/api/common/channel/map/reset-level",
            post(common_channel::map_reset_level),
        )
        .route(
            "/api/common/channel/map/thin/clear",
            get(common_channel::map_thin_clear),
        )
        .route(
            "/api/common/channel/map/thin/progress",
            get(common_channel::map_thin_progress),
        )
        .route(
            "/api/common/channel/map/thin/save",
            get(common_channel::map_thin_save),
        )
        .route(
            "/api/common/channel/map/thin/draw",
            post(common_channel::map_thin_draw),
        )
        // ========== commonChannel 前端控制 ==========
        .route(
            "/api/common/channel/front-end/ptz",
            get(common_channel::front_end_ptz),
        )
        .route(
            "/api/common/channel/front-end/auxiliary",
            get(common_channel::front_end_auxiliary),
        )
        .route(
            "/api/common/channel/front-end/wiper",
            get(common_channel::front_end_wiper),
        )
        .route(
            "/api/common/channel/front-end/fi/iris",
            get(common_channel::front_end_iris),
        )
        .route(
            "/api/common/channel/front-end/fi/focus",
            get(common_channel::front_end_focus),
        )
        .route(
            "/api/common/channel/front-end/preset/query",
            get(common_channel::front_end_preset_query),
        )
        .route(
            "/api/common/channel/front-end/preset/add",
            get(common_channel::front_end_preset_add),
        )
        .route(
            "/api/common/channel/front-end/preset/call",
            get(common_channel::front_end_preset_call),
        )
        .route(
            "/api/common/channel/front-end/preset/delete",
            get(common_channel::front_end_preset_delete),
        )
        .route(
            "/api/common/channel/front-end/tour/point/add",
            get(common_channel::front_end_tour_point_add),
        )
        .route(
            "/api/common/channel/front-end/tour/point/delete",
            get(common_channel::front_end_tour_point_delete),
        )
        .route(
            "/api/common/channel/front-end/tour/speed",
            get(common_channel::front_end_tour_speed),
        )
        .route(
            "/api/common/channel/front-end/tour/time",
            get(common_channel::front_end_tour_time),
        )
        .route(
            "/api/common/channel/front-end/tour/start",
            get(common_channel::front_end_tour_start),
        )
        .route(
            "/api/common/channel/front-end/tour/stop",
            get(common_channel::front_end_tour_stop),
        )
        .route(
            "/api/common/channel/front-end/scan/set/speed",
            get(common_channel::front_end_scan_set_speed),
        )
        .route(
            "/api/common/channel/front-end/scan/set/left",
            get(common_channel::front_end_scan_set_left),
        )
        .route(
            "/api/common/channel/front-end/scan/set/right",
            get(common_channel::front_end_scan_set_right),
        )
        .route(
            "/api/common/channel/front-end/scan/start",
            get(common_channel::front_end_scan_start),
        )
        .route(
            "/api/common/channel/front-end/scan/stop",
            get(common_channel::front_end_scan_stop),
        )
        // ========== commonChannel 回放 ==========
        .route(
            "/api/common/channel/playback/query",
            get(common_channel::channel_playback_query),
        )
        .route(
            "/api/common/channel/playback",
            get(common_channel::channel_playback_start),
        )
        .route(
            "/api/common/channel/playback/stop",
            get(common_channel::channel_playback_stop),
        )
        .route(
            "/api/common/channel/playback/pause",
            get(common_channel::channel_playback_pause),
        )
        .route(
            "/api/common/channel/playback/resume",
            get(common_channel::channel_playback_resume),
        )
        .route(
            "/api/common/channel/playback/seek",
            get(common_channel::channel_playback_seek),
        )
        .route(
            "/api/common/channel/playback/speed",
            get(common_channel::channel_playback_speed),
        )
        // ========== 前端控制 front_end ==========
        .route(
            "/api/front-end/ptz/:device_id/:channel_id",
            get(front_end::ptz),
        )
        .route(
            "/api/front-end/auxiliary/:device_id/:channel_id",
            get(front_end::auxiliary),
        )
        .route(
            "/api/front-end/wiper/:device_id/:channel_id",
            get(front_end::wiper),
        )
        .route(
            "/api/front-end/fi/iris/:device_id/:channel_id",
            get(front_end::iris),
        )
        .route(
            "/api/front-end/fi/focus/:device_id/:channel_device_id",
            get(front_end::focus),
        )
        .route(
            "/api/front-end/preset/query/:device_id/:channel_device_id",
            get(front_end::preset_query),
        )
        .route(
            "/api/front-end/preset/add/:device_id/:channel_device_id",
            get(front_end::preset_add),
        )
        .route(
            "/api/front-end/preset/call/:device_id/:channel_device_id",
            get(front_end::preset_call),
        )
        .route(
            "/api/front-end/preset/delete/:device_id/:channel_device_id",
            get(front_end::preset_delete),
        )
        .route(
            "/api/front-end/cruise/point/add/:device_id/:channel_device_id",
            get(front_end::cruise_point_add),
        )
        .route(
            "/api/front-end/cruise/point/delete/:device_id/:channel_device_id",
            get(front_end::cruise_point_delete),
        )
        .route(
            "/api/front-end/cruise/speed/:device_id/:channel_device_id",
            get(front_end::cruise_speed),
        )
        .route(
            "/api/front-end/cruise/time/:device_id/:channel_device_id",
            get(front_end::cruise_time),
        )
        .route(
            "/api/front-end/cruise/start/:device_id/:channel_device_id",
            get(front_end::cruise_start),
        )
        .route(
            "/api/front-end/cruise/stop/:device_id/:channel_device_id",
            get(front_end::cruise_stop),
        )
        .route(
            "/api/front-end/scan/set/speed/:device_id/:channel_device_id",
            get(front_end::scan_set_speed),
        )
        .route(
            "/api/front-end/scan/set/left/:device_id/:channel_device_id",
            get(front_end::scan_set_left),
        )
        .route(
            "/api/front-end/scan/set/right/:device_id/:channel_device_id",
            get(front_end::scan_set_right),
        )
        .route(
            "/api/front-end/scan/start/:device_id/:channel_device_id",
            get(front_end::scan_start),
        )
        .route(
            "/api/front-end/scan/stop/:device_id/:channel_device_id",
            get(front_end::scan_stop),
        )
        // ========== JT1078 部标设备 ==========
        .route("/api/jt1078/terminal/list", get(jt1078::terminal_list))
        .route("/api/jt1078/terminal/query", get(jt1078::terminal_query))
        .route("/api/jt1078/terminal/add", post(jt1078::terminal_add))
        .route("/api/jt1078/terminal/update", post(jt1078::terminal_update))
        .route(
            "/api/jt1078/terminal/delete",
            delete(jt1078::terminal_delete),
        )
        .route(
            "/api/jt1078/terminal/channel/list",
            get(jt1078::channel_list),
        )
        .route(
            "/api/jt1078/terminal/channel/update",
            post(jt1078::channel_update),
        )
        .route(
            "/api/jt1078/terminal/channel/add",
            post(jt1078::channel_add),
        )
        .route("/api/jt1078/live/start", get(jt1078::live_start))
        .route("/api/jt1078/live/stop", get(jt1078::live_stop))
        .route("/api/jt1078/playback/start/", get(jt1078::playback_start))
        .route("/api/jt1078/playback/stop/", get(jt1078::playback_stop))
        .route(
            "/api/jt1078/playback/control",
            get(jt1078::playback_control),
        )
        .route(
            "/api/jt1078/playback/downloadUrl",
            get(jt1078::playback_download_url),
        )
        .route("/api/jt1078/ptz", get(jt1078::ptz))
        .route("/api/jt1078/wiper", get(jt1078::wiper))
        .route("/api/jt1078/fill-light", get(jt1078::fill_light))
        .route("/api/jt1078/record/list", get(jt1078::record_list))
        .route("/api/jt1078/config/get", get(jt1078::config_get))
        .route("/api/jt1078/config/set", post(jt1078::config_set))
        .route("/api/jt1078/attribute", get(jt1078::attribute))
        .route("/api/jt1078/link-detection", get(jt1078::link_detection))
        .route("/api/jt1078/position-info", get(jt1078::position_info))
        .route("/api/jt1078/text-msg", post(jt1078::text_msg))
        .route(
            "/api/jt1078/telephone-callback",
            get(jt1078::telephone_callback),
        )
        .route("/api/jt1078/driver-information", get(jt1078::driver_info))
        .route(
            "/api/jt1078/control/factory-reset",
            post(jt1078::factory_reset),
        )
        .route("/api/jt1078/control/reset", post(jt1078::reset))
        .route("/api/jt1078/control/connection", post(jt1078::connection))
        .route("/api/jt1078/control/door", get(jt1078::door))
        .route("/api/jt1078/media/attribute", get(jt1078::media_attribute))
        .route("/api/jt1078/media/list", post(jt1078::media_list))
        .route("/api/jt1078/set-phone-book", post(jt1078::set_phone_book))
        .route("/api/jt1078/shooting", post(jt1078::shooting))
        .route("/api/jt1078/talk/start", get(jt1078::talk_start))
        .route("/api/jt1078/talk/stop", get(jt1078::talk_stop))
        .route(
            "/api/jt1078/media/upload/one/upload",
            get(jt1078::media_upload_one),
        )
        // ========== 测试接口 ==========
        .route(
            "/api/sy/camera/list/ids",
            get(common_channel::camera_list_ids),
        )
        .route_layer(middleware::from_fn_with_state(
            state_clone.clone(),
            auth_middleware,
        ));

    let api_public = Router::new()
        .route("/api/user/login", get(user::login).post(user::login))
        .route("/api/user/logout", get(user::logout))
        .route("/api/zlm/hook", post(zlm_hook::handle_webhook))
        .route("/api/health", get(health_check));

    let api = api_public.merge(api_protected);
    let app = Router::new().merge(api).with_state(state.clone());

    // WebSocket：设备状态实时通知
    let app = app.route("/api/ws", get(websocket::ws_handler));

    // 告警管理
    let app = app
        .route("/api/alarm/list", get(alarm::alarm_list))
        .route("/api/alarm/detail/:id", get(alarm::alarm_detail))
        .route("/api/alarm/handle", post(alarm::alarm_handle))
        .route("/api/alarm/delete/:id", delete(alarm::alarm_delete))
        .route("/api/alarm/batch", delete(alarm::alarm_batch_delete))
        .route("/api/alarm/device/:device_id", delete(alarm::alarm_delete_by_device))
        .route("/api/alarm/before/:time", delete(alarm::alarm_delete_before_time));

    // 静态资源：前端构建产物（与 Java 版 static 目录一致）
    let static_dir = state
        .config
        .static_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|p| p.exists());
    let app = if let Some(dir) = static_dir {
        let index_path = dir.join("index.html");
        let serve_dir = tower_http::services::ServeDir::new(dir)
            .fallback(tower_http::services::ServeFile::new(index_path));
        app.nest_service("/", serve_dir)
    } else {
        tracing::warn!("未配置 static_dir 或目录不存在，仅提供 API");
        app
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    app.layer(cors)
}
