//! 烟雾测试：验证 config/application.toml 可以被反序列化为 AppConfig
//!
//! 跑法: cargo test --test config_toml_smoke -- --nocapture

use gbserver::config::load_config;

#[test]
fn toml_config_loads_and_deserializes() {
    let cfg = load_config().expect("TOML config should load and deserialize");

    // 必填字段
    assert_eq!(cfg.server.port, 18080);
    assert!(
        cfg.database.url.starts_with("postgres://")
            || cfg.database.url.starts_with("mysql://"),
        "database.url must be a SQL URL, got: {}",
        cfg.database.url
    );
    assert_eq!(cfg.jwt.expiration_minutes, 30);

    // Optional 但已配置
    let redis = cfg.redis.as_ref().expect("redis should be present");
    assert!(redis.url.starts_with("redis://"));

    let sip = cfg.sip.as_ref().expect("sip should be present");
    assert!(sip.enabled);
    assert_eq!(sip.port, 5060);
    assert_eq!(sip.tcp_port, 5061);
    assert_eq!(sip.device_id, "34020000002000000001");
    assert_eq!(sip.realm, "3402000000");

    let sr = sip
        .stream_reconnect
        .as_ref()
        .expect("sip.stream_reconnect should be present");
    assert_eq!(sr.max_retries, 3);
    assert_eq!(sr.retry_interval_secs, 5);

    let hb = sip
        .heartbeat
        .as_ref()
        .expect("sip.heartbeat should be present");
    assert_eq!(hb.timeout_multiplier, 3);
    assert_eq!(hb.check_interval_secs, 10);

    let zlm = cfg.zlm.as_ref().expect("zlm should be present");
    assert_eq!(zlm.stream_timeout, 10);
    assert!(zlm.hook_enabled);
    assert_eq!(zlm.hook_url, "http://127.0.0.1:18080/api/zlm/hook");
    assert!(
        !zlm.servers.is_empty(),
        "zlm.servers must have at least one entry"
    );
    let first = &zlm.servers[0];
    assert_eq!(first.id, "zlmediakit-1");
    assert_eq!(first.http_port, 8080);
    assert!(first.enabled);

    let jt = cfg.jt1078.as_ref().expect("jt1078 should be present");
    assert_eq!(jt.timeout_ms, Some(60000));
    assert_eq!(jt.retransmit_wait_ms, Some(200));
}