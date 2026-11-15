use std::collections::BTreeMap;

use porthmos_core::profiles::{ConnectionEntry, ConnectionSource, SecretEdit};
use porthmos_core::{Choice as CoreChoice, ConnectionForm, OptionField, OptionKind, ProtocolInfo};

use super::*;

const TYPED: &str = "hunter2-marker";

fn builtin() -> Vec<ProtocolInfo> {
    crate::testing::builtin_infos()
}

const MODES: &[CoreChoice] =
    &[CoreChoice { value: "fast", label: "Fast" }, CoreChoice { value: "safe", label: "Safe" }];

fn with_secret_option() -> Vec<ProtocolInfo> {
    let mut form = ConnectionForm::standard(99);
    form.options = vec![
        OptionField { key: "token", label: "Token", required: true, kind: OptionKind::Secret },
        OptionField {
            key: "mode",
            label: "Mode",
            required: false,
            kind: OptionKind::Choice { choices: MODES, default: "fast" },
        },
        OptionField { key: "tls", label: "Use TLS", required: false, kind: OptionKind::Toggle { default: true } },
        OptionField { key: "zone", label: "Zone", required: false, kind: OptionKind::Text { default: "eu" } },
    ];
    vec![ProtocolInfo { id: "custom", display_name: "Custom", description: "", limits_note: None, form }]
}

fn entry(protocol: &str) -> ConnectionEntry {
    ConnectionEntry {
        name: "prod".to_string(),
        protocol: protocol.to_string(),
        host: "example.com".to_string(),
        port: 2200,
        username: "me".to_string(),
        password: None,
        options: BTreeMap::from([("remote_path".to_string(), "/srv".to_string())]),
        group: Some("Work".to_string()),
        tags: vec!["a".to_string(), "b".to_string()],
        saved_password: true,
        in_keyring: vec!["password".to_string()],
        source: ConnectionSource::Profile,
    }
}

fn text_of(state: &FormState, key: &str) -> String {
    match state.fields().into_iter().find(|field| field.key == key).unwrap().kind {
        FieldKind::Text(value) => value,
        other => panic!("{key}: {other:?}"),
    }
}

fn fill_required(state: &mut FormState) {
    state.set_text("name", "n".to_string());
    for field in state.fields() {
        if !field.required {
            continue;
        }
        match field.kind {
            FieldKind::Text(value) if value.is_empty() => state.set_text(&field.key, "x".to_string()),
            FieldKind::Secret { .. } => state.set_secret(&field.key, "s".to_string()),
            _ => {}
        }
    }
}

fn info<'a>(infos: &'a [ProtocolInfo], id: &str) -> &'a ProtocolInfo {
    infos.iter().find(|info| info.id == id).unwrap()
}

#[test]
fn new_returns_none_without_protocols() {
    assert!(FormState::new(&[], None).is_none());
}

#[test]
fn add_form_starts_on_the_first_protocol_with_its_default_port() {
    let infos = builtin();
    let state = FormState::new(&infos, None).unwrap();

    assert_eq!(state.title(), "Add connection");
    assert_eq!(state.protocol(), infos[0].id);
    assert_eq!(text_of(&state, "port"), infos[0].form.port.default.to_string());
    assert!(state.original_name().is_none());
}

#[test]
fn every_builtin_protocol_builds_a_form_whose_filled_input_validates() {
    let infos = builtin();
    assert!(infos.len() >= 5);
    for protocol in &infos {
        let mut state = FormState::new(&infos, None).unwrap();
        state.set_protocol(protocol.id);
        fill_required(&mut state);

        let result = state.draft().validate(&protocol.form, None);

        assert!(result.is_ok(), "{}: {result:?}", protocol.id);
    }
}

#[test]
fn a_missing_required_field_is_rejected_by_validate() {
    let infos = builtin();
    let mut state = FormState::new(&infos, None).unwrap();
    state.set_protocol("sftp");
    state.set_text("name", "n".to_string());
    state.set_text("username", "me".to_string());

    let error = state.draft().validate(&info(&infos, "sftp").form, None).unwrap_err();

    assert_eq!(error, "Host can't be empty");
}

#[test]
fn required_flags_follow_the_protocol_form() {
    let infos = builtin();
    let mut state = FormState::new(&infos, None).unwrap();
    state.set_protocol("sftp");

    let required: Vec<String> =
        state.fields().into_iter().filter(|field| field.required).map(|field| field.key).collect();

    assert_eq!(required, ["name", "host", "username"]);
}

#[test]
fn changing_protocol_keeps_common_fields_and_resets_the_default_port() {
    let infos = builtin();
    let mut state = FormState::new(&infos, None).unwrap();
    state.set_protocol("sftp");
    state.set_text("name", "n".to_string());
    state.set_text("host", "h".to_string());
    state.set_text("username", "me".to_string());

    state.set_protocol("ftp");

    assert_eq!(text_of(&state, "name"), "n");
    assert_eq!(text_of(&state, "host"), "h");
    assert_eq!(text_of(&state, "username"), "me");
    assert_eq!(text_of(&state, "port"), info(&infos, "ftp").form.port.default.to_string());
}

#[test]
fn changing_protocol_keeps_a_custom_port() {
    let infos = builtin();
    let mut state = FormState::new(&infos, None).unwrap();
    state.set_protocol("sftp");
    state.set_text("port", "2222".to_string());

    state.set_protocol("ftp");

    assert_eq!(text_of(&state, "port"), "2222");
}

#[test]
fn edit_form_prefills_every_field_from_the_entry() {
    let infos = builtin();
    let state = FormState::new(&infos, Some(&entry("sftp"))).unwrap();

    assert_eq!(state.title(), "Edit connection");
    assert_eq!(state.original_name(), Some("prod"));
    assert_eq!(state.protocol(), "sftp");
    assert_eq!(text_of(&state, "name"), "prod");
    assert_eq!(text_of(&state, "group"), "Work");
    assert_eq!(text_of(&state, "tags"), "a, b");
    assert_eq!(text_of(&state, "host"), "example.com");
    assert_eq!(text_of(&state, "port"), "2200");
    assert_eq!(text_of(&state, "username"), "me");
    assert_eq!(text_of(&state, "remote_path"), "/srv");
}

#[test]
fn edit_form_marks_the_saved_password_and_keeps_it_by_default() {
    let infos = builtin();
    let state = FormState::new(&infos, Some(&entry("sftp"))).unwrap();

    let password = state.fields().into_iter().find(|field| field.key == "password").unwrap();

    assert!(matches!(password.kind, FieldKind::Secret { saved: true, forgotten: false, typed: false }));
    assert_eq!(state.draft().password, SecretEdit::Keep);
}

#[test]
fn typing_a_password_replaces_it_and_forgetting_clears_it() {
    let infos = builtin();
    let mut state = FormState::new(&infos, Some(&entry("sftp"))).unwrap();

    state.set_secret("password", TYPED.to_string());
    assert_eq!(state.draft().password, SecretEdit::Replace(TYPED.to_string()));

    state.forget_secret("password");
    assert_eq!(state.draft().password, SecretEdit::Clear);
    assert_eq!(state.secret_text("password"), "");

    state.undo_forget_secret("password");
    assert_eq!(state.draft().password, SecretEdit::Keep);
}

#[test]
fn a_password_left_empty_on_a_new_connection_is_clear() {
    let infos = builtin();
    let state = FormState::new(&infos, None).unwrap();

    assert_eq!(state.draft().password, SecretEdit::Clear);
}

#[test]
fn the_saved_password_does_not_follow_a_protocol_change() {
    let infos = builtin();
    let mut state = FormState::new(&infos, Some(&entry("sftp"))).unwrap();

    state.set_protocol("ftp");
    assert_eq!(state.draft().password, SecretEdit::Clear);
    let password = state.fields().into_iter().find(|field| field.key == "password").unwrap();
    assert!(matches!(password.kind, FieldKind::Secret { saved: false, .. }));

    state.set_protocol("sftp");
    assert_eq!(state.draft().password, SecretEdit::Keep);
}

#[test]
fn an_unavailable_protocol_is_listed_as_not_available() {
    let infos = builtin();
    let state = FormState::new(&infos, Some(&entry("gopher"))).unwrap();

    let choices = state.protocol_choices();

    assert_eq!(state.protocol(), "gopher");
    assert!(choices.iter().any(|choice| choice.value == "gopher" && choice.label == "gopher (not available)"));
    assert_eq!(choices.len(), infos.len() + 1);
    assert_eq!(text_of(&state, "host"), "example.com");
}

#[test]
fn option_fields_round_trip_into_the_draft() {
    let infos = with_secret_option();
    let mut state = FormState::new(&infos, None).unwrap();

    assert_eq!(text_of(&state, "zone"), "eu");
    state.set_text("zone", "us".to_string());
    state.set_choice("mode", "safe");
    state.set_toggle("tls", false);
    state.set_secret("token", TYPED.to_string());
    let draft = state.draft();

    assert_eq!(draft.options.get("zone").map(String::as_str), Some("us"));
    assert_eq!(draft.options.get("mode").map(String::as_str), Some("safe"));
    assert_eq!(draft.options.get("tls").map(String::as_str), Some("false"));
    assert_eq!(draft.secret_options.get("token"), Some(&SecretEdit::Replace(TYPED.to_string())));
}

#[test]
fn option_defaults_are_in_the_draft_before_anything_is_touched() {
    let infos = with_secret_option();
    let draft = FormState::new(&infos, None).unwrap().draft();

    assert_eq!(draft.options.get("mode").map(String::as_str), Some("fast"));
    assert_eq!(draft.options.get("tls").map(String::as_str), Some("true"));
    assert_eq!(draft.options.get("zone").map(String::as_str), Some("eu"));
}

#[test]
fn a_saved_secret_option_is_kept_unless_touched() {
    let infos = with_secret_option();
    let mut saved = entry("custom");
    saved.in_keyring = vec!["token".to_string()];
    saved.saved_password = false;
    let mut state = FormState::new(&infos, Some(&saved)).unwrap();

    assert_eq!(state.draft().secret_options.get("token"), Some(&SecretEdit::Keep));
    state.forget_secret("token");
    assert_eq!(state.draft().secret_options.get("token"), Some(&SecretEdit::Clear));
}

#[test]
fn debug_output_never_contains_a_typed_secret() {
    let infos = with_secret_option();
    let mut state = FormState::new(&infos, None).unwrap();
    state.set_secret("password", TYPED.to_string());
    state.set_secret("token", TYPED.to_string());

    assert!(!format!("{state:?}").contains(TYPED));
    assert!(!format!("{:?}", state.draft()).contains(TYPED));
    assert!(!format!("{:?}", state.fields()).contains(TYPED));
}

#[test]
fn editing_the_port_marks_it_as_custom() {
    let infos = builtin();
    let mut state = FormState::new(&infos, None).unwrap();
    state.set_protocol("sftp");
    state.set_text("port", "22".to_string());
    state.set_text("port", "2022".to_string());

    state.set_protocol("ftp");

    assert_eq!(text_of(&state, "port"), "2022");
}

#[test]
fn check_reports_the_validation_message() {
    let infos = builtin();
    let mut state = FormState::new(&infos, None).unwrap();
    state.set_protocol("sftp");
    assert_eq!(state.check(), Err("Name can't be empty".to_string()));

    fill_required(&mut state);

    assert_eq!(state.check(), Ok(()));
}
