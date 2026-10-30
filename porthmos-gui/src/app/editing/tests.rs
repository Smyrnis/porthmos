use std::ffi::OsString;
use std::path::PathBuf;

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::edit::{EditChoice, EditQuestionKind, EditorCommand, EditorExit};
use porthmos_core::{Command, Entry, Event, Location, Severity, VfsPath};

use crate::menus::{MenuItem, MoreItem};
use crate::modal::Modal;
use crate::shortcuts::KeyPress;
use crate::testing::Harness;

use super::super::*;

fn press(harness: &mut Harness, key: Key) {
    let text = match &key {
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    };
    harness.press(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::empty(),
        text,
    }));
}

fn named(harness: &mut Harness, key: Named) {
    press(harness, Key::Named(key));
}

fn file(folder: &VfsPath, name: &str) -> Entry {
    Entry { name: name.to_string(), path: folder.join(name), size: 10, ..Entry::default() }
}

fn local_harness() -> Harness {
    let mut harness = Harness::new();
    let folder = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: folder.clone(),
        entries: vec![Entry { is_dir: true, ..file(&folder, "docs") }, file(&folder, "a.txt")],
    });
    harness.sent();
    harness
}

fn on_row(harness: &mut Harness, row: usize) {
    harness.app.local.panel.set_cursor(row);
}

fn editor(program: &str, args: &[&str]) -> EditorCommand {
    EditorCommand { program: program.to_string(), args: args.iter().map(|arg| arg.to_string()).collect() }
}

fn ready(harness: &mut Harness, edit_id: u64, editor: EditorCommand) {
    harness.feed(Event::EditReady { edit_id, file: PathBuf::from(format!("/tmp/porthmos/{edit_id}/a.txt")), editor });
}

fn strings(values: &[OsString]) -> Vec<String> {
    values.iter().map(|value| value.to_string_lossy().to_string()).collect()
}

fn window_editor(harness: &mut Harness, edit_id: u64) -> u64 {
    ready(harness, edit_id, editor("code", &["--wait"]));
    harness.launched().last().expect("a job").token
}

#[test]
fn e_and_f3_send_edit_file_for_the_cursor_file() {
    let mut harness = local_harness();
    on_row(&mut harness, 2);

    press(&mut harness, Key::Character("e".into()));
    named(&mut harness, Named::F3);

    let path = harness.start_folder().join("a.txt");
    let command = Command::EditFile { location: Location::Local, path };
    assert_eq!(harness.sent(), vec![command.clone(), command]);
}

#[test]
fn a_folder_or_parent_row_is_refused_with_a_notice() {
    let mut harness = local_harness();

    for row in [0, 1] {
        on_row(&mut harness, row);
        press(&mut harness, Key::Character("e".into()));
    }

    assert!(harness.sent().is_empty());
    let notices = harness.app.notices();
    assert_eq!(notices.len(), 2);
    assert!(
        notices.iter().all(|notice| notice.severity == Severity::Warning && notice.message == "Can't edit a folder")
    );
}

#[test]
fn several_selected_files_are_refused_with_a_notice() {
    let mut harness = local_harness();
    harness.press(Message::ActionRequested(Action::SelectAll));
    harness.sent();

    press(&mut harness, Key::Character("e".into()));

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.notices().last().unwrap().message, "Select one file to edit");
}

#[test]
fn edit_on_the_remote_panel_uses_its_session() {
    let mut harness = local_harness();
    harness.feed(Event::Connected { session: 5, name: "alpha".to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(5),
        path: VfsPath::new("/srv"),
        entries: vec![file(&VfsPath::new("/srv"), "r.log")],
    });
    named(&mut harness, Named::Tab);
    harness.sent();
    harness.app.side_mut(PanelSide::Remote).unwrap().panel.set_cursor(1);

    press(&mut harness, Key::Character("e".into()));

    assert_eq!(
        harness.sent(),
        vec![Command::EditFile { location: Location::Session(5), path: VfsPath::new("/srv/r.log") }]
    );
}

#[test]
fn a_configured_window_editor_is_spawned_with_the_file_last() {
    let mut harness = local_harness();

    ready(&mut harness, 1, editor("code", &["--wait", "--new-window"]));

    let jobs = harness.launched();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].spec.program, OsString::from("code"));
    assert_eq!(strings(&jobs[0].spec.args), ["--wait", "--new-window", "/tmp/porthmos/1/a.txt"]);
    assert!(harness.sent().is_empty());
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_terminal_editor_is_spawned_through_the_terminal_launcher() {
    let mut harness = local_harness();
    harness.install_terminals(&["alacritty"]);

    ready(&mut harness, 1, editor("nano", &[]));

    let jobs = harness.launched();
    assert_eq!(jobs[0].spec.program, OsString::from("alacritty"));
    assert_eq!(strings(&jobs[0].spec.args), ["-e", "nano", "/tmp/porthmos/1/a.txt"]);
}

#[test]
fn a_bare_vi_opens_the_default_app_and_shows_the_waiting_dialog() {
    let mut harness = local_harness();

    ready(&mut harness, 3, editor("vi", &[]));

    let jobs = harness.launched();
    assert_eq!(jobs[0].spec.program, OsString::from("xdg-open"));
    assert_eq!(strings(&jobs[0].spec.args), ["/tmp/porthmos/3/a.txt"]);
    assert!(matches!(harness.app.modal(), Some(Modal::EditWaiting { edit_id: 3, name }) if name == "a.txt"));
    assert!(harness.sent().is_empty());
}

#[test]
fn done_sends_finish_edit_success() {
    let mut harness = local_harness();
    ready(&mut harness, 3, editor("vi", &[]));
    harness.launched();

    harness.press(Message::EditDoneEditing);

    assert_eq!(harness.sent(), vec![Command::FinishEdit { edit_id: 3, exit: EditorExit::Success }]);
    assert!(harness.app.modal().is_none());
}

#[test]
fn enter_is_done_but_escape_leaves_the_waiting_dialog_open() {
    let mut harness = local_harness();
    ready(&mut harness, 3, editor("vi", &[]));
    harness.launched();

    named(&mut harness, Named::Escape);
    assert!(matches!(harness.app.modal(), Some(Modal::EditWaiting { .. })));
    assert!(harness.sent().is_empty());
    named(&mut harness, Named::Enter);

    assert_eq!(harness.sent(), vec![Command::FinishEdit { edit_id: 3, exit: EditorExit::Success }]);
}

#[test]
fn the_default_app_exiting_cleanly_does_not_end_the_edit() {
    let mut harness = local_harness();
    ready(&mut harness, 3, editor("vi", &[]));
    let token = harness.launched()[0].token;

    harness.finish_process(token, EditorExit::Success);

    assert!(harness.sent().is_empty());
    assert!(matches!(harness.app.modal(), Some(Modal::EditWaiting { .. })));
}

#[test]
fn the_default_app_failing_ends_the_edit_and_closes_the_dialog() {
    let mut harness = local_harness();
    ready(&mut harness, 3, editor("vi", &[]));
    let token = harness.launched()[0].token;

    harness.finish_process(token, EditorExit::LaunchFailed("no xdg-open".to_string()));

    assert_eq!(
        harness.sent(),
        vec![Command::FinishEdit { edit_id: 3, exit: EditorExit::LaunchFailed("no xdg-open".to_string()) }]
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn the_editor_exiting_sends_finish_edit_with_its_exit() {
    let mut harness = local_harness();
    let token = window_editor(&mut harness, 1);

    harness.finish_process(token, EditorExit::Success);

    assert_eq!(harness.sent(), vec![Command::FinishEdit { edit_id: 1, exit: EditorExit::Success }]);
}

#[test]
fn a_nonzero_exit_is_sent_as_status() {
    let mut harness = local_harness();
    let token = window_editor(&mut harness, 1);

    harness.finish_process(token, EditorExit::Status(2));

    assert_eq!(harness.sent(), vec![Command::FinishEdit { edit_id: 1, exit: EditorExit::Status(2) }]);
}

#[test]
fn a_launch_failure_sends_launch_failed_and_nothing_else() {
    let mut harness = local_harness();
    let token = window_editor(&mut harness, 1);

    harness.finish_process(token, EditorExit::LaunchFailed("not found".to_string()));

    assert_eq!(
        harness.sent(),
        vec![Command::FinishEdit { edit_id: 1, exit: EditorExit::LaunchFailed("not found".to_string()) }]
    );
    harness.finish_process(token, EditorExit::Success);
    assert!(harness.sent().is_empty());
}

#[test]
fn the_terminal_error_text_goes_to_finish_edit() {
    let mut harness = local_harness();
    harness.install_terminals(&[]);

    ready(&mut harness, 4, editor("nano", &[]));

    assert!(harness.launched().is_empty());
    let sent = harness.sent();
    assert!(
        matches!(
            sent.as_slice(),
            [Command::FinishEdit { edit_id: 4, exit: EditorExit::LaunchFailed(message) }]
                if message.starts_with("No terminal program found")
        ),
        "{sent:?}"
    );
}

#[test]
fn an_unknown_process_exit_is_ignored() {
    let mut harness = local_harness();

    harness.finish_process(99, EditorExit::Success);

    assert!(harness.sent().is_empty());
}

#[test]
fn two_edits_keep_separate_ids() {
    let mut harness = local_harness();
    let first = window_editor(&mut harness, 1);
    let second = window_editor(&mut harness, 2);

    assert_ne!(first, second);
    harness.finish_process(second, EditorExit::Success);
    harness.finish_process(first, EditorExit::Status(1));

    assert_eq!(
        harness.sent(),
        vec![
            Command::FinishEdit { edit_id: 2, exit: EditorExit::Success },
            Command::FinishEdit { edit_id: 1, exit: EditorExit::Status(1) },
        ]
    );
}

#[test]
fn an_upload_question_has_upload_and_discard() {
    let mut harness = local_harness();
    harness.feed(Event::EditQuestion { edit_id: 1, name: "a.txt".to_string(), kind: EditQuestionKind::Upload });
    assert!(matches!(
        harness.app.modal(),
        Some(Modal::EditQuestion { edit_id: 1, kind: EditQuestionKind::Upload, .. })
    ));

    named(&mut harness, Named::Enter);
    assert_eq!(harness.sent(), vec![Command::ResolveEdit { edit_id: 1, choice: EditChoice::Upload }]);

    harness.feed(Event::EditQuestion { edit_id: 2, name: "b.txt".to_string(), kind: EditQuestionKind::Upload });
    named(&mut harness, Named::Escape);
    assert_eq!(harness.sent(), vec![Command::ResolveEdit { edit_id: 2, choice: EditChoice::Cancel }]);
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_conflict_question_defaults_to_keep_a_copy() {
    let mut harness = local_harness();
    harness.feed(Event::EditQuestion { edit_id: 1, name: "a.txt".to_string(), kind: EditQuestionKind::Conflict });
    named(&mut harness, Named::Enter);
    assert_eq!(harness.sent(), vec![Command::ResolveEdit { edit_id: 1, choice: EditChoice::KeepCopy }]);

    harness.feed(Event::EditQuestion { edit_id: 2, name: "a.txt".to_string(), kind: EditQuestionKind::Conflict });
    named(&mut harness, Named::Escape);
    assert_eq!(harness.sent(), vec![Command::ResolveEdit { edit_id: 2, choice: EditChoice::Cancel }]);
}

#[test]
fn each_conflict_button_sends_its_choice() {
    for choice in [EditChoice::Upload, EditChoice::KeepCopy, EditChoice::Cancel] {
        let mut harness = local_harness();
        harness.feed(Event::EditQuestion { edit_id: 6, name: "a.txt".to_string(), kind: EditQuestionKind::Conflict });

        harness.press(Message::EditAnswered(choice));

        assert_eq!(harness.sent(), vec![Command::ResolveEdit { edit_id: 6, choice }]);
        assert!(harness.app.modal().is_none());
    }
}

#[test]
fn an_answer_without_an_edit_question_sends_nothing() {
    let mut harness = local_harness();

    harness.press(Message::EditAnswered(EditChoice::Upload));

    assert!(harness.sent().is_empty());
}

#[test]
fn edits_busy_is_stored() {
    let mut harness = local_harness();
    assert!(!harness.app.edits_busy());

    harness.feed(Event::EditsBusy(true));
    assert!(harness.app.edits_busy());
    harness.feed(Event::EditsBusy(false));

    assert!(!harness.app.edits_busy());
}

#[test]
fn edit_questions_wait_behind_an_open_modal() {
    let mut harness = local_harness();
    harness.press(Message::ActionRequested(Action::NewFolder));
    assert!(matches!(harness.app.modal(), Some(Modal::NewFolder { .. })));

    harness.feed(Event::EditQuestion { edit_id: 1, name: "a.txt".to_string(), kind: EditQuestionKind::Upload });
    assert!(matches!(harness.app.modal(), Some(Modal::NewFolder { .. })));
    named(&mut harness, Named::Escape);

    assert!(matches!(harness.app.modal(), Some(Modal::EditQuestion { edit_id: 1, .. })));
}

#[test]
fn the_same_question_is_not_queued_twice() {
    let mut harness = local_harness();
    for _ in 0..2 {
        harness.feed(Event::EditQuestion { edit_id: 1, name: "a.txt".to_string(), kind: EditQuestionKind::Upload });
    }

    named(&mut harness, Named::Enter);

    assert!(harness.app.modal().is_none());
    assert_eq!(harness.sent().len(), 1);
}

#[test]
fn the_context_menu_and_more_items_open_the_edit() {
    let mut harness = local_harness();
    on_row(&mut harness, 2);
    let command = Command::EditFile { location: Location::Local, path: harness.start_folder().join("a.txt") };

    harness.press(Message::RowRightPressed(PanelSide::Local, 2));
    harness.press(Message::MenuItemChosen(MenuItem::Edit));
    assert_eq!(harness.sent(), vec![command.clone()]);

    harness.press(Message::MoreItemChosen(MoreItem::Edit));
    assert_eq!(harness.sent(), vec![command]);
}
