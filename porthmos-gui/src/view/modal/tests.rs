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

fn with_reply(result: Option<Result<Arc<DiffOutcome>, porthmos_core::Message>>) -> Harness {
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
        with_reply(Some(Err("Comparison failed: a.txt".into()))),
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
    assert_eq!(question_width(&DiffPane::Failed("x".into())), DIALOG_WIDTH);
}

#[test]
fn show_differences_is_a_real_button_message() {
    let mut harness = conflict_harness();

    harness.press(Message::EditShowDiff);

    assert!(matches!(harness.app.modal(), Some(Modal::EditQuestion { diff: DiffPane::Comparing, .. })));
}

fn properties_harness(entries: &[(&str, bool)]) -> Harness {
    let mut harness = Harness::new();
    let folder = harness.start_folder();
    let entries = entries
        .iter()
        .map(|(name, is_dir)| porthmos_core::Entry {
            name: name.to_string(),
            path: folder.join(*name),
            is_dir: *is_dir,
            size: 10,
            ..porthmos_core::Entry::default()
        })
        .collect();
    harness.feed(Event::Listed { location: porthmos_core::Location::Local, path: folder, entries });
    harness.sent();
    harness
}

fn move_down_and_select(harness: &mut Harness) {
    use iced::keyboard::key::Named;
    for named in [Named::ArrowDown, Named::Space] {
        let key = iced::keyboard::Key::Named(named);
        harness.press(Message::KeyPressed(crate::shortcuts::KeyPress {
            key: key.clone(),
            modified_key: key,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
        }));
    }
}

#[test]
fn the_properties_overlay_builds_for_every_state_and_for_extreme_sizes() {
    let mut harness = properties_harness(&[("a.iso", false), ("b", true), ("bad\u{1b}[31m\nname", false)]);
    for _ in 0..3 {
        move_down_and_select(&mut harness);
    }
    harness.press(Message::ActionRequested(crate::shortcuts::Action::Properties));
    assert!(overlay(&harness.app).is_some());

    harness.press(Message::PropertiesCompute);
    harness.feed(Event::Measured {
        request_id: 1,
        result: Ok(porthmos_core::properties::Measured { bytes: u64::MAX, files: u64::MAX, ..Default::default() }),
    });
    harness.feed(Event::Checksummed {
        request_id: 1,
        path: harness.start_folder().join("a.iso"),
        result: Ok(porthmos_core::properties::Digest { hex: "ab".repeat(32), bytes: 3 }),
    });
    assert!(overlay(&harness.app).is_some());

    harness.press(Message::ModalDeclined);
    assert!(overlay(&harness.app).is_some());
}

#[test]
fn the_properties_overlay_builds_in_a_tiny_window() {
    let mut harness = properties_harness(&[("a.iso", false)]);
    move_down_and_select(&mut harness);
    harness.press(Message::ActionRequested(crate::shortcuts::Action::Properties));
    harness.resize(iced::Size::new(200.0, 120.0));

    assert!(overlay(&harness.app).is_some());
}
