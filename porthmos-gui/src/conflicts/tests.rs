use porthmos_core::Command;
use porthmos_core::transfer::conflicts::{ConflictInfo, Resolution};
use porthmos_core::transfer::plan::ExistingFile;

use super::ConflictQueue;

fn existing(size: u64) -> Option<ExistingFile> {
    Some(ExistingFile { size, modified: Some(1_000), is_dir: false })
}

fn conflict(name: &str) -> ConflictInfo {
    ConflictInfo {
        display_name: name.to_string(),
        existing: existing(5),
        partial: None,
        new_size: 9,
        new_modified: Some(2_000),
    }
}

fn partial_only(name: &str) -> ConflictInfo {
    ConflictInfo {
        display_name: name.to_string(),
        existing: None,
        partial: existing(3),
        new_size: 9,
        new_modified: Some(2_000),
    }
}

fn names(queue: &ConflictQueue) -> Option<String> {
    queue.current().map(|view| view.file_name)
}

fn resolve(batch_id: u64, answers: &[Resolution]) -> Command {
    Command::ResolveConflicts { batch_id, answers: Some(answers.to_vec()) }
}

#[test]
fn an_empty_queue_has_nothing_to_show() {
    let mut queue = ConflictQueue::default();

    assert!(queue.current().is_none());
    assert!(queue.is_empty());
    assert_eq!(queue.answer(Some(Resolution::Skip), false), None);
}

#[test]
fn a_batch_asks_about_each_file_in_order() {
    let mut queue = ConflictQueue::default();
    queue.found(1, vec![conflict("a"), conflict("b"), conflict("c")]);

    let first = queue.current().unwrap();
    assert_eq!((first.file_name.as_str(), first.index, first.total), ("a", 0, 3));
    queue.answer(Some(Resolution::Skip), false);
    assert_eq!(names(&queue).as_deref(), Some("b"));
    assert_eq!(queue.current().unwrap().index, 1);
    queue.answer(Some(Resolution::Skip), false);
    assert_eq!(names(&queue).as_deref(), Some("c"));
}

#[test]
fn answering_every_file_sends_resolve_conflicts_once_with_all_answers() {
    let mut queue = ConflictQueue::default();
    queue.found(7, vec![conflict("a"), conflict("b")]);

    assert_eq!(queue.answer(Some(Resolution::Overwrite), false), None);
    let sent = queue.answer(Some(Resolution::Rename), false);

    assert_eq!(sent, Some(resolve(7, &[Resolution::Overwrite, Resolution::Rename])));
    assert!(queue.is_empty());
}

#[test]
fn apply_to_the_rest_fills_only_the_files_that_fit() {
    let mut queue = ConflictQueue::default();
    queue.found(1, vec![conflict("a"), conflict("b"), partial_only("c"), conflict("d")]);

    assert_eq!(queue.answer(Some(Resolution::Overwrite), true), None);

    let view = queue.current().unwrap();
    assert_eq!((view.file_name.as_str(), view.index), ("c", 2));
    let sent = queue.answer(Some(Resolution::Resume), false);
    assert_eq!(
        sent,
        Some(resolve(1, &[Resolution::Overwrite, Resolution::Overwrite, Resolution::Resume, Resolution::Overwrite]))
    );
}

#[test]
fn apply_to_the_rest_with_skip_answers_everything() {
    let mut queue = ConflictQueue::default();
    queue.found(1, vec![conflict("a"), partial_only("b"), conflict("c")]);

    let sent = queue.answer(Some(Resolution::Skip), true);

    assert_eq!(sent, Some(resolve(1, &[Resolution::Skip, Resolution::Skip, Resolution::Skip])));
}

#[test]
fn cancelling_sends_none_for_the_batch() {
    let mut queue = ConflictQueue::default();
    queue.found(4, vec![conflict("a"), conflict("b")]);
    queue.answer(Some(Resolution::Skip), false);

    let sent = queue.answer(None, true);

    assert_eq!(sent, Some(Command::ResolveConflicts { batch_id: 4, answers: None }));
    assert!(queue.is_empty());
}

#[test]
fn two_batches_are_asked_one_after_the_other() {
    let mut queue = ConflictQueue::default();
    queue.found(1, vec![conflict("a")]);
    queue.found(2, vec![conflict("x"), conflict("y")]);

    assert_eq!(queue.answer(Some(Resolution::Skip), false), Some(resolve(1, &[Resolution::Skip])));
    assert_eq!(names(&queue).as_deref(), Some("x"));
    queue.answer(Some(Resolution::Skip), false);
    assert_eq!(
        queue.answer(Some(Resolution::Overwrite), false),
        Some(resolve(2, &[Resolution::Skip, Resolution::Overwrite]))
    );
    assert!(queue.current().is_none());
}

#[test]
fn a_withdrawn_batch_is_dropped_and_reports_that_it_was_showing() {
    let mut queue = ConflictQueue::default();
    queue.found(1, vec![conflict("a")]);
    queue.found(2, vec![conflict("x")]);

    assert!(queue.withdraw(&[1]));

    assert_eq!(names(&queue).as_deref(), Some("x"));
}

#[test]
fn a_withdrawn_batch_that_is_not_showing_leaves_the_current_dialog_alone() {
    let mut queue = ConflictQueue::default();
    queue.found(1, vec![conflict("a"), conflict("b")]);
    queue.answer(Some(Resolution::Skip), false);
    queue.found(2, vec![conflict("x")]);

    assert!(!queue.withdraw(&[2]));

    assert_eq!(names(&queue).as_deref(), Some("b"));
    assert_eq!(queue.current().unwrap().index, 1);
    assert!(!queue.withdraw(&[99]));
}

#[test]
fn resume_is_offered_only_for_partial_files() {
    let mut queue = ConflictQueue::default();
    queue.found(1, vec![conflict("whole"), partial_only("half")]);

    assert!(!queue.current().unwrap().can_resume());
    assert_eq!(queue.answer(Some(Resolution::Resume), false), None);
    assert_eq!(queue.current().unwrap().index, 0);
    queue.answer(Some(Resolution::Skip), false);
    assert!(queue.current().unwrap().can_resume());
}

#[test]
fn the_view_says_whether_the_new_file_is_newer_or_older() {
    let mut queue = ConflictQueue::default();
    let newer = conflict("newer");
    let older = ConflictInfo { new_modified: Some(500), ..conflict("older") };
    let same = ConflictInfo { new_modified: Some(1_000), ..conflict("same") };
    let unknown = ConflictInfo { new_modified: None, ..conflict("unknown") };
    queue.found(1, vec![newer, older, same, unknown]);

    let mut words = Vec::new();
    for _ in 0..4 {
        words.push(queue.current().unwrap().freshness());
        queue.answer(Some(Resolution::Skip), false);
    }

    assert_eq!(words, [Some("newer"), Some("older"), None, None]);
}

#[test]
fn an_empty_batch_is_ignored() {
    let mut queue = ConflictQueue::default();

    queue.found(1, Vec::new());

    assert!(queue.is_empty());
}

#[test]
fn ten_thousand_conflicts_answer_with_apply_to_rest_in_one_command() {
    let mut queue = ConflictQueue::default();
    queue.found(9, (0..10_000).map(|n| conflict(&format!("f{n}"))).collect());

    let sent = queue.answer(Some(Resolution::Overwrite), true);

    match sent {
        Some(Command::ResolveConflicts { batch_id: 9, answers: Some(answers) }) => {
            assert_eq!(answers.len(), 10_000);
            assert!(answers.iter().all(|answer| *answer == Resolution::Overwrite));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn answering_ten_thousand_files_one_by_one_stays_linear() {
    let mut queue = ConflictQueue::default();
    queue.found(9, (0..10_000).map(|n| conflict(&format!("f{n}"))).collect());

    let mut sent = None;
    for _ in 0..10_000 {
        sent = queue.answer(Some(Resolution::Skip), false);
    }

    assert!(matches!(sent, Some(Command::ResolveConflicts { answers: Some(answers), .. }) if answers.len() == 10_000));
}
