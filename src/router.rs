use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::auth_middleware;
use crate::handlers::{
    device, device_control, device_stub, platform, play, playback, server, stream, stub, user,
};
use crate::zlm::hook as zlm_hook;
use crate::AppState;

pub fn app(state: AppState) -> Router {
    let state_clone = state.clone();
    let api_protected = Router::new()
        .route("/api/user/userInfo", post(user::user_info))
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
        .route_layer(middleware::from_fn_with_state(
            state_clone.clone(),
            auth_middleware,
        ))
        .with_state(state_clone.clone());

    let api_public = Router::new()
        .route("/api/user/login", get(user::login).post(user::login))
        .route("/api/user/logout", get(user::logout))
        .route("/api/zlm/hook", post(zlm_hook::handle_webhook))
        .with_state(state_clone.clone());

    let api = api_public.merge(api_protected);
    let app = Router::new().merge(api).with_state(state.clone());

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
