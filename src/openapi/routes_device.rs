//! `device` 域的 受保护 OpenAPI 路由注册。
//!
//! 逐条 `routes!()` 注册（一次只能放一条）；迁移说明与三个坑见
//! `src/openapi/routes_region.rs` 文件头。
//!
//! 本文件内容由 handler 上的 `#[utoipa::path]` 注解决定：新增接口 =
//! 写注解 + 重新跑生成脚本，不要手写路径字符串。

use utoipa_axum::routes;

use super::{DocumentedRoutes, RoutesAccumulator};

pub fn routes() -> DocumentedRoutes {
    let mut acc = RoutesAccumulator::default();
    acc.add(routes!(crate::handlers::device::device_keepalive_statistics));
    acc.add(routes!(crate::handlers::device::device_register_statistics));
    acc.add(routes!(crate::handlers::device::query_channels));
    acc.add(routes!(crate::handlers::device::query_device_latency));
    acc.add(routes!(crate::handlers::device::query_devices));
    acc.add(routes!(crate::handlers::device_batch::batch_control));
    acc.add(routes!(crate::handlers::device_control::config_query_basic_param));
    acc.add(routes!(crate::handlers::device_control::config_query_svac_decode));
    acc.add(routes!(crate::handlers::device_control::config_query_svac_encode));
    acc.add(routes!(crate::handlers::device_control::config_query_video_param));
    acc.add(routes!(crate::handlers::device_control::config_set_basic_param));
    acc.add(routes!(crate::handlers::device_control::config_set_video_param));
    acc.add(routes!(crate::handlers::device_control::device_config_query));
    acc.add(routes!(crate::handlers::device_control::device_config_update));
    acc.add(routes!(crate::handlers::device_control::device_drag_zoom_in));
    acc.add(routes!(crate::handlers::device_control::device_drag_zoom_out));
    acc.add(routes!(crate::handlers::device_control::device_guard));
    acc.add(routes!(crate::handlers::device_control::device_home_position));
    acc.add(routes!(crate::handlers::device_control::device_iframe));
    acc.add(routes!(crate::handlers::device_control::device_preset));
    acc.add(routes!(crate::handlers::device_control::device_ptz));
    acc.add(routes!(crate::handlers::device_control::device_reboot));
    acc.add(routes!(crate::handlers::device_control::device_reset_alarm));
    acc.add(routes!(crate::handlers::device_control::device_teleboot));
    acc.add(routes!(crate::handlers::device_control::subscribe_catalog));
    acc.add(routes!(crate::handlers::device_query::channel_raw));
    acc.add(routes!(crate::handlers::device_query::device_alarm_query));
    acc.add(routes!(crate::handlers::device_query::device_config_query));
    acc.add(routes!(crate::handlers::device_query::device_info));
    acc.add(routes!(crate::handlers::device_query::device_info_query));
    acc.add(routes!(crate::handlers::device_query::device_status));
    acc.add(routes!(crate::handlers::device_query::device_status_path));
    acc.add(routes!(crate::handlers::device_query::get_play_url));
    acc.add(routes!(crate::handlers::device_query::get_ssrc));
    acc.add(routes!(crate::handlers::device_query::list_snapshots));
    acc.add(routes!(crate::handlers::device_query::snap_path));
    acc.add(routes!(crate::handlers::device_query::snap_query));
    acc.add(routes!(crate::handlers::device_query::ssrc_query));
    acc.add(routes!(crate::handlers::device_query::stream_info));
    acc.add(routes!(crate::handlers::device_query::sync_status_path));
    acc.add(routes!(crate::handlers::device_stub::channel_audio));
    acc.add(routes!(crate::handlers::device_stub::channel_one));
    acc.add(routes!(crate::handlers::device_stub::channel_stream_identification_update));
    acc.add(routes!(crate::handlers::device_stub::config_basic_param));
    acc.add(routes!(crate::handlers::device_stub::control_record));
    acc.add(routes!(crate::handlers::device_stub::device_add));
    acc.add(routes!(crate::handlers::device_stub::device_delete));
    acc.add(routes!(crate::handlers::device_stub::device_one));
    acc.add(routes!(crate::handlers::device_stub::device_sync));
    acc.add(routes!(crate::handlers::device_stub::device_transport));
    acc.add(routes!(crate::handlers::device_stub::device_tree));
    acc.add(routes!(crate::handlers::device_stub::device_update));
    acc.add(routes!(crate::handlers::device_stub::query_streams));
    acc.add(routes!(crate::handlers::device_stub::sub_channels));
    acc.add(routes!(crate::handlers::device_stub::subscribe_alarm));
    acc.add(routes!(crate::handlers::device_stub::subscribe_mobile_position));
    acc.add(routes!(crate::handlers::device_stub::sync_status));
    acc.add(routes!(crate::handlers::device_stub::tree_channel));

    acc.finish()
}
