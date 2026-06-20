//! Phase 7.2: 集群节点发现 + 心跳。
//!
//! 设计：
//! - `ClusterRegistry` 封装 Redis SET（活跃节点）+ ZSET（心跳时间戳）
//! - `start_heartbeat` 每 10s 刷新一次本节点心跳；超过 60s 未刷新的节点视为离线
//! - `list_active_nodes` 用 ZRANGEBYSCORE 过滤存活节点
//! - 在生产部署中，Redis 是 Phase 7 强依赖；单节点模式下跳过集群检查
//!   （通过 `config.cluster.single_node_mode = true`）

pub mod registry;

pub use registry::{ClusterConfig, ClusterNode, ClusterRegistry};
