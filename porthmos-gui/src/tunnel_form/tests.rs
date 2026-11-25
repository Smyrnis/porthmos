use porthmos_core::{Endpoint, Listen, SpecError, TunnelKey, TunnelKind, TunnelSpec};

use super::*;

fn local_draft() -> TunnelDraft {
    let mut draft = TunnelDraft::new("prod");
    draft.name = "postgres".into();
    draft.listen_port = "5432".into();
    draft.target_host = "db.internal".into();
    draft.target_port = "5432".into();
    draft
}

fn dynamic_draft() -> TunnelDraft {
    let mut draft = TunnelDraft::new("prod");
    draft.name = "socks".into();
    draft.kind = DraftKind::Dynamic;
    draft.listen_port = "1080".into();
    draft
}

fn fields(result: Result<TunnelSpec, Vec<(Field, DraftError)>>) -> Vec<(Field, DraftError)> {
    result.expect_err("the draft should be refused")
}

#[test]
fn a_valid_local_draft_builds_the_expected_spec() {
    let mut draft = local_draft();
    draft.auto_start = true;

    let spec = draft.build().unwrap();

    assert_eq!(spec.connection, "prod");
    assert_eq!(spec.name, "postgres");
    assert_eq!(
        spec.kind,
        TunnelKind::Local {
            listen: "127.0.0.1:5432".parse::<Listen>().unwrap(),
            target: "db.internal:5432".parse::<Endpoint>().unwrap(),
        }
    );
    assert!(spec.auto_start);
    assert!(!spec.allow_external);
}

#[test]
fn a_valid_dynamic_draft_ignores_the_target_fields() {
    let mut draft = dynamic_draft();
    draft.target_host = "not a host at all".into();
    draft.target_port = "garbage".into();

    let spec = draft.build().unwrap();

    assert_eq!(spec.kind, TunnelKind::Dynamic { listen: "127.0.0.1:1080".parse::<Listen>().unwrap() });
}

#[test]
fn every_invalid_field_is_reported_with_its_own_error() {
    let mut draft = TunnelDraft::new("");
    draft.name = String::new();
    draft.listen_port = "80a".into();
    draft.target_host = String::new();
    draft.target_port = "0".into();

    let problems = fields(draft.build());

    assert!(problems.contains(&(Field::Connection, SpecError::EmptyConnection)), "{problems:?}");
    assert!(problems.contains(&(Field::Name, SpecError::EmptyName)), "{problems:?}");
    assert!(problems.contains(&(Field::ListenPort, SpecError::BadPort)), "{problems:?}");
    assert!(problems.contains(&(Field::TargetHost, SpecError::EmptyHost)), "{problems:?}");
    assert!(problems.contains(&(Field::TargetPort, SpecError::BadPort)), "{problems:?}");
}

#[test]
fn names_hosts_and_ports_are_checked_one_rule_each() {
    let mut long = local_draft();
    long.name = "n".repeat(65);
    assert_eq!(fields(long.build()), vec![(Field::Name, SpecError::NameTooLong)]);

    let mut hidden = local_draft();
    hidden.name = "a\u{202e}b".into();
    assert_eq!(fields(hidden.build()), vec![(Field::Name, SpecError::NameHasControl)]);

    let mut spaced = local_draft();
    spaced.target_host = "db internal".into();
    assert_eq!(fields(spaced.build()), vec![(Field::TargetHost, SpecError::BadHost)]);

    let mut port_zero = local_draft();
    port_zero.listen_port = "0".into();
    assert_eq!(fields(port_zero.build()), vec![(Field::ListenPort, SpecError::BadPort)]);

    let mut too_big = local_draft();
    too_big.listen_port = "65536".into();
    assert_eq!(fields(too_big.build()), vec![(Field::ListenPort, SpecError::BadPort)]);
}

#[test]
fn surrounding_spaces_are_ignored() {
    let mut draft = local_draft();
    draft.name = "  postgres ".into();
    draft.listen_port = " 5432 ".into();
    draft.target_host = " db.internal ".into();
    draft.target_port = " 5432".into();

    let spec = draft.build().unwrap();

    assert_eq!(spec.name, "postgres");
}

#[test]
fn a_non_loopback_listen_host_asks_for_the_warning_and_needs_the_checkbox() {
    let mut draft = local_draft();
    assert!(!draft.needs_external_warning());

    draft.listen_host = "0.0.0.0".into();
    assert!(draft.needs_external_warning());
    assert_eq!(fields(draft.build()), vec![(Field::ListenHost, SpecError::ExternalNotAllowed)]);

    draft.allow_external = true;
    let spec = draft.build().unwrap();
    assert!(spec.allow_external);
    assert_eq!(spec.kind.listen().to_string(), "0.0.0.0:5432");
}

#[test]
fn the_ipv6_loopback_is_not_external_and_is_built_with_brackets() {
    let mut draft = local_draft();
    draft.listen_host = "::1".into();

    assert!(!draft.needs_external_warning());
    assert_eq!(draft.build().unwrap().kind.listen().to_string(), "[::1]:5432");
}

#[test]
fn localhost_is_not_accepted_as_a_listen_host() {
    let mut draft = local_draft();
    draft.listen_host = "localhost".into();

    assert_eq!(fields(draft.build()), vec![(Field::ListenHost, SpecError::BadHost)]);
    assert!(!draft.needs_external_warning());

    draft.listen_host = String::new();
    assert_eq!(fields(draft.build()), vec![(Field::ListenHost, SpecError::EmptyHost)]);
}

#[test]
fn editing_round_trips_a_spec_through_the_draft() {
    let mut original = local_draft();
    original.listen_host = "10.0.0.5".into();
    original.allow_external = true;
    original.auto_start = true;
    let spec = original.build().unwrap();

    let draft = TunnelDraft::from_spec(&spec);

    assert_eq!(draft.original, Some(TunnelKey { connection: "prod".into(), name: "postgres".into() }));
    assert_eq!(draft.build().unwrap(), spec);

    let dynamic = dynamic_draft().build().unwrap();
    assert_eq!(TunnelDraft::from_spec(&dynamic).build().unwrap(), dynamic);
}

#[test]
fn changes_land_in_their_own_field() {
    let mut draft = TunnelDraft::new("a");

    draft.apply(DraftChange::Name("n".into()));
    draft.apply(DraftChange::Kind(DraftKind::Dynamic));
    draft.apply(DraftChange::ListenHost("::1".into()));
    draft.apply(DraftChange::ListenPort("9".into()));
    draft.apply(DraftChange::TargetHost("h".into()));
    draft.apply(DraftChange::TargetPort("8".into()));
    draft.apply(DraftChange::AllowExternal(true));
    draft.apply(DraftChange::AutoStart(true));
    draft.apply(DraftChange::Connection("b".into()));

    assert_eq!(draft.name, "n");
    assert_eq!(draft.kind, DraftKind::Dynamic);
    assert_eq!((draft.listen_host.as_str(), draft.listen_port.as_str()), ("::1", "9"));
    assert_eq!((draft.target_host.as_str(), draft.target_port.as_str()), ("h", "8"));
    assert!(draft.allow_external && draft.auto_start);
    assert_eq!(draft.connection, "b");
}

#[test]
fn a_new_draft_listens_on_the_loopback_by_default() {
    let draft = TunnelDraft::new("a");

    assert_eq!(draft.listen_host, "127.0.0.1");
    assert_eq!(draft.kind, DraftKind::Local);
    assert!(!draft.allow_external && !draft.auto_start && draft.original.is_none());
}

#[test]
fn a_target_host_with_a_colon_that_is_not_ipv6_is_refused_in_the_form() {
    let mut draft = local_draft();
    draft.target_host = "db:5432".into();

    assert_eq!(fields(draft.build()), vec![(Field::TargetHost, SpecError::BadHost)]);
}
