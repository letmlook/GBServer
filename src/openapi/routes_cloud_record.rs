//! `cloud_record` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::cloud_record_extra::collect_delete));
    acc.add(routes!(crate::handlers::cloud_record_extra::download_file));
    acc.add(routes!(crate::handlers::cloud_record_extra::download_zip));
    acc.add(routes!(crate::handlers::cloud_record_extra::list_url));
    acc.add(routes!(crate::handlers::cloud_record_extra::zip));

    acc.finish()
}
