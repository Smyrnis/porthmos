use std::{collections::BTreeMap, path::Path};

use crossterm::event::KeyEventState;
use porthmos_core::{Answer, ConnectionForm, OptionField, OptionKind, ProtocolInfo, profiles::ProfileDraft};

use super::*;
use crate::{
    app::testing::{TestApp, test_app, test_app_with_protocols},
    widgets::{
        dialog::{FieldKind, FormLayout, confirm::ConfirmFocus},
        filter_line::FilterLine,
    },
};

fn app() -> TestApp {
    test_app(Path::new("/d"))
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn connection(name: &str, source: ConnectionSource) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: format!("{name}.example.com"),
        port: 22,
        username: "deploy".to_string(),
        password: None,
        options: Default::default(),
        source,
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
    }
}

fn sample_protocols() -> Vec<ProtocolInfo> {
    let mut sftp = ConnectionForm::standard(22);
    sftp.options.push(OptionField {
        key: "identity_file",
        label: "Identity file",
        required: false,
        kind: OptionKind::Text { default: "" },
    });
    vec![
        ProtocolInfo { id: "sftp", display_name: "SFTP", description: "", limits_note: None, form: sftp },
        ProtocolInfo {
            id: "ftp",
            display_name: "FTP",
            description: "",
            limits_note: None,
            form: ConnectionForm::standard(21),
        },
    ]
}

fn screen_with(protocols: Vec<ProtocolInfo>, entries: Vec<ConnectionEntry>) -> TestApp {
    let mut test = test_app_with_protocols(Path::new("/d"), protocols);
    test.app.screen = Screen::Connections;
    test.app.connections.replace(entries);
    test.app.connections.cursor = 0;
    test
}

fn on_connections_screen(entries: Vec<ConnectionEntry>) -> TestApp {
    screen_with(sample_protocols(), entries)
}

fn ctrl_s() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
}

fn set_field(test: &mut TestApp, key: &str, value: &str) {
    if let Some(Dialog::Form(form)) = test.app.dialog.as_mut()
        && let Some(field) = form.fields.iter_mut().find(|field| field.key == key)
    {
        field.value = value.to_string();
    }
}

fn fill_form(test: &mut TestApp, values: [&str; 4]) {
    for (key, value) in ["name", "host", "port", "username"].into_iter().zip(values) {
        set_field(test, key, value);
    }
}

#[test]
fn open_connections_switches_screen_and_loads_entries() {
    let mut test = app();

    test.app.apply_action(Action::OpenConnections);

    assert_eq!(test.app.screen, Screen::Connections);
    assert_eq!(test.sent(), vec![Command::ListProfiles]);
}

#[test]
fn the_profile_list_arrives_from_the_core() {
    let mut test = app();
    test.app.connections.cursor = 5;

    test.app.apply_core_event(Event::Profiles(vec![connection("a", ConnectionSource::Profile)]));

    assert_eq!(test.app.connections.entries().len(), 1);
    assert_eq!(test.app.connections.cursor, 0);
}

#[test]
fn connecting_asks_the_core_and_shows_the_attempt() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);

    test.app.apply_action(Action::Open);
    test.app.apply_core_event(Event::Connecting { name: "prod".to_string() });

    assert_eq!(test.sent(), vec![Command::Connect { profile: "prod".to_string() }]);
    assert_eq!(test.app.connection_status, ConnectionStatus::Connecting("prod".to_string()));
}

#[test]
fn connecting_to_an_unreachable_host_reports_failure() {
    let mut test = app();
    test.app.connection_status = ConnectionStatus::Connecting("unreachable".to_string());

    test.app.apply_core_event(Event::ConnectFailed {
        name: "unreachable".to_string(),
        message: "Unable to connect to unreachable:\nConnection refused".to_string(),
    });

    match test.app.connection_status {
        ConnectionStatus::Failed(_) => {}
        ref other => panic!("expected Failed, got {other:?}"),
    }
    assert_eq!(test.app.notifications.current().unwrap().severity, Severity::Error);
}

#[test]
fn a_new_session_gets_a_tab_and_becomes_active() {
    let mut test = app();

    test.app.apply_core_event(Event::Connected { session: 7, name: "prod".to_string(), shell_available: true });

    assert_eq!(test.app.sessions.active().unwrap().id, 7);
    assert_eq!(test.app.connection_status, ConnectionStatus::Disconnected);
}

#[test]
fn panel_event_listed_updates_the_matching_sessions_panel() {
    let mut test = app();
    let id = test.connect(1, "test");

    test.list_remote(id, "/home/user", Vec::new());

    assert_eq!(test.app.sessions.active().unwrap().panel.path(), Path::new("/home/user"));
}

#[test]
fn panel_event_listed_for_a_vanished_session_is_dropped() {
    let mut test = app();

    test.list_remote(999, "/x", Vec::new());

    assert!(test.app.sessions.is_empty());
}

#[test]
fn panel_event_failed_shows_a_notification_when_the_session_still_exists() {
    let mut test = app();
    test.connect(1, "test");

    test.app.apply_core_event(Event::Notice { severity: Severity::Error, message: "boom".to_string() });

    assert_eq!(test.notification().as_deref(), Some("boom"));
}

#[test]
fn connecting_to_an_already_connected_host_switches_instead_of_reconnecting() {
    let mut test = on_connections_screen(vec![connection("test", ConnectionSource::Profile)]);
    let first = test.connect(1, "test");
    test.connect(2, "other");

    test.app.connect_to_selected();

    assert_eq!(test.app.sessions.active_id(), Some(first));
    assert!(test.sent().is_empty());
    assert_eq!(test.app.connection_status, ConnectionStatus::Disconnected);
}

#[test]
fn disconnect_selected_reports_a_confirmation_notification() {
    let mut test = on_connections_screen(vec![connection("test", ConnectionSource::Profile)]);
    let session = test.connect(1, "test");

    test.app.apply_action(Action::Disconnect);

    assert_eq!(test.sent(), vec![Command::Disconnect { session }]);
    test.app
        .apply_core_event(Event::Notice { severity: Severity::Info, message: "Disconnected from test".to_string() });
    test.app.apply_core_event(Event::Disconnected { session, name: "test".to_string() });
    assert!(test.app.sessions.is_empty());
    assert_eq!(test.notification().as_deref(), Some("Disconnected from test"));
}

#[test]
fn a_password_question_opens_a_masked_prompt() {
    let mut test = app();

    test.app.apply_core_event(Event::Question {
        request_id: 4,
        question: Question::Password { username: "u".to_string(), name: "srv".to_string() },
    });

    match test.app.dialog {
        Some(Dialog::Form(ref dialog)) => {
            assert_eq!(dialog.title, "Password for u@srv");
            assert_eq!(dialog.fields[0].key, "password");
            assert_eq!(dialog.fields[0].kind, crate::widgets::dialog::FieldKind::Masked);
        }
        _ => panic!("expected the password prompt"),
    }
}

#[test]
fn submitting_the_password_answers_the_question() {
    let mut test = app();
    test.app.apply_core_event(Event::Question {
        request_id: 4,
        question: Question::Password { username: "u".to_string(), name: "srv".to_string() },
    });

    for c in "pw".chars() {
        test.app.apply_dialog_key(key(KeyCode::Char(c)));
    }
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::Answer { request_id: 4, answer: Some(Answer::Password("pw".to_string())), save: false }]
    );
}

#[test]
fn escaping_the_password_prompt_cancels_the_connection() {
    let mut test = app();
    test.app.connection_status = ConnectionStatus::Connecting("srv".to_string());
    test.app.apply_core_event(Event::Question {
        request_id: 4,
        question: Question::Password { username: "u".to_string(), name: "srv".to_string() },
    });

    test.app.apply_dialog_key(key(KeyCode::Esc));

    assert!(test.app.dialog.is_none());
    assert_eq!(test.sent(), vec![Command::Answer { request_id: 4, answer: None, save: false }]);
    assert_eq!(test.app.connection_status, ConnectionStatus::Disconnected);
}

fn ask_to_trust_a_host_key(test: &mut TestApp) {
    test.app.connection_status = ConnectionStatus::Connecting("web".to_string());
    test.app.apply_core_event(Event::Question {
        request_id: 9,
        question: Question::TrustHostKey {
            name: "web".to_string(),
            host: "example.com".to_string(),
            port: 2222,
            key_type: "ssh-ed25519".to_string(),
            fingerprint: "SHA256:abc".to_string(),
        },
    });
}

#[test]
fn a_host_key_question_shows_the_host_and_fingerprint_with_no_focused() {
    let mut test = app();

    ask_to_trust_a_host_key(&mut test);

    match test.app.dialog {
        Some(Dialog::Confirm(ref dialog)) => {
            assert_eq!(
                dialog.message,
                "web (example.com:2222) is not a known host.\n\
                 ssh-ed25519 SHA256:abc\n\
                 Trust this key and add it to ~/.ssh/known_hosts?"
            );
            assert_eq!(dialog.focus, ConfirmFocus::No);
        }
        _ => panic!("expected the host key confirmation"),
    }
}

#[test]
fn confirming_the_host_key_trusts_it() {
    let mut test = app();
    ask_to_trust_a_host_key(&mut test);

    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert!(test.app.dialog.is_none());
    assert_eq!(test.sent(), vec![Command::Answer { request_id: 9, answer: Some(Answer::Confirmed), save: false }]);
    assert_eq!(test.app.connection_status, ConnectionStatus::Connecting("web".to_string()));
}

#[test]
fn declining_the_host_key_cancels_the_connection() {
    let mut test = app();
    ask_to_trust_a_host_key(&mut test);

    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert!(test.app.dialog.is_none());
    assert_eq!(test.sent(), vec![Command::Answer { request_id: 9, answer: None, save: false }]);
    assert_eq!(test.app.connection_status, ConnectionStatus::Disconnected);
}

#[test]
fn add_connection_dialog_saves_a_new_profile_on_submit() {
    let mut test = on_connections_screen(Vec::new());

    test.app.apply_action(Action::AddConnection);
    assert!(matches!(test.app.dialog, Some(Dialog::Form(_))));
    fill_form(&mut test, ["prod", "server.example.com", "2222", "deploy"]);
    set_field(&mut test, "password", "hunter2");
    test.app.apply_dialog_key(ctrl_s());

    assert_eq!(
        test.sent(),
        vec![Command::SaveProfile {
            original: None,
            draft: Box::new(ProfileDraft {
                name: "prod".to_string(),
                host: "server.example.com".to_string(),
                port: "2222".to_string(),
                username: "deploy".to_string(),
                password: porthmos_core::profiles::SecretEdit::Replace("hunter2".into()),
                protocol: "sftp".to_string(),
                remote_path: String::new(),
                options: BTreeMap::from([("identity_file".to_string(), String::new())]),
                group: String::new(),
                tags: String::new(),
                secret_options: Default::default(),
            }),
        }]
    );
    test.app.apply_core_event(Event::ProfileSaved);
    assert!(test.app.dialog.is_none());
}

#[test]
fn add_connection_dialog_keeps_the_dialog_open_on_invalid_port() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);
    fill_form(&mut test, ["prod", "server.example.com", "not-a-port", "deploy"]);
    test.app.apply_dialog_key(key(KeyCode::Enter));

    test.app.apply_core_event(Event::ProfileRejected { message: "Port must be a number from 1-65535".to_string() });

    match test.app.dialog {
        Some(Dialog::Form(ref form)) => assert!(form.error.is_some()),
        _ => panic!("expected the form dialog to stay open with an error"),
    }
}

#[test]
fn add_connection_dialog_keeps_the_dialog_open_on_port_zero() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);
    fill_form(&mut test, ["prod", "server.example.com", "0", "deploy"]);
    test.app.apply_dialog_key(key(KeyCode::Enter));

    test.app.apply_core_event(Event::ProfileRejected { message: "Port must be a number from 1-65535".to_string() });

    match test.app.dialog {
        Some(Dialog::Form(ref form)) => assert_eq!(form.error.as_deref(), Some("Port must be a number from 1-65535")),
        _ => panic!("expected the form dialog to stay open with an error"),
    }
}

#[test]
fn add_connection_dialog_rejects_a_name_that_already_exists() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);
    fill_form(&mut test, ["prod", "new.example.com", "22", "deploy"]);
    test.app.apply_dialog_key(key(KeyCode::Enter));

    test.app
        .apply_core_event(Event::ProfileRejected { message: "A connection named \"prod\" already exists".to_string() });

    match test.app.dialog {
        Some(Dialog::Form(ref form)) => assert!(form.error.is_some()),
        _ => panic!("expected the form dialog to stay open with an error"),
    }
}

#[test]
fn edit_connection_rejects_renaming_onto_an_existing_name() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);
    test.app.apply_action(Action::Rename);
    set_field(&mut test, "name", "staging");
    test.app.apply_dialog_key(ctrl_s());

    assert!(matches!(
        test.sent().as_slice(),
        [Command::SaveProfile { original: Some(original), draft }] if original == "prod" && draft.name == "staging"
    ));
    test.app.apply_core_event(Event::ProfileRejected {
        message: "A connection named \"staging\" already exists".to_string(),
    });
    assert!(matches!(test.app.dialog, Some(Dialog::Form(ref form)) if form.error.is_some()));
}

#[test]
fn add_connection_action_does_nothing_outside_the_connections_screen() {
    let mut test = app();
    test.app.apply_action(Action::AddConnection);
    assert!(test.app.dialog.is_none());
}

#[test]
fn add_connection_dialog_stays_open_when_saving_fails() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);
    fill_form(&mut test, ["prod", "server.example.com", "22", "deploy"]);
    test.app.apply_dialog_key(key(KeyCode::Enter));

    test.app.apply_core_event(Event::Notice { severity: Severity::Error, message: "Not a directory".to_string() });

    assert!(test.app.dialog.is_some(), "the form must stay open so the user's input isn't lost on a save error");
    assert!(test.app.notifications.current().is_some());
}

#[test]
fn editing_a_connection_sends_its_identity_file_and_remote_folder_from_the_form() {
    let mut entry = connection("prod", ConnectionSource::Profile);
    entry.options = BTreeMap::from([
        ("identity_file".to_string(), "/home/user/.ssh/id_ed25519".to_string()),
        ("remote_path".to_string(), "/var/www".to_string()),
    ]);
    let mut test = on_connections_screen(vec![entry]);

    test.app.apply_action(Action::Rename);
    set_field(&mut test, "host", "new.example.com");
    test.app.apply_dialog_key(ctrl_s());

    assert!(matches!(
        test.sent().as_slice(),
        [Command::SaveProfile { original: Some(original), draft }]
            if original == "prod"
                && draft.host == "new.example.com"
                && draft.remote_path == "/var/www"
                && draft.options.get("identity_file").map(String::as_str) == Some("/home/user/.ssh/id_ed25519")
    ));
}

#[test]
fn adding_a_connection_without_any_protocol_shows_a_notice_instead_of_a_form() {
    let mut test = screen_with(Vec::new(), Vec::new());

    test.app.apply_action(Action::AddConnection);

    assert!(test.app.dialog.is_none());
    assert_eq!(
        test.app.notifications.current().map(|notification| notification.message.as_str()),
        Some("No protocols are available in this build")
    );
}

#[test]
fn changing_the_protocol_choice_rebuilds_the_open_form() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);

    test.app.apply_dialog_key(key(KeyCode::Down));

    let Some(Dialog::Form(form)) = &test.app.dialog else { panic!("the form closed") };
    assert_eq!(form.focused, 0);
    assert_eq!(form.value("protocol").as_deref(), Some("ftp"));
    assert_eq!(form.value("port").as_deref(), Some("21"));
    assert!(form.value("identity_file").is_none());
}

#[test]
fn renaming_a_connection_to_a_new_name_deletes_the_old_profile() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);

    test.app.apply_action(Action::Rename);
    set_field(&mut test, "name", "production");
    test.app.apply_dialog_key(ctrl_s());

    assert!(matches!(
        test.sent().as_slice(),
        [Command::SaveProfile { original: Some(original), draft }] if original == "prod" && draft.name == "production"
    ));
}

#[test]
fn delete_connection_action_opens_a_confirm_dialog() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);

    test.app.apply_action(Action::DeleteConnection);

    assert!(matches!(test.app.dialog, Some(Dialog::Confirm(_))));
}

#[test]
fn confirming_delete_connection_removes_the_saved_profile() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);

    test.app.apply_action(Action::DeleteConnection);
    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert!(test.app.dialog.is_none());
    assert_eq!(test.sent(), vec![Command::DeleteProfile { name: "prod".to_string() }]);
}

#[test]
fn delete_connection_on_an_ssh_config_entry_does_not_open_a_dialog() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::SshConfig)]);

    test.app.apply_action(Action::DeleteConnection);

    assert!(test.app.dialog.is_none());
    assert!(test.app.notifications.current().is_some());
}

#[test]
fn editing_a_profile_with_an_unknown_saved_choice_opens_on_the_default() {
    let mut protocols = sample_protocols();
    protocols[1].form.options.push(OptionField {
        key: "security",
        label: "Security",
        required: false,
        kind: OptionKind::Choice { choices: SECURITY_CHOICES, default: "explicit" },
    });
    let mut entry = connection("files", ConnectionSource::Profile);
    entry.protocol = "ftp".to_string();
    entry.options = BTreeMap::from([("security".to_string(), "weird".to_string())]);
    let mut test = screen_with(protocols, vec![entry]);

    test.app.apply_action(Action::Rename);

    let Some(Dialog::Form(form)) = &test.app.dialog else { panic!("the edit form did not open") };
    assert_eq!(form.value("security").as_deref(), Some("explicit"));
}

const SECURITY_CHOICES: &[porthmos_core::Choice] = &[
    porthmos_core::Choice { value: "none", label: "None" },
    porthmos_core::Choice { value: "explicit", label: "Explicit TLS" },
];

fn ask_to_trust_a_certificate(test: &mut TestApp) {
    test.app.connection_status = ConnectionStatus::Connecting("nas".to_string());
    test.app.apply_core_event(Event::Question {
        request_id: 11,
        question: Question::TrustCertificate {
            name: "nas".to_string(),
            host: "192.168.1.10".to_string(),
            port: 21,
            fingerprint: "SHA256:abc".to_string(),
            subject: "CN=nas.local".to_string(),
            expires: "2027-03-01".to_string(),
        },
    });
}

#[test]
fn a_certificate_question_shows_subject_expiry_and_fingerprint_with_no_focused() {
    let mut test = app();
    ask_to_trust_a_certificate(&mut test);

    match test.app.dialog {
        Some(Dialog::Confirm(ref dialog)) => {
            assert_eq!(
                dialog.message,
                "nas (192.168.1.10:21) presented a certificate that isn't trusted.\n\
                 Subject: CN=nas.local   Expires: 2027-03-01\n\
                 SHA256:abc\n\
                 Trust it and remember it?"
            );
            assert_eq!(dialog.focus, ConfirmFocus::No);
        }
        _ => panic!("expected the certificate confirmation"),
    }
}

#[test]
fn confirming_the_certificate_trusts_it() {
    let mut test = app();
    ask_to_trust_a_certificate(&mut test);

    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert_eq!(test.sent(), vec![Command::Answer { request_id: 11, answer: Some(Answer::Confirmed), save: false }]);
}

#[test]
fn declining_the_certificate_cancels_the_connection() {
    let mut test = app();
    ask_to_trust_a_certificate(&mut test);

    test.app.apply_dialog_key(key(KeyCode::Esc));

    assert_eq!(test.sent(), vec![Command::Answer { request_id: 11, answer: None, save: false }]);
    assert_eq!(test.app.connection_status, ConnectionStatus::Disconnected);
}

fn grouped(name: &str, group: &str) -> ConnectionEntry {
    let mut entry = connection(name, ConnectionSource::Profile);
    entry.group = Some(group.to_string());
    entry
}

#[test]
fn enter_on_a_group_toggles_it_and_enter_on_a_connection_connects() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app.connections.replace(vec![grouped("web", "Work")]);

    test.app.apply_key(key(KeyCode::Enter));
    assert_eq!(test.app.connections.rows().len(), 2);
    assert!(test.sent().is_empty());

    test.app.apply_key(key(KeyCode::Down));
    test.app.apply_key(key(KeyCode::Enter));
    assert_eq!(test.sent(), vec![Command::Connect { profile: "web".into() }]);
}

#[test]
fn right_opens_and_left_closes_a_group_on_the_connections_screen() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app.connections.replace(vec![grouped("web", "Work")]);

    test.app.apply_key(key(KeyCode::Right));
    assert_eq!(test.app.connections.rows().len(), 2);
    test.app.apply_key(key(KeyCode::Left));
    assert_eq!(test.app.connections.rows().len(), 1);
}

#[test]
fn slash_filters_the_connections_and_esc_clears_before_leaving() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app
        .connections
        .replace(vec![connection("web", ConnectionSource::Profile), connection("db", ConnectionSource::Profile)]);

    test.app.apply_key(key(KeyCode::Char('/')));
    test.app.apply_key(key(KeyCode::Char('w')));
    test.app.apply_key(key(KeyCode::Enter));
    assert_eq!(test.app.connections.rows().len(), 1);

    test.app.apply_key(key(KeyCode::Esc));
    assert_eq!(test.app.connections.rows().len(), 2);
    assert_eq!(test.app.screen, Screen::Connections);
    test.app.apply_key(key(KeyCode::Esc));
    assert_eq!(test.app.screen, Screen::Files);
}

#[test]
fn typing_in_the_filter_narrows_and_another_key_closes_the_line_and_acts() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app
        .connections
        .replace(vec![connection("web", ConnectionSource::Profile), connection("db", ConnectionSource::Profile)]);

    test.app.apply_key(key(KeyCode::Char('/')));
    test.app.apply_key(key(KeyCode::Char('d')));
    assert_eq!(test.app.connections.match_counts(), (1, 2));
    test.app.apply_key(key(KeyCode::F(9)));

    assert!(!test.app.connections.editing_filter());
    assert_eq!(test.app.connections.filter(), Some("d"));
    assert_eq!(test.sent(), vec![Command::ListProfiles]);
}

#[test]
fn edit_and_delete_leave_orphan_label_rows_alone_and_enter_only_offers_the_fix() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app.connections.replace(vec![ConnectionEntry::orphan_labels(
        "web1".into(),
        Default::default(),
        ConnectionSource::ShadowedSshHost,
    )]);

    test.app.announced_missing.insert("web1".into());

    test.app.apply_action(Action::Rename);
    test.app.apply_action(Action::DeleteConnection);
    assert!(test.app.dialog.is_none());

    test.app.apply_action(Action::Open);
    assert!(matches!(test.app.dialog, Some(Dialog::List(_))));
    assert!(test.sent().is_empty());
}

#[test]
fn the_cursor_stays_on_the_same_connection_after_a_reload() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app
        .connections
        .replace(vec![connection("b", ConnectionSource::Profile), connection("c", ConnectionSource::Profile)]);
    test.app.apply_key(key(KeyCode::Down));

    test.app.apply_core_event(Event::Profiles(vec![
        connection("a", ConnectionSource::Profile),
        connection("b", ConnectionSource::Profile),
        connection("c", ConnectionSource::Profile),
    ]));

    assert_eq!(test.app.connections.selected_entry().map(|entry| entry.name.as_str()), Some("c"));
}

#[test]
fn f2_on_an_ssh_host_opens_the_labels_form_and_enter_sends_labels() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app.connections.replace(vec![connection("web1", ConnectionSource::SshConfig)]);

    test.app.apply_action(Action::Rename);
    let Some(Dialog::Form(form)) = &test.app.dialog else { panic!("no form") };
    assert_eq!(form.title, "Labels for web1");
    test.app.apply_key(key(KeyCode::Char('W')));
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::SaveSshLabels { name: "web1".into(), group: "W".into(), tags: String::new() }]
    );
}

#[test]
fn a_rejected_labels_save_shows_in_the_form() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app.connections.replace(vec![connection("web1", ConnectionSource::SshConfig)]);
    test.app.apply_action(Action::Rename);

    test.app.apply_core_event(Event::ProfileRejected { message: "web1 is not in ~/.ssh/config".into() });

    let Some(Dialog::Form(form)) = &test.app.dialog else { panic!("form closed") };
    assert_eq!(form.error.as_deref(), Some("web1 is not in ~/.ssh/config"));
}

#[test]
fn after_a_save_the_connection_is_revealed() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app.connections.replace(vec![connection("aaa", ConnectionSource::Profile)]);
    test.app.apply_action(Action::AddConnection);
    test.app.submit_connection_form(vec![("name", " web ".into()), ("group", "Work/Web".into())], None);

    test.app.apply_core_event(Event::ProfileSaved);
    let mut saved = connection("web", ConnectionSource::Profile);
    saved.group = Some("Work/Web".into());
    test.app.apply_core_event(Event::Profiles(vec![connection("aaa", ConnectionSource::Profile), saved]));

    assert_eq!(test.app.connections.selected_entry().map(|entry| entry.name.as_str()), Some("web"));
}

#[test]
fn after_saving_ssh_labels_the_host_is_revealed_in_its_group() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app
        .connections
        .replace(vec![connection("aaa", ConnectionSource::Profile), connection("web1", ConnectionSource::SshConfig)]);
    test.app.apply_key(key(KeyCode::Down));
    test.app.apply_action(Action::Rename);
    test.app.apply_key(key(KeyCode::Char('W')));
    test.app.apply_key(key(KeyCode::Enter));

    test.app.apply_core_event(Event::ProfileSaved);
    let mut labelled = connection("web1", ConnectionSource::SshConfig);
    labelled.group = Some("W".into());
    test.app.apply_core_event(Event::Profiles(vec![connection("aaa", ConnectionSource::Profile), labelled]));

    assert_eq!(test.app.connections.selected_entry().map(|entry| entry.name.as_str()), Some("web1"));
    assert!(test.app.dialog.is_none());
}

#[test]
fn renaming_and_regrouping_reveals_the_new_name() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    let mut old = connection("old", ConnectionSource::Profile);
    old.group = Some("A".into());
    test.app.connections.replace(vec![old]);
    test.app.submit_connection_form(vec![("name", "new".into()), ("group", "B/C".into())], Some("old".into()));

    test.app.apply_core_event(Event::ProfileSaved);
    let mut renamed = connection("new", ConnectionSource::Profile);
    renamed.group = Some("B/C".into());
    test.app.apply_core_event(Event::Profiles(vec![renamed]));

    assert_eq!(test.app.connections.selected_entry().map(|entry| entry.name.as_str()), Some("new"));
    assert_eq!(test.app.connections.rows().len(), 3);
}

#[test]
fn a_listing_without_a_save_does_not_move_the_cursor() {
    let mut test = app();
    test.app.screen = Screen::Connections;
    test.app
        .connections
        .replace(vec![connection("a", ConnectionSource::Profile), connection("b", ConnectionSource::Profile)]);
    test.app.submit_connection_form(vec![("name", "b".into())], None);
    test.app.apply_core_event(Event::ProfileRejected { message: "bad".into() });

    test.app.apply_core_event(Event::Profiles(vec![
        connection("a", ConnectionSource::Profile),
        connection("b", ConnectionSource::Profile),
    ]));

    assert_eq!(test.app.connections.cursor, 0);
}

#[test]
fn cancelling_the_form_forgets_the_pending_reveal() {
    let mut test = screen_with(
        sample_protocols(),
        vec![connection("a", ConnectionSource::Profile), connection("b", ConnectionSource::Profile)],
    );
    test.app.apply_action(Action::AddConnection);
    assert!(matches!(test.app.dialog, Some(Dialog::Form(_))));
    test.app.submit_connection_form(vec![("name", "b".into())], None);
    test.app.apply_key(key(KeyCode::Esc));

    test.app.apply_core_event(Event::ProfileSaved);
    test.app.apply_core_event(Event::Profiles(vec![
        connection("a", ConnectionSource::Profile),
        connection("b", ConnectionSource::Profile),
    ]));

    assert_eq!(test.app.connections.cursor, 0);
}

#[test]
fn delete_on_a_shadowed_labels_row_does_not_disconnect_the_same_named_profile() {
    let mut test = app();
    test.connect(5, "foo");
    test.app.screen = Screen::Connections;
    test.app.announced_missing.insert("foo".into());
    test.app.connections.replace(vec![
        connection("foo", ConnectionSource::Profile),
        ConnectionEntry::orphan_labels("foo".into(), Default::default(), ConnectionSource::ShadowedSshHost),
    ]);
    test.sent();
    test.app.connections.cursor = 1;

    test.app.apply_action(Action::Delete);

    assert!(test.sent().is_empty());
}

fn ask_password(test: &mut TestApp, request_id: u64) {
    test.app.apply_core_event(Event::Question {
        request_id,
        question: Question::Password { username: "u".into(), name: "web".into() },
    });
}

fn prompt(test: &TestApp) -> &crate::widgets::dialog::FormDialog {
    match &test.app.dialog {
        Some(Dialog::Form(form)) => form,
        _ => panic!("no password prompt"),
    }
}

fn type_text(test: &mut TestApp, text: &str) {
    for character in text.chars() {
        test.app.apply_key(key(KeyCode::Char(character)));
    }
}

#[test]
fn the_password_prompt_offers_saving_at_the_remembered_choice() {
    let mut test = app();
    test.app.apply_core_event(Event::SaveChoice { save: true });
    ask_password(&mut test, 7);

    assert_eq!(prompt(&test).title, "Password for u@web");
    assert_eq!(prompt(&test).fields[1].label, "Save in keyring");
    assert_eq!(prompt(&test).value("save").as_deref(), Some("true"));

    type_text(&mut test, "p w");
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::Answer { request_id: 7, answer: Some(Answer::Password("p w".into())), save: true }]
    );
    assert!(test.app.dialog.is_none());
}

#[test]
fn the_first_prompt_starts_at_no() {
    let mut test = app();
    ask_password(&mut test, 1);

    assert_eq!(prompt(&test).value("save").as_deref(), Some("false"));
}

#[test]
fn changing_the_choice_is_remembered_and_keeping_it_is_not() {
    let mut test = app();
    ask_password(&mut test, 1);
    test.app.apply_key(key(KeyCode::Tab));
    test.app.apply_key(key(KeyCode::Right));
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![
            Command::RememberSaveChoice { save: true },
            Command::Answer { request_id: 1, answer: Some(Answer::Password(String::new())), save: true },
        ]
    );

    ask_password(&mut test, 2);
    assert_eq!(prompt(&test).value("save").as_deref(), Some("true"));
    type_text(&mut test, "x");
    test.app.apply_key(key(KeyCode::Enter));
    assert_eq!(
        test.sent(),
        vec![Command::Answer { request_id: 2, answer: Some(Answer::Password("x".into())), save: true }]
    );
}

#[test]
fn without_a_keyring_the_prompt_has_no_save_field_and_never_asks_to_save() {
    let mut test = app();
    test.app.apply_core_event(Event::SaveChoice { save: true });
    test.app.apply_core_event(Event::KeyringStatus { available: false });
    ask_password(&mut test, 1);

    assert!(prompt(&test).value("save").is_none());
    assert_eq!(prompt(&test).fields.len(), 1);
    type_text(&mut test, "pw");
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::Answer { request_id: 1, answer: Some(Answer::Password("pw".into())), save: false }]
    );
}

#[test]
fn the_save_field_returns_when_the_keyring_becomes_available() {
    let mut test = app();
    test.app.apply_core_event(Event::KeyringStatus { available: false });
    test.app.apply_core_event(Event::KeyringStatus { available: true });
    ask_password(&mut test, 1);

    assert!(prompt(&test).value("save").is_some());
}

#[test]
fn escaping_the_new_prompt_still_cancels() {
    let mut test = app();
    ask_password(&mut test, 3);
    type_text(&mut test, "abc");

    test.app.apply_key(key(KeyCode::Esc));

    assert_eq!(test.sent(), vec![Command::Answer { request_id: 3, answer: None, save: false }]);
    assert!(test.app.dialog.is_none());
}

#[test]
fn sent_commands_never_print_the_typed_password() {
    let mut test = app();
    ask_password(&mut test, 1);
    type_text(&mut test, "hunter2");
    test.app.apply_key(key(KeyCode::Enter));

    assert!(!format!("{:?}", test.sent()).contains("hunter2"));
}

#[test]
fn without_a_keyring_the_connection_form_says_why() {
    let mut test = on_connections_screen(vec![connection("web", ConnectionSource::Profile)]);
    test.app.apply_action(Action::AddConnection);
    assert!(matches!(&test.app.dialog, Some(Dialog::Form(form)) if form.hint.is_none()));
    test.app.dialog = None;

    test.app.apply_core_event(Event::KeyringStatus { available: false });
    test.app.apply_action(Action::AddConnection);
    let hint = |test: &TestApp| match &test.app.dialog {
        Some(Dialog::Form(form)) => form.hint.clone(),
        _ => panic!("no form"),
    };
    assert_eq!(
        hint(&test).as_deref(),
        Some("Passwords are not saved: no system keyring. They are kept until Porthmos quits.")
    );
    test.app.dialog = None;

    test.app.apply_action(Action::Rename);
    assert!(hint(&test).is_some());
}

#[test]
fn the_labels_form_can_forget_a_saved_password() {
    let mut host = connection("web1", ConnectionSource::SshConfig);
    host.saved_password = true;
    let mut test = on_connections_screen(vec![host]);

    test.app.apply_action(Action::Rename);
    test.app.apply_key(key(KeyCode::Tab));
    test.app.apply_key(key(KeyCode::Tab));
    test.app.apply_key(key(KeyCode::Right));
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![
            Command::SaveSshLabels { name: "web1".into(), group: String::new(), tags: String::new() },
            Command::ForgetSshPassword { alias: "web1".into() },
        ]
    );
}

#[test]
fn keeping_the_saved_password_sends_no_forget() {
    let mut host = connection("web1", ConnectionSource::SshConfig);
    host.saved_password = true;
    let mut test = on_connections_screen(vec![host]);

    test.app.apply_action(Action::Rename);
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::SaveSshLabels { name: "web1".into(), group: String::new(), tags: String::new() }]
    );
}

#[test]
fn the_status_line_shows_a_waiting_keyring() {
    use ratatui::{Terminal, backend::TestBackend};
    let status = |test: &TestApp| {
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal.draw(|frame| test.app.render_hint_bar(frame, frame.area())).unwrap();
        terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect::<String>()
    };
    let mut test = app();

    test.app.apply_core_event(Event::KeyringWaiting { waiting: true });
    assert!(status(&test).contains("Waiting for the system keyring\u{2026}"), "{}", status(&test));

    test.app.apply_core_event(Event::KeyringWaiting { waiting: false });
    assert!(!status(&test).contains("Waiting for the system keyring"));
}

#[test]
fn the_password_prompt_shows_bullets_never_the_password() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut test = app();
    ask_password(&mut test, 1);
    type_text(&mut test, "s3cret");

    let mut terminal = Terminal::new(TestBackend::new(70, 10)).unwrap();
    terminal.draw(|frame| test.app.dialog.as_ref().unwrap().render(frame, frame.area(), &Theme::default())).unwrap();
    let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();

    assert!(screen.contains("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"), "{screen}");
    assert!(!screen.contains("s3cret"), "{screen}");
}

fn open_form(test: &TestApp) -> &FormDialog {
    let Some(Dialog::Form(form)) = &test.app.dialog else { panic!("no form is open") };
    form
}

#[test]
fn the_add_form_is_a_screen_form() {
    let mut test = on_connections_screen(Vec::new());

    test.app.apply_action(Action::AddConnection);

    assert_eq!(open_form(&test).layout, FormLayout::Screen);
}

#[test]
fn the_edit_form_is_a_screen_form_and_starts_on_the_protocol_list() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);

    test.app.apply_action(Action::Rename);

    assert_eq!((open_form(&test).layout, open_form(&test).focused), (FormLayout::Screen, 0));
}

#[test]
fn editing_still_lets_the_protocol_change_with_up_and_down() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);
    test.app.apply_action(Action::Rename);

    test.app.apply_dialog_key(key(KeyCode::Down));

    assert_eq!(open_form(&test).value("protocol").as_deref(), Some("ftp"));
    assert_eq!(open_form(&test).focused, 0);
}

#[test]
fn tab_then_arrow_keys_walk_the_connection_fields() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);

    test.app.apply_dialog_key(key(KeyCode::Tab));
    assert_eq!(open_form(&test).fields[open_form(&test).focused].key, "name");
    test.app.apply_dialog_key(key(KeyCode::Down));
    assert_eq!(open_form(&test).fields[open_form(&test).focused].key, "group");
    test.app.apply_dialog_key(key(KeyCode::Up));
    test.app.apply_dialog_key(key(KeyCode::Up));
    assert_eq!(open_form(&test).focused, 0);
}

#[test]
fn other_dialog_forms_stay_popups() {
    let mut test = app();
    ask_password(&mut test, 1);
    assert_eq!(open_form(&test).layout, FormLayout::Popup);

    let mut labels = on_connections_screen(vec![connection("web1", ConnectionSource::SshConfig)]);
    labels.app.apply_action(Action::Rename);
    assert_eq!(open_form(&labels).layout, FormLayout::Popup);
}

#[test]
fn typing_in_the_name_field_never_triggers_actions() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);
    test.app.apply_dialog_key(key(KeyCode::Tab));

    for character in ['q', 'n', 'e', 'd', '/', '?', '1'] {
        test.app.apply_key(key(KeyCode::Char(character)));
    }

    assert_eq!(open_form(&test).value("name").as_deref(), Some("qned/?1"));
    assert_eq!(test.app.screen, Screen::Connections);
    assert!(!test.app.should_quit);
    assert!(test.sent().is_empty());
}

#[test]
fn f8_on_the_connections_screen_disconnects_through_the_disconnect_action() {
    let mut test = on_connections_screen(vec![connection("test", ConnectionSource::Profile)]);
    let session = test.connect(1, "test");

    test.app.apply_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(8),
        crossterm::event::KeyModifiers::NONE,
    ));

    assert_eq!(test.sent(), vec![Command::Disconnect { session }]);
    assert!(test.app.dialog.is_none());
}

#[test]
fn typing_in_the_popup_password_form_only_types() {
    let mut test = app();
    ask_password(&mut test, 1);
    test.sent();

    test.type_text(crate::app::testing::TYPED);

    assert_eq!(prompt(&test).fields[0].value, crate::app::testing::TYPED);
    test.assert_inert(Screen::Files);
}

#[test]
fn typing_in_the_screen_connection_form_only_types() {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);
    if let Some(Dialog::Form(form)) = test.app.dialog.as_mut() {
        form.focused = form.fields.iter().position(|field| field.key == "name").unwrap();
    }
    test.sent();

    test.type_text(crate::app::testing::TYPED);

    assert_eq!(open_form(&test).value("name").as_deref(), Some(crate::app::testing::TYPED));
    test.assert_inert(Screen::Connections);
}

#[test]
fn n_on_the_connections_screen_adds_a_connection_and_does_not_make_a_folder() {
    let mut test = on_connections_screen(Vec::new());
    test.sent();

    test.type_text("n");

    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert!(!matches!(test.app.dialog, Some(Dialog::TextInput(_))));
}

fn ctrl_t() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)
}

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn test_request(commands: Vec<Command>) -> (u64, Option<String>, Box<ProfileDraft>) {
    match commands.as_slice() {
        [Command::TestConnection { test_id, original, draft }] => (*test_id, original.clone(), draft.clone()),
        other => panic!("{other:?}"),
    }
}

fn open_add_form() -> TestApp {
    let mut test = on_connections_screen(Vec::new());
    test.app.apply_action(Action::AddConnection);
    fill_form(&mut test, ["nas", "nas.example.com", "22", "deploy"]);
    if let Some(Dialog::Form(form)) = test.app.dialog.as_mut() {
        form.focused = form.fields.iter().position(|field| field.key == "name").unwrap();
    }
    test.sent();
    test
}

fn open_edit_form(entry: ConnectionEntry) -> TestApp {
    let mut test = on_connections_screen(vec![entry]);
    test.app.apply_action(Action::Rename);
    test.sent();
    test
}

#[test]
fn ctrl_t_in_the_add_form_sends_test_connection_with_a_draft_and_no_original() {
    let mut test = open_add_form();

    test.app.apply_key(ctrl_t());

    let (test_id, original, draft) = test_request(test.sent());
    assert_eq!((test_id, original), (0, None));
    assert_eq!(
        (draft.name.as_str(), draft.host.as_str(), draft.username.as_str()),
        ("nas", "nas.example.com", "deploy")
    );
    assert_eq!(draft.protocol, "sftp");
    assert_eq!(open_form(&test).test, Some(TestState::Running));
}

#[test]
fn ctrl_t_in_the_edit_form_sends_the_original_name() {
    let mut test = open_edit_form(connection("prod", ConnectionSource::Profile));

    test.app.apply_key(ctrl_t());

    let (_, original, draft) = test_request(test.sent());
    assert_eq!(original.as_deref(), Some("prod"));
    assert_eq!(draft.name, "prod");
}

#[test]
fn a_kept_password_stays_keep_in_the_draft() {
    let mut entry = connection("prod", ConnectionSource::Profile);
    entry.saved_password = true;
    entry.in_keyring = vec!["password".to_string()];
    let mut test = open_edit_form(entry);

    test.app.apply_key(ctrl_t());

    let (_, _, draft) = test_request(test.sent());
    assert_eq!(draft.password, porthmos_core::profiles::SecretEdit::Keep);
}

#[test]
fn pressing_ctrl_t_while_a_test_runs_does_nothing() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();

    test.app.apply_key(ctrl_t());

    assert!(test.sent().is_empty());
}

#[test]
fn an_edit_while_a_test_runs_cancels_it_and_clears_the_line() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();

    test.app.apply_key(key_event(KeyCode::Char('x')));

    assert_eq!(test.sent(), vec![Command::CancelTest]);
    assert_eq!(open_form(&test).test, None);
}

#[test]
fn a_protocol_change_cancels_the_test() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();
    if let Some(Dialog::Form(form)) = test.app.dialog.as_mut() {
        form.focused = 0;
    }

    test.app.apply_key(key_event(KeyCode::Down));

    assert!(test.sent().contains(&Command::CancelTest));
    assert_eq!(open_form(&test).test, None);
}

#[test]
fn a_finished_event_stores_the_outcome_and_the_next_test_uses_a_new_id() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();

    test.app.apply_core_event(Event::TestFinished { test_id: 0, outcome: TestOutcome::Cancelled });

    assert_eq!(open_form(&test).test, Some(TestState::Finished(TestOutcome::Cancelled)));
    assert!(test.sent().is_empty());
    test.app.apply_key(ctrl_t());
    let (test_id, _, _) = test_request(test.sent());
    assert_eq!(test_id, 1);
}

#[test]
fn a_finished_event_for_an_old_id_is_ignored() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.app.apply_key(key_event(KeyCode::Char('x')));
    test.sent();

    test.app.apply_core_event(Event::TestFinished { test_id: 0, outcome: TestOutcome::Cancelled });

    assert_eq!(open_form(&test).test, None);
}

#[test]
fn a_finished_event_without_an_open_form_is_ignored() {
    let mut test = on_connections_screen(Vec::new());

    test.app.apply_core_event(Event::TestFinished { test_id: 0, outcome: TestOutcome::Cancelled });

    assert!(test.app.dialog.is_none());
}

#[test]
fn cancelling_the_form_while_a_test_runs_sends_cancel_test() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();

    test.app.apply_key(key_event(KeyCode::Esc));

    assert_eq!(test.sent(), vec![Command::CancelTest]);
    assert!(test.app.dialog.is_none());
}

#[test]
fn saving_the_form_while_a_test_runs_sends_cancel_test_before_the_save() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();

    test.app.apply_key(ctrl_s());

    let sent = test.sent();
    assert_eq!(sent[0], Command::CancelTest);
    assert!(matches!(sent[1], Command::SaveProfile { .. }), "{sent:?}");
}

#[test]
fn ctrl_t_elsewhere_still_opens_the_transfers_screen() {
    let mut test = app();

    test.app.apply_key(ctrl_t());

    assert_eq!(test.app.screen, Screen::Transfers);
}

#[test]
fn ctrl_t_in_the_password_prompt_does_nothing() {
    let mut test = app();
    ask_password(&mut test, 1);
    test.sent();

    test.app.apply_key(ctrl_t());

    assert!(test.sent().is_empty());
    assert_eq!(prompt(&test).fields[0].value, "");
}

fn screen_text(test: &TestApp) -> String {
    crate::app::testing::screen_rows(&crate::app::testing::render_app(&test.app, 100, 30)).join("\n")
}

#[test]
fn the_form_screen_shows_the_testing_line_then_the_result_line() {
    let mut test = open_add_form();
    assert!(!screen_text(&test).contains("testing"));

    test.app.apply_key(ctrl_t());
    assert!(screen_text(&test).contains("\u{25cb} testing\u{2026}"));

    test.app.apply_core_event(Event::TestFinished {
        test_id: 0,
        outcome: TestOutcome::Failed { stage: porthmos_core::TestStage::Login, message: "denied".into() },
    });
    let text = screen_text(&test);
    assert!(text.contains("\u{25cf} failed at login \u{b7} denied"), "{text}");
    assert!(!text.contains("testing"));
}

#[test]
fn the_edit_form_shows_the_result_line_too() {
    let mut test = open_edit_form(connection("prod", ConnectionSource::Profile));
    test.app.apply_key(ctrl_t());
    test.app.apply_core_event(Event::TestFinished { test_id: 0, outcome: TestOutcome::Cancelled });

    assert!(screen_text(&test).contains("\u{25cf} test cancelled"));
}

#[test]
fn a_typed_password_is_never_on_the_screen_with_a_result_line() {
    let mut test = open_add_form();
    set_field(&mut test, "password", "hunter2-secret");
    test.app.apply_key(ctrl_t());

    assert!(!screen_text(&test).contains("hunter2"));
}

fn ask_test_password(test: &mut TestApp, test_id: u64, request_id: u64) {
    test.app.apply_core_event(Event::TestQuestion {
        test_id,
        request_id,
        question: Question::Password { username: "deploy".into(), name: "nas".into() },
    });
}

fn running_test_with_prompt() -> TestApp {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();
    ask_test_password(&mut test, 0, 31);
    test
}

#[test]
fn a_test_question_opens_the_masked_prompt_without_the_save_field() {
    let mut test = open_add_form();
    test.app.apply_core_event(Event::SaveChoice { save: true });
    test.app.apply_key(ctrl_t());
    test.sent();

    ask_test_password(&mut test, 0, 31);

    assert_eq!(prompt(&test).title, "Password for deploy@nas");
    assert_eq!(prompt(&test).fields.len(), 1);
    assert_eq!(prompt(&test).fields[0].key, "password");
    assert!(matches!(prompt(&test).fields[0].kind, FieldKind::Masked));
    assert!(test.app.stashed_form.is_some());
    assert!(matches!(test.app.pending_action, Some(PendingAction::SubmitTestPassword { request_id: 31 })));
    assert!(test.sent().is_empty());
}

#[test]
fn submitting_answers_with_save_false_and_restores_the_form_with_its_values() {
    let mut test = open_add_form();
    test.app.apply_core_event(Event::SaveChoice { save: true });
    test.app.apply_key(ctrl_t());
    test.sent();
    ask_test_password(&mut test, 0, 31);

    test.type_text("p w");
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::Answer { request_id: 31, answer: Some(Answer::Password("p w".into())), save: false }]
    );
    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert_eq!(open_form(&test).value("name").as_deref(), Some("nas"));
    assert_eq!(open_form(&test).value("host").as_deref(), Some("nas.example.com"));
    assert_eq!(open_form(&test).test, Some(TestState::Running));
    assert!(test.app.stashed_form.is_none());
    assert!(matches!(test.app.pending_action, Some(PendingAction::AddConnection)));
    assert!(test.app.connection_test.is_some());
}

#[test]
fn esc_answers_none_ends_the_test_as_cancelled_and_restores_the_form() {
    let mut test = running_test_with_prompt();

    test.app.apply_key(key(KeyCode::Esc));

    assert_eq!(test.sent(), vec![Command::Answer { request_id: 31, answer: None, save: false }]);
    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert!(test.app.stashed_form.is_none());
    assert!(matches!(test.app.connection_status, ConnectionStatus::Disconnected));
    test.app.apply_core_event(Event::TestFinished { test_id: 0, outcome: TestOutcome::Cancelled });
    assert_eq!(open_form(&test).test, Some(TestState::Finished(TestOutcome::Cancelled)));
}

#[test]
fn esc_on_the_test_prompt_does_not_touch_the_connection_status() {
    let mut test = running_test_with_prompt();
    test.app.connection_status = ConnectionStatus::Connecting("web".into());

    test.app.apply_key(key(KeyCode::Esc));

    assert!(matches!(test.app.connection_status, ConnectionStatus::Connecting(_)));
}

#[test]
fn the_form_focus_and_typed_values_survive_the_prompt() {
    let mut test = open_add_form();
    set_field(&mut test, "password", "keep-me");
    let (focused, cursor) = {
        let form = open_form(&test);
        (form.focused, form.fields[form.focused].cursor)
    };
    test.app.apply_key(ctrl_t());
    test.sent();
    ask_test_password(&mut test, 0, 31);
    test.type_text("abc");
    test.app.apply_key(key(KeyCode::Enter));
    test.sent();

    let form = open_form(&test);
    assert_eq!(form.focused, focused);
    assert_eq!(form.fields[form.focused].cursor, cursor);
    assert_eq!(form.value("password").as_deref(), Some("keep-me"));
    assert_eq!(form.value("protocol").as_deref(), Some("sftp"));
}

#[test]
fn a_regular_connect_question_arriving_while_a_form_is_open_stashes_and_restores_the_form() {
    let mut test = open_add_form();

    ask_password(&mut test, 9);

    assert_eq!(prompt(&test).title, "Password for u@web");
    assert!(test.app.stashed_form.is_some());
    test.app.apply_key(key(KeyCode::Esc));
    assert_eq!(test.sent(), vec![Command::Answer { request_id: 9, answer: None, save: false }]);
    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert_eq!(open_form(&test).value("name").as_deref(), Some("nas"));
    assert!(matches!(test.app.pending_action, Some(PendingAction::AddConnection)));
}

#[test]
fn a_regular_trust_question_while_a_form_is_open_restores_the_form_after_the_answer() {
    let mut test = open_add_form();
    ask_to_trust_a_host_key(&mut test);
    assert!(matches!(test.app.dialog, Some(Dialog::Confirm(_))));

    test.app.apply_key(key(KeyCode::Char('y')));
    test.sent();

    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert!(test.app.stashed_form.is_none());
}

#[test]
fn a_password_prompt_never_shows_the_typed_text() {
    let mut test = running_test_with_prompt();
    test.type_text("hunter2-secret");

    assert!(!screen_text(&test).contains("hunter2"));
    assert!(matches!(prompt(&test).fields[0].kind, FieldKind::Masked));
    assert!(!format!("{:?}", prompt(&test).fields[0].kind).contains("hunter2"));
}

#[test]
fn typing_in_the_test_prompt_triggers_no_action() {
    let mut test = running_test_with_prompt();

    test.type_text(crate::app::testing::TYPED);

    assert_eq!(prompt(&test).fields[0].value, crate::app::testing::TYPED);
    test.assert_inert(Screen::Connections);
}

#[test]
fn the_prompts_esc_is_the_only_way_out_and_the_form_is_not_closed_by_it() {
    let mut test = running_test_with_prompt();

    test.app.apply_key(key(KeyCode::Esc));

    assert!(matches!(test.app.dialog, Some(Dialog::Form(_))));
    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert!(!test.sent().contains(&Command::CancelTest));
}

#[test]
fn the_pending_edit_action_is_restored_exactly() {
    let mut test = open_edit_form(connection("prod", ConnectionSource::Profile));
    test.app.apply_key(ctrl_t());
    test.sent();
    ask_test_password(&mut test, 0, 31);
    test.app.apply_key(key(KeyCode::Esc));
    test.app.apply_core_event(Event::TestFinished { test_id: 0, outcome: TestOutcome::Cancelled });
    test.sent();

    match &test.app.pending_action {
        Some(PendingAction::EditConnection { original }) => assert_eq!(original.name, "prod"),
        _ => panic!("the edit action was lost"),
    }
    test.app.apply_key(ctrl_t());
    let (_, original, _) = test_request(test.sent());
    assert_eq!(original.as_deref(), Some("prod"));
}

#[test]
fn a_test_finished_while_the_prompt_is_open_is_stored_for_the_restored_form() {
    let mut test = running_test_with_prompt();

    test.app.apply_core_event(Event::TestFinished { test_id: 0, outcome: TestOutcome::Cancelled });
    assert!(test.app.connection_test.is_none());
    assert!(matches!(test.app.dialog, Some(Dialog::Form(_))));
    test.app.apply_key(key(KeyCode::Esc));

    assert_eq!(open_form(&test).test, Some(TestState::Finished(TestOutcome::Cancelled)));
}

#[test]
fn the_running_test_is_not_cancelled_by_keys_typed_in_the_prompt() {
    let mut test = running_test_with_prompt();

    test.type_text("abc");
    test.app.apply_key(key(KeyCode::Backspace));

    assert!(test.sent().is_empty());
    assert!(test.app.connection_test.is_some());
}

#[test]
fn a_second_question_while_one_is_stashed_does_not_lose_the_form() {
    let mut test = running_test_with_prompt();

    ask_password(&mut test, 9);

    assert_eq!(test.sent(), vec![Command::Answer { request_id: 31, answer: None, save: false }]);
    assert_eq!(prompt(&test).title, "Password for u@web");
    assert!(test.app.stashed_form.is_some());
    test.app.apply_key(key(KeyCode::Esc));
    test.sent();
    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert_eq!(open_form(&test).value("name").as_deref(), Some("nas"));
}

#[test]
fn a_test_question_for_an_old_test_is_ignored() {
    let mut test = open_add_form();
    test.app.apply_key(ctrl_t());
    test.sent();

    ask_test_password(&mut test, 99, 31);

    assert_eq!(open_form(&test).layout, FormLayout::Screen);
    assert!(test.app.stashed_form.is_none());
    assert!(test.sent().is_empty());
}

#[test]
fn a_test_question_with_no_form_open_is_ignored() {
    let mut test = app();

    ask_test_password(&mut test, 0, 31);

    assert!(test.app.dialog.is_none());
    assert!(test.app.stashed_form.is_none());
}

#[test]
fn enter_on_a_connected_session_just_switches_to_it() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);
    test.connect(2, "prod");
    test.sent();

    test.app.apply_action(Action::Open);

    assert!(test.sent().is_empty());
}

#[test]
fn enter_on_a_lost_session_drops_it_and_connects_again() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);
    test.connect(2, "prod");
    test.app.apply_core_event(Event::SessionHealth { session: 2, healthy: false });
    test.sent();

    test.app.apply_action(Action::Open);

    assert_eq!(test.sent(), vec![Command::Disconnect { session: 2 }, Command::Connect { profile: "prod".to_string() }]);
}

#[test]
fn a_restored_session_is_switched_to_again_not_reconnected() {
    let mut test = on_connections_screen(vec![connection("prod", ConnectionSource::Profile)]);
    test.connect(2, "prod");
    test.app.apply_core_event(Event::SessionHealth { session: 2, healthy: false });
    test.app.apply_core_event(Event::SessionHealth { session: 2, healthy: true });
    test.sent();

    test.app.apply_action(Action::Open);

    assert!(test.sent().is_empty());
}
