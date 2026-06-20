//! Phase 7.2: `ClusterRegistry` — Redis-backed 集群节点注册表。
//!
//! ## Redis 数据布局
//! - `gb:cluster:nodes` (SET)        — 所有曾出现过 + 在 TTL 内的节点
//! - `gb:cluster:heartbeat` (ZSET)   — 节点 → 最近心跳 unix_seconds 分数
//!
//! ## 调用约定
//! - `touch_node` 每 10s 由后台 task 调用一次，刷新本节点心跳
//! - `evict_expired` 每次 touch 后调用，删除 60s 未刷新的节点（同时从 SET + ZSET）
//! - `list_active_nodes` 任意线程调用，返回最近 60s 内有心跳的节点列表
//! - 在 `single_node_mode = true` 时，list_active_nodes 仅返本节点，不依赖 Redis

use std::sync::Arc;
use std::time::Duration;

use redis::aio::ConnectionManager;
use redis::AsyncCommands;

/// Phase 7.2: 单个集群节点的元数据。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClusterNode {
    pub node_id: String,
    pub addr: String,           // "http://10.0.0.5:8080"
    pub role: String,           // "primary" / "secondary"
    pub last_heartbeat_secs: i64,
}

/// Phase 7.2: 集群配置（来自 application.toml [cluster] 段落）。
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    pub enabled: bool,
    pub single_node_mode: bool,
    pub node_id: String,
    pub addr: String,
    pub role: String,
    pub heartbeat_interval: Duration,
    pub heartbeat_ttl: Duration,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            single_node_mode: true,
            node_id: format!("node-{}", uuid_like()),
            addr: "http://127.0.0.1:8080".to_string(),
            role: "primary".to_string(),
            heartbeat_interval: Duration::from_secs(10),
            heartbeat_ttl: Duration::from_secs(60),
        }
    }
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}

const KEY_NODES: &str = "gb:cluster:nodes";
const KEY_HEARTBEAT: &str = "gb:cluster:heartbeat";

/// Phase 7.2: 集群注册表。
pub struct ClusterRegistry {
    config: ClusterConfig,
    redis: Option<Arc<tokio::sync::Mutex<ConnectionManager>>>,
}

impl ClusterRegistry {
    /// Create a registry that talks to Redis. If `redis` is None, the registry
    /// is functional but always reports only the local node (single_node_mode).
    pub fn new(config: ClusterConfig, redis: Option<Arc<tokio::sync::Mutex<ConnectionManager>>>) -> Self {
        Self { config, redis }
    }

    pub fn config(&self) -> &ClusterConfig {
        &self.config
    }

    /// Phase 7.2: Touch this node's heartbeat in Redis. Safe to call when Redis
    /// is unavailable — just logs a warning.
    pub async fn touch(&self) {
        let Some(redis) = self.redis.as_ref() else {
            return;
        };
        let now = chrono::Utc::now().timestamp();
        let mut conn = redis.lock().await.clone();
        let _: Result<(), _> = redis::pipe()
            .atomic()
            .sadd(KEY_NODES, &self.config.node_id).ignore()
            .zadd(KEY_HEARTBEAT, &self.config.node_id, now).ignore()
            .query_async(&mut conn).await;
    }

    /// Phase 7.2: Evict nodes whose last heartbeat is older than ttl.
    pub async fn evict_expired(&self) -> Vec<String> {
        let Some(redis) = self.redis.as_ref() else {
            return Vec::new();
        };
        let cutoff = chrono::Utc::now().timestamp() - self.config.heartbeat_ttl.as_secs() as i64;
        let mut conn = redis.lock().await.clone();
        // ZRANGEBYSCORE 0 cutoff → 取过期节点
        let expired: Vec<String> = match conn.zrangebyscore(KEY_HEARTBEAT, 0, cutoff).await {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        if !expired.is_empty() {
            let _: Result<(), _> = redis::pipe()
                .atomic()
                .zrembyscore(KEY_HEARTBEAT, 0, cutoff).ignore()
                .srem(KEY_NODES, &expired).ignore()
                .query_async(&mut conn).await;
        }
        expired
    }

    /// Phase 7.2: List active cluster nodes (heartbeat within ttl).
    /// In single_node_mode without Redis, returns only the local node.
    pub async fn list_active(&self) -> Vec<ClusterNode> {
        if self.config.single_node_mode && self.redis.is_none() {
            return vec![self.local_node()];
        }
        let Some(redis) = self.redis.as_ref() else {
            return vec![self.local_node()];
        };
        let cutoff = chrono::Utc::now().timestamp() - self.config.heartbeat_ttl.as_secs() as i64;
        let mut conn = redis.lock().await.clone();
        let now = chrono::Utc::now().timestamp();
        let ids: Vec<String> = match conn.zrangebyscore(KEY_HEARTBEAT, cutoff, i64::MAX).await {
            Ok(v) => v,
            Err(_) => return vec![self.local_node()],
        };
        ids.into_iter().map(|id| ClusterNode {
            addr: if id == self.config.node_id { self.config.addr.clone() } else { String::new() },
            last_heartbeat_secs: now,
            node_id: id,
            role: self.config.role.clone(),
        }).collect()
    }

    pub fn local_node(&self) -> ClusterNode {
        ClusterNode {
            node_id: self.config.node_id.clone(),
            addr: self.config.addr.clone(),
            role: self.config.role.clone(),
            last_heartbeat_secs: chrono::Utc::now().timestamp(),
        }
    }

    /// Start the background heartbeat task. Returns a JoinHandle that can be aborted.
    pub fn start_heartbeat_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                self.touch().await;
                let _ = self.evict_expired().await;
                tokio::time::sleep(self.config.heartbeat_interval).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_config_default() {
        let cfg = ClusterConfig::default();
        assert!(cfg.single_node_mode);
        assert_eq!(cfg.heartbeat_interval, Duration::from_secs(10));
        assert_eq!(cfg.heartbeat_ttl, Duration::from_secs(60));
        assert!(!cfg.node_id.is_empty());
    }

    #[test]
    fn test_cluster_node_serde() {
        let n = ClusterNode {
            node_id: "node-1".into(),
            addr: "http://10.0.0.1:8080".into(),
            role: "primary".into(),
            last_heartbeat_secs: 1_700_000_000,
        };
        let s = serde_json::to_string(&n).unwrap();
        let back: ClusterNode = serde_json::from_str(&s).unwrap();
        assert_eq!(back.node_id, "node-1");
        assert_eq!(back.role, "primary");
    }

    #[tokio::test]
    async fn test_registry_without_redis_returns_local_node() {
        let cfg = ClusterConfig::default();
        let reg = ClusterRegistry::new(cfg, None);
        let active = reg.list_active().await;
        assert_eq!(active.len(), 1);
        assert!(!active[0].node_id.is_empty());
    }

    #[tokio::test]
    async fn test_registry_local_node_helper() {
        let cfg = ClusterConfig {
            node_id: "test-node".into(),
            addr: "http://localhost:9999".into(),
            role: "secondary".into(),
            ..Default::default()
        };
        let reg = ClusterRegistry::new(cfg, None);
        let n = reg.local_node();
        assert_eq!(n.node_id, "test-node");
        assert_eq!(n.addr, "http://localhost:9999");
        assert_eq!(n.role, "secondary");
    }

    #[tokio::test]
    async fn test_registry_evict_empty_when_no_redis() {
        let cfg = ClusterConfig::default();
        let reg = ClusterRegistry::new(cfg, None);
        let evicted = reg.evict_expired().await;
        assert!(evicted.is_empty());
    }

    #[test]
    fn test_touch_without_redis_is_noop() {
        // Synchronous check that touch() compiles and runs without redis.
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let cfg = ClusterConfig::default();
        let reg = ClusterRegistry::new(cfg, None);
        rt.block_on(async {
            reg.touch().await;
        });
    }
}
