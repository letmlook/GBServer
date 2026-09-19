//! `user` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::alarm::alarm_batch_delete));
    acc.add(routes!(crate::handlers::alarm::alarm_clear));
    acc.add(routes!(crate::handlers::alarm::alarm_delete));
    acc.add(routes!(crate::handlers::alarm::alarm_delete_batch));
    acc.add(routes!(crate::handlers::alarm::alarm_delete_before_time));
    acc.add(routes!(crate::handlers::alarm::alarm_delete_by_device));
    acc.add(routes!(crate::handlers::alarm::alarm_detail));
    acc.add(routes!(crate::handlers::alarm::alarm_handle));
    acc.add(routes!(crate::handlers::alarm::alarm_list));
    acc.add(routes!(crate::handlers::parity_extras::alarm_snap));
    acc.add(routes!(crate::handlers::parity_extras::channel_map_thin_tile));
    acc.add(routes!(crate::handlers::parity_extras::channel_map_tile));
    acc.add(routes!(crate::handlers::parity_extras::front_end_common));
    acc.add(routes!(crate::handlers::parity_extras::server_config));
    acc.add(routes!(crate::handlers::parity_extras::server_version));
    acc.add(routes!(crate::handlers::role::role_add));
    acc.add(routes!(crate::handlers::role::role_all));
    acc.add(routes!(crate::handlers::role::role_delete));
    acc.add(routes!(crate::handlers::user::add_user));
    acc.add(routes!(crate::handlers::user::all_users));
    acc.add(routes!(crate::handlers::user::change_password));
    acc.add(routes!(crate::handlers::user::change_password_for_admin));
    acc.add(routes!(crate::handlers::user::change_push_key));
    acc.add(routes!(crate::handlers::user::delete_user));
    acc.add(routes!(crate::handlers::user::logout));
    acc.add(routes!(crate::handlers::user::update_user));
    acc.add(routes!(crate::handlers::user::user_info));
    acc.add(routes!(crate::handlers::user::users));

    acc.finish()
}
