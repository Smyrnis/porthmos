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
    test.app.apply_core_event(Event::SearchDone { truncated: false, incomplete: false });

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
fn opening_a_result_asks_core_to_reveal_it_in_the_right_panel() {
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
            Command::Reveal { location: Location::Session(session), path: VfsPath::from("/srv/logs/x.log") },
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
    test.app.apply_core_event(Event::Revealed {
        location: Location::Session(session),
        folder: many.clone(),
        name: "f198.txt".to_string(),
    });

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
    test.app.apply_core_event(Event::Revealed {
        location: Location::Local,
        folder: dir.clone(),
        name: "f17.txt".to_string(),
    });

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
    test.app.apply_core_event(Event::Revealed {
        location: Location::Local,
        folder: dir.clone(),
        name: "f17.txt".to_string(),
    });

    assert_eq!((test.app.screen, cursor_name(&test.app.local)), (Screen::Transfers, Some("f17.txt")));
}

#[test]
fn a_reveal_for_a_folder_that_is_not_the_one_shown_leaves_the_cursor_alone() {
    let mut test = test_app(&VfsPath::new("/d"));
    let dir = VfsPath::new("/d");
    test.list_local(many_files(&dir, 1, 20));
    test.app.local.cursor = 2;
    let before = cursor_name(&test.app.local).map(str::to_string);

    test.app.apply_core_event(Event::Revealed {
        location: Location::Local,
        folder: VfsPath::new("/elsewhere"),
        name: "f17.txt".to_string(),
    });

    assert_eq!(cursor_name(&test.app.local).map(str::to_string), before);
}

fn open_search_with_preview(test: &mut crate::app::testing::TestApp) {
    test.app.search_preview = true;
    test.app.apply_action(Action::OpenSearch);
    test.sent();
}

fn found(test: &mut crate::app::testing::TestApp, name: &str, is_dir: bool) {
    test.app.apply_core_event(Event::SearchFound(entry(&VfsPath::new("/d"), name, is_dir)));
}

fn hints(commands: Vec<Command>) -> Vec<(u64, VfsPath)> {
    commands
        .into_iter()
        .filter_map(|command| match command {
            Command::PreviewHint { request_id, path, .. } => Some((request_id, path)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_first_result_arriving_is_hinted_when_the_setting_is_on() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);

    found(&mut test, "a.txt", false);

    let sent = hints(test.sent());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, VfsPath::from("/d/a.txt"));
}

#[test]
fn more_results_arriving_do_not_hint_again_while_the_selection_stays() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    test.sent();

    found(&mut test, "b.txt", false);

    assert!(hints(test.sent()).is_empty());
}

#[test]
fn moving_the_cursor_hints_the_new_file_with_a_fresh_request_id() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    found(&mut test, "b.txt", false);
    let first = hints(test.sent());

    test.app.apply_search_key(key(KeyCode::Down));

    let second = hints(test.sent());
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].1, VfsPath::from("/d/b.txt"));
    assert!(second[0].0 > first[0].0);
}

#[test]
fn no_hint_is_sent_when_the_setting_is_off() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    test.sent();

    found(&mut test, "a.txt", false);
    found(&mut test, "b.txt", false);
    test.app.apply_search_key(key(KeyCode::Down));
    test.app.apply_search_key(key(KeyCode::PageDown));

    assert!(test.sent().is_empty());
    assert!(test.app.search.as_ref().unwrap().view.pane.is_none());
}

#[test]
fn moving_onto_a_folder_cancels_the_preview_and_requests_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    found(&mut test, "sub", true);
    test.sent();

    test.app.apply_search_key(key(KeyCode::Down));

    assert_eq!(test.sent(), vec![Command::CancelPreview]);
}

#[test]
fn a_new_pattern_clears_the_pane_and_cancels_the_preview() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    test.sent();

    test.app.apply_search_key(key(KeyCode::Char('a')));

    assert!(test.app.search.as_ref().unwrap().view.pane.is_none());
    assert!(test.sent().contains(&Command::CancelPreview));
}

#[test]
fn closing_the_search_cancels_a_running_preview() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    test.sent();

    test.app.apply_search_key(key(KeyCode::Esc));

    let sent = test.sent();
    assert_eq!(sent.len(), 2);
    assert!(sent.contains(&Command::CancelSearch));
    assert!(sent.contains(&Command::CancelPreview));
}

#[test]
fn opening_a_result_cancels_a_running_preview() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    test.sent();

    test.app.apply_search_key(key(KeyCode::Enter));

    assert!(test.sent().contains(&Command::CancelPreview));
}

#[test]
fn a_ready_answer_for_the_current_request_fills_the_pane() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    let (request_id, _) = hints(test.sent()).remove(0);

    test.app.apply_core_event(Event::PreviewReady {
        request_id,
        name: "a.txt".to_string(),
        result: Ok(std::sync::Arc::new(porthmos_core::preview::Preview {
            size: Some(5),
            shown: 5,
            truncated: false,
            content: porthmos_core::preview::PreviewContent::Text("hello".to_string()),
        })),
    });

    let pane = test.app.search.as_ref().unwrap().view.pane.as_ref().unwrap();
    assert!(!pane.is_loading());
}

#[test]
fn an_answer_for_a_file_the_cursor_already_left_is_ignored() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    found(&mut test, "b.txt", false);
    let (old_id, _) = hints(test.sent()).remove(0);
    test.app.apply_search_key(key(KeyCode::Down));

    test.app.apply_core_event(Event::PreviewReady {
        request_id: old_id,
        name: "a.txt".to_string(),
        result: Err("stale".to_string()),
    });

    let pane = test.app.search.as_ref().unwrap().view.pane.as_ref().unwrap();
    assert!(pane.is_loading());
}

#[test]
fn page_down_scrolls_the_pane_not_the_list() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    found(&mut test, "b.txt", false);
    let (request_id, _) = hints(test.sent()).remove(0);
    test.app.apply_core_event(Event::PreviewReady {
        request_id,
        name: "a.txt".to_string(),
        result: Ok(std::sync::Arc::new(porthmos_core::preview::Preview {
            size: Some(400),
            shown: 400,
            truncated: false,
            content: porthmos_core::preview::PreviewContent::Text("x\n".repeat(200)),
        })),
    });
    crate::app::testing::render_app(&test.app, 80, 30);

    test.app.apply_search_key(key(KeyCode::PageDown));

    let view = &test.app.search.as_ref().unwrap().view;
    assert_eq!(view.selected, 0);
    assert!(view.pane.as_ref().unwrap().top() > 0);
    assert!(test.sent().is_empty());
}

#[test]
fn a_tall_terminal_draws_the_file_below_the_results() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    found(&mut test, "a.txt", false);
    let (request_id, _) = hints(test.sent()).remove(0);
    test.app.apply_core_event(Event::PreviewReady {
        request_id,
        name: "a.txt".to_string(),
        result: Ok(std::sync::Arc::new(porthmos_core::preview::Preview {
            size: Some(11),
            shown: 11,
            truncated: false,
            content: porthmos_core::preview::PreviewContent::Text("hello world".to_string()),
        })),
    });

    let terminal = crate::app::testing::render_app(&test.app, 80, 30);

    let (_, list_row) = crate::app::testing::find_text(&terminal, "a.txt").unwrap();
    let (_, content_row) = crate::app::testing::find_text(&terminal, "hello world").unwrap();
    assert!(content_row > list_row + 3, "{list_row} {content_row}");
}

#[test]
fn a_short_terminal_keeps_every_result_visible_and_draws_no_pane() {
    let mut test = test_app(&VfsPath::new("/d"));
    open_search_with_preview(&mut test);
    for name in ["a.txt", "b.txt", "c.txt"] {
        found(&mut test, name, false);
    }

    let terminal = crate::app::testing::render_app(&test.app, 80, 12);

    for name in ["a.txt", "b.txt", "c.txt"] {
        assert!(crate::app::testing::find_text(&terminal, name).is_some(), "{name}");
    }
    assert!(crate::app::testing::find_text(&terminal, "preview \u{b7}").is_none());
}

#[test]
fn the_hint_bar_mentions_scrolling_the_preview_only_while_the_setting_is_on() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    let off = crate::app::testing::render_app(&test.app, 100, 30);
    test.app.search_preview = true;
    let on = crate::app::testing::render_app(&test.app, 100, 30);

    assert!(crate::app::testing::find_text(&off, "scroll preview").is_none());
    assert!(crate::app::testing::find_text(&on, "scroll preview").is_some());
}

#[test]
fn searching_this_computer_says_so_with_the_folder() {
    let mut test = test_app(&VfsPath::new("/d/projects"));

    test.app.apply_action(Action::OpenSearch);

    assert_eq!(test.app.search.as_ref().unwrap().view.scope, "this computer \u{b7} /d/projects");
}

#[test]
fn searching_a_connection_names_it_with_its_folder() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(2, "prod");
    test.list_remote(session, "/srv/data", vec![entry(&VfsPath::new("/srv/data"), "a", false)]);
    test.app.active_panel = ActivePanel::Remote;

    test.app.apply_action(Action::OpenSearch);

    assert_eq!(test.app.search.as_ref().unwrap().view.scope, "prod \u{b7} /srv/data");
}

#[test]
fn a_search_that_skipped_folders_warns_that_results_may_be_missing() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);

    test.app.apply_core_event(Event::SearchDone { truncated: false, incomplete: true });

    let notice = test.app.notifications.current().unwrap();
    assert_eq!(notice.severity, Severity::Warning);
    assert_eq!(notice.message, porthmos_core::SEARCH_INCOMPLETE_NOTE);
}

#[test]
fn a_complete_search_says_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);

    test.app.apply_core_event(Event::SearchDone { truncated: false, incomplete: false });

    assert!(test.app.notifications.current().is_none());
}
