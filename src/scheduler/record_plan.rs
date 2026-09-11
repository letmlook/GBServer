use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Notify, RwLock};
use chrono::{Datelike, Timelike, Utc};

use crate::db::{Pool, record_plan};
use crate::zlm::ZlmClient;

/// 全局唤醒句柄。
///
/// WVP 在 `RecordPlanServiceImpl.link()` 里**同步**调用一次 `execution()`，
/// 所以"关联通道"后立刻就开始录像。我们的调度是 60 秒一轮，若不唤醒，
/// 用户关联完通道最长要等一分钟才见效果（且看起来像没生效）。
static WAKE: OnceLock<Arc<Notify>> = OnceLock::new();

fn waker() -> Arc<Notify> {
    WAKE.get_or_init(|| Arc::new(Notify::new())).clone()
}

/// 请求录像计划调度器立刻执行一次 tick（关联/取消关联通道后调用）。
pub fn wake_record_plan_scheduler() {
    waker().notify_one();
}

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
    /// SIP 服务端句柄：录像计划必须能**自己拉起设备流**。
    ///
    /// 此前只对"已经存在的流"调 `startRecord`：如果没人正在看这个通道，
    /// ZLM 里根本没有该流，`startRecord` 直接报错（而且日志是 debug 级），
    /// 于是**录像计划永远不会产出文件** —— 而"无人值守也要录"正是它的用途。
    sip_server: Option<Arc<crate::sip::SipServer>>,
    /// 立即执行一次 tick 的唤醒源（见 [`wake_record_plan_scheduler`]）
    tick_now: Arc<Notify>,
}

impl RecordPlanScheduler {
    pub fn new(pool: Pool, zlm_client: Option<Arc<ZlmClient>>) -> Self {
        Self {
            pool,
            zlm_client,
            zlm_clients: HashMap::new(),
            active_recordings: Arc::new(RwLock::new(HashMap::new())),
            state_store: None,
            sip_server: None,
            tick_now: waker(),
        }
    }

    /// 注入 SIP 服务端（用于按需拉起设备流后再录像）。
    pub fn set_sip_server(&mut self, sip: Arc<crate::sip::SipServer>) {
        self.sip_server = Some(sip);
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
            tokio::select! {
                _ = interval.tick() => {}
                _ = self.tick_now.notified() => {
                    tracing::debug!("RecordPlanScheduler: 收到立即调度请求");
                }
            }
            if let Err(e) = self.tick().await {
                tracing::warn!("RecordPlanScheduler tick error: {}", e);
            }
        }
    }

    async fn tick(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // **必须用本地时间**：计划的 `week_day`/`start`/`stop` 表达的是
        // "本地时间的星期几 / 当天第几分钟"（前端按本地时间生成）。
        // 此前用 `Utc::now()`，在 UTC+8 部署下整条计划会**偏移 8 小时**。
        let now = chrono::Local::now();
        // WVP `RecordPlanServiceImpl.queryCurrentChannelRecord()` 用
        // `LocalDateTime.now().getDayOfWeek().getValue()`：ISO 口径，周一=1 … 周日=7
        let current_weekday = now.weekday().number_from_monday() as i32;
        // WVP 用 `now.getHour() * 60 + now.getMinute()`：**当天第几分钟**（0..1439），
        // 不是秒。早期实现按"当天第几秒"比对，任何计划都不可能命中。
        let current_minutes = now.hour() as i32 * 60 + now.minute() as i32;

        let plans = record_plan::list_paged(&self.pool, 1, 1000, None).await?;
        
        let mut channels_to_record: Vec<(i64, String, String, i32, Option<String>)> = Vec::new();

        for plan in &plans {
            let items = record_plan::list_items(&self.pool, plan.id as i64).await?;
            if !schedule_matches(&items, current_weekday, current_minutes) {
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

            // 先确保设备流存在：没人观看时 ZLM 里没有这个流，直接 startRecord
            // 只会失败（这正是"录像计划永远没有文件"的原因）。
            if let Some(ref sip) = self.sip_server {
                match sip.start_live_stream(&device_id, &gb_channel_id, 15).await {
                    Ok(sid) => tracing::info!(
                        "RecordPlanScheduler: 已按期拉起设备流 {}（channel={}）",
                        sid,
                        channel_id
                    ),
                    Err(e) => tracing::warn!(
                        "RecordPlanScheduler: 拉起设备流失败 {}/{}: {}（本次不录像）",
                        device_id,
                        gb_channel_id,
                        e
                    ),
                }
            } else {
                tracing::debug!(
                    "RecordPlanScheduler: 未注入 SIP 服务端，假定流已存在（{}）",
                    stream
                );
            }

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
                    // 用 warn：录像没起来是**功能没实现**，不该藏在 debug 里
                    tracing::warn!(
                        "RecordPlanScheduler: 启动录像失败 {}/{}: {}",
                        app,
                        stream,
                        e
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

/// 判断某个计划条目集合是否覆盖 `(weekday, minutes_of_day)`。
///
/// 口径与 WVP `RecordPlanMapper.queryRecordIng` 完全一致：
///
/// ```sql
/// where wrpi.week_day = #{week} and wrpi.start <= #{index} and stop >= #{index}
/// ```
///
/// * `weekday`：**ISO**，周一=1 … 周日=7（`number_from_monday()`）；
/// * `minutes`：**当天第几分钟**，0..1439（`hour*60 + minute`）；
/// * 区间**闭区间** `[start, stop]`（两端都含），不是秒、也不是半开区间。
///
/// 三个字段缺一不可：`start`/`stop`/`week_day` 任一为 `NULL` 的条目在
/// WVP 的 SQL 里也永远不匹配（`NULL <= x` 为 unknown），这里显式返回 false。
fn schedule_matches(items: &[record_plan::RecordPlanItem], weekday: i32, minutes: i32) -> bool {
    items.iter().any(|item| {
        let (Some(start), Some(stop), Some(day)) = (item.start, item.stop, item.week_day) else {
            return false;
        };
        day == weekday && start <= minutes && minutes <= stop
    })
}

#[cfg(test)]
mod schedule_tests {
    use super::*;

    fn item(start: i32, stop: i32, weekday: i32) -> record_plan::RecordPlanItem {
        record_plan::RecordPlanItem {
            id: 1,
            start: Some(start),
            stop: Some(stop),
            week_day: Some(weekday),
            plan_id: Some(1),
            create_time: None,
            update_time: None,
        }
    }

    #[test]
    fn matches_closed_window_in_minutes() {
        // 周二（ISO=2）10:00-11:00 → 600..660 分钟
        let items = vec![item(600, 660, 2)];
        assert!(schedule_matches(&items, 2, 600), "起点 10:00 应包含");
        assert!(schedule_matches(&items, 2, 659), "10:59 应包含");
        assert!(schedule_matches(&items, 2, 660), "终点 11:00 闭区间应包含（与 WVP stop >= index 一致）");
        assert!(!schedule_matches(&items, 2, 599), "09:59 不应包含");
        assert!(!schedule_matches(&items, 2, 661));
        // 周几不匹配（周一）
        assert!(!schedule_matches(&items, 1, 600));
    }

    #[test]
    fn seconds_are_not_minutes() {
        // 回归保护：早期实现拿"当天第几秒"去比分钟数，导致 10:00 的计划
        // 只有 00:10 那一瞬间能命中。600 秒 = 00:10 必须**不**匹配 10:00-11:00。
        let items = vec![item(600, 660, 2)];
        assert!(
            !schedule_matches(&items, 2, 600 / 60),
            "10 分钟（600 秒）不是起点"
        );
    }

    #[test]
    fn iso_weekday_monday_is_one() {
        // 周一（ISO=1）全天
        let items = vec![item(0, 1439, 1)];
        assert!(schedule_matches(&items, 1, 0));
        assert!(schedule_matches(&items, 1, 1439));
        // 周日 = 7
        assert!(!schedule_matches(&items, 7, 720));
        // 旧实现用 0..6（num_days_from_monday），周一=0；ISO 口径下 0 不是合法星期
        assert!(!schedule_matches(&items, 0, 720));
    }

    #[test]
    fn multiple_windows_any_match_wins() {
        let items = vec![item(0, 60, 1), item(120, 180, 1)];
        assert!(schedule_matches(&items, 1, 30));
        assert!(!schedule_matches(&items, 1, 90), "两个窗口之间不应匹配");
        assert!(schedule_matches(&items, 1, 120));
    }

    #[test]
    fn missing_fields_never_match() {
        let mut it = item(0, 1439, 1);
        it.week_day = None;
        assert!(!schedule_matches(&[it], 1, 100), "缺 week_day 不匹配任何一天");

        let mut it = item(0, 1439, 1);
        it.start = None;
        assert!(!schedule_matches(&[it], 1, 100), "缺 start 不匹配");

        let mut it = item(0, 1439, 1);
        it.stop = None;
        assert!(!schedule_matches(&[it], 1, 100), "缺 stop 不匹配");
    }

    #[test]
    fn full_day_window_covers_every_minute() {
        let items = vec![item(0, 1439, 3)];
        for m in [0, 1, 719, 1438, 1439] {
            assert!(schedule_matches(&items, 3, m), "第 {m} 分钟应命中全天窗");
        }
    }
}
