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
            let new_status = match client.get_server_config().await {
                Ok(_) => ZlmServerStatus::Online,
                Err(_) => ZlmServerStatus::Offline,
            };

            if *status != new_status {
                tracing::info!("ZLM server {} status changed: {:?} -> {:?}", id, status, new_status);
                *status = new_status;

                if let Some(ref pool) = self.pool {
                    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    let online = new_status == ZlmServerStatus::Online;
                    if let Err(e) =
                        crate::db::media_server::update_status(pool, id, online, &now).await
                    {
                        tracing::error!(
                            "ZLM 节点 {} 状态写库失败 (online={}): {}",
                            id, online, e
                        );
                    }
                }
                if new_status == ZlmServerStatus::Online {
                    reconfigure.push((id.clone(), client.clone()));
                }
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
