// ! StateStore — Redis-backed unified state abstraction
//!
//! 提供双模式存储：
//! - StateStore::in_memory()  — 无 Redis 时（开发/测试）
//! - StateStore::redis()       — 有 Redis 时（生产集群）
//!
//! 状态类别：设备在线/流状态/会话状态/GPS位置/告警/级联SendRtp/媒体节点负载

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use chrono::{DateTime, Utc};

// ---------------------------------------------------------------------------
// 状态数据模型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceOnlineState {
    pub online: bool,
    pub ip: String,
    pub port: u16,
    pub last_seen: DateTime<Utc>,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamState {
    pub app: String,
    pub stream_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub ssrc: Option<String>,
    pub call_id: Option<String>,
    pub media_server_id: String,
    pub online: bool,
    pub has_audio: bool,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InviteSessionState {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub session_type: String,
    pub zlm_stream_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaServerLoad {
    pub server_id: String,
    pub stream_count: i64,
    pub rtp_server_count: i32,
    pub online: bool,
    pub last_keepalive: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MobilePositionState {
    pub device_id: String,
    pub lat: f64,
    pub lon: f64,
    pub speed: Option<f64>,
    pub direction: Option<i32>,
    pub time: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActiveRecordingState {
    pub channel_id: i64,
    pub device_id: String,
    pub gb_channel_id: String,
    pub plan_id: i32,
    pub app: String,
    pub stream: String,
    pub media_server_id: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CascadeSendRtpState {
    pub cascade_call_id: String,
    pub platform_id: String,
    pub channel_id: String,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub active: bool,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum StateEvent {
    DeviceOnline(DeviceOnlineState),
    StreamChanged(StreamState),
    InviteSessionChanged(InviteSessionState),
    PositionChanged(MobilePositionState),
    CascadeSendRtpChanged(CascadeSendRtpState),
    MediaServerChanged(MediaServerLoad),
}

// ---------------------------------------------------------------------------
// StateBackend trait
// ---------------------------------------------------------------------------

pub trait StateBackend: Send + Sync {
    fn device_online_set(&self, id: &str, state: &DeviceOnlineState);
    fn device_online_get(&self, id: &str) -> Option<DeviceOnlineState>;
    fn device_online_all(&self) -> Vec<(String, DeviceOnlineState)>;

    fn stream_set(&self, id: &str, state: &StreamState);
    fn stream_get(&self, id: &str) -> Option<StreamState>;
    fn stream_del(&self, id: &str);
    fn stream_all(&self) -> Vec<(String, StreamState)>;

    fn invite_set(&self, id: &str, state: &InviteSessionState);
    fn invite_get(&self, id: &str) -> Option<InviteSessionState>;
    fn invite_del(&self, id: &str);

    fn media_server_set(&self, id: &str, state: &MediaServerLoad);
    fn media_server_get(&self, id: &str) -> Option<MediaServerLoad>;
    fn media_server_all(&self) -> Vec<(String, MediaServerLoad)>;
    fn media_server_select_least_loaded(&self) -> Option<String>;

    fn position_set(&self, id: &str, state: &MobilePositionState);
    fn position_get(&self, id: &str) -> Option<MobilePositionState>;

    fn cascade_sendrtp_set(&self, id: &str, state: &CascadeSendRtpState);
    fn cascade_sendrtp_get(&self, id: &str) -> Option<CascadeSendRtpState>;
    fn cascade_sendrtp_del(&self, id: &str);

    // E1: scheduler/record_plan active recordings
    fn active_recording_set(&self, channel_id: i64, state: &ActiveRecordingState);
    fn active_recording_get(&self, channel_id: i64) -> Option<ActiveRecordingState>;
    fn active_recording_del(&self, channel_id: i64);
    fn active_recordings_count(&self) -> usize;
}

// ---------------------------------------------------------------------------
// In-memory backend
// ---------------------------------------------------------------------------

pub struct InMemoryBackend {
    devices: RwLock<HashMap<String, DeviceOnlineState>>,
    streams: RwLock<HashMap<String, StreamState>>,
    invites: RwLock<HashMap<String, InviteSessionState>>,
    media_servers: RwLock<HashMap<String, MediaServerLoad>>,
    positions: RwLock<HashMap<String, MobilePositionState>>,
    cascade_sendrtp: RwLock<HashMap<String, CascadeSendRtpState>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self {
            devices: RwLock::new(HashMap::new()),
            streams: RwLock::new(HashMap::new()),
            invites: RwLock::new(HashMap::new()),
            media_servers: RwLock::new(HashMap::new()),
            positions: RwLock::new(HashMap::new()),
            cascade_sendrtp: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self { Self::new() }
}

impl StateBackend for InMemoryBackend {
    fn device_online_set(&self, id: &str, state: &DeviceOnlineState) {
        self.devices.blocking_write().insert(id.to_string(), state.clone());
    }
    fn device_online_get(&self, id: &str) -> Option<DeviceOnlineState> {
        self.devices.blocking_read().get(id).cloned()
    }
    fn device_online_all(&self) -> Vec<(String, DeviceOnlineState)> {
        self.devices.blocking_read().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    fn stream_set(&self, id: &str, state: &StreamState) {
        self.streams.blocking_write().insert(id.to_string(), state.clone());
    }
    fn stream_get(&self, id: &str) -> Option<StreamState> {
        self.streams.blocking_read().get(id).cloned()
    }
    fn stream_del(&self, id: &str) {
        self.streams.blocking_write().remove(id);
    }
    fn stream_all(&self) -> Vec<(String, StreamState)> {
        self.streams.blocking_read().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    fn invite_set(&self, id: &str, state: &InviteSessionState) {
        self.invites.blocking_write().insert(id.to_string(), state.clone());
    }
    fn invite_get(&self, id: &str) -> Option<InviteSessionState> {
        self.invites.blocking_read().get(id).cloned()
    }
    fn invite_del(&self, id: &str) {
        self.invites.blocking_write().remove(id);
    }

    fn media_server_set(&self, id: &str, state: &MediaServerLoad) {
        self.media_servers.blocking_write().insert(id.to_string(), state.clone());
    }
    fn media_server_get(&self, id: &str) -> Option<MediaServerLoad> {
        self.media_servers.blocking_read().get(id).cloned()
    }
    fn media_server_all(&self) -> Vec<(String, MediaServerLoad)> {
        self.media_servers.blocking_read().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    fn media_server_select_least_loaded(&self) -> Option<String> {
        self.media_servers.blocking_read()
            .iter()
            .filter(|(_, v)| v.online)
            .min_by_key(|(_, v)| v.stream_count)
            .map(|(k, _)| k.clone())
    }

    fn position_set(&self, id: &str, state: &MobilePositionState) {
        self.positions.blocking_write().insert(id.to_string(), state.clone());
    }
    fn position_get(&self, id: &str) -> Option<MobilePositionState> {
        self.positions.blocking_read().get(id).cloned()
    }

    fn cascade_sendrtp_set(&self, id: &str, state: &CascadeSendRtpState) {
        self.cascade_sendrtp.blocking_write().insert(id.to_string(), state.clone());
    }
    fn cascade_sendrtp_get(&self, id: &str) -> Option<CascadeSendRtpState> {
        self.cascade_sendrtp.blocking_read().get(id).cloned()
    }
    fn cascade_sendrtp_del(&self, id: &str) {
        self.cascade_sendrtp.blocking_write().remove(id);
    }

    // E1: scheduler/record_plan active recordings
    fn active_recording_set(&self, channel_id: i64, state: &ActiveRecordingState) {
        self.cascade_sendrtp.blocking_write(); // ensure lock ordering
        let mut recordings = self.cascade_sendrtp.blocking_write();
        recordings.insert(format!("rec:{}", channel_id), CascadeSendRtpState {
            cascade_call_id: format!("rec:{}", channel_id),
            platform_id: state.media_server_id.clone(),
            channel_id: format!("{}", channel_id),
            upstream_host: format!("{}/{}", state.app, state.stream),
            upstream_port: 0,
            active: true,
            started_at: state.started_at,
        });
    }
    fn active_recording_get(&self, channel_id: i64) -> Option<ActiveRecordingState> {
        let key = format!("rec:{}", channel_id);
        let recordings = self.cascade_sendrtp.blocking_read();
        recordings.get(&key).map(|_| ActiveRecordingState {
            channel_id,
            device_id: String::new(),
            gb_channel_id: String::new(),
            plan_id: 0,
            app: String::new(),
            stream: String::new(),
            media_server_id: String::new(),
            started_at: chrono::Utc::now(),
        })
    }
    fn active_recording_del(&self, channel_id: i64) {
        let key = format!("rec:{}", channel_id);
        self.cascade_sendrtp.blocking_write().remove(&key);
    }
    fn active_recordings_count(&self) -> usize {
        self.cascade_sendrtp.blocking_read()
            .iter()
            .filter(|(k, _)| k.starts_with("rec:"))
            .count()
    }
}

// ---------------------------------------------------------------------------
// Redis backend stub
// ---------------------------------------------------------------------------

/// Real Redis backend using `ConnectionManager` (auto-reconnects, multiplexed).
/// Since `StateBackend` trait methods are sync, we use `tokio::task::block_in_place`
/// + `Handle::current().block_on` to bridge into the async Redis API.
/// Each method gracefully no-ops if Redis is unavailable.
pub struct RedisBackend {
    url: String,
    manager: tokio::sync::RwLock<Option<redis::aio::ConnectionManager>>,
}

const KEY_PREFIX: &str = "gb:";
fn k_device(id: &str) -> String { format!("{}device:online:{}", KEY_PREFIX, id) }
fn k_stream(id: &str) -> String { format!("{}stream:{}", KEY_PREFIX, id) }
fn k_invite(id: &str) -> String { format!("{}invite:{}", KEY_PREFIX, id) }
fn k_ms(id: &str) -> String { format!("{}ms:load:{}", KEY_PREFIX, id) }
fn k_ms_count(server_id: &str) -> String { format!("{}ms:streams:{}", KEY_PREFIX, server_id) }
fn k_ms_zset() -> String { format!("{}ms:zset", KEY_PREFIX) }
fn k_position(id: &str) -> String { format!("{}position:{}", KEY_PREFIX, id) }
fn k_sendrtp(id: &str) -> String { format!("{}sendrtp:{}", KEY_PREFIX, id) }

impl RedisBackend {
    pub fn new(url: &str) -> Self {
        Self { url: url.to_string(), manager: tokio::sync::RwLock::new(None) }
    }

    async fn connect(&self) {
        if self.manager.read().await.is_some() { return; }
        let mut w = self.manager.write().await;
        if w.is_some() { return; }
        let client = match redis::Client::open(self.url.as_str()) {
            Ok(c) => c,
            Err(e) => { tracing::warn!("Redis Client::open failed: {}", e); return; }
        };
        // Bound the connect attempt so unreachable Redis fails fast (1.5s).
        let connect_fut = redis::aio::ConnectionManager::new(client);
        match tokio::time::timeout(std::time::Duration::from_millis(1500), connect_fut).await {
            Ok(Ok(mgr)) => { *w = Some(mgr); tracing::info!("Redis backend connected: {}", self.url); }
            Ok(Err(e)) => tracing::warn!("Redis ConnectionManager::new failed: {}", e),
            Err(_) => tracing::warn!("Redis connect timed out after 1.5s for {}", self.url),
        }
    }

    async fn get_conn(&self) -> Option<redis::aio::ConnectionManager> {
        if self.manager.read().await.is_none() {
            self.connect().await;
        }
        self.manager.read().await.clone()
    }
}

/// Bridge for reads: future returns Option<T>, helper returns Option<T> (None on no-runtime).
fn block_on_opt<F, T>(fut: F) -> Option<T>
where
    F: std::future::Future<Output = Option<T>>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => None,
    }
}

/// Bridge for writes: fire-and-forget, returns ().
fn block_on_run<F: std::future::Future<Output = ()>>(fut: F) {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(fut));
    }
}

impl StateBackend for RedisBackend {
    fn device_online_set(&self, id: &str, state: &DeviceOnlineState) {
        let key = k_device(id);
        let ttl = state.ttl_secs.max(1);
        let payload = match serde_json::to_string(state) {
            Ok(p) => p,
            Err(e) => { tracing::warn!("device_online_set serialize: {}", e); return; }
        };
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.set_ex(&key, &payload, ttl).await;
            }
        });
    }
    fn device_online_get(&self, id: &str) -> Option<DeviceOnlineState> {
        let key = k_device(id);
        let raw = block_on_opt::<_, String>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let v: Option<String> = conn.get(&key).await.ok()?;
            v
        })?;
        serde_json::from_str(&raw).ok()
    }
    fn device_online_all(&self) -> Vec<(String, DeviceOnlineState)> {
        let pattern = format!("{}device:online:*", KEY_PREFIX);
        block_on_opt::<_, Vec<(String, DeviceOnlineState)>>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let keys: Vec<String> = conn.keys(&pattern).await.ok()?;
            let mut out = Vec::with_capacity(keys.len());
            let prefix = format!("{}device:online:", KEY_PREFIX);
            for key in keys {
                let v: Option<String> = conn.get(&key).await.ok().flatten();
                if let Some(v) = v {
                    if let Ok(state) = serde_json::from_str::<DeviceOnlineState>(&v) {
                        let id = key.strip_prefix(&prefix).unwrap_or(&key).to_string();
                        out.push((id, state));
                    }
                }
            }
            Some(out)
        }).unwrap_or_default()
    }

    fn stream_set(&self, id: &str, state: &StreamState) {
        let key = k_stream(id);
        let payload = match serde_json::to_string(state) { Ok(p) => p, Err(_) => return };
        let ms = state.media_server_id.clone();
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.set(&key, &payload).await;
                let _: Result<(), _> = conn.incr(&k_ms_count(&ms), 1).await;
                let _: Result<(), _> = conn.sadd(&format!("{}ms:servers", KEY_PREFIX), &ms).await;
            }
        });
    }
    fn stream_get(&self, id: &str) -> Option<StreamState> {
        let key = k_stream(id);
        let raw = block_on_opt::<_, String>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let v: Option<String> = conn.get(&key).await.ok()?;
            v
        })?;
        serde_json::from_str(&raw).ok()
    }
    fn stream_del(&self, id: &str) {
        let key = k_stream(id);
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let prev: Option<String> = conn.get(&key).await.ok().flatten();
                let _: Result<(), _> = conn.del(&key).await;
                if let Some(p) = prev {
                    if let Ok(state) = serde_json::from_str::<StreamState>(&p) {
                        let _: Result<i64, _> = conn.decr(&k_ms_count(&state.media_server_id), 1).await;
                    }
                }
            }
        });
    }
    fn stream_all(&self) -> Vec<(String, StreamState)> {
        let pattern = format!("{}stream:*", KEY_PREFIX);
        block_on_opt::<_, Vec<(String, StreamState)>>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let keys: Vec<String> = conn.keys(&pattern).await.ok()?;
            let mut out = Vec::with_capacity(keys.len());
            let prefix = format!("{}stream:", KEY_PREFIX);
            for key in keys {
                let v: Option<String> = conn.get(&key).await.ok().flatten();
                if let Some(v) = v {
                    if let Ok(state) = serde_json::from_str::<StreamState>(&v) {
                        let id = key.strip_prefix(&prefix).unwrap_or(&key).to_string();
                        out.push((id, state));
                    }
                }
            }
            Some(out)
        }).unwrap_or_default()
    }

    fn invite_set(&self, id: &str, state: &InviteSessionState) {
        let key = k_invite(id);
        let payload = match serde_json::to_string(state) { Ok(p) => p, Err(_) => return };
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.set(&key, &payload).await;
            }
        });
    }
    fn invite_get(&self, id: &str) -> Option<InviteSessionState> {
        let key = k_invite(id);
        let raw = block_on_opt::<_, String>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let v: Option<String> = conn.get(&key).await.ok()?;
            v
        })?;
        serde_json::from_str(&raw).ok()
    }
    fn invite_del(&self, id: &str) {
        let key = k_invite(id);
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.del(&key).await;
            }
        });
    }

    fn media_server_set(&self, id: &str, state: &MediaServerLoad) {
        let key = k_ms(id);
        let payload = match serde_json::to_string(state) { Ok(p) => p, Err(_) => return };
        let count = state.stream_count;
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.set(&key, &payload).await;
                let _: Result<(), _> = conn.zadd(&k_ms_zset(), id, count).await;
                let _: Result<(), _> = conn.sadd(&format!("{}ms:servers", KEY_PREFIX), id).await;
            }
        });
    }
    fn media_server_get(&self, id: &str) -> Option<MediaServerLoad> {
        let key = k_ms(id);
        let raw = block_on_opt::<_, String>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let v: Option<String> = conn.get(&key).await.ok()?;
            v
        })?;
        serde_json::from_str(&raw).ok()
    }
    fn media_server_all(&self) -> Vec<(String, MediaServerLoad)> {
        let pattern = format!("{}ms:load:*", KEY_PREFIX);
        block_on_opt::<_, Vec<(String, MediaServerLoad)>>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let keys: Vec<String> = conn.keys(&pattern).await.ok()?;
            let mut out = Vec::with_capacity(keys.len());
            let prefix = format!("{}ms:load:", KEY_PREFIX);
            for key in keys {
                let v: Option<String> = conn.get(&key).await.ok().flatten();
                if let Some(v) = v {
                    if let Ok(state) = serde_json::from_str::<MediaServerLoad>(&v) {
                        let id = key.strip_prefix(&prefix).unwrap_or(&key).to_string();
                        out.push((id, state));
                    }
                }
            }
            Some(out)
        }).unwrap_or_default()
    }
    fn media_server_select_least_loaded(&self) -> Option<String> {
        block_on_opt::<_, String>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let pick: Vec<String> = conn.zrange(&k_ms_zset(), 0, 0).await.ok()?;
            pick.into_iter().next()
        })
    }

    fn position_set(&self, id: &str, state: &MobilePositionState) {
        let key = k_position(id);
        let payload = match serde_json::to_string(state) { Ok(p) => p, Err(_) => return };
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.set(&key, &payload).await;
                let _: Result<(), _> = conn.expire(&key, 60).await;
            }
        });
    }
    fn position_get(&self, id: &str) -> Option<MobilePositionState> {
        let key = k_position(id);
        let raw = block_on_opt::<_, String>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let v: Option<String> = conn.get(&key).await.ok()?;
            v
        })?;
        serde_json::from_str(&raw).ok()
    }

    fn cascade_sendrtp_set(&self, id: &str, state: &CascadeSendRtpState) {
        let key = k_sendrtp(id);
        let payload = match serde_json::to_string(state) { Ok(p) => p, Err(_) => return };
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.set(&key, &payload).await;
            }
        });
    }
    fn cascade_sendrtp_get(&self, id: &str) -> Option<CascadeSendRtpState> {
        let key = k_sendrtp(id);
        let raw = block_on_opt::<_, String>(async {
            use redis::AsyncCommands;
            let mut conn = self.get_conn().await?;
            let v: Option<String> = conn.get(&key).await.ok()?;
            v
        })?;
        serde_json::from_str(&raw).ok()
    }
    fn cascade_sendrtp_del(&self, id: &str) {
        let key = k_sendrtp(id);
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.del(&key).await;
            }
        });
    }

    fn active_recording_set(&self, channel_id: i64, state: &ActiveRecordingState) {
        let key = format!("wvp:recording:{}", channel_id);
        let value = match serde_json::to_string(state) {
            Ok(s) => s,
            Err(_) => return,
        };
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.set_ex(&key, value, 86400).await;
            }
        });
    }
    fn active_recording_get(&self, channel_id: i64) -> Option<ActiveRecordingState> {
        let key = format!("wvp:recording:{}", channel_id);
        let value: Option<String> = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    use redis::AsyncCommands;
                    match self.get_conn().await {
                        Some(mut conn) => conn.get(&key).await.unwrap_or(None),
                        None => None,
                    }
                })
            })
        } else {
            None
        };
        value.and_then(|s| serde_json::from_str(&s).ok())
    }
    fn active_recording_del(&self, channel_id: i64) {
        let key = format!("wvp:recording:{}", channel_id);
        block_on_run(async {
            use redis::AsyncCommands;
            if let Some(mut conn) = self.get_conn().await {
                let _: Result<(), _> = conn.del(&key).await;
            }
        });
    }
    fn active_recordings_count(&self) -> usize {
        let pattern = "wvp:recording:*";
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    use redis::AsyncCommands;
                    match self.get_conn().await {
                        Some(mut conn) => conn.keys::<_, Vec<String>>(pattern).await.unwrap_or_default(),
                        None => Vec::new(),
                    }
                })
            }).len()
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Unified StateStore
// ---------------------------------------------------------------------------

pub struct StateStore {
    backend: Arc<dyn StateBackend>,
    event_tx: broadcast::Sender<StateEvent>,
}

impl StateStore {
    pub fn in_memory() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { backend: Arc::new(InMemoryBackend::new()), event_tx: tx }
    }

    pub fn redis(url: &str) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { backend: Arc::new(RedisBackend::new(url)), event_tx: tx }
    }

    pub fn with_backend(backend: Arc<dyn StateBackend>) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { backend, event_tx: tx }
    }

    // Device
    pub fn set_device_online(&self, id: &str, state: DeviceOnlineState) {
        self.backend.device_online_set(id, &state);
        let _ = self.event_tx.send(StateEvent::DeviceOnline(state));
    }
    pub fn get_device_online(&self, id: &str) -> Option<DeviceOnlineState> {
        self.backend.device_online_get(id)
    }
    pub fn all_devices_online(&self) -> Vec<(String, DeviceOnlineState)> {
        self.backend.device_online_all()
    }

    // Streams
    pub fn set_stream(&self, id: &str, state: StreamState) {
        self.backend.stream_set(id, &state);
        let _ = self.event_tx.send(StateEvent::StreamChanged(state));
    }
    pub fn get_stream(&self, id: &str) -> Option<StreamState> {
        self.backend.stream_get(id)
    }
    pub fn remove_stream(&self, id: &str) {
        self.backend.stream_del(id);
    }
    pub fn all_streams(&self) -> Vec<(String, StreamState)> {
        self.backend.stream_all()
    }

    // Sessions
    pub fn set_invite_session(&self, id: &str, state: InviteSessionState) {
        self.backend.invite_set(id, &state);
        let _ = self.event_tx.send(StateEvent::InviteSessionChanged(state));
    }
    pub fn get_invite_session(&self, id: &str) -> Option<InviteSessionState> {
        self.backend.invite_get(id)
    }
    pub fn remove_invite_session(&self, id: &str) {
        self.backend.invite_del(id);
    }

    // Media servers
    pub fn set_media_server(&self, id: &str, state: MediaServerLoad) {
        self.backend.media_server_set(id, &state);
        let _ = self.event_tx.send(StateEvent::MediaServerChanged(state));
    }
    pub fn get_media_server(&self, id: &str) -> Option<MediaServerLoad> {
        self.backend.media_server_get(id)
    }
    pub fn all_media_servers(&self) -> Vec<(String, MediaServerLoad)> {
        self.backend.media_server_all()
    }
    pub fn select_least_loaded_server(&self) -> Option<String> {
        self.backend.media_server_select_least_loaded()
    }

    // Position
    pub fn set_position(&self, id: &str, state: MobilePositionState) {
        self.backend.position_set(id, &state);
        let _ = self.event_tx.send(StateEvent::PositionChanged(state));
    }
    pub fn get_position(&self, id: &str) -> Option<MobilePositionState> {
        self.backend.position_get(id)
    }

    // Cascade SendRtp
    pub fn set_cascade_sendrtp(&self, id: &str, state: CascadeSendRtpState) {
        self.backend.cascade_sendrtp_set(id, &state);
        let _ = self.event_tx.send(StateEvent::CascadeSendRtpChanged(state));
    }
    pub fn get_cascade_sendrtp(&self, id: &str) -> Option<CascadeSendRtpState> {
        self.backend.cascade_sendrtp_get(id)
    }
    pub fn remove_cascade_sendrtp(&self, id: &str) {
        self.backend.cascade_sendrtp_del(id);
    }

    // E1: scheduler/record_plan active recordings
    pub fn set_active_recording(&self, channel_id: i64, state: ActiveRecordingState) {
        self.backend.active_recording_set(channel_id, &state);
    }
    pub fn get_active_recording(&self, channel_id: i64) -> Option<ActiveRecordingState> {
        self.backend.active_recording_get(channel_id)
    }
    pub fn remove_active_recording(&self, channel_id: i64) {
        self.backend.active_recording_del(channel_id);
    }
    pub fn active_recordings_count(&self) -> usize {
        self.backend.active_recordings_count()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StateEvent> {
        self.event_tx.subscribe()
    }
}

impl Default for StateStore {
    fn default() -> Self { Self::in_memory() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_online() {
        let store = StateStore::in_memory();
        store.set_device_online("dev1", DeviceOnlineState {
            online: true, ip: "10.0.0.1".into(), port: 5060,
            last_seen: Utc::now(), ttl_secs: 60,
        });
        let s = store.get_device_online("dev1").unwrap();
        assert!(s.online);
        assert_eq!(s.ip, "10.0.0.1");
    }

    #[test]
    fn test_least_loaded_server() {
        let store = StateStore::in_memory();
        store.set_media_server("zlm-a", MediaServerLoad {
            server_id: "zlm-a".into(), stream_count: 100, rtp_server_count: 20,
            online: true, last_keepalive: Utc::now(),
        });
        store.set_media_server("zlm-b", MediaServerLoad {
            server_id: "zlm-b".into(), stream_count: 5, rtp_server_count: 1,
            online: true, last_keepalive: Utc::now(),
        });
        store.set_media_server("zlm-c", MediaServerLoad {
            server_id: "zlm-c".into(), stream_count: 50, rtp_server_count: 10,
            online: false, last_keepalive: Utc::now(),
        });
        // zlm-c offline, zlm-b has fewest streams
        assert_eq!(store.select_least_loaded_server().unwrap(), "zlm-b");
    }

    #[test]
    fn test_stream_lifecycle() {
        let store = StateStore::in_memory();
        store.set_stream("s1", StreamState {
            app: "rtp".into(), stream_id: "s1".into(),
            device_id: "dev1".into(), channel_id: "ch1".into(),
            ssrc: Some("0100000001".into()), call_id: Some("call-1".into()),
            media_server_id: "zlm-1".into(), online: true, has_audio: true,
            last_activity: Utc::now(),
        });
        assert_eq!(store.all_streams().len(), 1);
        store.remove_stream("s1");
        assert!(store.get_stream("s1").is_none());
    }

    #[test]
    fn test_cascade_sendrtp() {
        let store = StateStore::in_memory();
        store.set_cascade_sendrtp("cascade-001", CascadeSendRtpState {
            cascade_call_id: "cascade-001".into(), platform_id: "plat-1".into(),
            channel_id: "ch1".into(), upstream_host: "10.0.0.100".into(),
            upstream_port: 30000, active: true, started_at: Utc::now(),
        });
        let s = store.get_cascade_sendrtp("cascade-001").unwrap();
        assert_eq!(s.upstream_port, 30000);
        store.remove_cascade_sendrtp("cascade-001");
        assert!(store.get_cascade_sendrtp("cascade-001").is_none());
    }
}


#[cfg(test)]
mod redis_backend_tests {
    use super::*;
    use crate::state_store::*;

    fn make_device_state(id: &str) -> DeviceOnlineState {
        DeviceOnlineState {
            online: true,
            ip: "127.0.0.1".to_string(),
            port: 5060,
            last_seen: chrono::Utc::now(),
            ttl_secs: 60,
        }
    }

    fn make_stream_state(server: &str) -> StreamState {
        StreamState {
            app: "live".to_string(),
            stream_id: "ch001".to_string(),
            device_id: "34020000002000000001".to_string(),
            channel_id: "34020000002000000002".to_string(),
            ssrc: Some("0x1234".to_string()),
            call_id: Some("call-1".to_string()),
            media_server_id: server.to_string(),
            online: true,
            has_audio: true,
            last_activity: chrono::Utc::now(),
        }
    }

    fn make_media_server(id: &str, count: i64) -> MediaServerLoad {
        MediaServerLoad {
            server_id: id.to_string(),
            stream_count: count,
            rtp_server_count: 5,
            online: true,
            last_keepalive: chrono::Utc::now(),
        }
    }

    /// All state structs serialize and round-trip via serde_json (so RedisBackend can persist them).
    #[test]
    fn test_device_online_state_serde_roundtrip() {
        let s = make_device_state("dev-1");
        let j = serde_json::to_string(&s).expect("serialize");
        let back: DeviceOnlineState = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back.ip, "127.0.0.1");
        assert_eq!(back.port, 5060);
        assert!(back.online);
    }

    #[test]
    fn test_stream_state_serde_roundtrip() {
        let s = make_stream_state("zlm-a");
        let j = serde_json::to_string(&s).expect("serialize");
        let back: StreamState = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back.app, "live");
        assert_eq!(back.media_server_id, "zlm-a");
        assert_eq!(back.ssrc.as_deref(), Some("0x1234"));
    }

    #[test]
    fn test_media_server_load_serde_roundtrip() {
        let s = make_media_server("zlm-b", 42);
        let j = serde_json::to_string(&s).expect("serialize");
        let back: MediaServerLoad = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back.stream_count, 42);
        assert!(back.online);
    }

    /// Key builders produce namespaced keys with the gb: prefix.
    #[test]
    fn test_key_prefixes_are_namespaced() {
        assert!(k_device("abc").starts_with("gb:device:online:"));
        assert!(k_stream("abc").starts_with("gb:stream:"));
        assert!(k_invite("abc").starts_with("gb:invite:"));
        assert!(k_ms("abc").starts_with("gb:ms:load:"));
        assert!(k_ms_count("abc").starts_with("gb:ms:streams:"));
        assert!(k_position("abc").starts_with("gb:position:"));
        assert!(k_sendrtp("abc").starts_with("gb:sendrtp:"));
        assert!(k_ms_zset().starts_with("gb:ms:zset"));
    }

    /// RedisBackend constructs without panic even with a bad URL (connection is lazy).
    #[test]
    fn test_redis_backend_construction_does_not_panic() {
        let _ = RedisBackend::new("redis://127.0.0.1:1");
        let _ = RedisBackend::new("redis://invalid-host:6379");
    }

    /// RedisBackend with unreachable Redis: all calls are no-ops (no panic, no error return).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_redis_backend_unreachable_is_noop() {
        let backend = RedisBackend::new("redis://127.0.0.1:1");

        let s = make_device_state("dev-x");
        backend.device_online_set("dev-x", &s);
        assert!(backend.device_online_get("dev-x").is_none());
        assert_eq!(backend.device_online_all().len(), 0);

        let ss = make_stream_state("zlm-a");
        backend.stream_set("stream-x", &ss);
        assert!(backend.stream_get("stream-x").is_none());
        backend.stream_del("stream-x");
        assert_eq!(backend.stream_all().len(), 0);

        let inv = InviteSessionState {
            call_id: "c1".to_string(), device_id: "d".to_string(),
            channel_id: "ch".to_string(), session_type: "play".to_string(),
            zlm_stream_id: Some("stream-x".to_string()), status: "active".to_string(),
            created_at: chrono::Utc::now(), last_activity: chrono::Utc::now(),
        };
        backend.invite_set("inv-x", &inv);
        assert!(backend.invite_get("inv-x").is_none());
        backend.invite_del("inv-x");

        let ms = make_media_server("zlm-a", 3);
        backend.media_server_set("zlm-a", &ms);
        assert!(backend.media_server_get("zlm-a").is_none());
        assert!(backend.media_server_select_least_loaded().is_none());
        assert_eq!(backend.media_server_all().len(), 0);

        let pos = MobilePositionState {
            device_id: "d".to_string(), lat: 31.0, lon: 121.0,
            speed: Some(10.0), direction: Some(90), time: "2026-06-10T12:00:00".to_string(),
        };
        backend.position_set("d", &pos);
        assert!(backend.position_get("d").is_none());

        let rt = CascadeSendRtpState {
            cascade_call_id: "rt-1".to_string(), platform_id: "p".to_string(),
            channel_id: "ch".to_string(), upstream_host: "127.0.0.1".to_string(),
            upstream_port: 9000, active: true, started_at: chrono::Utc::now(),
        };
        backend.cascade_sendrtp_set("rt-1", &rt);
        assert!(backend.cascade_sendrtp_get("rt-1").is_none());
        backend.cascade_sendrtp_del("rt-1");
    }

    /// StateStore::redis() constructs without panic even with bad URL.
    #[test]
    fn test_state_store_redis_constructs_without_panic() {
        let _ = StateStore::redis("redis://127.0.0.1:1");
    }
}
