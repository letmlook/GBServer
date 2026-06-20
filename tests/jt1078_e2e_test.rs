//! Phase 6.6: End-to-end JT1078 terminal simulator tests.
//!
//! These tests exercise the full command → response lifecycle using
//! `JtCommandWaiter` for correlation. They do NOT require a real terminal,
//! real ZLM, or real SIP device — they only validate the protocol logic.

use gbserver::jt1078::command::{self, build_jt808_frame};
use gbserver::jt1078::command_waiter::JtCommandWaiter;

/// Test 1: Register 0x0100 round-trip via JtCommandWaiter
#[tokio::test]
async fn jt_e2e_test_register_correlation() {
    let waiter = JtCommandWaiter::new().with_timeout(5);
    let (_key, rx) = waiter.register("13812340001", 0x0100, 1, Some(5));
    assert_eq!(waiter.pending_count(), 1);
    // Simulate 0x8100 register response
    assert!(waiter.complete("13812340001", 0x0100, 1, vec![0x00]));
    assert_eq!(waiter.pending_count(), 0);
    let resp = rx.await.expect("should receive response");
    assert_eq!(resp, vec![0x00u8]);
}

/// Test 2: PTZ 0x9301 send → 0x0001 ack round-trip
#[tokio::test]
async fn jt_e2e_test_ptz_send_and_ack() {
    let waiter = JtCommandWaiter::new().with_timeout(5);
    // Build PTZ frame
    let (b1, b2, h, v) = command::ptz_direction_bytes("UP", 5);
    let body = command::build_ptz_control(1, b1, b2, h, v, 0);
    let frame = build_jt808_frame(0x9301, "13812340001", 2, &body);
    assert_eq!(frame[0], 0x7E);
    assert_eq!(u16::from_be_bytes([frame[1], frame[2]]), 0x9301);
    // Register waiter
    let (_key, rx) = waiter.register("13812340001", 0x9301, 2, Some(5));
    // Simulate 0x0001 general common response
    let resolved = waiter.try_resolve_by_response("13812340001", 0x9301, 2, 0);
    assert!(resolved);
    let resp = rx.await.expect("should receive");
    assert_eq!(resp, vec![0u8]); // result=0 (success)
}

/// Test 3: Live video 0x9101 → 0x0001 ack
#[tokio::test]
async fn jt_e2e_test_live_video_correlation() {
    let waiter = JtCommandWaiter::new().with_timeout(5);
    let body = command::build_live_video_request(0, 0, false);
    let _ = build_jt808_frame(0x9101, "13812340001", 3, &body);
    let (_key, rx) = waiter.register("13812340001", 0x9101, 3, Some(5));
    waiter.try_resolve_by_response("13812340001", 0x9101, 3, 0);
    let resp = rx.await.expect("should receive");
    assert_eq!(resp, vec![0u8]);
}

/// Test 4: Heartbeat 0x0002 — no waiter needed (one-way)
#[tokio::test]
async fn jt_e2e_test_heartbeat() {
    let body = command::encode_time_bcd("2026-06-20T14:30:00");
    let frame = build_jt808_frame(0x0002, "13812340001", 4, &body);
    assert_eq!(frame[0], 0x7E);
    assert_eq!(u16::from_be_bytes([frame[1], frame[2]]), 0x0002);
}

/// Test 5: Register 0x8100 response with auth code
#[tokio::test]
async fn jt_e2e_test_register_response_with_auth_code() {
    let body = command::build_register_response_body(1, 0, "AUTH123");
    assert_eq!(body.len(), 2 + 1 + 7);
    assert_eq!(u16::from_be_bytes([body[0], body[1]]), 1);
    assert_eq!(body[2], 0); // success
    assert_eq!(&body[3..], b"AUTH123");
}

/// Test 6: 0x0001 general common response parsing
#[tokio::test]
async fn jt_e2e_test_general_common_response() {
    let waiter = JtCommandWaiter::new().with_timeout(5);
    let (_key, rx) = waiter.register("13812340001", 0x9102, 5, Some(5));
    // 0x0001 body: reply_serial(2) + reply_msg_id(2) + result(1)
    let body: Vec<u8> = vec![
        0x00, 0x05, // reply_serial = 5
        0x91, 0x02, // reply_msg_id = 0x9102
        0x00,       // result = 0 (success)
    ];
    assert!(waiter.complete("13812340001", 0x9102, 5, body.clone()));
    let received = rx.await.expect("should receive");
    assert_eq!(received, body);
}

/// Test 7: timeout cleanup removes expired waiters
#[tokio::test]
async fn jt_e2e_test_timeout_cleanup() {
    let waiter = JtCommandWaiter::new().with_timeout(0);
    waiter.register("13812340001", 0x9301, 6, Some(0));
    assert_eq!(waiter.pending_count(), 1);
    // Sleep briefly to let entry expire
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let removed = waiter.cleanup_expired();
    assert!(!removed.is_empty());
    assert_eq!(waiter.pending_count(), 0);
}

/// Test 8: Response parser — 0x0100 register request
#[test]
fn jt_e2e_test_parse_register_request() {
    use gbserver::jt1078::response_parser::parse_register_request;
    let mut body = Vec::new();
    body.extend_from_slice(&0x0100u16.to_be_bytes());
    body.extend_from_slice(&0x0200u16.to_be_bytes());
    body.extend_from_slice(b"AAAAA");
    body.extend_from_slice(&vec![b'B'; 20]);
    body.extend_from_slice(b"1234567");
    body.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90]);
    let req = parse_register_request(&body).expect("parse ok");
    assert_eq!(req.province_id, 0x0100);
    assert_eq!(req.city_id, 0x0200);
    assert_eq!(req.terminal_id, "1234567");
}

/// Test 9: 0x0200 location report parsing
#[test]
fn jt_e2e_test_parse_location_report() {
    use gbserver::jt1078::response_parser::parse_location_report;
    let mut body = vec![0u8; 28];
    body[8..12].copy_from_slice(&39_904_321u32.to_be_bytes());
    body[12..16].copy_from_slice(&116_407_215u32.to_be_bytes());
    body[22..28].copy_from_slice(&[0x26, 0x06, 0x20, 0x14, 0x30, 0x00]);
    let loc = parse_location_report(&body).expect("parse ok");
    assert!((loc.latitude - 39.904_321).abs() < 0.001);
    assert!((loc.longitude - 116.407_215).abs() < 0.001);
}

/// Test 10: 0x0107 query params response parsing
#[test]
fn jt_e2e_test_parse_query_params_response() {
    use gbserver::jt1078::response_parser::parse_query_params_response;
    let mut body = Vec::new();
    body.extend_from_slice(&0x0010u32.to_be_bytes()); // param_id
    body.push(4u8); // length
    body.extend_from_slice(&0x12345678u32.to_be_bytes()); // value
    let params = parse_query_params_response(&body).expect("parse ok");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].param_id, 0x10);
    assert_eq!(params[0].param_length, 4);
}
