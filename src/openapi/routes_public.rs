//! 公开端点中、handler 不在 `src/handlers/` 下的两个：ZLM 事件回调与集群 RPC。
//!
//! 这两个端点故意不经鉴权中间件，各自用别的方式保护：
//! * `/api/zlm/hook` —— 由 ZLM 调用，靠 hook 配置的可达性 + ZLM 侧配置
//! * `/api/rpc` —— 集群对端调用，靠 `X-RPC-Secret`（`[rpc].secret`）
//!
//! 其余公开端点（health / metrics / websocket / login / share / snapshot /
//! talk audio）由生成脚本按注解归档到 `routes_*_public.rs`。

use utoipa_axum::routes;

use super::{DocumentedRoutes, RoutesAccumulator};

pub fn routes() -> DocumentedRoutes {
    let mut acc = RoutesAccumulator::default();

    acc.add(routes!(crate::zlm::hook::handle_webhook));
    acc.add(routes!(crate::router::rpc_endpoint));

    acc.finish()
}
