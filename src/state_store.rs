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

#[derive(Debug, Clone)]
pub struct DeviceOnlineState {
    pub online: bool,
    pub ip: String,
    pub port: u16,
    pub last_seen: DateTime<Utc>,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct MediaServerLoad {
    pub server_id: String,
    pub stream_count: i64,
    pub rtp_server_count: i32,
    pub online: bool,
    pub last_keepalive: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MobilePositionState {
    pub device_id: String,
    pub lat: f64,
    pub lon: f64,
    pub speed: Option<f64>,
    pub direction: Option<i32>,
    pub time: String,
}

#[derive(Debug, Clone)]
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

macro_rules! impl_state_backend {
    ($($name:ident: $t:ty),*) => {$(
        fn $name(&self, id: &str, state: &$t) {
            self.$name.blocking_write().insert(id.to_string(), state.clone());
        }
        fn $name"_get(&self, id: &str) -> Option<$t> {
            self.$name.blocking_read().get(id).cloned()
        }
        fn $name"_all(&self) -> Vec<(String, $t)> {
            self.$name.blocking_read().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        }
    )*}
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
}

// ---------------------------------------------------------------------------
// Redis backend stub
// ---------------------------------------------------------------------------

pub struct RedisBackend {
    url: String,
}

impl RedisBackend {
    pub fn new(url: &str) -> Self { Self { url: url.to_string() } }
}

impl StateBackend for RedisBackend {
    fn device_online_set(&self, _id: &str, _s: &DeviceOnlineState) {}
    fn device_online_get(&self, _id: &str) -> Option<DeviceOnlineState> { None }
    fn device_online_all(&self) -> Vec<(String, DeviceOnlineState)> { Vec::new() }
    fn stream_set(&self, _id: &str, _s: &StreamState) {}
    fn stream_get(&self, _id: &str) -> Option<StreamState> { None }
    fn stream_del(&self, _id: &str) {}
    fn stream_all(&self) -> Vec<(String, StreamState)> { Vec::new() }
    fn invite_set(&self, _id: &str, _s: &InviteSessionState) {}
    fn invite_get(&self, _id: &str) -> Option<InviteSessionState> { None }
    fn invite_del(&self, _id: &str) {}
    fn media_server_set(&self, _id: &str, _s: &MediaServerLoad) {}
    fn media_server_get(&self, _id: &str) -> Option<MediaServerLoad> { None }
    fn media_server_all(&self) -> Vec<(String, MediaServerLoad)> { Vec::new() }
    fn media_server_select_least_loaded(&self) -> Option<String> { None }
    fn position_set(&self, _id: &str, _s: &MobilePositionState) {}
    fn position_get(&self, _id: &str) -> Option<MobilePositionState> { None }
    fn cascade_sendrtp_set(&self, _id: &str, _s: &CascadeSendRtpState) {}
    fn cascade_sendrtp_get(&self, _id: &str) -> Option<CascadeSendRtpState> { None }
    fn cascade_sendrtp_del(&self, _id: &str) {}
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
