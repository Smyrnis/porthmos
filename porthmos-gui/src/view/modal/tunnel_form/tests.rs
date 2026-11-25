use porthmos_core::SpecError;

use super::*;
use crate::theme::ThemeSettings;

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

#[test]
fn the_title_and_accept_button_follow_new_or_edit() {
    let fresh = TunnelDraft::new("a");
    let mut editing = TunnelDraft::new("a");
    editing.original = Some(porthmos_core::TunnelKey { connection: "a".into(), name: "x".into() });

    assert_eq!((title(&fresh).as_str(), accept_title(&fresh).as_str()), ("New tunnel", "Create"));
    assert_eq!((title(&editing).as_str(), accept_title(&editing).as_str()), ("Edit tunnel", "Save"));
}

#[test]
fn a_problem_is_only_shown_under_its_own_field() {
    let palette = palette();
    let errors = vec![(Field::Name, SpecError::EmptyName)];

    assert!(problem(&errors, Field::Name, &palette).is_some());
    assert!(problem(&errors, Field::ListenPort, &palette).is_none());
}

#[test]
fn the_body_builds_for_both_kinds_with_errors_and_the_warning() {
    let palette = palette();
    let connections = vec!["a".to_string(), "b".to_string()];
    let mut draft = TunnelDraft::new("a");
    draft.listen_host = "0.0.0.0".into();
    let errors = vec![
        (Field::Connection, SpecError::EmptyConnection),
        (Field::Name, SpecError::EmptyName),
        (Field::ListenHost, SpecError::ExternalNotAllowed),
        (Field::ListenPort, SpecError::BadPort),
        (Field::TargetHost, SpecError::EmptyHost),
        (Field::TargetPort, SpecError::BadPort),
    ];

    let _ = body(&draft, &connections, &errors, &palette);
    draft.kind = DraftKind::Dynamic;
    let _ = body(&draft, &connections, &errors, &palette);
    draft.original = Some(porthmos_core::TunnelKey { connection: "a".into(), name: "x".into() });
    let _ = body(&draft, &connections, &[], &palette);
}

#[test]
fn the_body_builds_for_the_remote_kind_with_errors_and_the_warning() {
    let palette = palette();
    let mut draft = TunnelDraft::new("a");
    draft.kind = DraftKind::Remote;
    draft.listen_host = "0.0.0.0".into();
    let errors = vec![
        (Field::ListenHost, SpecError::ExternalNotAllowed),
        (Field::TargetHost, SpecError::EmptyHost),
        (Field::TargetPort, SpecError::BadPort),
    ];

    let _ = body(&draft, &["a".to_string()], &errors, &palette);
}

#[test]
fn the_remote_labels_replace_the_local_ones() {
    assert_eq!(
        (listen_label(DraftKind::Local).as_str(), target_label(DraftKind::Local).as_str()),
        ("Listen on", "Forward to")
    );
    assert_eq!(
        (listen_label(DraftKind::Remote).as_str(), target_label(DraftKind::Remote).as_str()),
        ("Server listens on", "Local target")
    );
    assert_eq!(listen_label(DraftKind::Dynamic), "Listen on");
}

#[test]
fn the_external_warning_names_the_server_side_for_a_remote_tunnel() {
    assert_ne!(external_warning(DraftKind::Remote), external_warning(DraftKind::Local));
    assert!(external_warning(DraftKind::Remote).contains("server"), "{}", external_warning(DraftKind::Remote));
}
