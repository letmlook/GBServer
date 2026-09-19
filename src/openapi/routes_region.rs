//! 区域（region）接口的 OpenAPI 路由注册。
//!
//! 模板（后续每个域照此写）：
//! * `routes!(handler)` **一次只放一条路由**
//! * 返回 `(schemas, paths, router)` 三元组；`router.rs` 把它并进受保护区
//! * path 来自宏产物 `paths.paths`，不手写字符串
//! * handler 上必须有 `#[utoipa::path(...)]`，否则 spec 里没这条

use utoipa_axum::routes;

use super::{DocumentedRoutes, RoutesAccumulator};

pub fn routes() -> DocumentedRoutes {
    let mut acc = RoutesAccumulator::default();

    acc.add(routes!(crate::handlers::region::region_one));
    acc.add(routes!(crate::handlers::region::region_page_list));
    acc.add(routes!(crate::handlers::region::region_sync));

    acc.finish()
}
