//! `sy` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::sy_camera::camera_cont_with_child));
    acc.add(routes!(crate::handlers::sy_camera::camera_control_play));
    acc.add(routes!(crate::handlers::sy_camera::camera_control_ptz));
    acc.add(routes!(crate::handlers::sy_camera::camera_control_stop));
    acc.add(routes!(crate::handlers::sy_camera::camera_list));
    acc.add(routes!(crate::handlers::sy_camera::camera_list_address));
    acc.add(routes!(crate::handlers::sy_camera::camera_list_box));
    acc.add(routes!(crate::handlers::sy_camera::camera_list_circle));
    acc.add(routes!(crate::handlers::sy_camera::camera_list_for_mobile));
    acc.add(routes!(crate::handlers::sy_camera::camera_list_polygon));
    acc.add(routes!(crate::handlers::sy_camera::camera_list_with_child));
    acc.add(routes!(crate::handlers::sy_camera::camera_meeting_list));

    // A1 补齐的 5 条契约端点（别名路由，复用既有实现）。
    acc.add(routes!(crate::handlers::sy_contract::camera_one));
    acc.add(routes!(crate::handlers::sy_contract::camera_update));
    acc.add(routes!(crate::handlers::sy_contract::push_play));
    acc.add(routes!(
        crate::handlers::sy_contract::push_play_without_check
    ));
    acc.add(routes!(crate::handlers::sy_contract::sy_test));

    // A1 契约中已有等价实现、但缺少 SY 前缀的 5 条：只加别名，不重写逻辑。
    // 收藏删除与平台版本一致，同时注册 GET 与 DELETE。
    acc.add(routes!(crate::handlers::sy_contract::record_collect_add));
    acc.add(routes!(crate::handlers::sy_contract::record_collect_delete));
    acc.add(routes!(
        crate::handlers::sy_contract::record_collect_delete_method
    ));
    acc.add(routes!(crate::handlers::sy_contract::record_list_url));
    acc.add(routes!(crate::handlers::sy_contract::record_zip));
    acc.add(routes!(crate::handlers::sy_contract::force_close));

    acc.finish()
}
