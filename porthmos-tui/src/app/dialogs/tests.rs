use std::path::Path;

use crossterm::event::{KeyCode, KeyEventState, KeyModifiers};

use super::*;
use crate::app::{
    Screen,
    testing::{TYPED, TYPED_WITHOUT_ANSWERS, TestApp, entry, test_app},
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn app_with_file(name: &str) -> TestApp {
    let mut test = test_app(Path::new("/d"));
    test.list_local(vec![entry(Path::new("/d"), name, false)]);
    test.app.local.cursor = test.app.local.rows().len() - 1;
    test
}

#[test]
fn mkdir_dialog_creates_a_directory_on_submit() {
    let mut test = test_app(Path::new("/d"));
    test.app.apply_action(Action::Mkdir);
    assert!(test.app.dialog.is_some());

    for c in "new_dir".chars() {
        test.app.apply_dialog_key(key(KeyCode::Char(c)));
    }
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert!(test.app.dialog.is_none());
    assert_eq!(test.sent(), vec![Command::CreateDir { location: Location::Local, path: PathBuf::from("/d/new_dir") }]);
}

#[test]
fn mkdir_on_the_remote_panel_goes_to_that_session() {
    let mut test = test_app(Path::new("/d"));
    let session = test.connect(2, "srv");
    test.list_remote(session, "/srv", Vec::new());
    test.app.active_panel = ActivePanel::Remote;
    test.app.apply_action(Action::Mkdir);

    test.app.apply_dialog_key(key(KeyCode::Char('x')));
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::CreateDir { location: Location::Session(session), path: PathBuf::from("/srv/x") }]
    );
}

#[test]
fn mkdir_dialog_cancelled_with_esc_creates_nothing() {
    let mut test = test_app(Path::new("/d"));
    test.app.apply_action(Action::Mkdir);
    test.app.apply_dialog_key(key(KeyCode::Char('x')));
    test.app.apply_dialog_key(key(KeyCode::Esc));

    assert!(test.app.dialog.is_none());
    assert!(test.sent().is_empty());
}

#[test]
fn delete_dialog_confirmed_with_y_deletes_the_target() {
    let mut test = app_with_file("doomed.txt");

    test.app.apply_action(Action::Delete);
    assert!(test.app.dialog.is_some());
    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert!(test.app.dialog.is_none());
    assert_eq!(
        test.sent(),
        vec![Command::Delete { location: Location::Local, paths: vec![PathBuf::from("/d/doomed.txt")] }]
    );
}

#[test]
fn delete_dialog_cancelled_with_n_deletes_nothing() {
    let mut test = app_with_file("safe.txt");

    test.app.apply_action(Action::Delete);
    test.app.apply_dialog_key(key(KeyCode::Char('n')));

    assert!(test.app.dialog.is_none());
    assert!(test.sent().is_empty());
}

#[test]
fn rename_dialog_prefills_the_current_name() {
    let mut test = app_with_file("old.txt");

    test.app.apply_action(Action::Rename);

    match test.app.dialog {
        Some(Dialog::TextInput(ref dialog)) => assert_eq!(dialog.value, "old.txt"),
        _ => panic!("expected a text input dialog"),
    }
}

#[test]
fn rename_dialog_submits_the_rename() {
    let mut test = app_with_file("old.txt");
    test.app.apply_action(Action::Rename);
    if let Some(Dialog::TextInput(dialog)) = test.app.dialog.as_mut() {
        dialog.value = "new.txt".to_string();
    }

    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::Rename {
            location: Location::Local,
            from: PathBuf::from("/d/old.txt"),
            to: PathBuf::from("/d/new.txt"),
        }]
    );
}

#[test]
fn failed_operation_sets_a_status_message() {
    let mut test = test_app(Path::new("/d"));

    test.app.apply_core_event(Event::Notice {
        severity: Severity::Error,
        message: "File exists (os error 17)".to_string(),
    });

    assert!(test.app.notifications.current().is_some());
}

#[test]
fn typing_in_a_text_input_dialog_only_types() {
    let mut test = app_with_file("a.txt");
    test.app.apply_action(Action::Mkdir);
    test.sent();

    test.type_text(TYPED);

    match &test.app.dialog {
        Some(Dialog::TextInput(dialog)) => assert_eq!(dialog.value, TYPED),
        _ => panic!("the dialog closed"),
    }
    test.assert_inert(Screen::Files);
}

#[test]
fn a_confirm_dialog_ignores_keys_it_does_not_own() {
    let mut test = app_with_file("a.txt");
    test.app.apply_action(Action::Delete);
    test.sent();

    test.type_text(TYPED_WITHOUT_ANSWERS);

    assert!(matches!(test.app.dialog, Some(Dialog::Confirm(_))));
    test.assert_inert(Screen::Files);
}

#[test]
fn a_refresh_while_the_delete_dialog_is_open_does_not_change_what_is_deleted() {
    let mut test = test_app(Path::new("/d"));
    test.list_local(vec![entry(Path::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_action(Action::Delete);

    test.list_local(vec![entry(Path::new("/d"), "aaa.bin", false), entry(Path::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert_eq!(
        test.sent(),
        vec![Command::Delete { location: Location::Local, paths: vec![PathBuf::from("/d/big.txt")] }]
    );
}

#[test]
fn a_delete_confirmed_after_switching_panels_still_targets_the_original_panel() {
    let mut test = app_with_file("doomed.txt");
    let session = test.connect(2, "srv");
    test.list_remote(session, "/srv", vec![entry(Path::new("/srv"), "keep.txt", false)]);
    test.app.apply_action(Action::Delete);

    test.app.active_panel = ActivePanel::Remote;
    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert_eq!(
        test.sent(),
        vec![Command::Delete { location: Location::Local, paths: vec![PathBuf::from("/d/doomed.txt")] }]
    );
}

#[test]
fn a_refresh_while_the_rename_dialog_is_open_does_not_change_what_is_renamed() {
    let mut test = test_app(Path::new("/d"));
    test.list_local(vec![entry(Path::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_action(Action::Rename);
    if let Some(Dialog::TextInput(dialog)) = test.app.dialog.as_mut() {
        dialog.value = "renamed.txt".to_string();
    }

    test.list_local(vec![entry(Path::new("/d"), "aaa.bin", false), entry(Path::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::Rename {
            location: Location::Local,
            from: PathBuf::from("/d/big.txt"),
            to: PathBuf::from("/d/renamed.txt"),
        }]
    );
}

fn submit_name(test: &mut TestApp, action: Action, name: &str) {
    test.app.apply_action(action);
    if let Some(Dialog::TextInput(dialog)) = test.app.dialog.as_mut() {
        dialog.value = name.to_string();
    }
    test.app.apply_dialog_key(key(KeyCode::Enter));
}

#[test]
fn a_rename_to_a_path_or_an_empty_name_is_refused() {
    for name in ["/tmp/escaped.txt", "../x", "a/b", ".", "..", ""] {
        let mut test = app_with_file("old.txt");

        submit_name(&mut test, Action::Rename, name);

        assert!(test.sent().is_empty(), "{name:?} was sent");
        assert!(test.notification().is_some(), "{name:?} gave no warning");
    }
}

#[test]
fn a_new_folder_named_like_a_path_or_empty_is_refused() {
    for name in ["/tmp/escaped", "../x", "a/b", ".", "..", ""] {
        let mut test = app_with_file("old.txt");

        submit_name(&mut test, Action::Mkdir, name);

        assert!(test.sent().is_empty(), "{name:?} was sent");
        assert!(test.notification().is_some(), "{name:?} gave no warning");
    }
}

#[test]
fn names_with_spaces_dots_and_unicode_are_accepted() {
    for name in ["my file (1).txt", ".hidden", "..x", "ünï.txt"] {
        let mut test = app_with_file("old.txt");

        submit_name(&mut test, Action::Mkdir, name);

        assert_eq!(
            test.sent(),
            vec![Command::CreateDir { location: Location::Local, path: PathBuf::from("/d").join(name) }]
        );
    }
}
