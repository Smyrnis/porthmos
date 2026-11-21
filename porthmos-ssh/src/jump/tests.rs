use std::collections::BTreeMap;

use super::*;

fn alias(name: &str, host_name: Option<&str>, user: Option<&str>, port: Option<u16>) -> SshConfigHost {
    SshConfigHost {
        name: name.to_string(),
        host_name: host_name.map(str::to_string),
        user: user.map(str::to_string),
        port,
        identity_file: None,
        proxy_jump: None,
        proxy_command: None,
    }
}

fn hop(user: Option<&str>, host: &str, port: Option<u16>) -> JumpHop {
    JumpHop { user: user.map(str::to_string), host: host.to_string(), port }
}

#[test]
fn a_single_host_is_one_hop() {
    assert_eq!(parse_hops("bastion"), Ok(vec![hop(None, "bastion", None)]));
}

#[test]
fn user_and_port_are_read() {
    assert_eq!(parse_hops("alice@bastion:2222"), Ok(vec![hop(Some("alice"), "bastion", Some(2222))]));
}

#[test]
fn hops_are_split_on_commas_and_spaces_around_them_are_ignored() {
    assert_eq!(parse_hops("a , b@c:22"), Ok(vec![hop(None, "a", None), hop(Some("b"), "c", Some(22))]));
}

#[test]
fn an_ssh_uri_is_accepted() {
    assert_eq!(parse_hops("ssh://alice@bastion:2200"), Ok(vec![hop(Some("alice"), "bastion", Some(2200))]));
}

#[test]
fn a_bracketed_ipv6_address_keeps_its_colons() {
    assert_eq!(parse_hops("[::1]:2222"), Ok(vec![hop(None, "::1", Some(2222))]));
    assert_eq!(parse_hops("u@[fe80::1]"), Ok(vec![hop(Some("u"), "fe80::1", None)]));
}

#[test]
fn a_bare_ipv6_address_is_a_host_without_a_port() {
    assert_eq!(parse_hops("fe80::1"), Ok(vec![hop(None, "fe80::1", None)]));
}

#[test]
fn bad_input_is_refused_with_a_reason() {
    for bad in ["", "   ", "a,,b", "@host", "host:0", "host:70000", "host:abc", "[::1", "alice@"] {
        assert!(parse_hops(bad).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn a_hop_naming_a_config_alias_takes_its_host_user_and_port() {
    let hosts = vec![alias("bastion", Some("203.0.113.5"), Some("deploy"), Some(2200))];

    let chain = resolve_chain("bastion", &hosts, "me").unwrap();

    assert_eq!(chain.len(), 1);
    assert_eq!(
        (chain[0].name.as_str(), chain[0].host.as_str(), chain[0].port, chain[0].username.as_str()),
        ("bastion", "203.0.113.5", 2200, "deploy")
    );
    assert_eq!(chain[0].password, None);
}

#[test]
fn what_the_hop_spells_out_beats_the_alias() {
    let hosts = vec![alias("bastion", Some("203.0.113.5"), Some("deploy"), Some(2200))];

    let chain = resolve_chain("root@bastion:2022", &hosts, "me").unwrap();

    assert_eq!((chain[0].username.as_str(), chain[0].port, chain[0].host.as_str()), ("root", 2022, "203.0.113.5"));
}

#[test]
fn an_unknown_host_is_used_literally_with_the_fallback_user_and_port_22() {
    let chain = resolve_chain("jump.example", &[], "me").unwrap();

    assert_eq!((chain[0].host.as_str(), chain[0].port, chain[0].username.as_str()), ("jump.example", 22, "me"));
}

#[test]
fn the_alias_identity_file_is_passed_on() {
    let mut bastion = alias("bastion", None, None, None);
    bastion.identity_file = Some(std::path::PathBuf::from("/keys/id"));

    let chain = resolve_chain("bastion", &[bastion], "me").unwrap();

    assert_eq!(chain[0].option("identity_file"), Some("/keys/id"));
}

#[test]
fn alias_names_match_without_regard_to_case() {
    let hosts = vec![alias("Bastion", Some("10.0.0.9"), None, None)];

    assert_eq!(resolve_chain("bastion", &hosts, "me").unwrap()[0].host, "10.0.0.9");
}

#[test]
fn a_jump_host_with_its_own_jump_is_reached_through_it_first() {
    let mut inner = alias("inner", Some("10.0.0.2"), None, None);
    inner.proxy_jump = Some("outer".to_string());
    let hosts = vec![inner, alias("outer", Some("198.51.100.1"), None, None)];

    let chain = resolve_chain("inner", &hosts, "me").unwrap();

    assert_eq!(chain.iter().map(|target| target.host.as_str()).collect::<Vec<_>>(), ["198.51.100.1", "10.0.0.2"]);
}

#[test]
fn a_loop_of_jumps_is_refused() {
    let mut a = alias("a", None, None, None);
    a.proxy_jump = Some("b".to_string());
    let mut b = alias("b", None, None, None);
    b.proxy_jump = Some("a".to_string());

    let error = resolve_chain("a", &[a, b], "me").unwrap_err();

    assert!(error.contains("loop") || error.contains("too many"), "{error}");
}

#[test]
fn too_long_a_chain_is_refused() {
    let spec = (0..=MAX_HOPS).map(|index| format!("h{index}")).collect::<Vec<_>>().join(",");

    assert!(resolve_chain(&spec, &[], "me").is_err());
}

#[test]
fn an_explicit_chain_keeps_its_order() {
    let chain = resolve_chain("first,second,third", &[], "me").unwrap();

    assert_eq!(chain.iter().map(|target| target.host.as_str()).collect::<Vec<_>>(), ["first", "second", "third"]);
}

fn plain_target(name: &str, host: &str, port: u16) -> Target {
    Target {
        name: name.to_string(),
        host: host.to_string(),
        port,
        username: "u".to_string(),
        password: None,
        options: BTreeMap::new(),
    }
}

fn jumping_alias() -> SshConfigHost {
    let mut host = alias("box", Some("10.0.0.2"), None, Some(2200));
    host.proxy_jump = Some("bastion".to_string());
    host
}

#[test]
fn a_profile_without_a_setting_inherits_the_jump_of_the_alias_it_matches() {
    let target = plain_target("box", "10.0.0.2", 2200);

    assert_eq!(effective_jump(&target, &[jumping_alias()]), Some("bastion"));
}

#[test]
fn an_explicit_setting_wins_over_the_alias() {
    let mut target = plain_target("box", "10.0.0.2", 2200);
    target.options.insert("proxy_jump".to_string(), "mine".to_string());

    assert_eq!(effective_jump(&target, &[jumping_alias()]), Some("mine"));
}

#[test]
fn an_explicit_none_switches_the_aliases_jump_off() {
    let mut target = plain_target("box", "10.0.0.2", 2200);
    target.options.insert("proxy_jump".to_string(), "none".to_string());

    assert_eq!(effective_jump(&target, &[jumping_alias()]), None);
}

#[test]
fn a_profile_that_points_at_another_host_than_the_alias_does_not_inherit() {
    let target = plain_target("box", "10.0.0.9", 2200);

    assert_eq!(effective_jump(&target, &[jumping_alias()]), None);
}

#[test]
fn a_profile_on_another_port_than_the_alias_does_not_inherit() {
    let target = plain_target("box", "10.0.0.2", 22);

    assert_eq!(effective_jump(&target, &[jumping_alias()]), None);
}

#[test]
fn a_profile_with_another_name_than_the_alias_does_not_inherit() {
    let target = plain_target("web", "10.0.0.2", 2200);

    assert_eq!(effective_jump(&target, &[jumping_alias()]), None);
}

#[test]
fn an_alias_without_a_host_name_matches_by_its_own_name_and_the_default_port() {
    let mut bare = alias("gate.example", None, None, None);
    bare.proxy_jump = Some("outer".to_string());
    let target = plain_target("gate.example", "gate.example", 22);

    assert_eq!(effective_jump(&target, &[bare]), Some("outer"));
}
