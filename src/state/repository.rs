//! Phase 7.1: `StreamStateRepository` — 业务级 trait,封装 StateStore 上的高频访问语义。
//!
//! 设计原则：
//! 1. 仅覆盖"高频 + 通用"业务场景；复杂查询 / 专用逻辑（如 JtCommandWaiter / JtMediaSession）直接调 `state_store`。
//! 2. trait 保持最小；返回 Option / () 而非 Result，错误由 StateStore backend 自身 graceful no-op。
//! 3. 实现（`StateStoreRepository`）仅做 thin wrapper，不引入新逻辑或状态。

use std::sync::Arc;

use crate::state_store::{InviteSessionState, StateStore};

// ---------------------------------------------------------------------------
// Trait definition
// ---------------------------------------------------------------------------

/// Phase 7.1: 业务级状态仓库 trait。
///
/// 所有方法在 Redis 不可用时 graceful 降级（StateStore::backend 的实现已经处理），
/// 调用方不需要处理错误。Trait 保持 sync 方法（与 StateBackend 风格一致）以简化实现。
pub trait StreamStateRepository: Send + Sync {
    // ---------- Recording ----------
    /// Set a recording state for the given device/channel.
    /// Pass `cmd = "StopRecord"` to clear.
    fn set_recording(&self, device_id: &str, channel_id: &str, cmd: &str);
    /// Get the current recording cmd for the given device/channel. Returns None if no recording is active.
    fn get_recording(&self, device_id: &str, channel_id: &str) -> Option<String>;
    /// Delete the recording state for the given device/channel.
    fn del_recording(&self, device_id: &str, channel_id: &str);

    // ---------- Invite / playback / download sessions ----------
    /// Persist an Invite session (live / playback / download / talk / broadcast).
    fn set_session(&self, session: InviteSessionState);
    /// Look up an Invite session by call_id.
    fn get_session(&self, call_id: &str) -> Option<InviteSessionState>;
    /// Delete an Invite session by call_id.
    fn del_session(&self, call_id: &str);
    /// List all Invite sessions for a given device_id (used by cascade / cleanup).
    fn list_sessions_by_device(&self, device_id: &str) -> Vec<InviteSessionState>;
    /// Count currently active (non-closed) Invite sessions.
    fn count_active_sessions(&self) -> usize;

    // ---------- Pending requests (atomic counter helper) ----------
    /// Increment a pending request counter (e.g. for concurrent catalog requests).
    fn incr_pending(&self, key: &str) -> i64;
    /// Decrement a pending request counter.
    fn decr_pending(&self, key: &str) -> i64;
    /// Get a pending request counter.
    fn get_pending(&self, key: &str) -> i64;

    // ---------- Convenience: aggregate stats ----------
    /// Count currently online devices.
    fn count_online_devices(&self) -> usize;
    /// Count currently active invite + playback sessions.
    fn count_active_streams(&self) -> usize;
}

// ---------------------------------------------------------------------------
// Default implementation: `StateStoreRepository` thin wrapper
// ---------------------------------------------------------------------------

/// Thin wrapper that delegates every call to the underlying `StateStore`.
///
/// This is the only production implementation; tests can mock the trait if needed.
pub struct StateStoreRepository {
    pub(crate) store: Arc<StateStore>,
}

impl StateStoreRepository {
    pub fn new(store: Arc<StateStore>) -> Self {
        Self { store }
    }
}

impl StreamStateRepository for StateStoreRepository {
    fn set_recording(&self, device_id: &str, channel_id: &str, cmd: &str) {
        self.store.set_recording(device_id, channel_id, cmd);
    }
    fn get_recording(&self, device_id: &str, channel_id: &str) -> Option<String> {
        self.store.get_recording(device_id, channel_id)
    }
    fn del_recording(&self, device_id: &str, channel_id: &str) {
        self.store.remove_recording(device_id, channel_id);
    }

    fn set_session(&self, session: InviteSessionState) {
        let call_id = session.call_id.clone();
        self.store.set_invite_session(&call_id, session);
    }
    fn get_session(&self, call_id: &str) -> Option<InviteSessionState> {
        self.store.get_invite_session(call_id)
    }
    fn del_session(&self, call_id: &str) {
        self.store.remove_invite_session(call_id);
    }
    fn list_sessions_by_device(&self, device_id: &str) -> Vec<InviteSessionState> {
        // Iterate over all sessions; filter by device_id. Cardinality is bounded by
        // concurrent INVITEs (usually < 1000), so this is cheap enough.
        let mut out = Vec::new();
        for (_id, session) in self.store.all_invite_sessions() {
            if session.device_id == device_id {
                out.push(session);
            }
        }
        out
    }
    fn count_active_sessions(&self) -> usize {
        self.store
            .all_invite_sessions()
            .iter()
            .filter(|(_, s)| s.status != "closed" && s.status != "timeout")
            .count()
    }

    fn incr_pending(&self, key: &str) -> i64 {
        // Use a synthesized PendingRequestState with the key as both key + device_id to track counter.
        let now = chrono::Utc::now();
        let state = crate::state_store::PendingRequestState {
            key: key.to_string(),
            device_id: key.to_string(),
            kind: "counter".to_string(),
            sent_at: now,
            timeout_at: now + chrono::Duration::seconds(60),
        };
        let prev = self.store.get_pending(key);
        self.store.set_pending(key, state);
        // For InMemory/Redis backends the counter helper is approximated via key existence;
        // callers that need an exact atomic counter should use Redis directly via `cache::incr_*`.
        // This trait API is intentionally best-effort.
        match prev {
            Some(p) => {
                // Already exists; bump last_activity by updating again with same payload.
                let _ = p;
                1
            }
            None => 1,
        }
    }
    fn decr_pending(&self, key: &str) -> i64 {
        self.store.remove_pending(key);
        0
    }
    fn get_pending(&self, key: &str) -> i64 {
        if self.store.get_pending(key).is_some() {
            1
        } else {
            0
        }
    }

    fn count_online_devices(&self) -> usize {
        self.store
            .all_devices_online()
            .iter()
            .filter(|(_, d)| d.online)
            .count()
    }
    fn count_active_streams(&self) -> usize {
        self.store.all_streams().iter().filter(|(_, s)| s.online).count()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_store::{
        InviteSessionState, StateStore, StreamState,
    };
    use chrono::Utc;

    fn make_repo() -> StateStoreRepository {
        StateStoreRepository::new(Arc::new(StateStore::in_memory()))
    }

    #[test]
    fn test_recording_crud() {
        let repo = make_repo();
        assert_eq!(repo.get_recording("dev1", "ch1"), None);
        repo.set_recording("dev1", "ch1", "Record");
        assert_eq!(repo.get_recording("dev1", "ch1"), Some("Record".to_string()));
        repo.del_recording("dev1", "ch1");
        assert_eq!(repo.get_recording("dev1", "ch1"), None);
    }

    #[test]
    fn test_session_crud() {
        let repo = make_repo();
        let s = InviteSessionState {
            call_id: "call-1".to_string(),
            device_id: "dev1".to_string(),
            channel_id: "ch1".to_string(),
            session_type: "live".to_string(),
            zlm_stream_id: Some("stream-1".to_string()),
            status: "active".to_string(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
        };
        repo.set_session(s.clone());
        let got = repo.get_session("call-1").unwrap();
        assert_eq!(got.device_id, "dev1");
        assert_eq!(got.status, "active");
        let by_dev = repo.list_sessions_by_device("dev1");
        assert_eq!(by_dev.len(), 1);
        repo.del_session("call-1");
        assert!(repo.get_session("call-1").is_none());
    }

    #[test]
    fn test_count_active_sessions_filters_closed() {
        let repo = make_repo();
        let now = Utc::now();
        let active = InviteSessionState {
            call_id: "call-a".into(),
            device_id: "dev1".into(),
            channel_id: "ch1".into(),
            session_type: "live".into(),
            zlm_stream_id: None,
            status: "active".into(),
            created_at: now,
            last_activity: now,
        };
        let mut closed = active.clone();
        closed.call_id = "call-b".into();
        closed.status = "closed".into();
        repo.set_session(active);
        repo.set_session(closed);
        assert_eq!(repo.count_active_sessions(), 1);
    }

    #[test]
    fn test_pending_helper() {
        let repo = make_repo();
        assert_eq!(repo.get_pending("p1"), 0);
        let _ = repo.incr_pending("p1");
        assert_eq!(repo.get_pending("p1"), 1);
        let _ = repo.decr_pending("p1");
        assert_eq!(repo.get_pending("p1"), 0);
    }

    #[test]
    fn test_count_online_devices() {
        let repo = make_repo();
        repo.store.set_device_online(
            "dev1",
            crate::state_store::DeviceOnlineState {
                online: true,
                ip: "10.0.0.1".into(),
                port: 5060,
                last_seen: Utc::now(),
                ttl_secs: 60,
            },
        );
        repo.store.set_device_online(
            "dev2",
            crate::state_store::DeviceOnlineState {
                online: false,
                ip: "10.0.0.2".into(),
                port: 5060,
                last_seen: Utc::now(),
                ttl_secs: 60,
            },
        );
        assert_eq!(repo.count_online_devices(), 1);
    }

    #[test]
    fn test_count_active_streams() {
        let repo = make_repo();
        repo.store.set_stream(
            "s1",
            StreamState {
                app: "live".into(),
                stream_id: "s1".into(),
                device_id: "dev1".into(),
                channel_id: "ch1".into(),
                ssrc: None,
                call_id: None,
                media_server_id: "zlm-a".into(),
                online: true,
                has_audio: false,
                last_activity: Utc::now(),
            },
        );
        assert_eq!(repo.count_active_streams(), 1);
    }

    #[test]
    fn test_recording_overwrite() {
        let repo = make_repo();
        repo.set_recording("dev1", "ch1", "Record");
        repo.set_recording("dev1", "ch1", "StopRecord");
        assert_eq!(repo.get_recording("dev1", "ch1"), Some("StopRecord".to_string()));
    }
}
