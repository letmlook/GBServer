use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZlmServerStatus {
    Online,
    Offline,
    Unknown,
}

pub struct ZlmHealthChecker {
    /// media_server_id → 该节点的 hook 回调地址（来自配置；缺省用本机地址）。
    ///
    /// 节点**上线时**要重新下发 hook 配置：ZLM 容器重启会丢掉运行期改过的
    /// hook 项（镜像里的 config.ini 是 `hook.enable=0`），而"等 ZLM 自己发
    /// on_server_started"这条路在 hook 被关掉时根本不会发生（鸡生蛋）。
    /// 后端自己重启后同样需要把配置补回去。
    hook_urls: std::collections::HashMap<String, String>,
    check_interval_secs: u64,
    clients: Arc<RwLock<Vec<(String, Arc<crate::zlm::ZlmClient>, ZlmServerStatus)>>>,
    pool: Option<crate::db::Pool>,
}

/// `general.mediaServerId` 与本平台登记的节点主键是否不一致。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DriftKind {
    /// 值存在但与本平台登记的不一致
    Mismatch(String),
    /// 键缺失（或被清空）
    Missing,
}

/// 核对节点配置里的 `general.mediaServerId`。
///
/// 该键决定 ZLM 在每个 hook 请求里带什么 `mediaServerId`：不一致时后端认不出事件
/// 来源、只能"回落到默认节点"，多节点部署下会把事件记到错误的节点上。
pub(crate) fn media_server_id_drift(
    expected: &str,
    cfg: &std::collections::HashMap<String, String>,
) -> Option<DriftKind> {
    match cfg.get("general.mediaServerId").map(|s| s.trim()) {
        None | Some("") => Some(DriftKind::Missing),
        Some(v) if v != expected => Some(DriftKind::Mismatch(v.to_string())),
        Some(_) => None,
    }
}

impl ZlmHealthChecker {
    pub fn new(check_interval_secs: u64) -> Self {
        Self {
            hook_urls: std::collections::HashMap::new(),
            check_interval_secs,
            clients: Arc::new(RwLock::new(Vec::new())),
            pool: None,
        }
    }

    pub fn set_pool(&mut self, pool: crate::db::Pool) {
        self.pool = Some(pool);
    }

    /// 注入各节点的 hook 回调地址（lib.rs 启动时按配置填充）。
    pub fn set_hook_urls(&mut self, urls: std::collections::HashMap<String, String>) {
        self.hook_urls = urls;
    }

    pub async fn add_client(&self, id: &str, client: Arc<crate::zlm::ZlmClient>) {
        let mut clients = self.clients.write().await;
        clients.push((id.to_string(), client, ZlmServerStatus::Unknown));
    }

    pub async fn check_all(&self) -> Vec<(String, ZlmServerStatus)> {
        let mut results = Vec::new();
        let mut clients = self.clients.write().await;
        // 需要重新下发 hook 配置的节点（锁外再做网络调用，避免长时间持有写锁）
        let mut reconfigure: Vec<(String, Arc<crate::zlm::ZlmClient>)> = Vec::new();

        for (id, client, status) in clients.iter_mut() {
            // 探活同时拿配置：这样可以在**不额外发请求**的前提下核对
            // `general.mediaServerId` 是否仍与本平台登记的节点主键一致。
            //
            // 该键决定 ZLM 在每个 hook 请求里带什么 `mediaServerId`。此前只在
            // "节点上线"那一次下发，一旦有人在 ZLM 侧手工改掉它（或节点一直在线
            // 从未发生状态跃迁），后端收到的事件就会带着一个认不出的 id，
            // 于是所有事件都"回落到默认节点" —— 多节点部署下会把事件记到错误的
            // 节点上，而日志里看不出异常。现在每次探活都会发现并纠正。
            let mut id_drift = false;
            let new_status = match client.get_server_config().await {
                Ok(cfg) => {
                    match media_server_id_drift(id, &cfg) {
                        Some(DriftKind::Mismatch(v)) => {
                            id_drift = true;
                            tracing::warn!(
                                "ZLM 节点 {} 的 general.mediaServerId 被改成了 {:?}，将重新对齐",
                                id,
                                v
                            );
                        }
                        Some(DriftKind::Missing) => {
                            id_drift = true;
                            tracing::warn!(
                                "ZLM 节点 {} 缺少 general.mediaServerId，将重新下发 {}",
                                id,
                                id
                            );
                        }
                        None => {}
                    }
                    ZlmServerStatus::Online
                }
                Err(_) => ZlmServerStatus::Offline,
            };

            // 与 last_keepalive_time 的比较/展示口径保持本地时区
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

            if new_status == ZlmServerStatus::Online {
                // **探活成功就是"这个节点活着"**，必须刷新 `last_keepalive_time`。
                //
                // 平台里有两条独立的健康判定：这条主动探活（每 10s HTTP 一次）和
                // `media_node` 的被动判定（拿 `last_keepalive_time` 与 now-timeout 比）。
                // 此前只有状态**变化**时才写库，于是当「ZLM → 平台」的 hook 通路不通时
                // （NAT / 反向代理 / `host.docker.internal` 解析异常都很常见），
                // 被动判定会因为时间戳不再更新而把**明明可以连通的节点**标记 offline，
                // `online/list`、节点选择与负载均衡随之全部失效。
                if let Some(ref pool) = self.pool {
                    if let Err(e) =
                        crate::db::media_server::update_last_keepalive(pool, id, &now).await
                    {
                        tracing::error!("ZLM 节点 {} 心跳刷新失败: {}", id, e);
                    }
                }
            }

            if *status != new_status {
                tracing::info!("ZLM server {} status changed: {:?} -> {:?}", id, status, new_status);
                *status = new_status;

                if let Some(ref pool) = self.pool {
                    if new_status == ZlmServerStatus::Offline {
                        let online = false;
                        if let Err(e) =
                            crate::db::media_server::update_status(pool, id, online, &now).await
                        {
                            tracing::error!(
                                "ZLM 节点 {} 状态写库失败 (online={}): {}",
                                id, online, e
                            );
                        }
                    }
                }
                if new_status == ZlmServerStatus::Online {
                    reconfigure.push((id.clone(), client.clone()));
                }
            }

            if id_drift && new_status == ZlmServerStatus::Online && !reconfigure.iter().any(|(rid, _)| rid == id) {
                reconfigure.push((id.clone(), client.clone()));
            }

            results.push((id.clone(), new_status));
        }
        drop(clients);

        // 节点上线：把 hook 配置补回去（否则 ZLM 的 hook 永远是关的，
        // 平台侧收不到任何"流已就绪/无人观看/录像完成"事件）
        for (id, client) in reconfigure {
            let Some(hook_url) = self.hook_urls.get(&id) else {
                tracing::warn!(
                    "ZLM 节点 {id} 上线，但配置里没有 hook_url，跳过 hook 下发"
                );
                continue;
            };
            // 节点身份：ZLM 在每个 hook 请求里带上 `mediaServerId`，值取自
            // `general.mediaServerId`。镜像默认是占位串 "your_server_id"，
            // 不改的话后端每次都要"按未知 id 回落到默认节点"，
            // 多节点部署时事件会被记到错误的节点上。
            match client
                .set_server_config_verified(&client.secret, "general.mediaServerId", &id)
                .await
            {
                Ok(true) => tracing::info!("ZLM 节点 {id} 的 general.mediaServerId 已对齐"),
                Ok(false) => tracing::warn!(
                    "ZLM 节点 {id} 的 general.mediaServerId 未生效（该版本键名可能不同）"
                ),
                Err(e) => tracing::warn!("ZLM 节点 {id} 下发 mediaServerId 失败: {e}"),
            }

            let items = crate::zlm::hook::hook_config_items(hook_url, &client.secret);
            let mut failed = 0usize;
            let mut unsupported: Vec<String> = Vec::new();
            for (key, value) in &items {
                match client.set_server_config_verified(&client.secret, key, value).await {
                    Ok(true) => {}
                    Ok(false) => unsupported.push(key.clone()),
                    Err(_) => failed += 1,
                }
            }
            if !unsupported.is_empty() {
                // 该 ZLM 版本没有这个事件键 —— 明确列出，而不是当成成功
                tracing::warn!(
                    "ZLM 节点 {id} 不支持以下 hook 键（本版本 config.ini 里没有）：{}",
                    unsupported.join(", ")
                );
            }
            tracing::info!(
                "ZLM 节点 {id} 上线，hook 配置已下发：{}/{} 生效，{failed} 项失败（{hook_url}）",
                items.len() - failed - unsupported.len(),
                items.len()
            );

            // 收流端口范围同样要在节点上线时补回去：容器里 config.ini 的
            // rtp_proxy.port_range 默认 30000-35000，而 compose 只发布了
            // 30000-30100，落在窗口外的端口收不到设备的 RTP。
            if let Some(ref pool) = self.pool {
                match crate::db::media_server::get_media_server_by_id(pool, &id).await {
                    Ok(Some(sv)) => {
                        if let Some(ref range) = sv.rtp_port_range {
                            match crate::zlm::client::set_rtp_port_range_verified(
                                &client,
                                &client.secret,
                                &["rtp_proxy.port_range", "rtp.port_range"],
                                range,
                            )
                            .await
                            {
                                Ok(key) => tracing::info!("ZLM 节点 {id} 的 {key} 已设为 {range}"),
                                Err(e) => tracing::warn!("ZLM 节点 {id} 端口范围下发失败: {e}"),
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::warn!("读取媒体节点 {id} 的端口范围失败: {e}"),
                }
            }
        }

        results
    }

    pub async fn get_online_clients(&self) -> Vec<(String, Arc<crate::zlm::ZlmClient>)> {
        let clients = self.clients.read().await;
        clients.iter()
            .filter(|(_, _, status)| *status == ZlmServerStatus::Online)
            .map(|(id, client, _)| (id.clone(), client.clone()))
            .collect()
    }

    pub async fn run_health_check_loop(&self) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.check_interval_secs)
        );

        loop {
            interval.tick().await;
            let results = self.check_all().await;
            for (id, status) in results {
                if status == ZlmServerStatus::Offline {
                    tracing::warn!("ZLM server {} is offline", id);
                }
            }
        }
    }
}

#[cfg(test)]
mod media_server_id_tests {
    use super::*;
    use std::collections::HashMap;

    /// 每次探活都会核对 `general.mediaServerId`：被手工改掉要能发现并纠正。
    #[test]
    fn detects_media_server_id_drift() {
        let mut cfg = HashMap::new();
        cfg.insert("general.mediaServerId".to_string(), "zlmediakit-1".to_string());
        assert_eq!(media_server_id_drift("zlmediakit-1", &cfg), None);

        cfg.insert("general.mediaServerId".to_string(), "hacked".to_string());
        assert_eq!(
            media_server_id_drift("zlmediakit-1", &cfg),
            Some(DriftKind::Mismatch("hacked".to_string()))
        );

        // 空串按缺失处理
        cfg.insert("general.mediaServerId".to_string(), "  ".to_string());
        assert_eq!(
            media_server_id_drift("zlmediakit-1", &cfg),
            Some(DriftKind::Missing)
        );

        cfg.remove("general.mediaServerId");
        assert_eq!(
            media_server_id_drift("zlmediakit-1", &cfg),
            Some(DriftKind::Missing)
        );
    }
}
