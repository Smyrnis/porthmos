use iced::keyboard::key::Named;
use porthmos_core::profiles::{ConnectionSource, SecretEdit};
use porthmos_core::{Command, Event, Severity};

use super::super::super::*;
use super::super::tests::{
    fill_new, form, harness, named, open_manager, profile, saved, select, type_field, typed, with_profiles,
};
use crate::connections::form::FieldKind;
use crate::connections::{MissingChoice, Screen};
use crate::dialogs::Typed;
use crate::testing::{Harness, builtin_infos};

const MARKER: &str = "hunter2-marker";

fn saved_draft(harness: &mut Harness) -> (Option<String>, porthmos_core::profiles::ProfileDraft) {
    match harness.sent().into_iter().next() {
        Some(Command::SaveProfile { original, draft }) => (original, *draft),
        other => panic!("{other:?}"),
    }
}

#[test]
fn add_opens_a_form_on_the_first_protocol() {
    let mut harness = with_profiles(Vec::new());

    harness.press(Message::ManagerAddPressed);

    assert_eq!(form(&harness).protocol(), builtin_infos()[0].id);
    assert!(form(&harness).original_name().is_none());
}

#[test]
fn add_without_protocols_only_warns() {
    let mut harness = Harness::new();
    open_manager(&mut harness);

    harness.press(Message::ManagerAddPressed);

    assert!(matches!(harness.app.screen(), Some(Screen::List)));
    assert_eq!(harness.app.notices().last().unwrap().severity, Severity::Warning);
}

#[test]
fn save_sends_save_profile_without_an_original_for_a_new_connection() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    fill_new(&mut harness);

    harness.press(Message::FormSavePressed);

    let (original, draft) = saved_draft(&mut harness);
    assert_eq!(original, None);
    assert_eq!((draft.name.as_str(), draft.host.as_str(), draft.username.as_str()), ("prod", "example.com", "me"));
    assert!(matches!(harness.app.screen(), Some(Screen::Form(_))));
}

#[test]
fn renaming_on_edit_sends_the_old_name_as_original_and_the_new_in_the_draft() {
    let mut harness = with_profiles(vec![saved("prod")]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);
    assert_eq!(form(&harness).original_name(), Some("prod"));
    type_field(&mut harness, "name", "prod2");

    harness.press(Message::FormSavePressed);

    let (original, draft) = saved_draft(&mut harness);
    assert_eq!(original.as_deref(), Some("prod"));
    assert_eq!(draft.name, "prod2");
}

#[test]
fn editing_without_touching_the_password_keeps_it() {
    let mut entry = saved("prod");
    entry.saved_password = true;
    entry.in_keyring = vec!["password".to_string()];
    let mut harness = with_profiles(vec![entry]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);

    harness.press(Message::FormSavePressed);

    assert_eq!(saved_draft(&mut harness).1.password, SecretEdit::Keep);
}

#[test]
fn forgetting_and_typing_a_password_reach_the_draft() {
    let mut entry = saved("prod");
    entry.saved_password = true;
    entry.in_keyring = vec!["password".to_string()];
    let mut harness = with_profiles(vec![entry]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);

    harness.press(Message::FormSecretForgotten("password".to_string()));
    harness.press(Message::FormSavePressed);
    assert_eq!(saved_draft(&mut harness).1.password, SecretEdit::Clear);

    harness.press(Message::FormSecretRestored("password".to_string()));
    harness.press(Message::FormSecretTyped("password".to_string(), Typed::new(MARKER.to_string())));
    harness.press(Message::FormSavePressed);
    assert_eq!(saved_draft(&mut harness).1.password, SecretEdit::Replace(MARKER.to_string()));
}

#[test]
fn an_invalid_form_shows_the_message_and_sends_nothing() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);

    harness.press(Message::FormSavePressed);

    assert!(harness.sent().is_empty());
    assert_eq!(form(&harness).error.as_deref(), Some("Name can't be empty"));
}

#[test]
fn profile_rejected_keeps_the_form_open_with_the_message() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    fill_new(&mut harness);
    harness.press(Message::FormSavePressed);

    harness.feed(Event::ProfileRejected { message: "A connection named \"prod\" already exists".to_string() });

    assert_eq!(form(&harness).error.as_deref(), Some("A connection named \"prod\" already exists"));
}

#[test]
fn typing_clears_the_error() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    harness.press(Message::FormSavePressed);

    type_field(&mut harness, "name", "x");

    assert!(form(&harness).error.is_none());
}

#[test]
fn profile_saved_closes_the_form_and_the_new_list_selects_the_profile() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    fill_new(&mut harness);
    harness.press(Message::FormSavePressed);
    harness.sent();

    harness.feed(Event::ProfileSaved);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
    harness.feed(Event::Profiles(vec![saved("other"), saved("prod")]));

    assert_eq!(harness.app.manager().selected_entry().unwrap().name, "prod");
}

#[test]
fn cancel_closes_the_form_and_sends_nothing() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    fill_new(&mut harness);

    harness.press(Message::ScreenCancelled);

    assert!(matches!(harness.app.screen(), Some(Screen::List)));
    assert!(harness.sent().is_empty());
}

#[test]
fn changing_the_protocol_rebuilds_the_option_fields() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    let has_security = |harness: &Harness| form(harness).fields().iter().any(|field| field.key == "security");
    harness.press(Message::FormChoiceChanged("protocol".to_string(), "sftp".to_string()));
    assert!(!has_security(&harness));

    harness.press(Message::FormChoiceChanged("protocol".to_string(), "webdav".to_string()));

    assert!(has_security(&harness));
    harness.press(Message::FormChoiceChanged("security".to_string(), "http".to_string()));
    let security = form(&harness).fields().into_iter().find(|field| field.key == "security").unwrap();
    assert!(matches!(security.kind, FieldKind::Choice { selected, .. } if selected.value == "http"));
}

#[test]
fn a_toggle_reaches_the_draft() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    harness.press(Message::FormChoiceChanged("protocol".to_string(), "ftp".to_string()));

    harness.press(Message::FormToggled("passive".to_string(), false));

    assert_eq!(form(&harness).draft().options.get("passive").map(String::as_str), Some("false"));
}

#[test]
fn delete_asks_first_then_sends_delete_profile() {
    let mut harness = with_profiles(vec![saved("prod")]);
    select(&mut harness, 0);

    harness.press(Message::ManagerDeletePressed);
    assert!(matches!(harness.app.screen(), Some(Screen::ConfirmDelete { name }) if name == "prod"));
    assert!(harness.sent().is_empty());

    harness.press(Message::DeleteConfirmed);
    assert_eq!(harness.sent(), vec![Command::DeleteProfile { name: "prod".to_string() }]);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
}

#[test]
fn enter_cancels_a_delete_and_the_button_confirms_it() {
    let mut harness = with_profiles(vec![saved("prod")]);
    select(&mut harness, 0);
    harness.press(Message::ManagerDeletePressed);

    named(&mut harness, Named::Enter);

    assert!(harness.sent().is_empty());
    assert!(matches!(harness.app.screen(), Some(Screen::List)));

    harness.press(Message::ManagerDeletePressed);
    harness.press(Message::DeleteConfirmed);

    assert_eq!(harness.sent(), vec![Command::DeleteProfile { name: "prod".to_string() }]);
}

#[test]
fn deleting_an_ssh_config_host_only_notifies() {
    let mut harness = with_profiles(vec![profile("host", ConnectionSource::SshConfig)]);
    select(&mut harness, 0);

    harness.press(Message::ManagerDeletePressed);

    assert!(matches!(harness.app.screen(), Some(Screen::List)));
    assert_eq!(
        harness.app.notices().last().unwrap().message,
        "This connection is defined in ~/.ssh/config and can't be deleted here"
    );
}

#[test]
fn editing_an_ssh_config_host_saves_only_its_labels() {
    let mut harness = with_profiles(vec![profile("host", ConnectionSource::SshConfig)]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);
    assert!(matches!(harness.app.screen(), Some(Screen::Labels(_))));
    harness.press(Message::LabelsGroupTyped("Work".to_string()));
    harness.press(Message::LabelsTagsTyped("a, b".to_string()));

    harness.press(Message::LabelsSavePressed);

    assert_eq!(
        harness.sent(),
        vec![Command::SaveSshLabels { name: "host".to_string(), group: "Work".to_string(), tags: "a, b".to_string() }]
    );
}

#[test]
fn forgetting_the_saved_ssh_password_also_sends_forget_ssh_password() {
    let mut entry = profile("host", ConnectionSource::SshConfig);
    entry.saved_password = true;
    let mut harness = with_profiles(vec![entry]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);
    harness.press(Message::LabelsForgetToggled(true));

    harness.press(Message::LabelsSavePressed);

    assert_eq!(
        harness.sent(),
        vec![
            Command::SaveSshLabels { name: "host".to_string(), group: String::new(), tags: String::new() },
            Command::ForgetSshPassword { alias: "host".to_string() },
        ]
    );
}

#[test]
fn an_orphan_label_offers_to_move_or_forget() {
    let mut harness = with_profiles(vec![
        profile("gone", ConnectionSource::MissingSshHost),
        profile("new", ConnectionSource::SshConfig),
    ]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);
    assert!(matches!(harness.app.screen(), Some(Screen::MissingHost { name, shadowed: false }) if name == "gone"));

    harness.press(Message::MissingHostChosen(MissingChoice::Move("new".to_string())));
    assert_eq!(harness.sent(), vec![Command::MoveSshLabels { from: "gone".to_string(), to: "new".to_string() }]);

    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);
    harness.press(Message::MissingHostChosen(MissingChoice::Forget));
    assert_eq!(harness.sent(), vec![Command::ForgetSshLabels { name: "gone".to_string() }]);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
}

#[test]
fn orphan_labels_are_announced_once() {
    let mut harness = harness();
    let orphans = vec![profile("gone", ConnectionSource::MissingSshHost)];
    let before = harness.app.notices().len();

    harness.feed(Event::Profiles(orphans.clone()));
    harness.feed(Event::Profiles(orphans));

    assert_eq!(harness.app.notices().len(), before + 1);
    assert_eq!(harness.app.notices().last().unwrap().severity, Severity::Warning);
}

#[test]
fn escape_leaves_the_form_then_the_manager() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);

    named(&mut harness, Named::Escape);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
    named(&mut harness, Named::Escape);
    assert!(!harness.app.manager_open());
}

#[test]
fn typing_letters_while_a_form_is_open_runs_no_shortcut() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    harness.sent();

    typed(&mut harness, "n");
    named(&mut harness, Named::Delete);
    named(&mut harness, Named::F5);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn the_secret_message_never_prints_its_text() {
    let message = Message::FormSecretTyped("password".to_string(), Typed::new(MARKER.to_string()));

    assert!(!format!("{message:?}").contains(MARKER));
}
