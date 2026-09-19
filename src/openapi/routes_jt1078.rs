//! `jt1078` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::jt1078::attribute));
    acc.add(routes!(crate::handlers::jt1078::channel_add));
    acc.add(routes!(crate::handlers::jt1078::channel_list));
    acc.add(routes!(crate::handlers::jt1078::channel_update));
    acc.add(routes!(crate::handlers::jt1078::config_get));
    acc.add(routes!(crate::handlers::jt1078::config_set));
    acc.add(routes!(crate::handlers::jt1078::connection));
    acc.add(routes!(crate::handlers::jt1078::door));
    acc.add(routes!(crate::handlers::jt1078::driver_info));
    acc.add(routes!(crate::handlers::jt1078::factory_reset));
    acc.add(routes!(crate::handlers::jt1078::fill_light));
    acc.add(routes!(crate::handlers::jt1078::link_detection));
    acc.add(routes!(crate::handlers::jt1078::live_start));
    acc.add(routes!(crate::handlers::jt1078::live_stop));
    acc.add(routes!(crate::handlers::jt1078::media_attribute));
    acc.add(routes!(crate::handlers::jt1078::media_list));
    acc.add(routes!(crate::handlers::jt1078::media_upload_one));
    acc.add(routes!(crate::handlers::jt1078::playback_control));
    acc.add(routes!(crate::handlers::jt1078::playback_download_url));
    acc.add(routes!(crate::handlers::jt1078::playback_start));
    acc.add(routes!(crate::handlers::jt1078::playback_stop));
    acc.add(routes!(crate::handlers::jt1078::position_info));
    acc.add(routes!(crate::handlers::jt1078::ptz));
    acc.add(routes!(crate::handlers::jt1078::record_list));
    acc.add(routes!(crate::handlers::jt1078::reset));
    acc.add(routes!(crate::handlers::jt1078::set_phone_book));
    acc.add(routes!(crate::handlers::jt1078::shooting));
    acc.add(routes!(crate::handlers::jt1078::talk_start));
    acc.add(routes!(crate::handlers::jt1078::talk_stop));
    acc.add(routes!(crate::handlers::jt1078::telephone_callback));
    acc.add(routes!(crate::handlers::jt1078::terminal_add));
    acc.add(routes!(crate::handlers::jt1078::terminal_delete));
    acc.add(routes!(crate::handlers::jt1078::terminal_list));
    acc.add(routes!(crate::handlers::jt1078::terminal_one));
    acc.add(routes!(crate::handlers::jt1078::terminal_query));
    acc.add(routes!(crate::handlers::jt1078::terminal_update));
    acc.add(routes!(crate::handlers::jt1078::text_msg));
    acc.add(routes!(crate::handlers::jt1078::wiper));
    acc.add(routes!(crate::handlers::jt1078_extra::area_circle_add));
    acc.add(routes!(crate::handlers::jt1078_extra::area_circle_delete));
    acc.add(routes!(crate::handlers::jt1078_extra::area_circle_edit));
    acc.add(routes!(crate::handlers::jt1078_extra::area_circle_query));
    acc.add(routes!(crate::handlers::jt1078_extra::area_circle_update));
    acc.add(routes!(crate::handlers::jt1078_extra::area_polygon_delete));
    acc.add(routes!(crate::handlers::jt1078_extra::area_polygon_query));
    acc.add(routes!(crate::handlers::jt1078_extra::area_polygon_set));
    acc.add(routes!(crate::handlers::jt1078_extra::area_rectangle_add));
    acc.add(routes!(crate::handlers::jt1078_extra::area_rectangle_delete));
    acc.add(routes!(crate::handlers::jt1078_extra::area_rectangle_edit));
    acc.add(routes!(crate::handlers::jt1078_extra::area_rectangle_query));
    acc.add(routes!(crate::handlers::jt1078_extra::area_rectangle_update));
    acc.add(routes!(crate::handlers::jt1078_extra::confirmation_alarm));
    acc.add(routes!(crate::handlers::jt1078_extra::live_continue));
    acc.add(routes!(crate::handlers::jt1078_extra::live_pause));
    acc.add(routes!(crate::handlers::jt1078_extra::live_switch));
    acc.add(routes!(crate::handlers::jt1078_extra::media_upload_delete));
    acc.add(routes!(crate::handlers::jt1078_extra::playback_download));
    acc.add(routes!(crate::handlers::jt1078_extra::record_start));
    acc.add(routes!(crate::handlers::jt1078_extra::record_stop));
    acc.add(routes!(crate::handlers::jt1078_extra::route_delete));
    acc.add(routes!(crate::handlers::jt1078_extra::route_query));
    acc.add(routes!(crate::handlers::jt1078_extra::route_set));
    acc.add(routes!(crate::handlers::jt1078_extra::snap));
    acc.add(routes!(crate::handlers::jt1078_extra::temp_position_tracking));
    acc.add(routes!(crate::handlers::jt1078_extra::terminal_channel_delete));
    acc.add(routes!(crate::handlers::jt1078_extra::terminal_channel_delete_query));
    acc.add(routes!(crate::handlers::jt1078_extra::terminal_channel_one));
    acc.add(routes!(crate::handlers::jt1078_extra::terminal_channel_one_query));

    acc.finish()
}
