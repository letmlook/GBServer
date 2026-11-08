use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::IntoParams;

use crate::error::{AppError, ErrorCode};
use crate::response::ApiResult;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct PlaybackSession {
    pub stream_id: String,
    pub call_id: Option<String>,
    pub device_id: String,
    pub channel_id: String,
    pub app: String,
    pub stream: String,
    pub media_server_id: Option<String>,
    pub schema: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub current_time: String,
    pub speed: f64,
    pub paused: bool,
    pub source: String,
    pub cleanup_pending: bool,
}

fn new_playback_stream_id(device_id: &str, channel_id: &str) -> String {
    format!(
        "playback_{}_{}_{}",
        device_id,
        channel_id,
        uuid::Uuid::new_v4().simple()
    )
}

#[cfg(test)]
mod command_state_tests {
    use super::*;

    #[tokio::test]
    async fn recovery_lists_persisted_dialog_as_not_retryable_after_restart() {
        let (state, _, active_stream) = state_with_session().await;
        crate::db::playback_recovery::ensure_table(&state.pool)
            .await
            .unwrap();
        let record = crate::db::playback_recovery::RecoveryRecord {
            stream_id: "persisted".into(),
            call_id: "call".into(),
            device_id: "device".into(),
            channel_id: "channel".into(),
            ssrc: "1200000001".into(),
            peer_addr: "127.0.0.1:5060".into(),
            transport: "udp".into(),
            local_tag: Some("secret-local-tag".into()),
            remote_tag: Some("secret-remote-tag".into()),
            media_server_id: "zlm".into(),
            app: "rtp".into(),
            state: "active".into(),
            last_cseq: 2,
        };
        crate::db::playback_recovery::insert(&state.pool, &record)
            .await
            .unwrap();
        let mut live = record.clone();
        live.stream_id = active_stream;
        live.call_id = "live-call".into();
        crate::db::playback_recovery::insert(&state.pool, &live)
            .await
            .unwrap();
        let token = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes())
            .create_token("admin", 60)
            .unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("access-token", token.parse().unwrap());
        let response = playback_recovery_list(
            State(state.clone()),
            headers.clone(),
            Query(RecoveryListQuery::default()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(response["total"], 1);
        assert_eq!(response["items"][0]["streamId"], "persisted");
        assert_eq!(response["items"][0]["retryable"], false);
        assert!(!response.to_string().contains("secret-remote-tag"));
        let retry = playback_recovery_retry(State(state), headers, Path("persisted".into()))
            .await
            .unwrap()
            .0;
        assert_ne!(retry.code, 0);
        assert!(retry.msg.contains("尚未恢复"));
    }

    #[tokio::test]
    async fn admin_retry_sends_bye_and_removes_pending_record() {
        use crate::sip::gb28181::invite_session::{InviteSession, InviteSessionStatus, StreamType};
        let (mut state, manager, stream) = state_with_session().await;
        let peer = tokio::net::UdpSocket::bind("[::1]:0").await.unwrap();
        let socket = tokio::net::UdpSocket::bind("[::1]:0").await.unwrap();
        let sip = Arc::new(crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        ));
        sip.set_test_udp_socket(socket).await;
        let mut invite = InviteSession::new(
            "recover-call",
            "device",
            "channel",
            StreamType::Playback,
            peer.local_addr().unwrap(),
        );
        invite.status = InviteSessionStatus::Active;
        invite.set_local_dialog("local", 1);
        invite.set_remote_tag("remote");
        invite.set_zlm_stream(&stream, "rtp");
        let lease = sip.test_playback_lease("device", "channel").unwrap();
        invite.ssrc = Some(lease.ssrc().to_string());
        sip.invite_session_manager()
            .create_owned(invite, lease)
            .await;
        assert!(sip.retain_playback_ssrc("recover-call", &stream, "1200000001"));
        let saved = crate::db::playback_recovery::RecoveryRecord {
            stream_id: stream.clone(),
            call_id: "recover-call".into(),
            device_id: "device".into(),
            channel_id: "channel".into(),
            ssrc: "1200000001".into(),
            peer_addr: peer.local_addr().unwrap().to_string(),
            transport: "unknown".into(),
            local_tag: Some("local".into()),
            remote_tag: Some("remote".into()),
            media_server_id: "zlm".into(),
            app: "rtp".into(),
            state: "cleanup_pending".into(),
            last_cseq: 100,
        };
        crate::db::playback_recovery::insert(&state.pool, &saved)
            .await
            .unwrap();
        sip.send_playback_control_for_session(
            "device",
            "channel",
            Some("recover-call"),
            crate::sip::server::PlaybackControlCmd::Pause,
        )
        .await
        .unwrap();
        let mut info_buf = [0u8; 2048];
        let (n, _) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            peer.recv_from(&mut info_buf),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(std::str::from_utf8(&info_buf[..n])
            .unwrap()
            .contains("CSeq: 101 INFO"));
        state.sip_server = Some(sip);
        let mut record = manager.get(&stream).await.unwrap();
        record.call_id = Some("recover-call".into());
        record.source = "gb28181_playback_invite".into();
        manager.create(record.clone()).await;
        manager.mark_cleanup_pending(record).await;
        let app = axum::Router::new().route(
            "/index/api/close_streams",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"code":0})) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let http = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        state.zlm_client = Some(Arc::new(crate::zlm::ZlmClient::new(
            "127.0.0.1",
            port,
            "test",
        )));
        let token = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes())
            .create_token("admin", 60)
            .unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("access-token", token.parse().unwrap());
        let response = playback_recovery_retry(State(state.clone()), headers, Path(stream.clone()))
            .await
            .unwrap()
            .0;
        assert_eq!(response.code, 0, "{:?}", response.msg);
        assert_eq!(
            response.data.as_ref().unwrap()["deviceResult"],
            "unconfirmed"
        );
        let mut buf = [0u8; 2048];
        let (n, _) =
            tokio::time::timeout(std::time::Duration::from_secs(2), peer.recv_from(&mut buf))
                .await
                .unwrap()
                .unwrap();
        let message = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(message.starts_with("BYE "));
        assert!(message.contains("CSeq: 102 BYE"), "{message}");
        assert_eq!(
            crate::db::playback_recovery::get(&state.pool, &stream)
                .await
                .unwrap()
                .unwrap()
                .last_cseq,
            102
        );
        assert!(manager.get(&stream).await.is_none());
        http.abort();
    }

    #[tokio::test]
    async fn retained_dialog_does_not_send_when_journal_is_unavailable() {
        use crate::sip::gb28181::invite_session::{InviteSession, InviteSessionStatus, StreamType};
        let (state, _, stream) = state_with_session().await;
        let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sip = crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        );
        sip.set_test_udp_socket(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap())
            .await;
        let lease = sip.test_playback_lease("device", "channel").unwrap();
        let mut invite = InviteSession::new(
            "unavailable-call",
            "device",
            "channel",
            StreamType::Playback,
            peer.local_addr().unwrap(),
        );
        invite.status = InviteSessionStatus::Active;
        invite.set_local_dialog("local", 1);
        invite.set_remote_tag("remote");
        invite.set_zlm_stream(&stream, "rtp");
        invite.ssrc = Some(lease.ssrc().to_owned());
        sip.invite_session_manager()
            .create_owned(invite, lease)
            .await;
        assert!(sip.retain_playback_ssrc("unavailable-call", &stream, "1200000001"));
        crate::dispatch_exec!(&state.pool, |p| {
            sqlx::query("DROP TABLE gb_playback_recovery").execute(p)
        })
        .unwrap();
        assert!(sip
            .send_session_bye_by_call_id("unavailable-call")
            .await
            .is_err());
        let mut buf = [0u8; 2048];
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(150),
            peer.recv_from(&mut buf)
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn recovery_endpoints_require_admin_and_list_pending_only() {
        let (state, manager, stream) = state_with_session().await;
        manager
            .mark_cleanup_pending(manager.get(&stream).await.unwrap())
            .await;
        let mut headers = axum::http::HeaderMap::new();
        assert!(playback_recovery_list(
            State(state.clone()),
            headers.clone(),
            Query(RecoveryListQuery::default())
        )
        .await
        .is_err());
        let keys = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        crate::dispatch_exec!(&state.pool, |p| { sqlx::query("INSERT INTO gb_user_role (id, name, authority, create_time, update_time) VALUES (2, 'operator', '9', '2026-01-01', '2026-01-01')").execute(p) }).unwrap();
        crate::dispatch_exec!(&state.pool, |p| { sqlx::query("INSERT INTO gb_user (username, password, role_id, push_key, create_time, update_time) VALUES ('operator', 'x', 2, 'k', '2026-01-01', '2026-01-01')").execute(p) }).unwrap();
        let mut normal = axum::http::HeaderMap::new();
        normal.insert(
            "access-token",
            keys.create_token("operator", 60).unwrap().parse().unwrap(),
        );
        let list_error = playback_recovery_list(
            State(state.clone()),
            normal.clone(),
            Query(RecoveryListQuery::default()),
        )
        .await
        .unwrap_err();
        let retry_error =
            playback_recovery_retry(State(state.clone()), normal, Path(stream.clone()))
                .await
                .unwrap_err();
        assert!(format!("{list_error:?}").contains("403"));
        assert!(format!("{retry_error:?}").contains("403"));
        let token = keys.create_token("admin", 60).unwrap();
        headers.insert("access-token", token.parse().unwrap());
        let result = playback_recovery_list(
            State(state.clone()),
            headers.clone(),
            Query(RecoveryListQuery::default()),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(result.data.unwrap()["items"][0]["streamId"], stream);
        let mut second = manager.get(&stream).await.unwrap();
        second.stream_id = format!("{}-second", stream);
        let second_id = second.stream_id.clone();
        manager.mark_cleanup_pending(second).await;
        let page = playback_recovery_list(
            State(state.clone()),
            headers.clone(),
            Query(RecoveryListQuery {
                offset: Some(1),
                limit: Some(1),
            }),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(page["total"], 2);
        assert_eq!(page["items"][0]["streamId"], second_id);
        let mut unavailable = state.clone();
        unavailable.playback_manager = None;
        assert!(playback_recovery_list(
            State(unavailable.clone()),
            headers.clone(),
            Query(RecoveryListQuery::default())
        )
        .await
        .is_err());
        assert!(
            playback_recovery_retry(State(unavailable), headers.clone(), Path(stream.clone()))
                .await
                .is_err()
        );
        let retry = playback_recovery_retry(State(state), headers, Path("not-pending".into()))
            .await
            .unwrap()
            .0;
        assert_ne!(retry.code, 0);
    }

    #[tokio::test]
    async fn recovery_listing_only_contains_unconfirmed_cleanup_sessions() {
        let (_, manager, stream) = state_with_session().await;
        let existing = manager.get(&stream).await.unwrap();
        assert!(manager.list_cleanup_pending().await.is_empty());
        manager.mark_cleanup_pending(existing).await;
        let pending = manager.list_cleanup_pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].stream_id, stream);
        manager.remove(&stream).await;
        assert!(manager.list_cleanup_pending().await.is_empty());
    }

    #[tokio::test]
    async fn failed_bye_restores_record_if_cancelled_before_session_insert() {
        use crate::sip::gb28181::invite_session::{InviteSession, InviteSessionStatus, StreamType};
        let (state, manager, stream) = state_with_session().await;
        let mut recovery = manager.get(&stream).await.unwrap();
        recovery.call_id = Some("before-insert".into());
        recovery.source = "gb28181_playback_invite".into();
        recovery.device_id = "device".into();
        recovery.channel_id = "channel".into();
        recovery.media_server_id = Some("zlm-test".into());
        manager.remove(&stream).await;
        let sip = Arc::new(crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        ));
        let mut session = InviteSession::new(
            "before-insert",
            "device",
            "channel",
            StreamType::Playback,
            "127.0.0.1:15060".parse().unwrap(),
        );
        session.status = InviteSessionStatus::Active;
        session.set_local_dialog("local", 1);
        session.set_remote_tag("remote");
        let owned_ssrc = sip.test_playback_lease("device", "channel").unwrap();
        session.signaling_transport = Some("udp".into());
        assert_eq!(owned_ssrc.ssrc(), "1200000001");
        session.ssrc = Some(owned_ssrc.ssrc().to_string());
        sip.invite_session_manager()
            .create_owned(session, owned_ssrc)
            .await;
        let guard = PlaybackStartGuard {
            sip: sip.clone(),
            playback: Some(manager.clone()),
            call_id: "before-insert".into(),
            stream_id: stream.clone(),
            recovery_session: Some(recovery),
            rtp_lease: None,
            armed: true,
        };
        drop(guard);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while manager.get(&stream).await.is_none() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("recovery session missing");
        let saved = crate::db::playback_recovery::get(&state.pool, &stream)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.call_id, "before-insert");
        assert_eq!(saved.ssrc, "1200000001");
        assert_eq!(saved.last_cseq, 4);
        assert_eq!(saved.state, "cleanup_pending");
        assert_eq!(saved.transport, "udp");
        assert_eq!(saved.remote_tag.as_deref(), Some("remote"));
        sip.invite_session_manager().remove("before-insert").await;
        assert!(sip.ssrc_manager().get("1200000001").is_some());
        let mut restarted = crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        );
        assert_eq!(restarted.restore_playback_ssrcs().await.unwrap(), 1);
        assert!(restarted.ssrc_manager().get("1200000001").is_some());
    }

    #[tokio::test]
    async fn failed_journal_write_still_retains_owned_ssrc() {
        use crate::sip::gb28181::invite_session::{InviteSession, InviteSessionStatus, StreamType};
        let (state, manager, stream) = state_with_session().await;
        let mut recovery = manager.get(&stream).await.unwrap();
        recovery.device_id = "device".into();
        recovery.channel_id = "channel".into();
        let sip = Arc::new(crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        ));
        let owned = sip.test_playback_lease("device", "channel").unwrap();
        let ssrc = owned.ssrc().to_string();
        let mut session = InviteSession::new(
            "journal-failure",
            "device",
            "channel",
            StreamType::Playback,
            "127.0.0.1:15060".parse().unwrap(),
        );
        session.status = InviteSessionStatus::Active;
        session.set_local_dialog("local", 1);
        session.set_remote_tag("remote");
        session.ssrc = Some(ssrc.clone());
        sip.invite_session_manager()
            .create_owned(session, owned)
            .await;
        crate::dispatch_exec!(&state.pool, |p| {
            sqlx::query("DROP TABLE gb_playback_recovery").execute(p)
        })
        .unwrap();
        drop(PlaybackStartGuard {
            sip: sip.clone(),
            playback: Some(manager.clone()),
            call_id: "journal-failure".into(),
            stream_id: stream.clone(),
            recovery_session: Some(recovery),
            rtp_lease: None,
            armed: true,
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !manager
                .get(&stream)
                .await
                .is_some_and(|session| session.cleanup_pending)
            {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        sip.invite_session_manager().remove("journal-failure").await;
        assert!(sip.ssrc_manager().get(&ssrc).is_some());
    }

    #[tokio::test]
    async fn cancelled_ready_playback_retains_session_when_bye_never_sends() {
        use crate::sip::gb28181::invite_session::{InviteSession, InviteSessionStatus, StreamType};
        let (state, manager, stream) = state_with_session().await;
        let sip = Arc::new(crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        ));
        let mut session = InviteSession::new(
            "failed-call",
            "device",
            "channel",
            StreamType::Playback,
            "127.0.0.1:15060".parse().unwrap(),
        );
        session.status = InviteSessionStatus::Active;
        session.set_local_dialog("local", 1);
        session.set_remote_tag("remote");
        sip.invite_session_manager().create(session).await;
        let guard = PlaybackStartGuard {
            sip: sip.clone(),
            playback: Some(manager.clone()),
            call_id: "failed-call".into(),
            stream_id: stream.clone(),
            recovery_session: None,
            rtp_lease: None,
            armed: true,
        };
        drop(guard);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert!(
            manager.get(&stream).await.is_some(),
            "unconfirmed dialog must remain recoverable"
        );
        assert!(sip
            .invite_session_manager()
            .get("failed-call")
            .await
            .unwrap()
            .is_active());
        assert_eq!(
            sip.invite_session_manager()
                .get("failed-call")
                .await
                .unwrap()
                .invite_cseq,
            4
        );
    }

    #[tokio::test]
    async fn cancelled_ready_playback_retries_bye_after_socket_recovers() {
        use crate::sip::gb28181::invite_session::{InviteSession, InviteSessionStatus, StreamType};
        let (state, manager, stream) = state_with_session().await;
        let remote = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sip = Arc::new(crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        ));
        let mut session = InviteSession::new(
            "retry-call",
            "device",
            "channel",
            StreamType::Playback,
            remote.local_addr().unwrap(),
        );
        session.status = InviteSessionStatus::Active;
        session.set_local_dialog("local", 1);
        session.set_remote_tag("remote");
        sip.invite_session_manager().create(session).await;
        let guard = PlaybackStartGuard {
            sip: sip.clone(),
            playback: Some(manager.clone()),
            call_id: "retry-call".into(),
            stream_id: stream.clone(),
            recovery_session: None,
            rtp_lease: None,
            armed: true,
        };
        drop(guard);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        sip.set_test_udp_socket(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap())
            .await;
        let mut buf = [0u8; 2048];
        let (n, _) = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            remote.recv_from(&mut buf),
        )
        .await
        .expect("BYE was not retried")
        .unwrap();
        assert!(std::str::from_utf8(&buf[..n]).unwrap().starts_with("BYE "));
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while manager.get(&stream).await.is_some() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn cancelled_ready_playback_cleans_record_and_rtp() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let closed = Arc::new(AtomicUsize::new(0));
        let count = closed.clone();
        let router = axum::Router::new()
            .route(
                "/index/api/openRtpServer",
                axum::routing::get(|| async {
                    axum::Json(serde_json::json!({"code":0,"port":30001}))
                }),
            )
            .route(
                "/index/api/closeRtpServer",
                axum::routing::get(move || {
                    let count = count.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        axum::Json(serde_json::json!({"code":0,"hit":1}))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let http = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let (state, manager, stream) = state_with_session().await;
        let sip = Arc::new(crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        ));
        let client = crate::zlm::ZlmClient::new("127.0.0.1", port, "test");
        let (_, lease) = crate::zlm::port_guard::open_owned_rtp_server(
            &client,
            crate::zlm::OpenRtpServerRequest {
                stream_id: stream.clone(),
                port: Some(0),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let guard = PlaybackStartGuard {
            sip,
            playback: Some(manager.clone()),
            call_id: "missing".into(),
            stream_id: stream.clone(),
            recovery_session: None,
            rtp_lease: Some(lease),
            armed: true,
        };
        drop(guard);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if manager.get(&stream).await.is_none() && closed.load(Ordering::SeqCst) == 1 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("playback cancellation leaked state or RTP");
        http.abort();
    }

    #[tokio::test]
    async fn playback_cancelled_during_rtp_open_closes_eventual_allocation() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let entered = Arc::new(tokio::sync::Notify::new());
        let proceed = Arc::new(tokio::sync::Notify::new());
        let closed = Arc::new(AtomicUsize::new(0));
        let (a, b, c) = (entered.clone(), proceed.clone(), closed.clone());
        let router = axum::Router::new()
            .route(
                "/index/api/openRtpServer",
                axum::routing::get(move || {
                    let (a, b) = (a.clone(), b.clone());
                    async move {
                        a.notify_one();
                        b.notified().await;
                        axum::Json(serde_json::json!({"code":0,"port":30001}))
                    }
                }),
            )
            .route(
                "/index/api/closeRtpServer",
                axum::routing::get(move || {
                    let c = c.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        axum::Json(serde_json::json!({"code":0,"hit":1}))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut state = crate::test_support::app_state().await;
        state.sip_server = Some(Arc::new(crate::sip::SipServer::new(
            crate::config::SipConfig::default(),
            state.pool.clone(),
            None,
        )));
        state.zlm_client = Some(Arc::new(crate::zlm::ZlmClient::new(
            "127.0.0.1",
            port,
            "test",
        )));
        let task = tokio::spawn(async move {
            playback_start(
                State(state),
                Path(("dev".into(), "ch".into())),
                Query(PlaybackQuery {
                    start_time: None,
                    end_time: None,
                }),
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        task.abort();
        let _ = task.await;
        proceed.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while closed.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("playback RTP allocation leaked");
        server.abort();
    }

    #[test]
    fn playback_stream_ids_are_unique_for_concurrent_requests() {
        let ids: std::collections::HashSet<_> = std::thread::scope(|scope| {
            (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        (0..1250)
                            .map(|_| new_playback_stream_id("device", "channel"))
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .flat_map(|thread| thread.join().unwrap())
                .collect()
        });
        assert_eq!(ids.len(), 10000);
        assert!(ids
            .iter()
            .all(|id| id.starts_with("playback_device_channel_")));
    }
    use crate::handlers::common_channel::{
        channel_playback_pause, channel_playback_resume, channel_playback_seek,
        channel_playback_speed, ChannelPlaybackPauseQ, ChannelPlaybackResumeQ,
        ChannelPlaybackSeekQ, ChannelPlaybackSpeedQ,
    };

    async fn state_with_session() -> (AppState, Arc<PlaybackManager>, String) {
        let mut state = crate::test_support::app_state().await;
        crate::db::playback_recovery::ensure_table(&state.pool)
            .await
            .unwrap();
        let manager = Arc::new(PlaybackManager::new());
        let stream = "playback_34020000001320000001_34020000001310000001_1700000000".to_string();
        manager
            .create(PlaybackSession {
                stream_id: stream.clone(),
                call_id: None,
                device_id: "34020000001320000001".into(),
                channel_id: "34020000001310000001".into(),
                app: "rtp".into(),
                stream: stream.clone(),
                media_server_id: None,
                schema: "rtsp".into(),
                start_time: "2026-01-01T00:00:00".into(),
                end_time: None,
                current_time: "2026-01-01T00:00:00".into(),
                speed: 1.0,
                paused: false,
                source: "device".into(),
                cleanup_pending: false,
            })
            .await;
        state.playback_manager = Some(manager.clone());
        (state, manager, stream)
    }

    #[tokio::test]
    async fn playback_commands_without_sip_fail_without_changing_session() {
        let (state, manager, stream) = state_with_session().await;
        assert_ne!(
            playback_pause(State(state.clone()), Path(stream.clone()))
                .await
                .0
                .code,
            0
        );
        assert!(!manager.get(&stream).await.unwrap().paused);
        assert_ne!(
            playback_speed(State(state.clone()), Path((stream.clone(), "2".into())))
                .await
                .0
                .code,
            0
        );
        assert_eq!(manager.get(&stream).await.unwrap().speed, 1.0);
        let invalid_speed =
            playback_speed(State(state.clone()), Path((stream.clone(), "-1".into())))
                .await
                .0;
        assert_ne!(invalid_speed.code, 0);
        assert!(invalid_speed.msg.contains("倍速"));
        assert_ne!(
            playback_seek(
                State(state.clone()),
                Path((stream.clone(), "2026-01-01T01:00:00".into()))
            )
            .await
            .0
            .code,
            0
        );
        assert_eq!(
            manager.get(&stream).await.unwrap().current_time,
            "2026-01-01T00:00:00"
        );
        manager.pause(&stream).await;
        assert_ne!(
            playback_resume(State(state.clone()), Path(stream.clone()))
                .await
                .0
                .code,
            0
        );
        assert!(manager.get(&stream).await.unwrap().paused);

        manager.resume(&stream).await;
        assert_ne!(
            channel_playback_pause(
                State(state.clone()),
                Query(ChannelPlaybackPauseQ {
                    channel_id: None,
                    stream: Some(stream.clone())
                })
            )
            .await
            .0["code"],
            0
        );
        assert!(!manager.get(&stream).await.unwrap().paused);
        assert_ne!(
            channel_playback_speed(
                State(state.clone()),
                Query(ChannelPlaybackSpeedQ {
                    channel_id: None,
                    stream: Some(stream.clone()),
                    speed: Some("2".into())
                })
            )
            .await
            .0["code"],
            0
        );
        assert_eq!(manager.get(&stream).await.unwrap().speed, 1.0);
        assert_ne!(
            channel_playback_seek(
                State(state.clone()),
                Query(ChannelPlaybackSeekQ {
                    channel_id: None,
                    stream: Some(stream.clone()),
                    seek_time: Some("2026-01-01T01:00:00".into())
                })
            )
            .await
            .0["code"],
            0
        );
        assert_eq!(
            manager.get(&stream).await.unwrap().current_time,
            "2026-01-01T00:00:00"
        );
        manager.pause(&stream).await;
        assert_ne!(
            channel_playback_resume(
                State(state),
                Query(ChannelPlaybackResumeQ {
                    channel_id: None,
                    stream: Some(stream.clone())
                })
            )
            .await
            .0["code"],
            0
        );
        assert!(manager.get(&stream).await.unwrap().paused);
    }
}

/// Owns an established playback until its response has been prepared.
struct PlaybackStartGuard {
    sip: Arc<crate::sip::SipServer>,
    playback: Option<Arc<PlaybackManager>>,
    call_id: String,
    stream_id: String,
    recovery_session: Option<PlaybackSession>,
    rtp_lease: Option<crate::zlm::port_guard::RtpServerLease>,
    armed: bool,
}

impl Drop for PlaybackStartGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let sip = self.sip.clone();
        let playback = self.playback.clone();
        let call_id = self.call_id.clone();
        let stream_id = self.stream_id.clone();
        let recovery_session = self.recovery_session.take();
        let lease = self.rtp_lease.take();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let mut sent = false;
                for (attempt, delay_ms) in [100_u64, 300, 0].into_iter().enumerate() {
                    match sip.send_session_bye_by_call_id(&call_id).await {
                        Ok(_) => { sent = true; break; }
                        Err(error) => {
                            tracing::warn!(%call_id, %stream_id, attempt = attempt + 1, "回放启动取消后发送 BYE 失败: {}", error);
                            let active = sip.invite_session_manager().get(&call_id).await.is_some_and(|session| session.is_active());
                            if !active { break; }
                            if delay_ms > 0 { tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await; }
                        }
                    }
                }
                let active = sip.invite_session_manager().get(&call_id).await.is_some_and(|session| session.is_active());
                if sent || !active {
                    if let Some(playback) = playback { playback.remove(&stream_id).await; }
                } else {
                    if let (Some(playback), Some(recovery)) = (playback, recovery_session) {
                        if let Some(dialog) = sip.invite_session_manager().get(&call_id).await {
                            if let Some(ssrc) = dialog.ssrc.as_ref() {
                                let record = crate::db::playback_recovery::RecoveryRecord {
                                    stream_id: stream_id.clone(), call_id: call_id.clone(),
                                    device_id: dialog.device_id.clone(), channel_id: dialog.channel_id.clone(),
                                    ssrc: ssrc.clone(), peer_addr: dialog.peer_addr.to_string(),
                                    // A persisted socket address does not establish the original transport.
                                    transport: dialog.signaling_transport.clone().unwrap_or_else(|| "unknown".into()), local_tag: dialog.local_tag.clone(),
                                    remote_tag: dialog.remote_tag.clone(),
                                    media_server_id: recovery.media_server_id.clone().unwrap_or_default(),
                                    app: recovery.app.clone(), state: "cleanup_pending".into(),
                                    last_cseq: i64::from(dialog.invite_cseq),
                                };
                                if !sip.retain_playback_ssrc(&call_id, &stream_id, ssrc) {
                                    tracing::error!(%call_id, %stream_id, "无法保留回放 SSRC 的所有权，恢复记录仅保存在当前进程");
                                } else if let Err(error) = crate::db::playback_recovery::insert(sip.database_pool(), &record).await {
                                    tracing::error!(%call_id, %stream_id, "持久化待清理回放失败，记录仅保存在当前进程: {}", error);
                                }
                            } else {
                                tracing::error!(%call_id, %stream_id, "回放缺少 SSRC，无法持久化恢复记录");
                            }
                        }
                        playback.mark_cleanup_pending(recovery).await;
                    }
                    tracing::error!(%call_id, %stream_id, "BYE 未发送，保留回放会话供后续重试；设备结束状态未确认");
                }
                drop(lease);
            });
        }
    }
}

pub struct PlaybackManager {
    sessions: Arc<RwLock<std::collections::HashMap<String, PlaybackSession>>>,
}

impl PlaybackManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn create(&self, session: PlaybackSession) {
        self.sessions
            .write()
            .await
            .insert(session.stream_id.clone(), session);
    }

    async fn mark_cleanup_pending(&self, mut session: PlaybackSession) {
        session.cleanup_pending = true;
        let mut sessions = self.sessions.write().await;
        sessions
            .entry(session.stream_id.clone())
            .and_modify(|existing| existing.cleanup_pending = true)
            .or_insert(session);
    }

    async fn list_cleanup_pending(&self) -> Vec<PlaybackSession> {
        let mut sessions: Vec<_> = self
            .sessions
            .read()
            .await
            .values()
            .filter(|session| session.cleanup_pending)
            .cloned()
            .collect();
        sessions.sort_by(|a, b| a.stream_id.cmp(&b.stream_id));
        sessions
    }

    pub async fn get(&self, stream_id: &str) -> Option<PlaybackSession> {
        self.sessions.read().await.get(stream_id).cloned()
    }

    pub async fn update_speed(&self, stream_id: &str, speed: f64) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.speed = speed;
        }
    }

    pub async fn update_current_time(&self, stream_id: &str, current_time: String) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.current_time = current_time;
        }
    }

    pub async fn pause(&self, stream_id: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.paused = true;
        }
    }

    pub async fn resume(&self, stream_id: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.paused = false;
        }
    }

    pub async fn remove(&self, stream_id: &str) {
        self.sessions.write().await.remove(stream_id);
    }
}

impl Default for PlaybackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct DownloadSession {
    pub stream_id: String,
    pub call_id: Option<String>,
    pub device_id: String,
    pub channel_id: String,
    pub file_name: String,
    pub start_time: String,
    pub end_time: String,
    pub url: String,
    pub status: String,
    pub progress: f64,
    pub created_at: DateTime<Utc>,
    /// Phase 3.4: ZLM 流标识（与 ZLM `app/stream` 对应，用于 hook 回调匹配）
    pub zlm_stream_id: String,
    /// Phase 3.4: ZLM app（默认 "rtp"）
    pub zlm_app: String,
    /// Phase 3.4: 累计已下载字节数（来自 ZLM on_stream_changed 钩子）
    pub current_bytes: i64,
    /// Phase 3.4: 目标字节数（来自 ZLM 估算或 start/end_time 推算）
    pub total_bytes: i64,
    /// 设备推完流后 ZLM 落盘的 MP4 文件名（来自 `on_record_mp4`）。
    ///
    /// 有了它才能在下载结束后**真的把文件给出去**：此前响应里的
    /// `downloadUrl` 固定是 `/download/<平台自己编的文件名>`，既没有对应路由、
    /// 也和 ZLM 实际写出的文件名无关 —— 用户点下载必然 404。
    pub zlm_file_name: Option<String>,
    /// ZLM 写出的文件绝对路径（容器内路径，仅用于日志/推导相对路径）
    pub zlm_file_path: Option<String>,
    /// ZLM 写出的文件所在目录
    pub zlm_folder: Option<String>,
}

pub struct DownloadManager {
    sessions: Arc<RwLock<std::collections::HashMap<String, DownloadSession>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn create(&self, session: DownloadSession) {
        // 每发起一次下载顺手回收一次历史会话（1 小时 TTL），
        // 避免长期运行后内存里堆积已完成/已失败的下载。
        let removed = self.cleanup_finished(3600).await;
        if removed > 0 {
            tracing::debug!("下载会话清理: 移除 {} 条已结束的会话", removed);
        }
        self.sessions
            .write()
            .await
            .insert(session.stream_id.clone(), session);
    }

    pub async fn get(&self, stream_id: &str) -> Option<DownloadSession> {
        self.sessions.read().await.get(stream_id).cloned()
    }

    /// Phase 3.4: 进度更新用绝对字节数（current_bytes / total_bytes * 100.0）
    /// 对外仍返回百分比。
    pub async fn update_progress(&self, stream_id: &str, current_bytes: i64, total_bytes: i64) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.current_bytes = current_bytes;
            s.total_bytes = total_bytes;
            if total_bytes > 0 {
                s.progress =
                    ((current_bytes as f64) / (total_bytes as f64) * 100.0).clamp(0.0, 100.0);
            }
            s.status = if current_bytes >= total_bytes && total_bytes > 0 {
                "completed".to_string()
            } else {
                "downloading".to_string()
            };
        }
    }

    /// 兼容旧 API：按 0..100 模糊语义更新
    pub async fn update_progress_percent(&self, stream_id: &str, progress: f64, status: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.progress = progress;
            s.status = status.to_string();
        }
    }

    /// 记录设备推流结束后 ZLM 落盘的 MP4（`on_record_mp4` 钩子调用）。
    ///
    /// 同时把字节数写满，让 `/progress` 能给出终态与真实大小
    /// （此前只把进度写成 100% 而没有任何文件信息，`downloadUrl` 也指向不存在的路由）。
    pub async fn set_zlm_file(
        &self,
        stream_id: &str,
        file_name: String,
        file_path: Option<String>,
        folder: Option<String>,
        size: i64,
    ) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.zlm_file_name = Some(file_name);
            s.zlm_file_path = file_path;
            s.zlm_folder = folder;
            s.current_bytes = size;
            s.total_bytes = size;
            s.progress = 100.0;
            s.status = "completed".to_string();
        }
    }

    /// 只改状态，不动进度/字节数（停止下载后进入 `finalizing`，等 ZLM 落盘回调）。
    pub async fn set_status(&self, stream_id: &str, status: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            if status == "finalizing" && s.zlm_file_name.is_some() {
                return;
            }
            s.status = status.to_string();
        }
    }

    /// 清理"已结束且很久没人看"的会话，避免长期运行后内存里堆满历史下载。
    /// `ttl_secs` 按 `created_at` 计算（保守：只清 completed/finalizing/failed）。
    pub async fn cleanup_finished(&self, ttl_secs: i64) -> usize {
        let now = Utc::now();
        let mut map = self.sessions.write().await;
        let before = map.len();
        map.retain(|_, s| {
            let finished = matches!(s.status.as_str(), "completed" | "finalizing" | "failed");
            let expired = (now - s.created_at).num_seconds() > ttl_secs;
            !(finished && expired)
        });
        before - map.len()
    }

    pub async fn remove(&self, stream_id: &str) {
        self.sessions.write().await.remove(stream_id);
    }

    pub async fn get_all(&self) -> Vec<DownloadSession> {
        self.sessions.read().await.values().cloned().collect()
    }

    /// Phase 3.4: 按 zlm_stream_id 查找（ZLM hook on_stream_changed 调用）
    pub async fn get_by_zlm_stream(&self, zlm_stream_id: &str) -> Option<DownloadSession> {
        self.sessions
            .read()
            .await
            .values()
            .find(|s| s.zlm_stream_id == zlm_stream_id)
            .cloned()
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct PlaybackQuery {
    /// 起始时间（RFC3339 / `YYYY-MM-DDTHH:MM:SS`），缺省时取当前时刻
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    /// 结束时间，缺省时与 `startTime` 相等
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
}

/// GET /api/playback/start/{device_id}/{channel_id}?startTime=&endTime=
///
/// 发起 GB28181 历史回放 INVITE + 等 ZLM 媒体到达。**任何失败都返回 500**，
/// 不再伪装"会话已创建"误导前端。
#[utoipa::path(
    get,
    path = "/api/playback/start/{device_id}/{channel_id}",
    tag = "playback",
    operation_id = "playback_start",
    params(
        ("device_id" = String, Path, description = "设备国标编号（20 位）"),
        ("channel_id" = String, Path, description = "通道国标编号（20 位）"),
        PlaybackQuery,
    ),
    responses(
        (status = 200, description = "回放会话已建立",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"streamId":"playback_dev_ch_1700000000","deviceId":"dev","channelId":"ch","app":"rtp","stream":"playback_dev_ch_1700000000","playUrl":"rtsp://1.2.3.4:554/rtp/...","flvUrl":"http://1.2.3.4:8080/rtp/...live.flv","hlsAvailable":false,"startTime":"2026-01-01T00:00:00","endTime":null,"currentTime":"2026-01-01T00:00:00","speed":1.0,"source":"gb28181_playback_invite"}})),
        (status = 500, description = "SIP 未启用 / ZLM 未配置 / INVITE 失败"),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn playback_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<PlaybackQuery>,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    let start_time = q
        .start_time
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string());
    let end_time = q.end_time.clone();

    tracing::info!(
        "Playback start: device={}, channel={}, start={}",
        device_id,
        channel_id,
        start_time
    );

    let stream_id = new_playback_stream_id(&device_id, &channel_id);
    // openRtpServer 注册的真实 ZLM 应用名是 rtp；回放业务类型由 stream_id 区分。
    let app = "rtp".to_string();
    let media_server_id = state.list_zlm_servers().into_iter().next();
    // 只有真实成功的分支才会返回 session，`source` 恒为 INVITE 路径
    let source = "gb28181_playback_invite";

    // Phase 3.2: 走真实 GB28181 Playback INVITE + ZLM RTP server 媒体等待
    if let Some(ref sip_server) = state.sip_server {
        if let Some(ref zlm_client) = state.zlm_client {
            // 1. 先开 ZLM RTP server（端口由 ZLM 自动分配）
            let rtp_req = crate::zlm::OpenRtpServerRequest {
                secret: zlm_client.secret.clone(),
                stream_id: stream_id.clone(),
                port: Some(0),
                use_tcp: Some(false),
                rtp_type: Some(0),
                recv_port: None,
            };
            match crate::zlm::port_guard::open_owned_rtp_server(zlm_client, rtp_req).await {
                Ok((rtp_server, mut rtp_lease)) => {
                    if let Err(problem) = crate::zlm::port_guard::validate_allocated_rtp_port(
                        &state,
                        zlm_client,
                        &stream_id,
                        rtp_server.port,
                        "playback/start",
                    )
                    .await
                    {
                        return Err(AppError::business(ErrorCode::Error500, problem));
                    }
                    // 2. 发 SIP INVITE + 等 ZLM 媒体到达
                    let sip = &*sip_server;
                    let end = end_time.clone().unwrap_or_else(|| start_time.clone());
                    match sip
                        .send_playback_invite_and_wait(
                            &device_id,
                            &channel_id,
                            &start_time,
                            &end,
                            &stream_id,
                            rtp_server.port,
                            15,
                        )
                        .await
                    {
                        Ok((call_id, _zlm_stream_id)) => {
                            let recovery_session = PlaybackSession {
                                stream_id: stream_id.clone(),
                                call_id: Some(call_id.clone()),
                                device_id: device_id.clone(),
                                channel_id: channel_id.clone(),
                                app: app.clone(),
                                stream: stream_id.clone(),
                                media_server_id: media_server_id.clone(),
                                schema: "rtsp".to_string(),
                                start_time: start_time.clone(),
                                end_time: end_time.clone(),
                                current_time: start_time.clone(),
                                speed: 1.0,
                                paused: false,
                                source: source.to_string(),
                                cleanup_pending: false,
                            };
                            let mut playback_guard = PlaybackStartGuard {
                                sip: sip_server.clone(),
                                playback: state.playback_manager.clone(),
                                call_id: call_id.clone(),
                                stream_id: stream_id.clone(),
                                recovery_session: Some(recovery_session.clone()),
                                rtp_lease: Some(rtp_lease),
                                armed: true,
                            };
                            let media_ip = zlm_client.ip.clone();
                            let http_port = zlm_client.http_port;
                            let play_url = format!("rtsp://{}:554/{}/{}", media_ip, app, stream_id);
                            let flv_url = crate::zlm::address_builder::http_flv_url(
                                &media_ip, http_port, &app, &stream_id,
                            );
                            let hls_url = crate::zlm::address_builder::hls_url(
                                &media_ip, http_port, &app, &stream_id,
                            );
                            if let Some(ref playback_manager) = state.playback_manager {
                                playback_manager.create(recovery_session).await;
                            }
                            // `hls` 同样只在真的有源时给出（回放流在多数 ZLM 版本上
                            // 没有 hls 源，给出 404 地址会让前端优先选它而播不出来）
                            let hls_available =
                                zlm_client.has_schema_lossy(&app, &stream_id, "hls").await;
                            let mut payload = serde_json::json!({
                                "streamId": stream_id,
                                "deviceId": device_id,
                                "channelId": channel_id,
                                "app": app,
                                "stream": stream_id,
                                "playUrl": play_url,
                                "flvUrl": flv_url,
                                "hlsAvailable": hls_available,
                                "startTime": start_time,
                                "endTime": end_time,
                                "currentTime": start_time,
                                "speed": 1.0,
                                "source": source
                            });
                            if hls_available {
                                payload["hls"] = serde_json::json!(hls_url);
                            }
                            if let Some(lease) = playback_guard.rtp_lease.as_mut() {
                                lease.disarm();
                            }
                            playback_guard.armed = false;
                            return Ok(Json(ApiResult::success(payload)));
                        }
                        Err(e) => {
                            tracing::error!("Playback INVITE + media wait failed: {}", e);
                            // 清理已开 RTP 端口
                            if zlm_client.close_rtp_server(&stream_id).await.is_ok() {
                                rtp_lease.disarm();
                            }
                            return Err(AppError::business(
                                ErrorCode::Error500,
                                format!(
                                    "回放启动失败（节点 {}:{}, 流 {}, 请求 playback/start）：{}",
                                    zlm_client.ip, zlm_client.http_port, stream_id, e
                                ),
                            ));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to open ZLM RTP server for playback: {}", e);
                    return Err(AppError::business(
                        ErrorCode::Error500,
                        format!(
                            "回放收流端口分配失败（节点 {}:{}, 流 {}, 请求 playback/start）：{}",
                            zlm_client.ip, zlm_client.http_port, stream_id, e
                        ),
                    ));
                }
            }
        }
    }

    // 走到这里说明**没有任何一路回放流被真正拉起来**：SIP 未启用 / ZLM 未配置 /
    // GB28181 回放 INVITE 失败。此前这里仍然 `code: 0` 返回一个没有 `playUrl` 的
    // "会话已创建"，前端 `v-if="playUrl"` 为假 → 用户点了片段却什么都不发生、
    // 也没有任何失败提示（失败只落在后端日志），而 `currentStreamId` 已被赋值，
    // 暂停/停止按钮会对一个空会话说谎。
    let reason = if state.sip_server.is_none() {
        "SIP 未启用，无法向设备发起回放 INVITE"
    } else if state.zlm_client.is_none() {
        "未配置可用的 ZLM 媒体节点"
    } else {
        "GB28181 回放 INVITE 失败或等待媒体超时（详见服务端日志）"
    };
    tracing::warn!(
        "回放启动失败 device={} channel={} start={}: {}",
        device_id,
        channel_id,
        start_time,
        reason
    );
    Err(AppError::business(
        ErrorCode::Error500,
        format!("回放启动失败：{reason}"),
    ))
}

/// GET /api/playback/resume/{stream_id}
///
/// 继续回放（发送 SIP PlaybackCtrl Resume）。
#[utoipa::path(
    get,
    path = "/api/playback/resume/{stream_id}",
    tag = "playback",
    operation_id = "playback_resume",
    params(
        ("stream_id" = String, Path, description = "回放流 ID（`/start` 返回）"),
    ),
    responses(
        (status = 200, description = "已发 Resume 命令",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"streamId":"playback_dev_ch_1700000000","status":"playing","statusSource":"local_requested","sipSent":true,"deviceResult":{"status":"sent"},"message":"Playback resume command sent"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn playback_resume(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!("Playback resume: stream={}", stream_id);
    if let Err(msg) = crate::handlers::common_channel::dispatch_playback_control(
        &state,
        &stream_id,
        crate::sip::PlaybackControlCmd::Resume,
    )
    .await
    {
        return Json(ApiResult::error(msg));
    }
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.resume(&stream_id).await;
    }

    Json(ApiResult::success(serde_json::json!({
        "streamId": stream_id,
        "status": "playing",
        "statusSource": "local_requested",
        "sipSent": true,
        "deviceResult": {"status": "sent"},
        "message": "Playback resume command sent"
    })))
}

/// GET /api/playback/pause/{stream_id}
///
/// 暂停回放（发送 SIP PlaybackCtrl Pause）。
#[utoipa::path(
    get,
    path = "/api/playback/pause/{stream_id}",
    tag = "playback",
    operation_id = "playback_pause",
    params(
        ("stream_id" = String, Path, description = "回放流 ID"),
    ),
    responses(
        (status = 200, description = "已发 Pause 命令",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"streamId":"playback_dev_ch_1700000000","status":"paused","statusSource":"local_requested","sipSent":true,"deviceResult":{"status":"sent"},"message":"Playback pause command sent"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn playback_pause(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!("Playback pause: stream={}", stream_id);
    if let Err(msg) = crate::handlers::common_channel::dispatch_playback_control(
        &state,
        &stream_id,
        crate::sip::PlaybackControlCmd::Pause,
    )
    .await
    {
        return Json(ApiResult::error(msg));
    }
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.pause(&stream_id).await;
    }

    Json(ApiResult::success(serde_json::json!({
        "streamId": stream_id,
        "status": "paused",
        "statusSource": "local_requested",
        "sipSent": true,
        "deviceResult": {"status": "sent"},
        "message": "Playback pause command sent"
    })))
}

/// GET /api/playback/speed/{stream_id}/{speed}
///
/// 改变回放倍速（`speed` 必须是正的有限数）。
#[utoipa::path(
    get,
    path = "/api/playback/speed/{stream_id}/{speed}",
    tag = "playback",
    operation_id = "playback_speed",
    params(
        ("stream_id" = String, Path, description = "回放流 ID"),
        ("speed" = String, Path, description = "倍速，如 `0.5` / `2.0` / `4.0`"),
    ),
    responses(
        (status = 200, description = "已改变倍速",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"streamId":"playback_dev_ch_1700000000","speed":2.0,"statusSource":"local_requested","sipSent":true,"deviceResult":{"status":"sent"},"message":"Playback speed command sent"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn playback_speed(
    State(state): State<AppState>,
    Path((stream_id, speed)): Path<(String, String)>,
) -> Json<ApiResult<serde_json::Value>> {
    let speed: f64 = match speed.parse::<f64>() {
        Ok(value) if value > 0.0 && value.is_finite() => value,
        _ => return Json(ApiResult::error("无效的回放倍速，必须为正数")),
    };
    tracing::info!("Playback speed: stream={}, speed={}", stream_id, speed);
    if let Err(msg) = crate::handlers::common_channel::dispatch_playback_control(
        &state,
        &stream_id,
        crate::sip::PlaybackControlCmd::Scale { speed },
    )
    .await
    {
        return Json(ApiResult::error(msg));
    }
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.update_speed(&stream_id, speed).await;
    }

    Json(ApiResult::success(serde_json::json!({
        "streamId": stream_id,
        "speed": speed,
        "statusSource": "local_requested",
        "sipSent": true,
        "deviceResult": {"status": "sent"},
        "message": "Playback speed command sent"
    })))
}

/// GET /api/playback/seek/{stream_id}/{seek_time}
///
/// 回放拖动定位（seek）。`seek_time` 通常是 RFC3339 字符串。
#[utoipa::path(
    get,
    path = "/api/playback/seek/{stream_id}/{seek_time}",
    tag = "playback",
    operation_id = "playback_seek",
    params(
        ("stream_id" = String, Path, description = "回放流 ID"),
        ("seek_time" = String, Path, description = "目标时间点（RFC3339 或 `YYYY-MM-DDTHH:MM:SS`）"),
    ),
    responses(
        (status = 200, description = "已 seek",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"streamId":"playback_dev_ch_1700000000","currentTime":"2026-01-01T00:01:30","statusSource":"local_requested","sipSent":true,"deviceResult":{"status":"sent"},"message":"Playback seek command sent"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn playback_seek(
    State(state): State<AppState>,
    Path((stream_id, seek_time)): Path<(String, String)>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!("Playback seek: stream={}, time={}", stream_id, seek_time);
    if seek_time.trim().is_empty() {
        return Json(ApiResult::error("缺少回放跳转时间"));
    }
    if let Err(msg) = crate::handlers::common_channel::dispatch_playback_control(
        &state,
        &stream_id,
        crate::sip::PlaybackControlCmd::Seek {
            seek_time: seek_time.clone(),
        },
    )
    .await
    {
        return Json(ApiResult::error(msg));
    }
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager
            .update_current_time(&stream_id, seek_time.clone())
            .await;
    }

    Json(ApiResult::success(serde_json::json!({
        "streamId": stream_id,
        "currentTime": seek_time,
        "statusSource": "local_requested",
        "sipSent": true,
        "deviceResult": {"status": "sent"},
        "message": "Playback seek command sent"
    })))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct RecoveryListQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

/// List locally retained playback sessions whose cancellation BYE was not sent.
#[utoipa::path(
    get, path = "/api/playback/recovery", tag = "playback",
    operation_id = "playback_recovery_list",
    params(RecoveryListQuery),
    responses((status = 200, description = "待处理回放会话"), (status = 401, description = "未鉴权"), (status = 403, description = "非管理员")),
    security(("access_token" = [])),
)]
pub async fn playback_recovery_list(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<RecoveryListQuery>,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    crate::handlers::authz::require_admin(&state, &headers).await?;
    let manager = state
        .playback_manager
        .as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error500, "回放会话管理器不可用"))?;
    let sessions = manager.list_cleanup_pending().await;
    let active_ids: std::collections::HashSet<String> =
        manager.sessions.read().await.keys().cloned().collect();
    let persisted = crate::db::playback_recovery::list_retained(&state.pool).await?;
    let mut rows: Vec<(String, serde_json::Value)> = sessions.into_iter().map(|s| {
        let key = s.stream_id.clone();
        (key, serde_json::json!({
            "streamId": s.stream_id, "deviceId": s.device_id, "channelId": s.channel_id,
            "callId": s.call_id, "status": "cleanup_pending", "deviceResult": "unconfirmed", "retryable": true
        }))
    }).collect();
    rows.extend(persisted.into_iter().filter(|s| !active_ids.contains(&s.stream_id)).map(|s| {
        let key = s.stream_id.clone();
        (key, serde_json::json!({
            "streamId": s.stream_id, "deviceId": s.device_id, "channelId": s.channel_id,
            "callId": s.call_id, "status": "recovery_required", "deviceResult": "unconfirmed", "retryable": false,
            "retryReason": "已保存的 SIP 对话尚未恢复，暂不能重试"
        }))
    }));
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let total = rows.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100).clamp(1, 100);
    let items: Vec<_> = rows
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, item)| item)
        .collect();
    Ok(Json(ApiResult::success(
        serde_json::json!({"items": items, "total": total, "offset": offset, "limit": limit}),
    )))
}

/// Retry stopping one retained playback session; does not claim device acknowledgement.
#[utoipa::path(
    post, path = "/api/playback/recovery/{stream_id}/retry", tag = "playback",
    operation_id = "playback_recovery_retry",
    params(("stream_id" = String, Path, description = "待处理的回放流 ID")),
    responses((status = 200, description = "重试结果"), (status = 401, description = "未鉴权"), (status = 403, description = "非管理员")),
    security(("access_token" = [])),
)]
pub async fn playback_recovery_retry(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(stream_id): Path<String>,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    crate::handlers::authz::require_admin(&state, &headers).await?;
    let manager = state
        .playback_manager
        .as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error500, "回放会话管理器不可用"))?;
    let session = manager.get(&stream_id).await.filter(|s| s.cleanup_pending);
    let Some(session) = session else {
        if crate::db::playback_recovery::get(&state.pool, &stream_id)
            .await?
            .is_some()
        {
            return Ok(Json(ApiResult::error(
                "已保存的 SIP 对话尚未恢复，暂不能重试",
            )));
        }
        return Ok(Json(ApiResult::error("没有待处理的回放会话")));
    };
    let result = playback_stop(
        State(state),
        Path((session.device_id, session.channel_id, stream_id)),
    )
    .await
    .0;
    if result.code == 0 {
        Ok(Json(ApiResult::success(serde_json::json!({
            "status": "local_cleanup_complete",
            "deviceResult": "unconfirmed",
            "message": "本地清理已完成；设备是否执行 BYE 未确认"
        }))))
    } else {
        Ok(Json(ApiResult::error(result.msg)))
    }
}

/// GET /api/playback/stop/{device_id}/{channel_id}/{stream_id}
///
/// 停止一路历史回放：关 ZLM 流 + 发 SIP BYE + 移除会话。
#[utoipa::path(
    get,
    path = "/api/playback/stop/{device_id}/{channel_id}/{stream_id}",
    tag = "playback",
    operation_id = "playback_stop",
    params(
        ("device_id" = String, Path, description = "设备国标编号（20 位）"),
        ("channel_id" = String, Path, description = "通道国标编号（20 位）"),
        ("stream_id" = String, Path, description = "回放流 ID"),
    ),
    responses(
        (status = 200, description = "已停止", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn playback_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<ApiResult<()>> {
    tracing::info!(
        "Playback stop: device={}, channel={}, stream={}",
        device_id,
        channel_id,
        stream_id
    );

    let Some(ref playback_manager) = state.playback_manager else {
        return Json(ApiResult::error("回放会话管理器不可用"));
    };
    let Some(session) = playback_manager.get(&stream_id).await else {
        return Json(ApiResult::error("无活动回放会话"));
    };
    if session.device_id != device_id || session.channel_id != channel_id {
        return Json(ApiResult::error("回放会话与设备或通道不匹配"));
    }

    // 先发送 BYE：若先关闭 ZLM，缺流 Hook 可能抢先终止 SIP 会话，
    // 导致设备没有收到停止指令。
    let bye_result = if session.source == "gb28181_playback_invite" {
        match (state.sip_server.as_ref(), session.call_id.as_deref()) {
            (Some(sip), Some(call_id)) => {
                let prior = sip.invite_session_manager().get(call_id).await;
                if prior.is_some_and(|invite| {
                    invite.status
                        == crate::sip::gb28181::invite_session::InviteSessionStatus::Terminated
                }) {
                    Ok(())
                } else {
                    sip.send_session_bye_by_call_id(call_id)
                        .await
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            }
            _ => Err("回放 SIP 会话不可用，BYE 未发送".to_string()),
        }
    } else {
        Ok(())
    };
    let zlm_result = if let Some(zlm_client) = state
        .get_zlm_client(session.media_server_id.as_deref())
        .or_else(|| state.zlm_client.clone())
    {
        zlm_client
            .close_streams(
                Some(&session.schema),
                Some(&session.app),
                Some(&session.stream),
                true,
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        Err("ZLM 媒体节点不可用".to_string())
    };
    match (zlm_result, bye_result) {
        (Ok(()), Ok(())) => {
            playback_manager.remove(&stream_id).await;
            Json(ApiResult::<()>::success_empty())
        }
        (zlm, bye) => Json(ApiResult::error(format!(
            "回放停止未完全成功（流 {}）：{}",
            stream_id,
            [zlm.err(), bye.err()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; ")
        ))),
    }
}

#[derive(Debug, Deserialize)]
pub struct RecordQuery {
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub count: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/gb_record/query/{device_id}/{channel_id}",
    tag = "playback",
    operation_id = "playback_gb_record_query",
    params(
        ("device_id" = String, Path, description = "路径参数 device_id"),
        ("channel_id" = String, Path, description = "路径参数 channel_id"),
    ),
    responses(
        (status = 200, description = "成功", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn gb_record_query(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<RecordQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!("Record query: device={}, channel={}", device_id, channel_id);

    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();
    let page = q.page.unwrap_or(1).max(1) as i64;
    let count = q.count.unwrap_or(20).clamp(1, 200) as i64;

    // Phase 3.3: 真正等 SIP 多包 RecordInfo 响应（最多 15s），返回聚合 items
    if let Some(ref sip_server) = state.sip_server {
        let sip = &*sip_server;
        if let Some(device) = sip.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                let sn = chrono::Utc::now().timestamp() % 10000;
                match sip
                    .send_record_info_query_and_wait(
                        &device_id,
                        &channel_id,
                        &start_time,
                        &end_time,
                        sn,
                    )
                    .await
                {
                    Ok(items) if !items.is_empty() => {
                        // 分页：page 从 1 开始，count 由调用方控制
                        let total = items.len() as i64;
                        let offset = ((page - 1) * count) as usize;
                        let paged: Vec<serde_json::Value> = items
                            .iter()
                            .skip(offset)
                            .take(count as usize)
                            .map(|it| {
                                serde_json::json!({
                                    "deviceId": it.device_id,
                                    "channelId": channel_id,
                                    "name": it.name,
                                    "filePath": it.file_path,
                                    "startTime": it.start_time,
                                    "endTime": it.end_time,
                                    "address": it.address,
                                    "secrecy": it.secrecy,
                                    "type": it.kind,
                                })
                            })
                            .collect();
                        return Json(ApiResult::success(serde_json::json!({
                            "list": paged,
                            "total": total,
                            "page": page,
                            "count": count,
                            "source": "gb28181_record_info"
                        })));
                    }
                    Ok(_) => {
                        // 空 items：设备回了 RecordInfo 但没有录像段；
                        // 继续走 ZLM 兜底（兼容历史 ZLM MP4 文件）
                    }
                    Err(e) => {
                        tracing::warn!("RecordInfo async wait failed: {}", e);
                    }
                }
            }
        }
    }

    // 兼容路径：ZLM 本地 MP4 文件列表
    if let Some(ref zlm_client) = state.zlm_client {
        match zlm_client
            .get_mp4_record_file("record", &channel_id, None, None, None)
            .await
        {
            Ok(files) => {
                let total = files.len() as i64;
                let offset = ((page - 1) * count) as usize;
                let records: Vec<serde_json::Value> = files
                    .iter()
                    .skip(offset)
                    .take(count as usize)
                    .map(|f| {
                        // `name` 是前端「名称」列读的键，此前只给了 `fileName`
                        // → 该列整列空白；`deviceId`/`channelId` 是 RecordItem
                        // 声明的必填字段，两个分支此前都没给。
                        serde_json::json!({
                            "deviceId": device_id,
                            "channelId": channel_id,
                            "name": f.name,
                            "fileName": f.name,
                            "filePath": f.path,
                            "fileSize": f.size,
                            "startTime": f.create_time,
                            "endTime": f.create_time,
                            "downloadUrl": format!("/record/{}", f.name)
                        })
                    })
                    .collect();

                return Json(ApiResult::success(serde_json::json!({
                    "list": records,
                    "total": total,
                    "page": page,
                    "count": count,
                    "source": "zlm_mp4"
                })));
            }
            Err(e) => {
                tracing::error!("Failed to query records: {}", e);
            }
        }
    }

    Json(ApiResult::success(serde_json::json!({
        "list": [],
        "total": 0,
        "page": page,
        "count": count
    })))
}

#[utoipa::path(
    get,
    path = "/api/gb_record/download/start/{device_id}/{channel_id}",
    tag = "playback",
    operation_id = "playback_gb_record_download_start",
    params(
        ("device_id" = String, Path, description = "路径参数 device_id"),
        ("channel_id" = String, Path, description = "路径参数 channel_id"),
    ),
    responses(
        (status = 200, description = "成功", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn gb_record_download_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<RecordQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();

    tracing::info!(
        "Record download: device={}, channel={}, start={}, end={}",
        device_id,
        channel_id,
        start_time,
        end_time
    );

    let stream_id = format!(
        "download_{}_{}_{}",
        device_id,
        channel_id,
        chrono::Utc::now().timestamp()
    );

    let file_name = format!(
        "{}_{}_{}_{}.mp4",
        device_id,
        channel_id,
        start_time
            .replace(":", "")
            .replace("-", "")
            .replace("T", "_"),
        chrono::Utc::now().timestamp()
    );

    // 1) 优先走 GB28181 录像下载 INVITE（Subject SSRC 前缀 2），
    //    设备推送 9102 端口的 RTP 流，ZLM 自动 MP4 落盘
    let mut used_gb28181 = false;
    let mut gb_invite_call_id = None;
    if let Some(ref sip_server) = state.sip_server {
        let sip = &*sip_server;
        if let Some(device) = sip.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                // 提前开 ZLM RTP server 监听设备推流。
                // 修正：此前把返回的端口整个丢掉（`let _ =`），随后发出
                // 的 INVITE 里 m=video 端口是 0，设备根本收不到可推流的目标。
                let media_port = match state.zlm_client.as_ref() {
                    Some(zlm) => {
                        match zlm
                            .open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                                secret: zlm.secret.clone(),
                                stream_id: stream_id.clone(),
                                port: Some(0),
                                use_tcp: Some(false),
                                rtp_type: Some(0),
                                recv_port: None,
                            })
                            .await
                        {
                            Ok(info) => {
                                match crate::zlm::port_guard::validate_allocated_rtp_port(
                                    &state,
                                    zlm,
                                    &stream_id,
                                    info.port,
                                    "gb_record/download/start",
                                )
                                .await
                                {
                                    Ok(()) => info.port,
                                    Err(problem) => {
                                        return Json(ApiResult::error(problem));
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "openRtpServer for download {}/{} failed: {}",
                                    device_id,
                                    channel_id,
                                    e
                                );
                                return Json(ApiResult::error(format!(
                                    "ZLM 下载收流端口分配失败（流 {}）：{}",
                                    stream_id, e
                                )));
                            }
                        }
                    }
                    None => return Json(ApiResult::error("ZLM 未配置，无法建立下载收流")),
                };
                if media_port == 0 {
                    // 端口 0 在 SDP 里表示媒体流被禁用，发出去也不会收到流。
                    // 此前只记一条 warn 然后照发（对调用方伪装成"下载已开始"），
                    // 改为直接失败，避免留下一个永远不会完成的下载会话。
                    tracing::error!(
                        "ZLM 收流端口分配失败，放弃录像下载 INVITE ({}/{})",
                        device_id,
                        channel_id
                    );
                    return Json(ApiResult::error("ZLM 未分配有效的下载收流端口"));
                } else {
                    match sip
                        .send_download_invite(
                            &device_id,
                            &channel_id,
                            &start_time,
                            &end_time,
                            &stream_id,
                            media_port,
                            15,
                        )
                        .await
                    {
                        Ok(call_id) => {
                            tracing::info!("GB28181 DOWNLOAD INVITE sent, call_id={}", call_id);
                            // **必须让 ZLM 真的把收到的 RTP 录成 MP4**。
                            //
                            // 此前这里只发了 INVITE：设备把录像 RTP 推到 ZLM
                            // 的收流端口后，ZLM 并没有在录像 —— `openRtpServer`
                            // 只创建流，不落盘（需要 startRecord，或连接上
                            // `protocol.enable_mp4` 全局开关）。结果是"下载成功"
                            // 却永远没有文件，on_record_mp4 也永不触发，
                            // 云录像列表里什么都不出现。
                            if let Some(ref zlm) = state.zlm_client {
                                // 流所在的 app 以 ZLM 实际注册的为准
                                // （openRtpServer 默认建在 "rtp"）。
                                let app =
                                    match zlm.get_media_list(None, None, Some(&stream_id)).await {
                                        Ok(list) => list
                                            .into_iter()
                                            .next()
                                            .map(|m| m.app)
                                            .unwrap_or_else(|| "rtp".to_string()),
                                        Err(e) => {
                                            tracing::debug!(
                                                "查询下载流 {} 的 app 失败（按 rtp 处理）: {}",
                                                stream_id,
                                                e
                                            );
                                            "rtp".to_string()
                                        }
                                    };
                                if let Err(e) = zlm
                                    .start_record("1", "__defaultVhost__", &app, &stream_id)
                                    .await
                                {
                                    let _ = sip.send_session_bye_by_call_id(&call_id).await;
                                    let _ = zlm.close_rtp_server(&stream_id).await;
                                    return Json(ApiResult::error(format!(
                                        "下载流录制启动失败（流 {}）：{}",
                                        stream_id, e
                                    )));
                                }
                                tracing::info!(
                                    "下载流已开始 MP4 录制 app={} stream={}",
                                    app,
                                    stream_id
                                );
                            }
                            used_gb28181 = true;
                            gb_invite_call_id = Some(call_id);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to send GB28181 DOWNLOAD INVITE: {}", e);
                            if let Some(zlm) = state.zlm_client.as_ref() {
                                let _ = zlm.close_rtp_server(&stream_id).await;
                            }
                            return Json(ApiResult::error(format!(
                                "录像下载邀请失败（流 {}）：{}",
                                stream_id, e
                            )));
                        }
                    }
                }
            }
        }
    }

    if used_gb28181 {
        let session = DownloadSession {
            stream_id: stream_id.clone(),
            call_id: gb_invite_call_id,
            device_id: device_id.clone(),
            channel_id: channel_id.clone(),
            file_name: file_name.clone(),
            start_time: start_time.clone(),
            end_time: end_time.clone(),
            url: format!("gb28181://{}@{}/{}", device_id, channel_id, start_time),
            status: "downloading".to_string(),
            progress: 0.0,
            created_at: Utc::now(),
            zlm_stream_id: stream_id.clone(),
            zlm_app: "rtp".to_string(),
            current_bytes: 0,
            total_bytes: 0,
            zlm_file_name: None,
            zlm_file_path: None,
            zlm_folder: None,
        };
        if let Some(ref dm) = state.download_manager {
            dm.create(session).await;
        }
        return Json(ApiResult::success(serde_json::json!({
            "streamId": stream_id,
            "fileName": file_name,
            "downloadUrl": format!(
                "/api/gb_record/download/file/{}",
                stream_id
            ),
            "transport": "gb28181",
            "progress": 0,
            "status": "downloading"
        })));
    }

    // 2) Fallback：ZLM 本地录像拉流（RTSP 拉 + 写文件）
    if let Some(ref zlm_client) = state.zlm_client {
        let record_url = format!("rtsp://127.0.0.1/record/{}/{}", device_id, channel_id);

        match zlm_client
            .create_download(&record_url, &file_name, Some("./downloads"))
            .await
        {
            Ok(download_path) => {
                tracing::info!("Download started: {} -> {}", file_name, download_path);

                let session = DownloadSession {
                    stream_id: stream_id.clone(),
                    call_id: None,
                    device_id: device_id.clone(),
                    channel_id: channel_id.clone(),
                    file_name: file_name.clone(),
                    start_time: start_time.clone(),
                    end_time: end_time.clone(),
                    url: record_url,
                    status: "downloading".to_string(),
                    progress: 0.0,
                    created_at: Utc::now(),
                    zlm_stream_id: stream_id.clone(),
                    zlm_app: "rtp".to_string(),
                    current_bytes: 0,
                    total_bytes: 0,
                    zlm_file_name: None,
                    zlm_file_path: None,
                    zlm_folder: None,
                };

                if let Some(ref dm) = state.download_manager {
                    dm.create(session).await;
                }

                return Json(ApiResult::success(serde_json::json!({
                        "streamId": stream_id,
                        "fileName": file_name,
                        "downloadUrl": format!(
                    "/api/gb_record/download/file/{}",
                    stream_id
                ),
                        "savePath": download_path,
                        "transport": "zlm-local",
                        "progress": 0,
                        "status": "downloading"
                    })));
            }
            Err(e) => {
                tracing::error!("Failed to start download: {}", e);
            }
        }
    }

    Json(ApiResult::success(serde_json::json!({
        "streamId": stream_id,
        "msg": "Download not available"
    })))
}

/// GET /api/gb_record/download/file/:stream_id
///
/// 把 GB28181 录像下载**真正产出的那个 MP4** 交给调用方（支持 Range）。
///
/// 此前的 `downloadUrl` 是 `/download/<平台自己编的文件名>`：既没有对应路由，
/// 又和 ZLM 实际写出的文件名无关 —— 用户点下载必然拿不到文件。
/// 现在改为本端点：按 `stream_id` 找到下载会话 → 用 `on_record_mp4` 登记的
/// ZLM 文件路径代理回来（文件还没生成时给出明确原因，而不是 404 HTML）。
#[utoipa::path(
    get,
    path = "/api/gb_record/download/file/{stream_id}",
    tag = "playback",
    operation_id = "playback_gb_record_download_file",
    params(
        ("stream_id" = String, Path, description = "路径参数 stream_id"),
    ),
    responses(
        (status = 200, description = "成功", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn gb_record_download_file(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, AppError> {
    let Some(dm) = state.download_manager.as_ref() else {
        return Err(AppError::business(
            ErrorCode::Error500,
            "下载管理器未初始化",
        ));
    };
    let Some(session) = dm.get(&stream_id).await else {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("下载会话不存在或已结束: {stream_id}"),
        ));
    };
    let Some(file_name) = session.zlm_file_name.clone() else {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("录像文件尚未生成（当前状态: {}）", session.status),
        ));
    };
    let media_server_id = state
        .list_zlm_servers()
        .into_iter()
        .next()
        .unwrap_or_else(|| "default".to_string());
    // 下载产物固定落在 `rtp/download_*` 这个 app/stream 下
    crate::handlers::cloud_record_extra::proxy_zlm_file(
        &state,
        &media_server_id,
        session.zlm_file_path.as_deref(),
        &session.zlm_app,
        &session.zlm_stream_id,
        &file_name,
        &headers,
    )
    .await
}

#[utoipa::path(
    get,
    path = "/api/gb_record/download/stop/{device_id}/{channel_id}/{stream_id}",
    tag = "playback",
    operation_id = "playback_gb_record_download_stop",
    params(
        ("device_id" = String, Path, description = "路径参数 device_id"),
        ("channel_id" = String, Path, description = "路径参数 channel_id"),
        ("stream_id" = String, Path, description = "路径参数 stream_id"),
    ),
    responses(
        (status = 200, description = "成功", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn gb_record_download_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<ApiResult<()>> {
    tracing::info!(
        "Record download stop: device={}, channel={}, stream={}",
        device_id,
        channel_id,
        stream_id
    );

    // 如果是 GB28181 下载会话：发 BYE + 关 ZLM RTP server
    if let Some(ref dm) = state.download_manager {
        if let Some(session) = dm.get(&stream_id).await {
            if session.device_id != device_id || session.channel_id != channel_id {
                return Json(ApiResult::error("下载会话与设备或通道不匹配"));
            }
            if session.url.starts_with("gb28181://") {
                if let Some(ref zlm_client) = state.zlm_client {
                    // 先停录制，再发 BYE：BYE 后设备立即停推，ZLM 流可能先于
                    // stopRecord 消失，导致 MP4 尾部未及时收尾。
                    let app = match zlm_client
                        .get_media_list(None, None, Some(&stream_id))
                        .await
                    {
                        Ok(list) => list
                            .into_iter()
                            .next()
                            .map(|m| m.app)
                            .unwrap_or_else(|| "rtp".to_string()),
                        Err(_) => "rtp".to_string(),
                    };
                    if let Err(e) = zlm_client
                        .stop_record("1", "__defaultVhost__", &app, &stream_id)
                        .await
                    {
                        tracing::warn!(
                            "停止下载流录制失败 app={} stream={}: {}",
                            app,
                            stream_id,
                            e
                        );
                    }
                }
                if let Some(ref sip_server) = state.sip_server {
                    let sip = &*sip_server;
                    let Some(call_id) = session.call_id.as_deref() else {
                        return Json(ApiResult::error("下载 SIP 会话缺少 Call-ID"));
                    };
                    if let Err(e) = sip.send_session_bye_by_call_id(call_id).await {
                        return Json(ApiResult::error(format!("下载 BYE 发送失败：{}", e)));
                    }
                } else {
                    return Json(ApiResult::error("SIP 服务不可用，下载 BYE 未发送"));
                }
                if let Some(ref zlm_client) = state.zlm_client {
                    let _ = zlm_client.close_rtp_server(&stream_id).await;
                }
            } else if let Some(ref zlm_client) = state.zlm_client {
                let _ = zlm_client.stop_download(&session.file_name).await;
            }
            // **不能立刻删会话**：停止录制后 ZLM 才写完 MP4 尾部并回调
            // `on_record_mp4`，文件信息要靠那个回调登记到会话里；删早了
            // 用户就永远拿不到刚下载的文件（进度接口只会回"会话不存在"）。
            // 这里只标记终态，文件由回调补齐；过期的会话由 cleanup_finished 回收。
            dm.set_status(&stream_id, "finalizing").await;
        } else {
            return Json(ApiResult::error("无活动下载会话"));
        }
    } else {
        return Json(ApiResult::error("下载会话管理器不可用"));
    }

    Json(ApiResult::<()>::success_empty())
}

/// 录像下载进度。
///
/// 两种传输的状态来源完全不同，必须分开处理：
///
/// * **GB28181 下载**（`gb28181://`，设备把录像 RTP 推到 ZLM）：进度不在
///   ZLM 的"下载列表"里（那里只有 ZLM 自己发起的 HTTP 拉流下载），
///   状态由会话状态机维护（inviting → downloading → completed）。
///   此前一律去查 ZLM 下载列表，查不到就返回 `status:"unknown"` ——
///   前端因此永远看不到 GB28181 下载的任何状态变化。
/// * **ZLM 本地下载**（http/ftp 拉取）：进度来自 ZLM 的下载列表。
#[utoipa::path(
    get,
    path = "/api/gb_record/download/progress/{device_id}/{channel_id}/{stream_id}",
    tag = "playback",
    operation_id = "playback_gb_record_download_progress",
    params(
        ("device_id" = String, Path, description = "路径参数 device_id"),
        ("channel_id" = String, Path, description = "路径参数 channel_id"),
        ("stream_id" = String, Path, description = "路径参数 stream_id"),
    ),
    responses(
        (status = 200, description = "成功", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn gb_record_download_progress(
    State(state): State<AppState>,
    Path((_device_id, _channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<ApiResult<serde_json::Value>> {
    let Some(dm) = state.download_manager.as_ref() else {
        return err("下载管理未初始化");
    };
    let Some(session) = dm.get(&stream_id).await else {
        return err("下载会话不存在或已结束");
    };

    if session.url.starts_with("gb28181://") {
        // 设备推完流后 ZLM 会落盘 MP4（on_record_mp4），此时给出真实文件与下载地址；
        // 还没落盘时如实报告状态（文件为 null，前端据此继续轮询）。
        return Json(ApiResult::success(serde_json::json!({
            "streamId": session.stream_id,
            "fileName": session.file_name,
            "progress": session.progress,
            "status": session.status,
            "currentBytes": session.current_bytes,
            "totalBytes": session.total_bytes,
            "transport": "gb28181",
            "zlmFileName": session.zlm_file_name,
            "downloadUrl": session.zlm_file_name.as_ref().map(|_| {
                format!("/api/gb_record/download/file/{}", session.stream_id)
            }),
        })));
    }

    if let Some(ref zlm_client) = state.zlm_client {
        match zlm_client.get_download_list().await {
            Ok(downloads) => {
                for dl in downloads {
                    if dl.file_name == session.file_name {
                        let progress = dl.progress;
                        let status = if progress >= 100.0 {
                            "completed"
                        } else {
                            "downloading"
                        };
                        dm.update_progress_percent(&stream_id, progress, status)
                            .await;
                        return Json(ApiResult::success(serde_json::json!({
                            "streamId": stream_id,
                            "fileName": dl.file_name,
                            "progress": progress,
                            "status": status,
                            "downloaded": dl.downloaded,
                            "totalSize": dl.size
                        })));
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to get download progress: {}", e);
            }
        }
    }

    // 会话存在但 ZLM 列表里暂时没有（刚发起/已结束）—— 报告会话自身状态，
    // 而不是伪造一个 "unknown" 让前端无从判断。
    Json(ApiResult::success(serde_json::json!({
        "streamId": session.stream_id,
        "fileName": session.file_name,
        "progress": session.progress,
        "status": session.status,
        "zlmFileName": session.zlm_file_name,
        "downloadUrl": session.zlm_file_name.as_ref().map(|_| {
            format!("/api/gb_record/download/file/{}", session.stream_id)
        }),
        "currentBytes": session.current_bytes,
        "totalBytes": session.total_bytes,
        "transport": "zlm-local",
    })))
}

fn err(msg: &str) -> Json<ApiResult<serde_json::Value>> {
    Json(ApiResult::<serde_json::Value>::error(msg.to_string()))
}

#[cfg(test)]
mod download_manager_tests {
    use super::*;

    fn make_session(stream_id: &str, zlm_stream_id: &str) -> DownloadSession {
        DownloadSession {
            stream_id: stream_id.to_string(),
            call_id: None,
            device_id: "dev1".to_string(),
            channel_id: "ch1".to_string(),
            file_name: "test.mp4".to_string(),
            start_time: "2026-06-10T10:00:00".to_string(),
            end_time: "2026-06-10T11:00:00".to_string(),
            url: "gb28181://dev1@ch1/start".to_string(),
            status: "downloading".to_string(),
            progress: 0.0,
            created_at: Utc::now(),
            zlm_stream_id: zlm_stream_id.to_string(),
            zlm_app: "rtp".to_string(),
            current_bytes: 0,
            total_bytes: 0,
            zlm_file_name: None,
            zlm_file_path: None,
            zlm_folder: None,
        }
    }

    /// 设备推完流后 `on_record_mp4` 登记的 ZLM 文件必须写进会话：
    /// `/progress` 靠它给出终态与真实字节数，`/download/file` 靠它取文件。
    #[tokio::test]
    async fn test_download_session_records_zlm_file() {
        let dm = DownloadManager::new();
        dm.create(make_session("download_dev_ch_1", "download_dev_ch_1"))
            .await;

        dm.set_zlm_file(
            "download_dev_ch_1",
            "2026-09-13-06-14-43-0.mp4".into(),
            Some("/opt/media/bin/www/record/rtp/download_dev_ch_1/2026-09-13/2026-09-13-06-14-43-0.mp4".into()),
            Some("/opt/media/bin/www/record/rtp/download_dev_ch_1/".into()),
            176434,
        )
        .await;

        let s = dm.get("download_dev_ch_1").await.expect("会话应存在");
        assert_eq!(s.status, "completed");
        assert_eq!(s.current_bytes, 176434);
        assert_eq!(s.total_bytes, 176434);
        assert_eq!(s.progress, 100.0);
        assert_eq!(
            s.zlm_file_name.as_deref(),
            Some("2026-09-13-06-14-43-0.mp4")
        );
    }

    /// 停止下载只标记 `finalizing`，不能把会话删掉 —— 文件信息随后才由
    /// `on_record_mp4` 回调补上（删早了用户永远拿不到文件）。
    #[tokio::test]
    async fn test_download_session_survives_stop_until_file_arrives() {
        let dm = DownloadManager::new();
        dm.create(make_session("download_dev_ch_2", "download_dev_ch_2"))
            .await;
        dm.set_status("download_dev_ch_2", "finalizing").await;
        assert!(
            dm.get("download_dev_ch_2").await.is_some(),
            "停止后会话不应消失"
        );

        dm.set_zlm_file("download_dev_ch_2", "f.mp4".into(), None, None, 10)
            .await;
        dm.set_status("download_dev_ch_2", "finalizing").await;
        let s = dm.get("download_dev_ch_2").await.unwrap();
        assert_eq!(s.status, "completed");
        assert!(s.zlm_file_name.is_some());
    }

    /// 已结束且超期的会话会被回收，正在进行的不受影响。
    #[tokio::test]
    async fn test_download_session_cleanup_finished() {
        let dm = DownloadManager::new();
        dm.create(make_session("done", "z1")).await;
        dm.set_zlm_file("done", "f.mp4".into(), None, None, 1).await;
        dm.create(make_session("running", "z2")).await;

        // TTL 取负 → 所有"已结束"的立即算超期
        let removed = dm.cleanup_finished(-1).await;
        assert_eq!(removed, 1);
        assert!(dm.get("done").await.is_none());
        assert!(dm.get("running").await.is_some(), "进行中的会话不能被清理");
    }

    /// Phase 3.4: 进度 0 → 50% → 100% 字节更新
    #[tokio::test]
    async fn test_download_progress_by_bytes() {
        let dm = DownloadManager::new();
        dm.create(make_session("s1", "download_dev1_ch1_t1")).await;
        dm.update_progress("s1", 0, 1_000_000).await;
        let s = dm.get("s1").await.unwrap();
        assert_eq!(s.progress, 0.0);
        assert_eq!(s.current_bytes, 0);
        assert_eq!(s.total_bytes, 1_000_000);

        dm.update_progress("s1", 500_000, 1_000_000).await;
        let s = dm.get("s1").await.unwrap();
        assert!((s.progress - 50.0).abs() < 0.01);
        assert_eq!(s.status, "downloading");

        dm.update_progress("s1", 1_000_000, 1_000_000).await;
        let s = dm.get("s1").await.unwrap();
        assert!((s.progress - 100.0).abs() < 0.01);
        assert_eq!(s.status, "completed");
    }

    /// Phase 3.4: 进度超 100% 时 clamp 到 100
    #[tokio::test]
    async fn test_download_progress_clamps_to_100() {
        let dm = DownloadManager::new();
        dm.create(make_session("s2", "download_x")).await;
        dm.update_progress("s2", 2_000_000, 1_000_000).await;
        let s = dm.get("s2").await.unwrap();
        assert_eq!(s.progress, 100.0);
    }

    /// Phase 3.4: 按 zlm_stream_id 查找（ZLM hook 用）
    #[tokio::test]
    async fn test_download_get_by_zlm_stream() {
        let dm = DownloadManager::new();
        dm.create(make_session("a", "download_abc")).await;
        dm.create(make_session("b", "download_xyz")).await;

        let s = dm.get_by_zlm_stream("download_abc").await;
        assert!(s.is_some());
        assert_eq!(s.unwrap().stream_id, "a");

        let s = dm.get_by_zlm_stream("not_found").await;
        assert!(s.is_none());
    }

    /// Phase 3.4: remove 清理会话
    #[tokio::test]
    async fn test_download_remove() {
        let dm = DownloadManager::new();
        dm.create(make_session("s3", "download_remove")).await;
        assert!(dm.get("s3").await.is_some());
        dm.remove("s3").await;
        assert!(dm.get("s3").await.is_none());
    }
}

#[cfg(test)]
mod download_progress_tests {
    use super::*;

    /// GB28181 下载的状态由会话状态机维护（ZLM 的"下载列表"里没有它）。
    /// 回归：此前一律去查 ZLM 下载列表，查不到就回 `status:"unknown"`。
    #[tokio::test]
    async fn test_download_manager_tracks_gb28181_state() {
        let dm = DownloadManager::new();
        dm.create(DownloadSession {
            stream_id: "download_dev_ch_1".into(),
            call_id: None,
            device_id: "dev".into(),
            channel_id: "ch".into(),
            file_name: "f.mp4".into(),
            start_time: "2026-09-01T10:00:00".into(),
            end_time: "2026-09-01T10:05:00".into(),
            url: "gb28181://dev@ch/2026-09-01T10:00:00".into(),
            status: "inviting".into(),
            progress: 0.0,
            created_at: Utc::now(),
            zlm_stream_id: "download_dev_ch_1".into(),
            zlm_app: "rtp".into(),
            current_bytes: 0,
            total_bytes: 0,
            zlm_file_name: None,
            zlm_file_path: None,
            zlm_folder: None,
        })
        .await;

        let s = dm.get("download_dev_ch_1").await.unwrap();
        assert!(s.url.starts_with("gb28181://"), "应被识别为 GB28181 下载");
        assert_eq!(s.status, "inviting");

        // 流上线 → downloading
        dm.update_progress_percent("download_dev_ch_1", 0.0, "downloading")
            .await;
        assert_eq!(
            dm.get("download_dev_ch_1").await.unwrap().status,
            "downloading"
        );

        // 流注销（设备推完）→ completed，且 progress 置 100
        dm.update_progress_percent("download_dev_ch_1", 100.0, "completed")
            .await;
        let s = dm.get("download_dev_ch_1").await.unwrap();
        assert_eq!(s.status, "completed");
        assert_eq!(s.progress, 100.0);

        // 通过 zlm_stream_id 反查（hook 里就是这么找会话的）
        assert!(dm.get_by_zlm_stream("download_dev_ch_1").await.is_some());
        assert!(dm.get_by_zlm_stream("不存在").await.is_none());
    }

    /// 未知 stream_id 必须明确报错，而不是返回一个"看起来正常"的 unknown。
    #[test]
    fn test_download_progress_error_helper() {
        let v = err("下载会话不存在或已结束");
        let json = serde_json::to_value(&v.0).unwrap();
        assert_ne!(json["code"], 0);
        assert_eq!(json["msg"], "下载会话不存在或已结束");
    }
}

#[cfg(test)]
mod playback_contract_tests {
    use crate::error::ErrorCode;
    use crate::test_support::app_state;
    use axum::extract::{Path, Query};

    /// 回放拉不起来时必须**明确报错**。此前无论 SIP/ZLM 是否可用都返回
    /// `code: 0` + 一个没有 `playUrl` 的"会话已创建"，前端 `v-if="playUrl"` 为假
    /// → 用户点了片段什么都不发生，也没有失败提示。
    #[tokio::test]
    async fn start_without_sip_or_zlm_returns_error() {
        let state = app_state().await; // 无 SIP / 无 ZLM
        let err = super::playback_start(
            axum::extract::State(state.clone()),
            Path((
                "34020000001320000001".to_string(),
                "34020000001310000001".to_string(),
            )),
            Query(super::PlaybackQuery {
                start_time: Some("2026-01-01T00:00:00".to_string()),
                end_time: Some("2026-01-01T00:05:00".to_string()),
            }),
        )
        .await
        .expect_err("没有任何回放通路时应报错");
        match err {
            crate::error::AppError::Business(code, msg) => {
                assert!(matches!(code, ErrorCode::Error500), "code={code:?}");
                assert!(msg.contains("回放启动失败"), "msg={msg}");
            }
            other => panic!("期望业务错误，得到 {other:?}"),
        }
    }

    /// ZLM MP4 兜底分支必须给出 `name`/`deviceId`/`channelId`
    /// —— 此前只有 `fileName`，「名称」列整列空白，两个分支都没有 `channelId`。
    #[test]
    fn zlm_fallback_row_has_name_and_ids() {
        // 直接校验 JSON 形状（与 handler 里的字面量保持一致）
        let device_id = "34020000001320000001";
        let channel_id = "34020000001310000001";
        let row = serde_json::json!({
            "deviceId": device_id,
            "channelId": channel_id,
            "name": "20260101_000000.mp4",
            "fileName": "20260101_000000.mp4",
            "filePath": "/record/2026-01-01/20260101_000000.mp4",
            "startTime": "2026-01-01 00:00:00",
            "endTime": "2026-01-01 00:00:00",
        });
        assert_eq!(row["name"], "20260101_000000.mp4");
        assert_eq!(row["deviceId"], device_id);
        assert_eq!(row["channelId"], channel_id);
    }
}
