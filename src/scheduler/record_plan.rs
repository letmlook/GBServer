use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Datelike, Timelike, Utc};

use crate::db::{Pool, record_plan};
use crate::zlm::ZlmClient;

#[derive(Debug, Clone)]
struct ActiveRecording {
    channel_id: i64,
    device_id: String,
    gb_channel_id: String,
    plan_id: i32,
    app: String,
    stream: String,
    media_server_id: String,
    started_at: chrono::DateTime<Utc>,
}

pub struct RecordPlanScheduler {
    pool: Pool,
    zlm_client: Option<Arc<ZlmClient>>,
    /// 多节点客户端表（media_server_id → client），优先于默认客户端
    zlm_clients: HashMap<String, Arc<ZlmClient>>,
    active_recordings: Arc<RwLock<HashMap<i64, ActiveRecording>>>,
    /// E1: 可选 StateStore（让 active 录像状态跨节点共享）
    state_store: Option<Arc<crate::state_store::StateStore>>,
}

impl RecordPlanScheduler {
    pub fn new(pool: Pool, zlm_client: Option<Arc<ZlmClient>>) -> Self {
        Self {
            pool,
            zlm_client,
            zlm_clients: HashMap::new(),
            active_recordings: Arc::new(RwLock::new(HashMap::new())),
            state_store: None,
        }
    }

    /// 注入多节点客户端表（media_server_id → client），用于按设备解析 ZLM 节点
    pub fn set_zlm_clients(&mut self, clients: HashMap<String, Arc<ZlmClient>>) {
        self.zlm_clients = clients;
    }

    /// E1: 注入 StateStore
    pub fn set_state_store(&mut self, store: Arc<crate::state_store::StateStore>) {
        self.state_store = Some(store);
    }

    /// 按设备的 media_server_id 解析目标 ZLM 节点。
    /// 'auto'/空/未知 → 默认客户端（返回 media_server_id = "auto"）。
    fn resolve_client(&self, media_server_id: Option<&str>) -> Option<(String, Arc<ZlmClient>)> {
        match media_server_id.map(str::trim).filter(|s| !s.is_empty() && *s != "auto") {
            Some(id) => {
                if let Some(client) = self.zlm_clients.get(id) {
                    return Some((id.to_string(), client.clone()));
                }
                // 指定的节点不在客户端表中 → 退回默认节点
                self.zlm_client.as_ref().map(|c| ("auto".to_string(), c.clone()))
            }
            None => self.zlm_client.as_ref().map(|c| ("auto".to_string(), c.clone())),
        }
    }

    /// E1: 返回当前 active 录像数（包含 StateStore 中的）
    pub async fn active_count(&self) -> usize {
        let local = self.active_recordings.read().await.len();
        if let Some(ref store) = self.state_store {
            store.active_recordings_count().max(local)
        } else {
            local
        }
    }

    pub async fn run(&self) {
        tracing::info!("RecordPlanScheduler started");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = self.tick().await {
                tracing::warn!("RecordPlanScheduler tick error: {}", e);
            }
        }
    }

    async fn tick(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let current_weekday = now.weekday().num_days_from_monday() as i32;
        let current_seconds = now.hour() as i32 * 3600 + now.minute() as i32 * 60 + now.second() as i32;

        let plans = record_plan::list_paged(&self.pool, 1, 1000).await?;
        
        let mut channels_to_record: Vec<(i64, String, String, i32, Option<String>)> = Vec::new();

        for plan in &plans {
            let items = record_plan::list_items(&self.pool, plan.id as i64).await?;
            let mut in_schedule = false;
            for item in &items {
                if item.week_day.unwrap_or(-1) != current_weekday {
                    continue;
                }
                let start = item.start.unwrap_or(0);
                let stop = item.stop.unwrap_or(0);
                if current_seconds >= start && current_seconds < stop {
                    in_schedule = true;
                    break;
                }
            }
            if !in_schedule {
                continue;
            }

            #[derive(sqlx::FromRow)]
            struct ChannelRow {
                id: i64,
                device_id: Option<String>,
                gb_device_id: Option<String>,
                media_server_id: Option<String>,
            }

            #[cfg(feature = "postgres")]
            let channels: Vec<ChannelRow> = sqlx::query_as(
                "SELECT c.id, c.device_id, c.gb_device_id, d.media_server_id \
                 FROM gb_device_channel c LEFT JOIN gb_device d ON c.device_id = d.device_id \
                 WHERE c.record_plan_id = $1",
            )
            .bind(plan.id)
            .fetch_all(&self.pool)
            .await?;

            #[cfg(any(feature = "mysql", feature = "sqlite"))]
            let channels: Vec<ChannelRow> = sqlx::query_as(
                "SELECT c.id, c.device_id, c.gb_device_id, d.media_server_id \
                 FROM gb_device_channel c LEFT JOIN gb_device d ON c.device_id = d.device_id \
                 WHERE c.record_plan_id = ?",
            )
            .bind(plan.id)
            .fetch_all(&self.pool)
            .await?;

            for ch in channels {
                let device_id = ch.device_id.unwrap_or_default();
                let gb_channel_id = ch.gb_device_id.unwrap_or_default();
                if !device_id.is_empty() && !gb_channel_id.is_empty() {
                    channels_to_record.push((ch.id, device_id, gb_channel_id, plan.id, ch.media_server_id));
                }
            }
        }

        // 只在持读锁期间做集合差，实际 ZLM 调用放在锁外逐个执行
        let to_start: Vec<(i64, String, String, i32, Option<String>)> = {
            let active = self.active_recordings.read().await;
            channels_to_record.iter()
                .filter(|(cid, _, _, _, _)| !active.contains_key(cid))
                .cloned()
                .collect()
        };

        // 依次启动本 tick 内所有到期且未在录的通道（单个失败不影响后续）
        for (channel_id, device_id, gb_channel_id, plan_id, ms_id) in to_start {
            let Some((resolved_ms_id, zlm)) = self.resolve_client(ms_id.as_deref()) else {
                continue;
            };
            let app = "rtp";
            let stream = format!("{}_{}", device_id, gb_channel_id);

            match zlm.start_record("1", "__defaultVhost__", app, &stream).await {
                Ok(_) => {
                    tracing::info!(
                        "RecordPlanScheduler: started MP4 recording for channel {} stream {}/{} (node {})",
                        channel_id, app, stream, resolved_ms_id
                    );
                    let recording = ActiveRecording {
                        channel_id,
                        device_id: device_id.clone(),
                        gb_channel_id: gb_channel_id.clone(),
                        plan_id,
                        app: app.to_string(),
                        stream: stream.clone(),
                        media_server_id: resolved_ms_id.clone(),
                        started_at: Utc::now(),
                    };
                    self.active_recordings.write().await.insert(channel_id, recording.clone());

                    // E1: 同步写入 StateStore（跨节点可见）
                    if let Some(ref store) = self.state_store {
                        use crate::state_store::ActiveRecordingState;
                        store.set_active_recording(channel_id, ActiveRecordingState {
                            channel_id: recording.channel_id,
                            device_id: recording.device_id.clone(),
                            gb_channel_id: recording.gb_channel_id.clone(),
                            plan_id: recording.plan_id,
                            app: recording.app.clone(),
                            stream: recording.stream.clone(),
                            media_server_id: recording.media_server_id.clone(),
                            started_at: recording.started_at,
                        });
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "RecordPlanScheduler: failed to start recording for {}/{}: {}",
                        app, stream, e
                    );
                }
            }
        }

        let mut active = self.active_recordings.write().await;
        let mut to_remove = Vec::new();
        for (channel_id, recording) in active.iter() {
            let still_needed = channels_to_record.iter()
                .any(|(cid, _, _, _, _)| cid == channel_id);
            if !still_needed {
                // 按启动时解析的节点停止，避免默认节点与实际节点不一致
                if let Some((_, zlm)) = self.resolve_client(Some(&recording.media_server_id)) {
                    let _ = zlm.stop_record("1", "__defaultVhost__", &recording.app, &recording.stream).await;
                }
                tracing::info!(
                    "RecordPlanScheduler: stopped MP4 recording for channel {} stream {}/{}",
                    channel_id, recording.app, recording.stream
                );
                to_remove.push(*channel_id);
            }
        }
        for cid in to_remove {
            active.remove(&cid);
            // E1: 同步从 StateStore 删除
            if let Some(ref store) = self.state_store {
                store.remove_active_recording(cid);
            }
        }

        Ok(())
    }
}
