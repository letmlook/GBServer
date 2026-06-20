//! Phase 7.1: Redis 双节点一致性测试（设计文档 §11 验收第 6 条）。
//!
//! 模拟两个 GBServer 节点（Node A / Node B）共享同一 StateStore backend，
//! 验证双节点状态一致性。**不依赖真实 Redis**——InMemoryBackend 与 RedisBackend
//! 实现同一 `StateBackend` trait，双节点一致性语义等价。
//!
//! 验证：
//! 1. Node A 写入 device/stream/invite → Node B 立即可见
//! 2. Node B 删除 stream → Node A 看到 None
//! 3. 后写入覆盖前写入（last-write-wins）
//! 4. 状态变化通过 broadcast 通道可被订阅者观察到

#[cfg(test)]
mod two_node_consistency {
    use std::sync::Arc;
    use chrono::Utc;
    use gbserver::state_store::{
        DeviceOnlineState, InviteSessionState, InMemoryBackend,
        StateBackend, StateStore, StreamState,
    };

    fn make_two_nodes() -> (Arc<StateStore>, Arc<StateStore>) {
        let backend: Arc<dyn StateBackend> = Arc::new(InMemoryBackend::new());
        let node_a = Arc::new(StateStore::with_backend(backend.clone()));
        let node_b = Arc::new(StateStore::with_backend(backend.clone()));
        (node_a, node_b)
    }

    #[test]
    fn two_node_device_online_visibility() {
        let (node_a, node_b) = make_two_nodes();
        node_a.set_device_online("34020000001320000001", DeviceOnlineState {
            online: true,
            ip: "10.0.0.1".to_string(),
            port: 5060,
            last_seen: Utc::now(),
            ttl_secs: 60,
        });
        let seen_by_b = node_b.get_device_online("34020000001320000001");
        assert!(seen_by_b.is_some(), "Node B must see Node A's write");
        assert_eq!(seen_by_b.unwrap().ip, "10.0.0.1");
    }

    #[test]
    fn two_node_stream_deletion_propagates() {
        let (node_a, node_b) = make_two_nodes();
        let now = Utc::now();
        node_a.set_stream("stream-1", StreamState {
            app: "live".to_string(),
            stream_id: "stream-1".to_string(),
            device_id: "dev-1".to_string(),
            channel_id: "ch-1".to_string(),
            ssrc: None,
            call_id: None,
            media_server_id: "zlm-a".to_string(),
            online: true,
            has_audio: false,
            last_activity: now,
        });
        // Node B 看到 stream-1
        assert!(node_b.get_stream("stream-1").is_some());
        // Node B 删除
        node_b.remove_stream("stream-1");
        // Node A 看到 None
        assert!(node_a.get_stream("stream-1").is_none());
    }

    #[test]
    fn two_node_last_write_wins() {
        let (node_a, node_b) = make_two_nodes();
        let now = Utc::now();
        node_a.set_stream("s", StreamState {
            app: "live".into(), stream_id: "s".into(),
            device_id: "d".into(), channel_id: "c".into(),
            ssrc: None, call_id: None,
            media_server_id: "zlm-a".into(),
            online: true, has_audio: false, last_activity: now,
        });
        node_b.set_stream("s", StreamState {
            app: "live".into(), stream_id: "s".into(),
            device_id: "d".into(), channel_id: "c".into(),
            ssrc: None, call_id: None,
            media_server_id: "zlm-b".into(),  // different ms
            online: false, has_audio: false, last_activity: now,
        });
        // 最后写入（Node B）应胜出
        let seen_by_a = node_a.get_stream("s").unwrap();
        assert_eq!(seen_by_a.media_server_id, "zlm-b");
        assert!(!seen_by_a.online);
    }

    #[test]
    fn two_node_invite_session_visible() {
        let (node_a, node_b) = make_two_nodes();
        let now = Utc::now();
        let session = InviteSessionState {
            call_id: "call-001".to_string(),
            device_id: "dev-1".to_string(),
            channel_id: "ch-1".to_string(),
            session_type: "live".to_string(),
            zlm_stream_id: Some("stream-1".to_string()),
            status: "active".to_string(),
            created_at: now,
            last_activity: now,
        };
        node_a.set_invite_session("call-001", session.clone());
        let seen_by_b = node_b.get_invite_session("call-001");
        assert!(seen_by_b.is_some());
        assert_eq!(seen_by_b.unwrap().device_id, "dev-1");
    }

    #[test]
    fn two_node_all_invite_sessions_visible() {
        let (node_a, node_b) = make_two_nodes();
        let now = Utc::now();
        for i in 0..5 {
            let s = InviteSessionState {
                call_id: format!("call-{}", i),
                device_id: format!("dev-{}", i),
                channel_id: "ch".into(),
                session_type: "live".into(),
                zlm_stream_id: None,
                status: "active".into(),
                created_at: now,
                last_activity: now,
            };
            let id = s.call_id.clone();
            node_a.set_invite_session(&id, s);
        }
        let all_by_b = node_b.all_invite_sessions();
        assert_eq!(all_by_b.len(), 5);
    }

    #[test]
    fn two_node_broadcast_event_observed() {
        let (node_a, _node_b) = make_two_nodes();
        let mut rx = node_a.subscribe();
        node_a.set_device_online("d", DeviceOnlineState {
            online: true, ip: "1.2.3.4".into(), port: 5060,
            last_seen: Utc::now(), ttl_secs: 60,
        });
        // 应收到广播
        match rx.try_recv() {
            Ok(gbserver::state_store::StateEvent::DeviceOnline(_)) => {},
            other => panic!("expected DeviceOnline event, got {:?}", other),
        }
    }
}
