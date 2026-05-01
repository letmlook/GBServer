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
    active_recordings: Arc<RwLock<HashMap<i64, ActiveRecording>>>,
}

impl RecordPlanScheduler {
    pub fn new(pool: Pool, zlm_client: Option<Arc<ZlmClient>>) -> Self {
        Self {
            pool,
            zlm_client,
            active_recordings: Arc::new(RwLock::new(HashMap::new())),
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
        
        let mut channels_to_record: Vec<(i64, String, String, i32)> = Vec::new();

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
            }

            #[cfg(feature = "postgres")]
            let channels: Vec<ChannelRow> = sqlx::query_as(
                "SELECT id, device_id, gb_device_id FROM wvp_device_channel WHERE record_plan_id = $1",
            )
            .bind(plan.id)
            .fetch_all(&self.pool)
            .await?;

            #[cfg(feature = "mysql")]
            let channels: Vec<ChannelRow> = sqlx::query_as(
                "SELECT id, device_id, gb_device_id FROM wvp_device_channel WHERE record_plan_id = ?",
            )
            .bind(plan.id)
            .fetch_all(&self.pool)
            .await?;

            for ch in channels {
                let device_id = ch.device_id.unwrap_or_default();
                let gb_channel_id = ch.gb_device_id.unwrap_or_default();
                if !device_id.is_empty() && !gb_channel_id.is_empty() {
                    channels_to_record.push((ch.id, device_id, gb_channel_id, plan.id));
                }
            }
        }

        let active = self.active_recordings.read().await;
        for (channel_id, device_id, gb_channel_id, plan_id) in &channels_to_record {
            if active.contains_key(channel_id) {
                continue;
            }

            if let Some(ref zlm) = self.zlm_client {
                let app = "rtp";
                let stream = format!("{}_{}", device_id, gb_channel_id);
                
                match zlm.start_record("1", "__defaultVhost__", app, &stream).await {
                    Ok(_) => {
                        tracing::info!(
                            "RecordPlanScheduler: started MP4 recording for channel {} stream {}/{}",
                            channel_id, app, stream
                        );
                        drop(active);
                        self.active_recordings.write().await.insert(*channel_id, ActiveRecording {
                            channel_id: *channel_id,
                            device_id: device_id.clone(),
                            gb_channel_id: gb_channel_id.clone(),
                            plan_id: *plan_id,
                            app: app.to_string(),
                            stream: stream.clone(),
                            media_server_id: "zlmediakit-1".to_string(),
                            started_at: Utc::now(),
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::debug!(
                            "RecordPlanScheduler: failed to start recording for {}/{}: {}",
                            app, stream, e
                        );
                    }
                }
            }
        }
        drop(active);

        let mut active = self.active_recordings.write().await;
        let mut to_remove = Vec::new();
        for (channel_id, recording) in active.iter() {
            let still_needed = channels_to_record.iter()
                .any(|(cid, _, _, _)| cid == channel_id);
            if !still_needed {
                if let Some(ref zlm) = self.zlm_client {
                    let _ = zlm.stop_record("1", "__defaultVhost__", &recording.app, &recording.stream).await;
                    tracing::info!(
                        "RecordPlanScheduler: stopped MP4 recording for channel {} stream {}/{}",
                        channel_id, recording.app, recording.stream
                    );
                }
                to_remove.push(*channel_id);
            }
        }
        for cid in to_remove {
            active.remove(&cid);
        }

        Ok(())
    }
}
