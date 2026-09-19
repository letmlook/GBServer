//! `system` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::server::list_all_streams));
    acc.add(routes!(crate::handlers::server::map_config));
    acc.add(routes!(crate::handlers::server::map_model_icon_list));
    acc.add(routes!(crate::handlers::server::media_server_check));
    acc.add(routes!(crate::handlers::server::media_server_delete));
    acc.add(routes!(crate::handlers::server::media_server_list));
    acc.add(routes!(crate::handlers::server::media_server_load));
    acc.add(routes!(crate::handlers::server::media_server_media_info));
    acc.add(routes!(crate::handlers::server::media_server_one));
    acc.add(routes!(crate::handlers::server::media_server_online_list));
    acc.add(routes!(crate::handlers::server::media_server_record_check));
    acc.add(routes!(crate::handlers::server::media_server_save));
    acc.add(routes!(crate::handlers::server::resource_info));
    acc.add(routes!(crate::handlers::server::server_info));
    acc.add(routes!(crate::handlers::server::server_shutdown));
    acc.add(routes!(crate::handlers::server::system_config_info));
    acc.add(routes!(crate::handlers::server::system_info));
    acc.add(routes!(crate::handlers::server::zlm_proxy));
    acc.add(routes!(crate::handlers::system::online_users));
    acc.add(routes!(crate::handlers::system::system_info));
    acc.add(routes!(crate::handlers::system::system_stats));
    acc.add(routes!(crate::handlers::system::system_version));

    acc.finish()
}
