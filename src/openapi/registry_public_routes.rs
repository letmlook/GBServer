//! 公开路由聚合（不挂鉴权中间件）。
//!
//! **本文件由生成脚本产出**：新增域时重新生成，不要手改。

use super::{DocumentedRoutes, RoutesAccumulator};

pub fn public_routes() -> DocumentedRoutes {
    let mut acc = RoutesAccumulator::default();
    acc.merge(super::routes_device_public::routes());
    acc.merge(super::routes_health_public::routes());
    acc.merge(super::routes_live_public::routes());
    acc.merge(super::routes_public::routes());
    acc.merge(super::routes_user_public::routes());

    acc.finish()
}
