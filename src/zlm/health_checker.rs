use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZlmServerStatus {
    Online,
    Offline,
    Unknown,
}

pub struct ZlmHealthChecker {
    check_interval_secs: u64,
    clients: Arc<RwLock<Vec<(String, Arc<crate::zlm::ZlmClient>, ZlmServerStatus)>>>,
    pool: Option<crate::db::Pool>,
}

impl ZlmHealthChecker {
    pub fn new(check_interval_secs: u64) -> Self {
        Self {
            check_interval_secs,
            clients: Arc::new(RwLock::new(Vec::new())),
            pool: None,
        }
    }

    pub fn set_pool(&mut self, pool: crate::db::Pool) {
        self.pool = Some(pool);
    }

    pub async fn add_client(&self, id: &str, client: Arc<crate::zlm::ZlmClient>) {
        let mut clients = self.clients.write().await;
        clients.push((id.to_string(), client, ZlmServerStatus::Unknown));
    }

    pub async fn check_all(&self) -> Vec<(String, ZlmServerStatus)> {
        let mut results = Vec::new();
        let mut clients = self.clients.write().await;

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
                    let _ = crate::db::media_server::update_status(pool, id, online, &now).await;
                }
            }

            results.push((id.clone(), new_status));
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
