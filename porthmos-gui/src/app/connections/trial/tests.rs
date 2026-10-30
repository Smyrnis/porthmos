use iced::keyboard::key::Named;
use porthmos_core::profiles::SecretEdit;
use porthmos_core::{Answer, Command, Event, NotTrusted, Question, TestOutcome, TestReport, TestStage};

use super::super::super::*;
use super::super::tests::{fill_new, form, harness, named, open_manager, saved, select, type_field, with_profiles};
use crate::connections::Screen;
use crate::connections::trial::TestState;
use crate::dialogs::Typed;
use crate::testing::Harness;

const MARKER: &str = "hunter2-marker";

fn test_command(harness: &mut Harness) -> (u64, Option<String>, porthmos_core::profiles::ProfileDraft) {
    match harness.sent().into_iter().next() {
        Some(Command::TestConnection { test_id, original, draft }) => (test_id, original, *draft),
        other => panic!("{other:?}"),
    }
}

fn new_form() -> Harness {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    fill_new(&mut harness);
    harness.sent();
    harness
}

fn connected_report() -> TestOutcome {
    TestOutcome::Connected(TestReport {
        elapsed: std::time::Duration::from_millis(5),
        server: None,
        start_folder: porthmos_core::StartFolder::Unchecked,
    })
}

fn password_question() -> Question {
    Question::Password { username: "me".to_string(), name: "prod".to_string() }
}

fn start(harness: &mut Harness) -> u64 {
    harness.press(Message::FormTestPressed);
    test_command(harness).0
}

#[test]
fn test_sends_test_connection_with_the_draft_and_no_original_for_a_new_connection() {
    let mut harness = new_form();

    harness.press(Message::FormTestPressed);

    let (id, original, draft) = test_command(&mut harness);
    assert_eq!(id, 1);
    assert_eq!(original, None);
    assert_eq!((draft.name.as_str(), draft.host.as_str()), ("prod", "example.com"));
    assert_eq!(form(&harness).test, Some(TestState::Running));
}

#[test]
fn test_on_an_edited_connection_sends_its_original_name() {
    let mut harness = with_profiles(vec![saved("prod")]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);
    harness.sent();

    harness.press(Message::FormTestPressed);

    assert_eq!(test_command(&mut harness).1.as_deref(), Some("prod"));
}

#[test]
fn each_test_gets_a_new_id() {
    let mut harness = new_form();
    let first = start(&mut harness);
    harness.feed(Event::TestFinished { test_id: first, outcome: connected_report() });

    let second = start(&mut harness);

    assert_eq!(second, first + 1);
}

#[test]
fn testing_an_invalid_form_shows_the_message_and_sends_nothing() {
    let mut harness = with_profiles(Vec::new());
    harness.press(Message::ManagerAddPressed);
    harness.sent();

    harness.press(Message::FormTestPressed);

    assert!(harness.sent().is_empty());
    assert_eq!(form(&harness).error.as_deref(), Some("Name can't be empty"));
    assert!(form(&harness).test.is_none());
}

#[test]
fn pressing_test_again_while_running_sends_nothing() {
    let mut harness = new_form();
    start(&mut harness);

    harness.press(Message::FormTestPressed);

    assert!(harness.sent().is_empty());
}

#[test]
fn the_result_shows_in_the_form() {
    let mut harness = new_form();
    let id = start(&mut harness);

    harness.feed(Event::TestFinished { test_id: id, outcome: connected_report() });

    assert_eq!(form(&harness).test, Some(TestState::Finished(connected_report())));
}

#[test]
fn a_stale_result_is_ignored() {
    let mut harness = new_form();
    let id = start(&mut harness);

    harness.feed(Event::TestFinished { test_id: id + 5, outcome: connected_report() });

    assert_eq!(form(&harness).test, Some(TestState::Running));
}

#[test]
fn a_cancelled_outcome_clears_the_result() {
    let mut harness = new_form();
    let id = start(&mut harness);

    harness.feed(Event::TestFinished { test_id: id, outcome: TestOutcome::Cancelled });

    assert!(form(&harness).test.is_none());
}

#[test]
fn the_cancel_button_sends_cancel_test() {
    let mut harness = new_form();
    start(&mut harness);

    harness.press(Message::FormTestCancelled);

    assert_eq!(harness.sent(), vec![Command::CancelTest]);
    assert!(form(&harness).test.is_none());
}

#[test]
fn leaving_the_form_cancels_a_running_test() {
    let mut harness = new_form();
    start(&mut harness);

    harness.press(Message::ScreenCancelled);

    assert_eq!(harness.sent(), vec![Command::CancelTest]);
}

#[test]
fn closing_the_manager_cancels_a_running_test() {
    let mut harness = new_form();
    start(&mut harness);

    named(&mut harness, Named::Escape);
    named(&mut harness, Named::Escape);

    assert_eq!(harness.sent(), vec![Command::CancelTest]);
    assert!(!harness.app.manager_open());
}

#[test]
fn saving_cancels_a_running_test_first() {
    let mut harness = new_form();
    start(&mut harness);

    harness.press(Message::FormSavePressed);

    let sent = harness.sent();
    assert_eq!(sent[0], Command::CancelTest);
    assert!(matches!(sent[1], Command::SaveProfile { .. }));
}

#[test]
fn a_finished_test_does_not_send_cancel_on_leaving() {
    let mut harness = new_form();
    let id = start(&mut harness);
    harness.feed(Event::TestFinished { test_id: id, outcome: connected_report() });

    harness.press(Message::ScreenCancelled);

    assert!(harness.sent().is_empty());
}

#[test]
fn a_password_question_opens_a_prompt() {
    let mut harness = new_form();
    let id = start(&mut harness);

    harness.feed(Event::TestQuestion { test_id: id, request_id: 9, question: password_question() });

    let prompt = form(&harness).prompt.as_ref().unwrap();
    assert_eq!((prompt.request_id, prompt.username.as_str(), prompt.name.as_str()), (9, "me", "prod"));
}

#[test]
fn the_prompt_answer_is_sent_without_saving() {
    let mut harness = new_form();
    let id = start(&mut harness);
    harness.feed(Event::TestQuestion { test_id: id, request_id: 9, question: password_question() });
    harness.press(Message::TestPasswordTyped(Typed::new(MARKER.to_string())));

    harness.press(Message::TestPasswordSubmitted);

    assert_eq!(
        harness.sent(),
        vec![Command::Answer { request_id: 9, answer: Some(Answer::Password(MARKER.to_string())), save: false }]
    );
    assert!(form(&harness).prompt.is_none());
}

#[test]
fn cancelling_the_prompt_answers_none() {
    let mut harness = new_form();
    let id = start(&mut harness);
    harness.feed(Event::TestQuestion { test_id: id, request_id: 9, question: password_question() });

    harness.press(Message::TestPasswordCancelled);

    assert_eq!(harness.sent(), vec![Command::Answer { request_id: 9, answer: None, save: false }]);
}

#[test]
fn escape_cancels_the_prompt_and_keeps_the_form() {
    let mut harness = new_form();
    let id = start(&mut harness);
    harness.feed(Event::TestQuestion { test_id: id, request_id: 9, question: password_question() });

    named(&mut harness, Named::Escape);

    assert_eq!(harness.sent(), vec![Command::Answer { request_id: 9, answer: None, save: false }]);
    assert!(matches!(harness.app.screen(), Some(Screen::Form(_))));
}

#[test]
fn a_trust_question_is_answered_none_without_a_prompt() {
    let mut harness = new_form();
    let id = start(&mut harness);
    let question = Question::TrustHostKey {
        name: "prod".to_string(),
        host: "h".to_string(),
        port: 22,
        key_type: "ssh-ed25519".to_string(),
        fingerprint: "SHA256:x".to_string(),
    };

    harness.feed(Event::TestQuestion { test_id: id, request_id: 4, question });

    assert_eq!(harness.sent(), vec![Command::Answer { request_id: 4, answer: None, save: false }]);
    assert!(form(&harness).prompt.is_none());
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_question_from_a_stale_test_is_not_shown() {
    let mut harness = new_form();
    let id = start(&mut harness);

    harness.feed(Event::TestQuestion { test_id: id + 3, request_id: 9, question: password_question() });

    assert!(form(&harness).prompt.is_none());
}

#[test]
fn the_not_trusted_outcome_is_kept_for_the_form() {
    let mut harness = new_form();
    let id = start(&mut harness);
    let outcome =
        TestOutcome::NotTrusted(NotTrusted::HostKey { key_type: "k".to_string(), fingerprint: "f".to_string() });

    harness.feed(Event::TestFinished { test_id: id, outcome: outcome.clone() });

    assert_eq!(form(&harness).test, Some(TestState::Finished(outcome)));
}

#[test]
fn a_timed_out_test_can_be_run_again() {
    let mut harness = new_form();
    let id = start(&mut harness);
    harness.feed(Event::TestFinished { test_id: id, outcome: TestOutcome::TimedOut(TestStage::Connect) });

    harness.press(Message::FormTestPressed);

    assert!(matches!(harness.sent()[0], Command::TestConnection { .. }));
}

#[test]
fn the_password_messages_never_print_the_typed_text() {
    let message = Message::TestPasswordTyped(Typed::new(MARKER.to_string()));

    assert!(!format!("{message:?}").contains(MARKER));
}

#[test]
fn a_test_draft_keeps_the_saved_password_untouched() {
    let mut entry = saved("prod");
    entry.saved_password = true;
    entry.in_keyring = vec!["password".to_string()];
    let mut harness = with_profiles(vec![entry]);
    select(&mut harness, 0);
    harness.press(Message::ManagerEditPressed);
    type_field(&mut harness, "host", "other.example.com");
    harness.sent();

    harness.press(Message::FormTestPressed);

    assert_eq!(test_command(&mut harness).2.password, SecretEdit::Keep);
}

#[test]
fn closing_without_a_form_open_sends_nothing() {
    let mut harness = harness();
    open_manager(&mut harness);

    harness.press(Message::ManagerClosed);

    assert!(harness.sent().is_empty());
}
