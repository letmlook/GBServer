//! `stream` 域的 受保护 OpenAPI 路由注册。
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
    acc.add(routes!(crate::handlers::rtp_control::ps_get_test_port));
    acc.add(routes!(crate::handlers::rtp_control::ps_receive_close));
    acc.add(routes!(crate::handlers::rtp_control::ps_receive_close_query));
    acc.add(routes!(crate::handlers::rtp_control::ps_receive_open));
    acc.add(routes!(crate::handlers::rtp_control::ps_send_start));
    acc.add(routes!(crate::handlers::rtp_control::ps_send_stop));
    acc.add(routes!(crate::handlers::rtp_control::ps_send_stop_query));
    acc.add(routes!(crate::handlers::rtp_control::rtp_receive_close));
    acc.add(routes!(crate::handlers::rtp_control::rtp_receive_close_query));
    acc.add(routes!(crate::handlers::rtp_control::rtp_receive_open));
    acc.add(routes!(crate::handlers::rtp_control::rtp_send_start));
    acc.add(routes!(crate::handlers::rtp_control::rtp_send_stop));
    acc.add(routes!(crate::handlers::rtp_control::rtp_send_stop_query));
    acc.add(routes!(crate::handlers::stream::proxy_add));
    acc.add(routes!(crate::handlers::stream::proxy_delete));
    acc.add(routes!(crate::handlers::stream::proxy_ffmpeg_cmd_list));
    acc.add(routes!(crate::handlers::stream::proxy_list));
    acc.add(routes!(crate::handlers::stream::proxy_one));
    acc.add(routes!(crate::handlers::stream::proxy_save));
    acc.add(routes!(crate::handlers::stream::proxy_start));
    acc.add(routes!(crate::handlers::stream::proxy_stop));
    acc.add(routes!(crate::handlers::stream::proxy_update));
    acc.add(routes!(crate::handlers::stream::push_add));
    acc.add(routes!(crate::handlers::stream::push_batch_remove));
    acc.add(routes!(crate::handlers::stream::push_force_close));
    acc.add(routes!(crate::handlers::stream::push_list));
    acc.add(routes!(crate::handlers::stream::push_remove));
    acc.add(routes!(crate::handlers::stream::push_remove_form_gb));
    acc.add(routes!(crate::handlers::stream::push_save_to_gb));
    acc.add(routes!(crate::handlers::stream::push_start));
    acc.add(routes!(crate::handlers::stream::push_stop));
    acc.add(routes!(crate::handlers::stream::push_update));
    acc.add(routes!(crate::handlers::stream::push_upload));

    acc.finish()
}
