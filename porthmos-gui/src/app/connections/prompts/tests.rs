use iced::keyboard::key::Named;
use porthmos_core::{Answer, Command, Event, Question};

use super::super::super::*;
use super::super::tests::{named, open_manager, with_profiles};
use crate::connections::Screen;
use crate::dialogs::{Dialog, Typed};
use crate::testing::Harness;

fn password_question() -> Question {
    Question::Password { username: "me".to_string(), name: "prod".to_string() }
}

fn host_question() -> Question {
    Question::TrustHostKey {
        name: "prod".to_string(),
        host: "h".to_string(),
        port: 22,
        key_type: "ssh-ed25519".to_string(),
        fingerprint: "SHA256:x".to_string(),
    }
}

fn ask(harness: &mut Harness, request_id: u64) {
    harness.feed(Event::Question { request_id, question: password_question() });
}

fn save_choice(harness: &Harness) -> Option<bool> {
    match harness.app.dialog() {
        Some(Dialog::Password { save, .. }) => *save,
        other => panic!("{other:?}"),
    }
}

fn type_password(harness: &mut Harness, text: &str) {
    harness.press(Message::DialogTyped(Typed::new(text.to_string())));
}

#[test]
fn the_password_dialog_has_a_save_checkbox_when_the_keyring_is_available() {
    let mut harness = Harness::new();
    ask(&mut harness, 1);

    assert_eq!(save_choice(&harness), Some(false));
}

#[test]
fn there_is_no_checkbox_when_the_keyring_is_unavailable() {
    let mut harness = Harness::new();
    harness.feed(Event::KeyringStatus { available: false });
    ask(&mut harness, 1);

    assert_eq!(save_choice(&harness), None);
}

#[test]
fn the_remembered_choice_is_the_default() {
    let mut harness = Harness::new();
    harness.feed(Event::SaveChoice { save: true });
    ask(&mut harness, 1);

    assert_eq!(save_choice(&harness), Some(true));
}

#[test]
fn accepting_with_save_sends_the_answer_with_save_true() {
    let mut harness = Harness::new();
    harness.sent();
    ask(&mut harness, 5);
    type_password(&mut harness, "s3cret");
    harness.press(Message::PasswordSaveToggled(true));

    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![
            Command::RememberSaveChoice { save: true },
            Command::Answer { request_id: 5, answer: Some(Answer::Password("s3cret".to_string())), save: true },
        ]
    );
}

#[test]
fn an_unchanged_choice_sends_no_remember_command() {
    let mut harness = Harness::new();
    harness.feed(Event::SaveChoice { save: true });
    harness.sent();
    ask(&mut harness, 5);
    type_password(&mut harness, "pw");

    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![Command::Answer { request_id: 5, answer: Some(Answer::Password("pw".to_string())), save: true }]
    );
}

#[test]
fn a_changed_choice_is_remembered_once() {
    let mut harness = Harness::new();
    harness.sent();
    ask(&mut harness, 1);
    harness.press(Message::PasswordSaveToggled(true));
    harness.press(Message::ModalAccepted);
    harness.sent();

    ask(&mut harness, 2);
    assert_eq!(save_choice(&harness), Some(true));
    harness.press(Message::ModalAccepted);

    let sent = harness.sent();
    assert_eq!(sent.len(), 1);
    assert!(matches!(sent[0], Command::Answer { request_id: 2, save: true, .. }));
}

#[test]
fn declining_sends_none_and_remembers_nothing() {
    let mut harness = Harness::new();
    harness.sent();
    ask(&mut harness, 3);
    harness.press(Message::PasswordSaveToggled(true));

    harness.press(Message::ModalDeclined);

    assert_eq!(harness.sent(), vec![Command::Answer { request_id: 3, answer: None, save: false }]);
}

#[test]
fn without_a_keyring_the_answer_is_never_saved() {
    let mut harness = Harness::new();
    harness.feed(Event::KeyringStatus { available: false });
    harness.sent();
    ask(&mut harness, 3);
    type_password(&mut harness, "pw");
    harness.press(Message::PasswordSaveToggled(true));

    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![Command::Answer { request_id: 3, answer: Some(Answer::Password("pw".to_string())), save: false }]
    );
}

#[test]
fn a_trust_answer_is_confirmed_and_never_saved() {
    let mut harness = Harness::new();
    harness.sent();
    harness.feed(Event::Question { request_id: 8, question: host_question() });

    harness.press(Message::ModalAccepted);

    assert_eq!(harness.sent(), vec![Command::Answer { request_id: 8, answer: Some(Answer::Confirmed), save: false }]);
}

#[test]
fn the_keyring_hint_appears_only_without_a_keyring() {
    let mut harness = Harness::new();
    assert!(harness.app.keyring_hint().is_none());

    harness.feed(Event::KeyringStatus { available: false });

    assert_eq!(
        harness.app.keyring_hint(),
        Some("Passwords are not saved: no system keyring. They are kept until Porthmos quits.")
    );
}

#[test]
fn keyring_waiting_is_tracked() {
    let mut harness = Harness::new();
    assert!(!harness.app.keyring_waiting());

    harness.feed(Event::KeyringWaiting { waiting: true });
    assert!(harness.app.keyring_waiting());

    harness.feed(Event::KeyringWaiting { waiting: false });
    assert!(!harness.app.keyring_waiting());
}

#[test]
fn a_question_over_an_open_form_shows_on_top_and_returns_to_the_form() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    harness.sent();

    ask(&mut harness, 4);
    assert!(harness.app.dialog().is_some());
    named(&mut harness, Named::Escape);

    assert!(harness.app.dialog().is_none());
    assert!(matches!(harness.app.screen(), Some(Screen::Form(_))));
    assert_eq!(harness.sent(), vec![Command::Answer { request_id: 4, answer: None, save: false }]);
}

#[test]
fn a_question_over_the_manager_leaves_it_open() {
    let mut harness = Harness::new();
    open_manager(&mut harness);

    ask(&mut harness, 4);
    harness.press(Message::ModalDeclined);

    assert!(harness.app.manager_open());
}
