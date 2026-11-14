use gbserver::sip::gb28181::{invite_session::SipRoutingHeaders, recovery_routing::safe_routing};

#[test]
fn recovery_route_requires_matching_fresh_registration_and_transport() {
    use gbserver::sip::gb28181::{
        recovery_routing::matches_registration_route, register_nonce::RegistrationIngress,
    };
    let address = "192.0.2.10:5060".parse().unwrap();
    let other = "192.0.2.10:5061".parse().unwrap();
    let tcp = RegistrationIngress::Tcp(uuid::Uuid::new_v4());
    assert!(matches_registration_route(
        "192.0.2.10:5060",
        "udp",
        Some((address, RegistrationIngress::Udp))
    ));
    assert!(matches_registration_route(
        "192.0.2.10:5060",
        "tcp",
        Some((address, tcp))
    ));
    assert!(!matches_registration_route("192.0.2.10:5060", "udp", None));
    assert!(!matches_registration_route(
        "192.0.2.10:5060",
        "udp",
        Some((other, RegistrationIngress::Udp))
    ));
    assert!(!matches_registration_route(
        "192.0.2.10:5060",
        "udp",
        Some((address, tcp))
    ));
    assert!(!matches_registration_route(
        "192.0.2.10:5060",
        "tcp",
        Some((address, RegistrationIngress::Udp))
    ));
    assert!(!matches_registration_route(
        "192.0.2.10:5060",
        "unknown",
        Some((address, tcp))
    ));
    assert!(!matches_registration_route(
        "malformed",
        "udp",
        Some((address, RegistrationIngress::Udp))
    ));
}

#[test]
fn safe_routing_preserves_supported_uris_in_response_order() {
    let input = SipRoutingHeaders {
        contacts: vec!["<sip:device@192.0.2.10:5060>".into()],
        record_routes: vec![
            "<sip:edge.example:5060;lr>".into(),
            "<sips:[2001:db8::1]:5061;transport=tcp;lr>".into(),
        ],
    };
    let result = safe_routing(&input).unwrap();
    assert_eq!(result.contact_uri, "sip:device@192.0.2.10:5060");
    assert_eq!(
        result.record_route_uris,
        [
            "sip:edge.example:5060;lr",
            "sips:[2001:db8::1]:5061;transport=tcp;lr"
        ]
    );
}

#[test]
fn unsafe_or_ambiguous_routing_is_not_saved() {
    for bad in [
        "<sip:user:password@example.test>",
        "<sip:user%3Apass@example.test>",
        "<sip:user@example.test?Authorization=secret>",
        "<sip:user@example.test;password=secret>",
        "\"name\" <sip:user@example.test>",
        "<sip:a@example.test>, <sip:b@example.test>",
        "<sip:user@example.test>\r\nX-Injected: x",
        "<sip:user@[::1>:99999>",
        "*",
    ] {
        let input = SipRoutingHeaders {
            contacts: vec![bad.into()],
            record_routes: vec![],
        };
        assert!(
            safe_routing(&input).is_none(),
            "accepted unsafe input: {bad}"
        );
    }
    for input in [
        SipRoutingHeaders {
            contacts: vec![],
            record_routes: vec![],
        },
        SipRoutingHeaders {
            contacts: vec!["<sip:a@example.test>".into(), "<sip:b@example.test>".into()],
            record_routes: vec![],
        },
        SipRoutingHeaders {
            contacts: vec!["<sip:a@example.test>".into()],
            record_routes: vec!["<sip:proxy;password=secret>".into()],
        },
    ] {
        assert!(safe_routing(&input).is_none());
    }
}
