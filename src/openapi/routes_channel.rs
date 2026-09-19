//! `channel` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::common_channel::camera_list_ids));
    acc.add(routes!(crate::handlers::common_channel::channel_add));
    acc.add(routes!(crate::handlers::common_channel::channel_broadcast_start));
    acc.add(routes!(crate::handlers::common_channel::channel_broadcast_stop));
    acc.add(routes!(crate::handlers::common_channel::channel_delete));
    acc.add(routes!(crate::handlers::common_channel::channel_group_add));
    acc.add(routes!(crate::handlers::common_channel::channel_group_delete));
    acc.add(routes!(crate::handlers::common_channel::channel_one));
    acc.add(routes!(crate::handlers::common_channel::channel_play));
    acc.add(routes!(crate::handlers::common_channel::channel_play_stop));
    acc.add(routes!(crate::handlers::common_channel::channel_playback_pause));
    acc.add(routes!(crate::handlers::common_channel::channel_playback_query));
    acc.add(routes!(crate::handlers::common_channel::channel_playback_resume));
    acc.add(routes!(crate::handlers::common_channel::channel_playback_seek));
    acc.add(routes!(crate::handlers::common_channel::channel_playback_speed));
    acc.add(routes!(crate::handlers::common_channel::channel_playback_start));
    acc.add(routes!(crate::handlers::common_channel::channel_playback_stop));
    acc.add(routes!(crate::handlers::common_channel::channel_region_add));
    acc.add(routes!(crate::handlers::common_channel::channel_region_delete));
    acc.add(routes!(crate::handlers::common_channel::channel_reset));
    acc.add(routes!(crate::handlers::common_channel::channel_talk_start));
    acc.add(routes!(crate::handlers::common_channel::channel_talk_stop));
    acc.add(routes!(crate::handlers::common_channel::channel_update));
    acc.add(routes!(crate::handlers::common_channel::civilcode_list));
    acc.add(routes!(crate::handlers::common_channel::clear_unusual_civilcode));
    acc.add(routes!(crate::handlers::common_channel::clear_unusual_parent));
    acc.add(routes!(crate::handlers::common_channel::device_group_add));
    acc.add(routes!(crate::handlers::common_channel::device_group_delete));
    acc.add(routes!(crate::handlers::common_channel::device_region_add));
    acc.add(routes!(crate::handlers::common_channel::device_region_delete));
    acc.add(routes!(crate::handlers::common_channel::front_end_auxiliary));
    acc.add(routes!(crate::handlers::common_channel::front_end_drag_zoom_in));
    acc.add(routes!(crate::handlers::common_channel::front_end_drag_zoom_out));
    acc.add(routes!(crate::handlers::common_channel::front_end_focus));
    acc.add(routes!(crate::handlers::common_channel::front_end_home_position));
    acc.add(routes!(crate::handlers::common_channel::front_end_iris));
    acc.add(routes!(crate::handlers::common_channel::front_end_preset_add));
    acc.add(routes!(crate::handlers::common_channel::front_end_preset_call));
    acc.add(routes!(crate::handlers::common_channel::front_end_preset_delete));
    acc.add(routes!(crate::handlers::common_channel::front_end_preset_query));
    acc.add(routes!(crate::handlers::common_channel::front_end_ptz));
    acc.add(routes!(crate::handlers::common_channel::front_end_scan_set_left));
    acc.add(routes!(crate::handlers::common_channel::front_end_scan_set_right));
    acc.add(routes!(crate::handlers::common_channel::front_end_scan_set_speed));
    acc.add(routes!(crate::handlers::common_channel::front_end_scan_start));
    acc.add(routes!(crate::handlers::common_channel::front_end_scan_stop));
    acc.add(routes!(crate::handlers::common_channel::front_end_tour_point_add));
    acc.add(routes!(crate::handlers::common_channel::front_end_tour_point_delete));
    acc.add(routes!(crate::handlers::common_channel::front_end_tour_speed));
    acc.add(routes!(crate::handlers::common_channel::front_end_tour_start));
    acc.add(routes!(crate::handlers::common_channel::front_end_tour_stop));
    acc.add(routes!(crate::handlers::common_channel::front_end_tour_time));
    acc.add(routes!(crate::handlers::common_channel::front_end_wiper));
    acc.add(routes!(crate::handlers::common_channel::industry_list));
    acc.add(routes!(crate::handlers::common_channel::map_channel_list));
    acc.add(routes!(crate::handlers::common_channel::map_reset_level));
    acc.add(routes!(crate::handlers::common_channel::map_save_level));
    acc.add(routes!(crate::handlers::common_channel::map_thin_clear));
    acc.add(routes!(crate::handlers::common_channel::map_thin_draw));
    acc.add(routes!(crate::handlers::common_channel::map_thin_progress));
    acc.add(routes!(crate::handlers::common_channel::map_thin_save));
    acc.add(routes!(crate::handlers::common_channel::network_identification_list));
    acc.add(routes!(crate::handlers::common_channel::parent_list));
    acc.add(routes!(crate::handlers::common_channel::type_list));
    acc.add(routes!(crate::handlers::common_channel::unusual_civilcode_list));
    acc.add(routes!(crate::handlers::common_channel::unusual_parent_list));
    acc.add(routes!(crate::handlers::front_end::auxiliary));
    acc.add(routes!(crate::handlers::front_end::cruise_point_add));
    acc.add(routes!(crate::handlers::front_end::cruise_point_delete));
    acc.add(routes!(crate::handlers::front_end::cruise_speed));
    acc.add(routes!(crate::handlers::front_end::cruise_start));
    acc.add(routes!(crate::handlers::front_end::cruise_stop));
    acc.add(routes!(crate::handlers::front_end::cruise_time));
    acc.add(routes!(crate::handlers::front_end::focus));
    acc.add(routes!(crate::handlers::front_end::iris));
    acc.add(routes!(crate::handlers::front_end::legacy_front_end_command));
    acc.add(routes!(crate::handlers::front_end::preset_add));
    acc.add(routes!(crate::handlers::front_end::preset_call));
    acc.add(routes!(crate::handlers::front_end::preset_delete));
    acc.add(routes!(crate::handlers::front_end::preset_query));
    acc.add(routes!(crate::handlers::front_end::ptz));
    acc.add(routes!(crate::handlers::front_end::scan_set_left));
    acc.add(routes!(crate::handlers::front_end::scan_set_right));
    acc.add(routes!(crate::handlers::front_end::scan_set_speed));
    acc.add(routes!(crate::handlers::front_end::scan_start));
    acc.add(routes!(crate::handlers::front_end::scan_stop));
    acc.add(routes!(crate::handlers::front_end::wiper));
    acc.add(routes!(crate::handlers::position::position_history));
    acc.add(routes!(crate::handlers::position::position_latest));
    acc.add(routes!(crate::handlers::position::position_realtime));
    acc.add(routes!(crate::handlers::position::position_subscribe));

    acc.finish()
}
