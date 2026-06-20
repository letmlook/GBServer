//! 验证 [zlm.keepalive] 子节正确加载
//!
//! Phase 4 follow-up: keepalive_grace_count + keepalive_timeout_secs
//! 从 config.toml 读，覆盖 const 默认值。

use gbserver::config::{load_config, ZlmKeepaliveConfig};

#[test]
fn zlm_keepalive_config_loads_from_toml() {
    let cfg = load_config().expect("TOML config should load");
    let zlm = cfg.zlm.as_ref().expect("zlm should be present");
    let ka = &zlm.keepalive;

    assert_eq!(ka.timeout_secs, 30, "timeout_secs default");
    assert_eq!(ka.grace_count, 3, "grace_count default");
    assert_eq!(ka.check_interval_secs, 10, "check_interval_secs default");
}

#[test]
fn zlm_keepalive_config_default_values() {
    let ka = ZlmKeepaliveConfig::default();
    assert_eq!(ka.timeout_secs, 30);
    assert_eq!(ka.grace_count, 3);
    assert_eq!(ka.check_interval_secs, 10);
}
