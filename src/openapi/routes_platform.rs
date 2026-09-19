//! `platform` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::platform::catalog_add));
    acc.add(routes!(crate::handlers::platform::catalog_edit));
    acc.add(routes!(crate::handlers::platform::platform_add));
    acc.add(routes!(crate::handlers::platform::platform_channel_add));
    acc.add(routes!(crate::handlers::platform::platform_channel_custom_update));
    acc.add(routes!(crate::handlers::platform::platform_channel_device_add));
    acc.add(routes!(crate::handlers::platform::platform_channel_device_remove));
    acc.add(routes!(crate::handlers::platform::platform_channel_list));
    acc.add(routes!(crate::handlers::platform::platform_channel_push));
    acc.add(routes!(crate::handlers::platform::platform_channel_remove));
    acc.add(routes!(crate::handlers::platform::platform_delete));
    acc.add(routes!(crate::handlers::platform::platform_exit));
    acc.add(routes!(crate::handlers::platform::platform_info));
    acc.add(routes!(crate::handlers::platform::platform_query));
    acc.add(routes!(crate::handlers::platform::platform_server_config));
    acc.add(routes!(crate::handlers::platform::platform_update));

    acc.finish()
}
