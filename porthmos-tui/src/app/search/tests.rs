use porthmos_core::VfsPath;

use crossterm::event::{KeyCode, KeyEventState, KeyModifiers};

use super::*;
use crate::app::testing::{cursor_name, entry, test_app};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

#[test]
fn open_search_action_switches_to_the_search_screen_for_the_local_panel() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    assert_eq!(test.app.screen, Screen::Search);
}

#[test]
fn open_search_action_warns_when_remote_is_focused_without_a_connection() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.active_panel = ActivePanel::Remote;

    test.app.apply_action(Action::OpenSearch);

    assert_eq!(test.app.screen, Screen::Files);
    assert!(test.app.notifications.current().is_some());
}

#[test]
fn esc_in_search_closes_the_screen() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);

    test.app.apply_search_key(key(KeyCode::Esc));

    assert_eq!(test.app.screen, Screen::Files);
    assert!(test.app.search.is_none());
    assert_eq!(test.sent(), vec![Command::CancelSearch]);
}

#[test]
fn typing_a_pattern_streams_matching_results_back_into_the_search_view() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);

    for c in "target".chars() {
        test.app.apply_search_key(key(KeyCode::Char(c)));
    }
    test.app.apply_core_event(Event::SearchFound(entry(&VfsPath::new("/d"), "target.log", false)));
    test.app.apply_core_event(Event::SearchDone { truncated: false });

    assert!(test.app.search.as_ref().unwrap().view.results.iter().any(|entry| entry.name == "target.log"));
    assert_eq!(
        test.sent().last(),
        Some(&Command::Search {
            location: Location::Local,
            root: VfsPath::from("/d"),
            pattern: "*target*".to_string()
        })
    );
}

#[test]
fn rapid_pattern_changes_dispatch_only_one_search_for_the_final_pattern() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);

    for c in "aaa".chars() {
        test.app.apply_search_key(key(KeyCode::Char(c)));
    }

    let searches: Vec<String> = test
        .sent()
        .into_iter()
        .filter_map(|command| match command {
            Command::Search { pattern, .. } => Some(pattern),
            _ => None,
        })
        .collect();
    assert_eq!(searches.last().map(String::as_str), Some("*aaa*"), "the core debounces all but the last");
}

#[test]
fn clearing_the_pattern_cancels_the_search() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    test.app.apply_search_key(key(KeyCode::Char('a')));
    test.sent();

    test.app.apply_search_key(key(KeyCode::Backspace));

    assert_eq!(test.sent(), vec![Command::CancelSearch]);
}

#[test]
fn ctrl_c_cancels_without_leaving_the_screen() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);

    test.app.apply_search_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(test.app.screen, Screen::Search);
    assert_eq!(test.sent(), vec![Command::CancelSearch]);
}

#[test]
fn opening_a_result_lists_its_folder_in_the_right_panel() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(3, "srv");
    test.list_remote(session, "/srv", Vec::new());
    test.app.active_panel = ActivePanel::Remote;
    test.app.apply_action(Action::OpenSearch);
    test.app.apply_core_event(Event::SearchFound(entry(&VfsPath::new("/srv/logs"), "x.log", false)));
    test.sent();

    test.app.apply_search_key(key(KeyCode::Enter));

    assert_eq!(test.app.screen, Screen::Files);
    assert_eq!(
        test.sent(),
        vec![
            Command::CancelSearch,
            Command::List { location: Location::Session(session), path: Some(VfsPath::from("/srv/logs")) },
        ]
    );
}

#[test]
fn typing_in_the_search_pattern_only_types() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    test.sent();

    test.type_text(crate::app::testing::TYPED);

    assert_eq!(test.app.search.as_ref().unwrap().view.pattern, crate::app::testing::TYPED);
    assert!(test.sent().iter().all(|command| matches!(command, Command::Search { .. } | Command::CancelSearch)));
    assert_eq!(test.app.screen, Screen::Search);
    assert!(!test.app.should_quit);
    assert!(test.app.dialog.is_none());
}

fn many_files(dir: &VfsPath, first: u32, last: u32) -> Vec<porthmos_core::Entry> {
    (first..=last).map(|n| entry(dir, &format!("f{n}.txt"), false)).collect()
}

#[test]
fn opening_a_result_in_another_folder_puts_the_cursor_on_that_file_once_it_is_listed() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(3, "srv");
    let home = VfsPath::from("/home/x");
    test.list_remote(session, "/home/x", many_files(&home, 1, 8));
    test.app.active_panel = ActivePanel::Remote;
    test.app.sessions.by_id_mut(session).unwrap().panel.cursor = 4;
    test.app.apply_action(Action::OpenSearch);
    let many = VfsPath::from("/home/x/many");
    test.app.apply_core_event(Event::SearchFound(entry(&many, "f198.txt", false)));
    test.app.apply_search_key(key(KeyCode::Enter));

    test.list_remote(session, "/home/x/many", many_files(&many, 101, 199));

    let panel = &test.app.sessions.by_id(session).unwrap().panel;
    assert_eq!(cursor_name(panel), Some("f198.txt"));
}

#[test]
fn opening_a_result_in_the_current_folder_puts_the_cursor_on_that_file_once_it_is_listed() {
    let mut test = test_app(&VfsPath::new("/d"));
    let dir = VfsPath::new("/d");
    test.list_local(many_files(&dir, 1, 20));
    test.app.local.cursor = 2;
    test.app.apply_action(Action::OpenSearch);
    test.app.apply_core_event(Event::SearchFound(entry(&dir, "f17.txt", false)));
    test.app.apply_search_key(key(KeyCode::Enter));

    test.list_local(many_files(&dir, 1, 20));

    assert_eq!(cursor_name(&test.app.local), Some("f17.txt"));
}

#[test]
fn the_cursor_is_left_alone_when_the_search_is_closed_without_opening_a_result() {
    let mut test = test_app(&VfsPath::new("/d"));
    let dir = VfsPath::new("/d");
    test.list_local(many_files(&dir, 1, 20));
    test.app.local.cursor = 2;
    let before = cursor_name(&test.app.local).map(str::to_string);
    test.app.apply_action(Action::OpenSearch);
    test.app.apply_core_event(Event::SearchFound(entry(&dir, "f17.txt", false)));
    test.app.apply_search_key(key(KeyCode::Esc));

    test.list_local(many_files(&dir, 1, 20));

    assert_eq!(cursor_name(&test.app.local).map(str::to_string), before);
}

#[test]
fn the_cursor_moves_to_the_result_without_pulling_the_user_back_to_the_files_screen() {
    let mut test = test_app(&VfsPath::new("/d"));
    let dir = VfsPath::new("/d");
    test.list_local(many_files(&dir, 1, 20));
    test.app.apply_action(Action::OpenSearch);
    test.app.apply_core_event(Event::SearchFound(entry(&dir, "f17.txt", false)));
    test.app.apply_search_key(key(KeyCode::Enter));
    test.app.screen = Screen::Transfers;

    test.list_local(many_files(&dir, 1, 20));

    assert_eq!((test.app.screen, cursor_name(&test.app.local)), (Screen::Transfers, Some("f17.txt")));
}
