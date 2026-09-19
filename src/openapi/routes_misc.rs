//! `misc` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::stub::cloud_record_collect_add));
    acc.add(routes!(crate::handlers::stub::cloud_record_collect_delete));
    acc.add(routes!(crate::handlers::stub::cloud_record_collect_list));
    acc.add(routes!(crate::handlers::stub::cloud_record_date_list));
    acc.add(routes!(crate::handlers::stub::cloud_record_delete));
    acc.add(routes!(crate::handlers::stub::cloud_record_list));
    acc.add(routes!(crate::handlers::stub::cloud_record_load));
    acc.add(routes!(crate::handlers::stub::cloud_record_play_path));
    acc.add(routes!(crate::handlers::stub::cloud_record_seek));
    acc.add(routes!(crate::handlers::stub::cloud_record_speed));
    acc.add(routes!(crate::handlers::stub::cloud_record_task_add));
    acc.add(routes!(crate::handlers::stub::cloud_record_task_list));
    acc.add(routes!(crate::handlers::stub::common_channel_list));
    acc.add(routes!(crate::handlers::stub::group_add));
    acc.add(routes!(crate::handlers::stub::group_delete));
    acc.add(routes!(crate::handlers::stub::group_one));
    acc.add(routes!(crate::handlers::stub::group_path));
    acc.add(routes!(crate::handlers::stub::group_tree_list));
    acc.add(routes!(crate::handlers::stub::group_tree_query));
    acc.add(routes!(crate::handlers::stub::group_update));
    acc.add(routes!(crate::handlers::stub::log_file_download));
    acc.add(routes!(crate::handlers::stub::log_list));
    acc.add(routes!(crate::handlers::stub::record_plan_add));
    acc.add(routes!(crate::handlers::stub::record_plan_channel_list));
    acc.add(routes!(crate::handlers::stub::record_plan_delete));
    acc.add(routes!(crate::handlers::stub::record_plan_get));
    acc.add(routes!(crate::handlers::stub::record_plan_link));
    acc.add(routes!(crate::handlers::stub::record_plan_query));
    acc.add(routes!(crate::handlers::stub::record_plan_update));
    acc.add(routes!(crate::handlers::stub::region_add));
    acc.add(routes!(crate::handlers::stub::region_add_by_civil_code));
    acc.add(routes!(crate::handlers::stub::region_base_child_list));
    acc.add(routes!(crate::handlers::stub::region_delete));
    acc.add(routes!(crate::handlers::stub::region_description));
    acc.add(routes!(crate::handlers::stub::region_path));
    acc.add(routes!(crate::handlers::stub::region_query_child));
    acc.add(routes!(crate::handlers::stub::region_tree_list));
    acc.add(routes!(crate::handlers::stub::region_tree_query));
    acc.add(routes!(crate::handlers::stub::region_update));
    acc.add(routes!(crate::handlers::stub::user_api_key_add));
    acc.add(routes!(crate::handlers::stub::user_api_key_delete));
    acc.add(routes!(crate::handlers::stub::user_api_key_disable));
    acc.add(routes!(crate::handlers::stub::user_api_key_enable));
    acc.add(routes!(crate::handlers::stub::user_api_key_list));
    acc.add(routes!(crate::handlers::stub::user_api_key_remark));
    acc.add(routes!(crate::handlers::stub::user_api_key_reset));

    acc.finish()
}
