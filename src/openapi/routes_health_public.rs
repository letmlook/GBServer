//! `health` 域的 公开 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::health::liveness));
    acc.add(routes!(crate::handlers::health::readiness));
    acc.add(routes!(crate::handlers::metrics::metrics_handler));
    acc.add(routes!(crate::handlers::websocket::ws_handler));

    acc.finish()
}
