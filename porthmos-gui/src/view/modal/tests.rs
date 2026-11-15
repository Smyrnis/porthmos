use std::sync::Arc;

use porthmos_core::Event;
use porthmos_core::diff::{DiffKind, DiffLine, DiffOutcome};
use porthmos_core::edit::EditQuestionKind;

use super::{DIALOG_WIDTH, DIFF_WIDTH, overlay, question_width};
use crate::app::Message;
use crate::modal::{DiffPane, Modal};
use crate::testing::Harness;

fn conflict_harness() -> Harness {
    let mut harness = Harness::new();
    harness.feed(Event::EditQuestion { edit_id: 1, name: "a.txt".to_string(), kind: EditQuestionKind::Conflict });
    harness
}

fn changes() -> Arc<DiffOutcome> {
    Arc::new(DiffOutcome::Changes(vec![
        DiffLine { kind: DiffKind::Removed, text: "theirs".to_string() },
        DiffLine { kind: DiffKind::Added, text: "mine".to_string() },
    ]))
}

fn with_reply(result: Option<Result<Arc<DiffOutcome>, String>>) -> Harness {
    let mut harness = conflict_harness();
    if let Some(result) = result {
        harness.press(Message::EditShowDiff);
        harness.feed(Event::EditDiff { edit_id: 1, result });
    }
    harness
}

#[test]
fn the_conflict_question_renders_in_every_diff_state() {
    let mut comparing = conflict_harness();
    comparing.press(Message::EditShowDiff);
    let harnesses = [
        conflict_harness(),
        comparing,
        with_reply(Some(Ok(changes()))),
        with_reply(Some(Ok(Arc::new(DiffOutcome::RemoteGone)))),
        with_reply(Some(Err("Comparison failed: a.txt".to_string()))),
    ];

    for harness in harnesses {
        assert!(overlay(&harness.app).is_some());
    }
}

#[test]
fn the_dialog_is_wider_only_while_a_diff_is_shown() {
    assert_eq!(question_width(&DiffPane::Idle), DIALOG_WIDTH);
    assert_eq!(question_width(&DiffPane::Comparing), DIALOG_WIDTH);
    assert_eq!(question_width(&DiffPane::Ready(changes())), DIFF_WIDTH);
    assert_eq!(question_width(&DiffPane::Failed("x".to_string())), DIALOG_WIDTH);
}

#[test]
fn show_differences_is_a_real_button_message() {
    let mut harness = conflict_harness();

    harness.press(Message::EditShowDiff);

    assert!(matches!(harness.app.modal(), Some(Modal::EditQuestion { diff: DiffPane::Comparing, .. })));
}
