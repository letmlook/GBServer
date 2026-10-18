use gbserver::config::load_config;

#[test]
fn compose_scalar_environment_overrides_primary_zlm_node() {
    let overrides = [
        ("GBSERVER_ZLM_PRIMARY_IP", "192.0.2.10"),
        ("GBSERVER_ZLM_PRIMARY_HTTP_PORT", "18082"),
        (
            "GBSERVER_ZLM_PRIMARY_HOOK_URL",
            "http://gbserver:18080/api/zlm/hook",
        ),
        ("GBSERVER_ZLM_PRIMARY_RTC_EXTERN_IP", "192.0.2.11"),
    ];
    for (key, value) in overrides {
        std::env::set_var(key, value);
    }

    let cfg = load_config().expect("Compose scalar ZLM overrides should load");
    let primary = &cfg.zlm.expect("ZLM config").servers[0];
    assert_eq!(primary.ip, "192.0.2.10");
    assert_eq!(primary.http_port, 18082);
    assert_eq!(
        primary.hook_url.as_deref(),
        Some("http://gbserver:18080/api/zlm/hook")
    );
    assert_eq!(primary.rtc_extern_ip.as_deref(), Some("192.0.2.11"));

    std::env::set_var("GBSERVER_ZLM_PRIMARY_RTC_EXTERN_IP", "");
    let host_network_cfg = load_config().expect("empty RTC override should clear the TOML value");
    assert!(host_network_cfg.zlm.unwrap().servers[0]
        .rtc_extern_ip
        .is_none());

    for (key, _) in overrides {
        std::env::remove_var(key);
    }
}
