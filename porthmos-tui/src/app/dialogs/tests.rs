use porthmos_core::VfsPath;

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
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), name, false)]);
    test.app.local.cursor = test.app.local.rows().len() - 1;
    test
}

#[test]
fn mkdir_dialog_creates_a_directory_on_submit() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::Mkdir);
    assert!(test.app.dialog.is_some());

    for c in "new_dir".chars() {
        test.app.apply_dialog_key(key(KeyCode::Char(c)));
    }
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert!(test.app.dialog.is_none());
    assert_eq!(test.sent(), vec![Command::CreateDir { location: Location::Local, path: VfsPath::from("/d/new_dir") }]);
}

#[test]
fn mkdir_on_the_remote_panel_goes_to_that_session() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(2, "srv");
    test.list_remote(session, "/srv", Vec::new());
    test.app.active_panel = ActivePanel::Remote;
    test.app.apply_action(Action::Mkdir);

    test.app.apply_dialog_key(key(KeyCode::Char('x')));
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::CreateDir { location: Location::Session(session), path: VfsPath::from("/srv/x") }]
    );
}

#[test]
fn mkdir_dialog_cancelled_with_esc_creates_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
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
        vec![Command::Delete { location: Location::Local, paths: vec![VfsPath::from("/d/doomed.txt")] }]
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
            from: VfsPath::from("/d/old.txt"),
            to: VfsPath::from("/d/new.txt"),
        }]
    );
}

#[test]
fn failed_operation_sets_a_status_message() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_core_event(Event::Notice { severity: Severity::Error, message: "File exists (os error 17)".into() });

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
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_action(Action::Delete);

    test.list_local(vec![entry(&VfsPath::new("/d"), "aaa.bin", false), entry(&VfsPath::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert_eq!(
        test.sent(),
        vec![Command::Delete { location: Location::Local, paths: vec![VfsPath::from("/d/big.txt")] }]
    );
}

#[test]
fn a_delete_confirmed_after_switching_panels_still_targets_the_original_panel() {
    let mut test = app_with_file("doomed.txt");
    let session = test.connect(2, "srv");
    test.list_remote(session, "/srv", vec![entry(&VfsPath::new("/srv"), "keep.txt", false)]);
    test.app.apply_action(Action::Delete);

    test.app.active_panel = ActivePanel::Remote;
    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert_eq!(
        test.sent(),
        vec![Command::Delete { location: Location::Local, paths: vec![VfsPath::from("/d/doomed.txt")] }]
    );
}

#[test]
fn a_refresh_while_the_rename_dialog_is_open_does_not_change_what_is_renamed() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_action(Action::Rename);
    if let Some(Dialog::TextInput(dialog)) = test.app.dialog.as_mut() {
        dialog.value = "renamed.txt".to_string();
    }

    test.list_local(vec![entry(&VfsPath::new("/d"), "aaa.bin", false), entry(&VfsPath::new("/d"), "big.txt", false)]);
    test.app.local.cursor = 1;
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert_eq!(
        test.sent(),
        vec![Command::Rename {
            location: Location::Local,
            from: VfsPath::from("/d/big.txt"),
            to: VfsPath::from("/d/renamed.txt"),
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

fn dialog_value(test: &TestApp) -> Option<&str> {
    match test.app.dialog.as_ref() {
        Some(Dialog::TextInput(dialog)) => Some(dialog.value.as_str()),
        _ => None,
    }
}

fn dialog_error(test: &TestApp) -> Option<&str> {
    match test.app.dialog.as_ref() {
        Some(Dialog::TextInput(dialog)) => dialog.error.as_deref(),
        _ => None,
    }
}

#[test]
fn a_rename_to_a_bad_name_keeps_the_dialog_open_with_what_was_typed() {
    for name in ["/tmp/escaped.txt", "../x", "a/b", ".", "..", "", " lead", "tab\there"] {
        let mut test = app_with_file("old.txt");

        submit_name(&mut test, Action::Rename, name);

        assert!(test.sent().is_empty(), "{name:?} was sent");
        assert_eq!(dialog_value(&test), Some(name), "{name:?} was lost");
        assert!(dialog_error(&test).is_some(), "{name:?} gave no error in the dialog");
        assert!(test.notification().is_none(), "{name:?} also raised a toast");
        assert!(test.app.pending_action.is_some(), "{name:?} dropped the pending action");
    }
}

#[test]
fn a_new_folder_with_a_bad_name_keeps_the_dialog_open_with_what_was_typed() {
    for name in ["/tmp/escaped", "../x", "a/b", ".", "..", "", "trail ", "CON"] {
        let mut test = app_with_file("old.txt");

        submit_name(&mut test, Action::Mkdir, name);

        assert!(test.sent().is_empty(), "{name:?} was sent");
        assert_eq!(dialog_value(&test), Some(name), "{name:?} was lost");
        assert!(dialog_error(&test).is_some(), "{name:?} gave no error in the dialog");
        assert!(test.notification().is_none(), "{name:?} also raised a toast");
    }
}

#[test]
fn a_new_folder_named_like_an_existing_entry_is_refused_inside_the_dialog() {
    let mut test = app_with_file("old.txt");

    submit_name(&mut test, Action::Mkdir, "old.txt");

    assert!(test.sent().is_empty());
    assert_eq!(dialog_error(&test), Some("\"old.txt\" already exists"));
}

#[test]
fn a_rename_onto_another_existing_entry_is_refused_inside_the_dialog() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "a.txt", false), entry(&VfsPath::new("/d"), "b.txt", false)]);
    test.app.local.cursor = 1;

    submit_name(&mut test, Action::Rename, "b.txt");

    assert!(test.sent().is_empty());
    assert_eq!(dialog_error(&test), Some("\"b.txt\" already exists"));
}

#[test]
fn correcting_the_name_and_submitting_again_goes_through_and_closes_the_dialog() {
    let mut test = app_with_file("old.txt");
    submit_name(&mut test, Action::Mkdir, "a/b");
    assert!(dialog_error(&test).is_some());

    test.app.apply_dialog_key(key(KeyCode::End));
    test.app.apply_dialog_key(key(KeyCode::Backspace));
    test.app.apply_dialog_key(key(KeyCode::Backspace));
    test.app.apply_dialog_key(key(KeyCode::Char('-')));
    test.app.apply_dialog_key(key(KeyCode::Char('b')));
    test.app.apply_dialog_key(key(KeyCode::Enter));

    assert_eq!(test.sent(), vec![Command::CreateDir { location: Location::Local, path: VfsPath::from("/d/a-b") }]);
    assert!(test.app.dialog.is_none());
    assert!(test.app.pending_action.is_none());
}

#[test]
fn cancelling_after_a_refused_name_closes_the_dialog_and_sends_nothing() {
    let mut test = app_with_file("old.txt");
    submit_name(&mut test, Action::Mkdir, "a/b");

    test.app.apply_dialog_key(key(KeyCode::Esc));

    assert!(test.app.dialog.is_none());
    assert!(test.app.pending_action.is_none());
    assert!(test.sent().is_empty());
}

#[test]
fn names_with_spaces_dots_and_unicode_are_accepted() {
    for name in ["my file (1).txt", ".hidden", "..x", "ünï.txt"] {
        let mut test = app_with_file("old.txt");

        submit_name(&mut test, Action::Mkdir, name);

        assert_eq!(
            test.sent(),
            vec![Command::CreateDir { location: Location::Local, path: VfsPath::from("/d").join(name) }]
        );
    }
}

fn confirm_message(test: &TestApp) -> Option<&str> {
    match test.app.dialog.as_ref() {
        Some(Dialog::Confirm(dialog)) => Some(dialog.message.as_str()),
        _ => None,
    }
}

#[test]
fn the_delete_prompt_says_the_file_goes_from_this_computer() {
    let mut test = app_with_file("big.bin");

    test.app.apply_action(Action::Delete);

    assert_eq!(confirm_message(&test), Some("Delete \"big.bin\" from this computer?"));
}

#[test]
fn the_delete_prompt_names_the_connection_when_the_remote_panel_is_active_and_the_name_exists_on_both_sides() {
    let mut test = app_with_file("big.bin");
    let session = test.connect(5, "prod");
    test.list_remote(session, "/srv", vec![entry(&VfsPath::new("/srv"), "big.bin", false)]);
    test.app.active_panel = ActivePanel::Remote;
    test.app.sessions.active_mut().unwrap().panel.cursor = 1;

    test.app.apply_action(Action::Delete);

    assert_eq!(confirm_message(&test), Some("Delete \"big.bin\" from prod?"));
}

#[test]
fn deleting_a_folder_says_everything_inside_it_goes_too() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "photos", true)]);
    test.app.local.cursor = 1;

    test.app.apply_action(Action::Delete);

    assert!(confirm_message(&test).is_some_and(|message| {
        message.starts_with("Delete the folder \"photos\" and everything inside it from this computer?\n")
    }));
}

#[test]
fn deleting_a_selection_lists_the_names_and_the_place() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "a", false), entry(&VfsPath::new("/d"), "docs", true)]);
    test.app.local.cursor = 1;
    test.app.apply_action(Action::ToggleSelect);
    test.app.local.cursor = 2;
    test.app.apply_action(Action::ToggleSelect);

    test.app.apply_action(Action::Delete);

    assert!(confirm_message(&test).is_some_and(|message| {
        message.starts_with(
            "Delete 2 items from this computer?\ndocs/\na\nFolders are deleted with everything inside them.\n",
        )
    }));
}

fn counting_request(test: &mut TestApp) -> (RequestId, Location, Vec<VfsPath>) {
    let requests: Vec<_> = test
        .sent()
        .into_iter()
        .filter_map(|command| match command {
            Command::CountContents { request_id, location, folders } => Some((request_id, location, folders)),
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 1, "{requests:?}");
    requests.into_iter().next().unwrap()
}

fn folder_prompt_app() -> TestApp {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "photos", true)]);
    test.app.local.cursor = 1;
    test
}

fn counted_files(files: u64, folders: u64) -> Contents {
    Contents::Counted(porthmos_core::delete_prompt::count::ContentCount {
        files,
        folders,
        unreadable: 0,
        capped: false,
    })
}

#[test]
fn deleting_a_folder_asks_the_core_to_count_it_and_says_it_is_counting() {
    let mut test = folder_prompt_app();

    test.app.apply_action(Action::Delete);

    let (_, location, folders) = counting_request(&mut test);
    assert_eq!((location, folders), (Location::Local, vec![VfsPath::from("/d/photos")]));
    assert_eq!(
        confirm_message(&test),
        Some(
            "Delete the folder \"photos\" and everything inside it from this computer?\nCounting what is inside\u{2026}"
        )
    );
}

#[test]
fn deleting_only_files_asks_for_no_count() {
    let mut test = app_with_file("big.bin");

    test.app.apply_action(Action::Delete);

    assert!(test.sent().is_empty());
    assert_eq!(confirm_message(&test), Some("Delete \"big.bin\" from this computer?"));
}

#[test]
fn the_answer_replaces_the_counting_line() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    let (request_id, ..) = counting_request(&mut test);

    test.app.apply_core_event(Event::ContentsCounted { request_id, contents: counted_files(1204, 37) });

    assert_eq!(
        confirm_message(&test),
        Some(
            "Delete the folder \"photos\" and everything inside it from this computer?\nInside: 1,204 files and 37 folders."
        )
    );
}

#[test]
fn an_unavailable_count_is_said_plainly() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    let (request_id, ..) = counting_request(&mut test);

    test.app.apply_core_event(Event::ContentsCounted { request_id, contents: Contents::Unavailable });

    assert!(confirm_message(&test).is_some_and(|message| message.ends_with("\nCouldn't count what is inside.")));
}

#[test]
fn an_answer_for_another_request_changes_nothing() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    let (request_id, ..) = counting_request(&mut test);

    test.app.apply_core_event(Event::ContentsCounted { request_id: request_id + 1, contents: counted_files(5, 0) });

    assert!(confirm_message(&test).is_some_and(|message| message.ends_with("Counting what is inside\u{2026}")));
}

#[test]
fn an_answer_after_the_dialog_closed_is_ignored() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    let (request_id, ..) = counting_request(&mut test);
    test.app.apply_dialog_key(key(KeyCode::Char('n')));
    test.sent();

    test.app.apply_core_event(Event::ContentsCounted { request_id, contents: counted_files(5, 0) });

    assert!(test.app.dialog.is_none());
    assert!(test.sent().is_empty());
}

#[test]
fn cancelling_while_counting_stops_the_count() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    test.sent();

    test.app.apply_dialog_key(key(KeyCode::Esc));

    assert_eq!(test.sent(), vec![Command::CancelCount]);
}

#[test]
fn confirming_while_counting_stops_the_count_and_deletes() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    test.sent();

    test.app.apply_dialog_key(key(KeyCode::Char('y')));

    assert_eq!(
        test.sent(),
        vec![
            Command::CancelCount,
            Command::Delete { location: Location::Local, paths: vec![VfsPath::from("/d/photos")] }
        ]
    );
}

#[test]
fn closing_after_the_answer_arrived_does_not_cancel_anything() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    let (request_id, ..) = counting_request(&mut test);
    test.app.apply_core_event(Event::ContentsCounted { request_id, contents: counted_files(2, 0) });

    test.app.apply_dialog_key(key(KeyCode::Char('n')));

    assert!(test.sent().is_empty());
}

#[test]
fn a_selection_counts_only_its_folders_and_puts_the_line_after_the_folder_note() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "a", false), entry(&VfsPath::new("/d"), "docs", true)]);
    test.app.local.cursor = 1;
    test.app.apply_action(Action::ToggleSelect);
    test.app.local.cursor = 2;
    test.app.apply_action(Action::ToggleSelect);

    test.app.apply_action(Action::Delete);

    let (_, _, folders) = counting_request(&mut test);
    assert_eq!(folders, vec![VfsPath::from("/d/docs")]);
    assert_eq!(
        confirm_message(&test),
        Some(
            "Delete 2 items from this computer?\ndocs/\na\nFolders are deleted with everything inside them.\nCounting what is inside\u{2026}"
        )
    );
}

#[test]
fn counting_on_the_remote_panel_goes_to_that_session() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(5, "prod");
    test.list_remote(session, "/srv", vec![entry(&VfsPath::new("/srv"), "logs", true)]);
    test.app.active_panel = ActivePanel::Remote;
    test.app.sessions.active_mut().unwrap().panel.cursor = 1;

    test.app.apply_action(Action::Delete);

    let (_, location, folders) = counting_request(&mut test);
    assert_eq!((location, folders), (Location::Session(session), vec![VfsPath::from("/srv/logs")]));
}

#[test]
fn two_prompts_in_a_row_use_different_request_ids() {
    let mut test = folder_prompt_app();
    test.app.apply_action(Action::Delete);
    let (first, ..) = counting_request(&mut test);
    test.app.apply_dialog_key(key(KeyCode::Char('n')));
    test.sent();

    test.app.apply_action(Action::Delete);
    let (second, ..) = counting_request(&mut test);

    assert_ne!(first, second);
}
