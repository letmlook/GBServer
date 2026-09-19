//! `live` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::play::broadcast_start));
    acc.add(routes!(crate::handlers::play::broadcast_stop));
    acc.add(routes!(crate::handlers::play::play_convert_stop));
    acc.add(routes!(crate::handlers::play::play_start));
    acc.add(routes!(crate::handlers::play::play_stop));
    acc.add(routes!(crate::handlers::playback::gb_record_download_file));
    acc.add(routes!(crate::handlers::playback::gb_record_download_progress));
    acc.add(routes!(crate::handlers::playback::gb_record_download_start));
    acc.add(routes!(crate::handlers::playback::gb_record_download_stop));
    acc.add(routes!(crate::handlers::playback::gb_record_query));
    acc.add(routes!(crate::handlers::playback::playback_pause));
    acc.add(routes!(crate::handlers::playback::playback_resume));
    acc.add(routes!(crate::handlers::playback::playback_seek));
    acc.add(routes!(crate::handlers::playback::playback_speed));
    acc.add(routes!(crate::handlers::playback::playback_start));
    acc.add(routes!(crate::handlers::playback::playback_stop));
    acc.add(routes!(crate::handlers::talk::talk_ack));
    acc.add(routes!(crate::handlers::talk::talk_bye));
    acc.add(routes!(crate::handlers::talk::talk_invite));
    acc.add(routes!(crate::handlers::talk::talk_list));
    acc.add(routes!(crate::handlers::talk::talk_start));
    acc.add(routes!(crate::handlers::talk::talk_status));
    acc.add(routes!(crate::handlers::talk::talk_stop));
    acc.add(routes!(crate::handlers::webrtc::webrtc_play));

    acc.finish()
}
