use super::*;

fn endpoint(text: &str) -> Endpoint {
    text.parse().unwrap()
}

fn listen(text: &str) -> Listen {
    text.parse().unwrap()
}

fn local(listen_text: &str, target: &str) -> TunnelSpec {
    TunnelSpec {
        connection: "prod".to_string(),
        name: "db".to_string(),
        kind: Kind::Local { listen: listen(listen_text), target: endpoint(target) },
        allow_external: false,
        auto_start: false,
    }
}

fn dynamic(listen_text: &str) -> TunnelSpec {
    TunnelSpec {
        connection: "prod".to_string(),
        name: "socks".to_string(),
        kind: Kind::Dynamic { listen: listen(listen_text) },
        allow_external: false,
        auto_start: false,
    }
}

#[test]
fn an_endpoint_parses_hosts_names_and_bracketed_ipv6() {
    for text in ["db.internal:5432", "10.0.0.5:22", "[::1]:80"] {
        assert_eq!(endpoint(text).to_string(), text);
    }
    assert_eq!(endpoint("db.internal:5432"), Endpoint { host: "db.internal".to_string(), port: 5432 });
    assert_eq!(endpoint("[::1]:80"), Endpoint { host: "::1".to_string(), port: 80 });
    for text in ["", ":80", "host", "host:", "host:0", "host:65536", "host:abc", "a b:80", "[::1:80", "::1:80"] {
        assert!(text.parse::<Endpoint>().is_err(), "{text:?} should not parse");
    }
}

#[test]
fn endpoint_parse_failures_name_the_problem() {
    assert_eq!("".parse::<Endpoint>(), Err(SpecError::EmptyHost));
    assert_eq!(":80".parse::<Endpoint>(), Err(SpecError::EmptyHost));
    assert_eq!("host".parse::<Endpoint>(), Err(SpecError::BadPort));
    assert_eq!("host:0".parse::<Endpoint>(), Err(SpecError::BadPort));
    assert_eq!("host:65536".parse::<Endpoint>(), Err(SpecError::BadPort));
    assert_eq!("a b:80".parse::<Endpoint>(), Err(SpecError::BadHost));
    assert_eq!("a\nb:80".parse::<Endpoint>(), Err(SpecError::HostHasControl));
}

#[test]
fn a_listen_address_parses_ip_and_port_only() {
    assert_eq!(listen("127.0.0.1:5432").to_string(), "127.0.0.1:5432");
    assert_eq!(listen("[::1]:1080").to_string(), "[::1]:1080");
    for text in ["localhost:80", "0.0.0.0", "127.0.0.1:0", "127.0.0.1:70000", "", "[::1:80"] {
        assert!(text.parse::<Listen>().is_err(), "{text:?} should not parse");
    }
}

#[test]
fn loopback_is_recognised_for_both_families() {
    for text in ["127.0.0.1:1", "127.0.0.5:1", "[::1]:1"] {
        assert!(listen(text).is_loopback(), "{text}");
    }
    for text in ["0.0.0.0:1", "192.168.1.5:1", "[::]:1", "[::ffff:127.0.0.1]:1"] {
        assert!(!listen(text).is_loopback(), "{text}");
    }
}

#[test]
fn a_valid_local_and_dynamic_spec_validate() {
    assert_eq!(local("127.0.0.1:5432", "db.internal:5432").validate(), Ok(()));
    assert_eq!(dynamic("127.0.0.1:1080").validate(), Ok(()));
}

#[test]
fn names_must_be_one_to_sixty_four_printable_characters() {
    let mut spec = local("127.0.0.1:5432", "db:5432");
    spec.name = String::new();
    assert_eq!(spec.validate(), Err(SpecError::EmptyName));
    spec.name = "a".repeat(64);
    assert_eq!(spec.validate(), Ok(()));
    spec.name = "a".repeat(65);
    assert_eq!(spec.validate(), Err(SpecError::NameTooLong));
    for hostile in ["a\nb", "a\u{7f}b", "a\u{202e}b", "a\u{200b}b"] {
        spec.name = hostile.to_string();
        assert_eq!(spec.validate(), Err(SpecError::NameHasControl), "{hostile:?}");
    }
    spec.name = "σύνδεση".to_string();
    assert_eq!(spec.validate(), Ok(()));
}

#[test]
fn an_empty_connection_is_refused() {
    let mut spec = dynamic("127.0.0.1:1080");
    spec.connection = String::new();
    assert_eq!(spec.validate(), Err(SpecError::EmptyConnection));
}

#[test]
fn a_non_loopback_listener_needs_the_explicit_opt_in() {
    let mut spec = local("0.0.0.0:8080", "web:80");
    assert_eq!(spec.validate(), Err(SpecError::ExternalNotAllowed));
    spec.allow_external = true;
    assert_eq!(spec.validate(), Ok(()));
    let mut loopback = dynamic("127.0.0.1:1080");
    loopback.allow_external = true;
    assert_eq!(loopback.validate(), Ok(()));
    let mut socks = dynamic("[::]:1080");
    assert_eq!(socks.validate(), Err(SpecError::ExternalNotAllowed));
    socks.allow_external = true;
    assert_eq!(socks.validate(), Ok(()));
}

#[test]
fn ports_zero_are_refused_even_when_built_by_hand() {
    let mut spec = local("127.0.0.1:5432", "db:5432");
    if let Kind::Local { listen, .. } = &mut spec.kind {
        listen.port = 0;
    }
    assert_eq!(spec.validate(), Err(SpecError::BadPort));
    let mut spec = local("127.0.0.1:5432", "db:5432");
    if let Kind::Local { target, .. } = &mut spec.kind {
        target.port = 0;
    }
    assert_eq!(spec.validate(), Err(SpecError::BadPort));
}

#[test]
fn a_target_host_built_by_hand_is_checked_too() {
    let cases = [
        ("", SpecError::EmptyHost),
        ("a b", SpecError::BadHost),
        ("a\tb", SpecError::HostHasControl),
        ("a\0b", SpecError::HostHasControl),
    ];
    for (host, expected) in cases {
        let mut spec = local("127.0.0.1:5432", "db:5432");
        if let Kind::Local { target, .. } = &mut spec.kind {
            target.host = host.to_string();
        }
        assert_eq!(spec.validate(), Err(expected), "{host:?}");
    }
    let mut spec = local("127.0.0.1:5432", "db:5432");
    if let Kind::Local { target, .. } = &mut spec.kind {
        target.host = "h".repeat(256);
    }
    assert_eq!(spec.validate(), Err(SpecError::HostTooLong));
    if let Kind::Local { target, .. } = &mut spec.kind {
        target.host = "h".repeat(255);
    }
    assert_eq!(spec.validate(), Ok(()));
}

#[test]
fn the_kind_exposes_its_listener_and_label() {
    let spec = local("127.0.0.1:5432", "db:5432");
    assert_eq!(spec.kind.listen(), &listen("127.0.0.1:5432"));
    assert_eq!(spec.kind.label(), "-L");
    let spec = dynamic("127.0.0.1:1080");
    assert_eq!(spec.kind.listen(), &listen("127.0.0.1:1080"));
    assert_eq!(spec.kind.label(), "-D");
}

#[test]
fn the_key_is_connection_and_name() {
    assert_eq!(
        local("127.0.0.1:5432", "db:5432").key(),
        TunnelKey { connection: "prod".to_string(), name: "db".to_string() }
    );
}

#[test]
fn a_checked_endpoint_applies_the_host_rules_without_parsing_a_port() {
    assert_eq!(Endpoint::checked("db.internal".to_string(), 5432), Ok(endpoint("db.internal:5432")));
    assert_eq!(Endpoint::checked("::1".to_string(), 80), Ok(Endpoint { host: "::1".to_string(), port: 80 }));
    assert_eq!(Endpoint::checked(String::new(), 80), Err(SpecError::EmptyHost));
    assert_eq!(Endpoint::checked("a b".to_string(), 80), Err(SpecError::BadHost));
    assert_eq!(Endpoint::checked("a\u{1}b".to_string(), 80), Err(SpecError::HostHasControl));
    assert_eq!(Endpoint::checked("h".repeat(256), 80), Err(SpecError::HostTooLong));
    assert_eq!(Endpoint::checked("db".to_string(), 0), Err(SpecError::BadPort));
}

#[test]
fn every_spec_error_has_its_own_non_empty_text() {
    let all = [
        SpecError::EmptyName,
        SpecError::NameTooLong,
        SpecError::NameHasControl,
        SpecError::EmptyConnection,
        SpecError::EmptyHost,
        SpecError::HostTooLong,
        SpecError::HostHasControl,
        SpecError::BadHost,
        SpecError::BadPort,
        SpecError::ExternalNotAllowed,
    ];
    let texts: std::collections::BTreeSet<String> = all.iter().map(ToString::to_string).collect();

    assert_eq!(texts.len(), all.len());
    assert!(texts.iter().all(|text| !text.is_empty()));
}

#[test]
fn every_target_host_that_validates_survives_display_and_parse() {
    let hosts = ["db", "db.internal", "10.0.0.5", "::1", "fe80::1", "2001:db8::1"];
    for host in hosts {
        let mut spec = local("127.0.0.1:5432", "db:5432");
        if let Kind::Local { target, .. } = &mut spec.kind {
            target.host = host.to_string();
        }
        assert_eq!(spec.validate(), Ok(()), "{host}");
        let Kind::Local { target, .. } = &spec.kind else { unreachable!() };
        assert_eq!(target.to_string().parse::<Endpoint>().as_ref(), Ok(target), "{host}");
    }
}

#[test]
fn a_target_host_with_colons_must_be_an_ipv6_literal_and_brackets_are_refused() {
    for host in ["db:5432", "a:b", "[::1]", "[db]", "db]", "::g"] {
        let mut spec = local("127.0.0.1:5432", "db:5432");
        if let Kind::Local { target, .. } = &mut spec.kind {
            target.host = host.to_string();
        }
        assert_eq!(spec.validate(), Err(SpecError::BadHost), "{host}");
    }
}

fn remote(listen_text: &str, target: &str) -> TunnelSpec {
    TunnelSpec {
        connection: "prod".to_string(),
        name: "web".to_string(),
        kind: Kind::Remote { listen: Listen::parse_server_side(listen_text).unwrap(), target: endpoint(target) },
        allow_external: false,
        auto_start: false,
    }
}

#[test]
fn a_remote_spec_may_listen_on_port_zero_but_a_local_one_may_not() {
    assert_eq!(remote("127.0.0.1:0", "127.0.0.1:3000").validate(), Ok(()));
    let mut spec = local("127.0.0.1:5432", "db:5432");
    if let Kind::Local { listen, .. } = &mut spec.kind {
        listen.port = 0;
    }
    assert_eq!(spec.validate(), Err(SpecError::BadPort));
    let mut socks = dynamic("127.0.0.1:1080");
    if let Kind::Dynamic { listen } = &mut socks.kind {
        listen.port = 0;
    }
    assert_eq!(socks.validate(), Err(SpecError::BadPort));
}

#[test]
fn a_remote_target_port_zero_is_refused() {
    let mut spec = remote("127.0.0.1:8080", "127.0.0.1:3000");
    if let Kind::Remote { target, .. } = &mut spec.kind {
        target.port = 0;
    }

    assert_eq!(spec.validate(), Err(SpecError::BadPort));
}

#[test]
fn a_remote_target_host_is_checked_like_a_local_one() {
    let cases = [
        ("", SpecError::EmptyHost),
        ("a b", SpecError::BadHost),
        ("a\nb", SpecError::HostHasControl),
        ("db:5432", SpecError::BadHost),
        ("[::1]", SpecError::BadHost),
    ];
    for (host, expected) in cases {
        let mut spec = remote("127.0.0.1:8080", "127.0.0.1:3000");
        if let Kind::Remote { target, .. } = &mut spec.kind {
            target.host = host.to_string();
        }
        assert_eq!(spec.validate(), Err(expected), "{host:?}");
    }
}

#[test]
fn a_non_loopback_remote_bind_needs_the_opt_in() {
    let mut spec = remote("0.0.0.0:8080", "127.0.0.1:3000");
    assert_eq!(spec.validate(), Err(SpecError::ExternalNotAllowed));
    spec.allow_external = true;
    assert_eq!(spec.validate(), Ok(()));
    assert_eq!(remote("[::1]:8080", "127.0.0.1:3000").validate(), Ok(()));
    assert_eq!(remote("[::]:0", "127.0.0.1:3000").validate(), Err(SpecError::ExternalNotAllowed));
}

#[test]
fn the_remote_kind_exposes_its_listener_and_label() {
    let spec = remote("127.0.0.1:0", "127.0.0.1:3000");

    assert_eq!(spec.kind.label(), "-R");
    assert_eq!(spec.kind.listen(), &Listen::parse_server_side("127.0.0.1:0").unwrap());
}

#[test]
fn parse_server_side_accepts_port_zero_and_round_trips_through_display() {
    for text in ["127.0.0.1:0", "127.0.0.1:8080", "[::1]:0", "[::]:65535"] {
        let parsed = Listen::parse_server_side(text).unwrap();
        assert_eq!(parsed.to_string(), text);
        assert_eq!(Listen::parse_server_side(&parsed.to_string()), Ok(parsed));
    }
}

#[test]
fn parse_server_side_still_refuses_hostnames_and_garbage() {
    assert_eq!(Listen::parse_server_side("localhost:80"), Err(SpecError::BadHost));
    assert_eq!(Listen::parse_server_side("1.2.3.4:x"), Err(SpecError::BadPort));
    assert_eq!(Listen::parse_server_side("1.2.3.4:70000"), Err(SpecError::BadPort));
    assert!(Listen::parse_server_side("1.2.3.4").is_err());
    assert!("127.0.0.1:0".parse::<Listen>().is_err());
}
