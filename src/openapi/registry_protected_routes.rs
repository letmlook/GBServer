//! 受保护路由聚合（挂鉴权 + 审计中间件）。
//!
//! **本文件由生成脚本产出**：新增域时重新生成，不要手改。

use super::{DocumentedRoutes, RoutesAccumulator};

pub fn protected_routes() -> DocumentedRoutes {
    let mut acc = RoutesAccumulator::default();
    acc.merge(super::routes_channel::routes());
    acc.merge(super::routes_cloud_record::routes());
    acc.merge(super::routes_device::routes());
    acc.merge(super::routes_jt1078::routes());
    acc.merge(super::routes_live::routes());
    acc.merge(super::routes_misc::routes());
    acc.merge(super::routes_platform::routes());
    acc.merge(super::routes_region::routes());
    acc.merge(super::routes_stream::routes());
    acc.merge(super::routes_sy::routes());
    acc.merge(super::routes_system::routes());
    acc.merge(super::routes_user::routes());

    acc.finish()
}
