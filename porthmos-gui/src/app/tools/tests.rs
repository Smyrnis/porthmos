use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::{Command, Entry, Event, Location, VfsPath};

use crate::menus::MoreItem;
use crate::search::SearchStatus;
use crate::shortcuts::KeyPress;
use crate::testing::Harness;
use crate::tools::Tool;

use super::super::*;

fn press(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    let text = match &key {
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    };
    harness.press(Message::KeyPressed(KeyPress { key: key.clone(), modified_key: key, modifiers, text }));
}

fn named(harness: &mut Harness, key: Named) {
    press(harness, Key::Named(key), Modifiers::empty());
}

fn ctrl(harness: &mut Harness, character: &str) {
    press(harness, Key::Character(character.into()), Modifiers::CTRL);
}

fn open_search(harness: &mut Harness) {
    ctrl(harness, "f");
    harness.sent();
}

fn hit(path: &str) -> Entry {
    let path = VfsPath::new(path);
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    Entry { name, path, ..Entry::default() }
}

fn search(harness: &mut Harness, text: &str) {
    harness.press(Message::SearchTyped(text.to_string()));
}

fn found(harness: &mut Harness, path: &str) {
    harness.feed(Event::SearchFound(hit(path)));
}

fn search_state(harness: &Harness) -> &crate::search::SearchState {
    match harness.app.tool() {
        Some(Tool::Search(state)) => state,
        other => panic!("expected the search tool, got {other:?}"),
    }
}

fn remote_harness() -> Harness {
    let mut harness = Harness::new();
    harness.feed(Event::Connected { session: 1, name: "alpha".to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: VfsPath::new("/srv"),
        entries: vec![hit("/srv/a.txt")],
    });
    named(&mut harness, Named::Tab);
    harness.sent();
    harness
}

#[test]
fn ctrl_f_opens_search_on_the_active_panel() {
    let mut harness = Harness::new();
    harness.sent();

    ctrl(&mut harness, "f");

    assert_eq!(search_state(&harness).location(), Location::Local);
    assert_eq!(search_state(&harness).status(), &SearchStatus::Idle);
    assert!(harness.sent().is_empty());
}

#[test]
fn the_more_menu_item_opens_search() {
    let mut harness = Harness::new();
    harness.press(Message::MoreMenuToggled);

    harness.press(Message::MoreItemChosen(MoreItem::Search));

    assert!(matches!(harness.app.tool(), Some(Tool::Search(_))));
    assert!(!harness.app.more_open());
}

#[test]
fn typing_sends_a_wildcard_search_for_the_active_folder() {
    let mut harness = Harness::new();
    open_search(&mut harness);

    search(&mut harness, "abc");

    assert_eq!(
        harness.sent(),
        vec![Command::Search { location: Location::Local, root: harness.start_folder(), pattern: "*abc*".to_string() }]
    );
}

#[test]
fn typing_does_not_fire_panel_shortcuts() {
    let mut harness = Harness::new();
    let folder = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: folder.clone(),
        entries: vec![hit("/home/tester/a.txt")],
    });
    open_search(&mut harness);

    press(&mut harness, Key::Character("d".into()), Modifiers::empty());
    named(&mut harness, Named::Delete);
    named(&mut harness, Named::F7);
    named(&mut harness, Named::F5);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn arrow_keys_move_the_result_selection_not_the_panel_cursor() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/home/tester/a1");
    found(&mut harness, "/home/tester/a2");
    let cursor = harness.app.local().panel.cursor();

    named(&mut harness, Named::ArrowDown);

    assert_eq!(search_state(&harness).selected().unwrap().name, "a2");
    assert_eq!(harness.app.local().panel.cursor(), cursor);
    named(&mut harness, Named::ArrowUp);
    assert_eq!(search_state(&harness).selected().unwrap().name, "a1");
}

#[test]
fn results_arrive_and_enter_lists_the_parent_and_closes() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();
    found(&mut harness, "/home/tester/deep/a.txt");

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    let sent = harness.sent();
    assert!(sent.contains(&Command::CancelSearch), "{sent:?}");
    assert!(sent.contains(&Command::List { location: Location::Local, path: Some(VfsPath::new("/home/tester/deep")) }));
}

#[test]
fn enter_with_no_result_keeps_the_tool_open() {
    let mut harness = Harness::new();
    open_search(&mut harness);

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_some());
    assert!(harness.sent().is_empty());
}

#[test]
fn double_clicking_a_row_opens_that_result() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/home/tester/x/a1");
    found(&mut harness, "/home/tester/y/a2");
    harness.sent();

    harness.press(Message::SearchRowActivated(1));

    assert!(harness.app.tool().is_none());
    assert!(
        harness
            .sent()
            .contains(&Command::List { location: Location::Local, path: Some(VfsPath::new("/home/tester/y")) })
    );
}

#[test]
fn pressing_a_row_selects_it_and_the_open_button_opens_it() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/home/tester/x/a1");
    found(&mut harness, "/home/tester/y/a2");
    harness.sent();

    harness.press(Message::SearchRowPressed(1));
    assert_eq!(search_state(&harness).selected().unwrap().name, "a2");
    harness.press(Message::SearchOpenPressed);

    assert!(harness.app.tool().is_none());
}

#[test]
fn esc_closes_and_cancels_the_search() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();

    named(&mut harness, Named::Escape);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.sent(), vec![Command::CancelSearch]);
}

#[test]
fn the_close_button_closes_and_cancels_the_search() {
    let mut harness = Harness::new();
    open_search(&mut harness);

    harness.press(Message::ToolClosed);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.sent(), vec![Command::CancelSearch]);
}

#[test]
fn a_blank_pattern_cancels_the_running_search() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();

    search(&mut harness, "");

    assert_eq!(harness.sent(), vec![Command::CancelSearch]);
    assert_eq!(search_state(&harness).status(), &SearchStatus::Idle);
}

#[test]
fn late_search_events_after_closing_are_ignored() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    named(&mut harness, Named::Escape);

    found(&mut harness, "/home/tester/a");
    harness.feed(Event::SearchDone { truncated: true });
    harness.feed(Event::SearchFailed("late".to_string()));

    assert!(harness.app.tool().is_none());
}

#[test]
fn a_new_pattern_drops_results_of_the_old_one_at_once() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/home/tester/a1");

    search(&mut harness, "ab");

    assert!(search_state(&harness).results().is_empty());
}

#[test]
fn done_failed_and_truncated_events_reach_the_status_line() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.feed(Event::SearchDone { truncated: false });
    assert_eq!(search_state(&harness).status_text(), "Nothing found");

    search(&mut harness, "b");
    harness.feed(Event::SearchFailed("boom".to_string()));
    assert_eq!(search_state(&harness).status_text(), "Search failed: boom");
}

#[test]
fn searching_a_remote_panel_lists_in_that_session() {
    let mut harness = remote_harness();

    ctrl(&mut harness, "f");
    search(&mut harness, "a");
    assert_eq!(
        harness.sent(),
        vec![Command::Search {
            location: Location::Session(1),
            root: VfsPath::new("/srv"),
            pattern: "*a*".to_string()
        }]
    );
    found(&mut harness, "/srv/sub/a.txt");
    named(&mut harness, Named::Enter);

    assert_eq!(harness.app.active_side(), PanelSide::Remote);
    assert!(
        harness
            .sent()
            .contains(&Command::List { location: Location::Session(1), path: Some(VfsPath::new("/srv/sub")) })
    );
}

#[test]
fn disconnecting_the_searched_session_closes_the_tool() {
    let mut harness = remote_harness();
    ctrl(&mut harness, "f");

    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    assert!(harness.app.tool().is_none());
}

#[test]
fn menus_and_row_presses_are_blocked_while_a_tool_is_open() {
    let mut harness = remote_harness();
    named(&mut harness, Named::Tab);
    harness.sent();
    open_search(&mut harness);

    assert!(!harness.app.menus_allowed());
    harness.app.press_row(PanelSide::Remote, 0);

    assert_eq!(harness.app.active_side(), PanelSide::Local);
}

#[test]
fn keyboard_selection_scrolls_the_results_into_view() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    for index in 0..200 {
        found(&mut harness, &format!("/home/tester/a{index}"));
    }
    assert_eq!(harness.app.tool_scroll(), 0.0);

    for _ in 0..150 {
        named(&mut harness, Named::ArrowDown);
    }

    assert!(harness.app.tool_scroll() > 0.0);
}

fn bookmark_modal(harness: &Harness) -> Option<(String, Option<String>)> {
    match harness.app.modal() {
        Some(crate::modal::Modal::BookmarkName { text, error, .. }) => Some((text.clone(), error.clone())),
        _ => None,
    }
}

#[test]
fn ctrl_d_opens_the_name_dialog_with_the_folder_name() {
    let mut harness = Harness::new();

    ctrl(&mut harness, "d");

    assert_eq!(bookmark_modal(&harness), Some(("tester".to_string(), None)));
}

#[test]
fn the_more_menu_item_opens_the_name_dialog() {
    let mut harness = Harness::new();

    harness.press(Message::MoreItemChosen(MoreItem::BookmarkHere));

    assert!(bookmark_modal(&harness).is_some());
}

#[test]
fn accepting_sends_add_bookmark_with_the_active_location() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "d");
    harness.sent();

    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![Command::AddBookmark {
            label: "tester".to_string(),
            location: Location::Local,
            path: VfsPath::new("/home/tester")
        }]
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_typed_name_is_used_as_the_label() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "d");
    harness.sent();

    harness.press(Message::ModalTyped("My home".to_string()));
    harness.press(Message::ModalAccepted);

    assert!(matches!(&harness.sent()[..], [Command::AddBookmark { label, .. }] if label == "My home"));
}

#[test]
fn an_empty_name_shows_an_error_and_sends_nothing() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "d");
    harness.sent();

    harness.press(Message::ModalTyped("   ".to_string()));
    harness.press(Message::ModalAccepted);

    assert_eq!(bookmark_modal(&harness), Some(("   ".to_string(), Some("Enter a name".to_string()))));
    assert!(harness.sent().is_empty());
}

#[test]
fn typing_clears_the_error() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "d");
    harness.press(Message::ModalTyped(String::new()));
    harness.press(Message::ModalAccepted);

    harness.press(Message::ModalTyped("x".to_string()));

    assert_eq!(bookmark_modal(&harness), Some(("x".to_string(), None)));
}

#[test]
fn declining_sends_nothing() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "d");
    harness.sent();

    harness.press(Message::ModalDeclined);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn bookmarking_a_remote_folder_uses_its_session() {
    let mut harness = remote_harness();

    ctrl(&mut harness, "d");
    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![Command::AddBookmark {
            label: "srv".to_string(),
            location: Location::Session(1),
            path: VfsPath::new("/srv")
        }]
    );
}

#[test]
fn the_bookmark_list_from_core_is_kept() {
    let mut harness = Harness::new();

    harness.feed(Event::Bookmarks(vec![porthmos_core::config::bookmarks::Bookmark {
        label: "Home".to_string(),
        path: VfsPath::new("/home/me"),
        host: None,
    }]));

    assert_eq!(harness.app.bookmarks().len(), 1);
}

fn marked(label: &str, path: &str, host: Option<&str>) -> porthmos_core::config::bookmarks::Bookmark {
    porthmos_core::config::bookmarks::Bookmark {
        label: label.to_string(),
        path: VfsPath::new(path),
        host: host.map(str::to_string),
    }
}

fn feed_bookmarks(harness: &mut Harness, items: Vec<porthmos_core::config::bookmarks::Bookmark>) {
    harness.feed(Event::Bookmarks(items));
}

fn bookmarks_state(harness: &Harness) -> &crate::bookmarks::BookmarksState {
    match harness.app.tool() {
        Some(Tool::Bookmarks(state)) => state,
        other => panic!("expected the bookmarks tool, got {other:?}"),
    }
}

fn selected_label(harness: &Harness) -> Option<String> {
    bookmarks_state(harness).selected().map(|(_, bookmark)| bookmark.label.clone())
}

#[test]
fn ctrl_b_opens_the_bookmark_list() {
    let mut harness = Harness::new();
    feed_bookmarks(&mut harness, vec![marked("Home", "/home/me", None)]);

    ctrl(&mut harness, "b");

    assert_eq!(bookmarks_state(&harness).items().len(), 1);
    assert_eq!(selected_label(&harness).as_deref(), Some("Home"));
}

#[test]
fn the_more_menu_item_opens_the_bookmark_list() {
    let mut harness = Harness::new();

    harness.press(Message::MoreItemChosen(MoreItem::Bookmarks));

    assert!(matches!(harness.app.tool(), Some(Tool::Bookmarks(_))));
}

#[test]
fn go_local_lists_the_local_folder_and_closes() {
    let mut harness = remote_harness();
    feed_bookmarks(&mut harness, vec![marked("Data", "/data", None)]);
    ctrl(&mut harness, "b");
    harness.sent();

    harness.press(Message::BookmarkGoPressed);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.app.active_side(), PanelSide::Local);
    assert_eq!(harness.sent(), vec![Command::List { location: Location::Local, path: Some(VfsPath::new("/data")) }]);
}

#[test]
fn go_remote_activates_the_session_and_lists() {
    let mut harness = remote_harness();
    named(&mut harness, Named::Tab);
    feed_bookmarks(&mut harness, vec![marked("Web", "/var/www", Some("alpha"))]);
    ctrl(&mut harness, "b");
    harness.sent();

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.app.active_side(), PanelSide::Remote);
    assert_eq!(
        harness.sent(),
        vec![Command::List { location: Location::Session(1), path: Some(VfsPath::new("/var/www")) }]
    );
}

#[test]
fn go_remote_without_a_session_warns_and_stays_open() {
    let mut harness = Harness::new();
    feed_bookmarks(&mut harness, vec![marked("Web", "/var/www", Some("beta"))]);
    ctrl(&mut harness, "b");
    harness.sent();

    harness.press(Message::BookmarkRowActivated(0));

    assert!(harness.app.tool().is_some());
    assert!(harness.sent().is_empty());
    let notice = harness.app.notices().last().unwrap();
    assert_eq!(notice.message, "Connect to beta first");
    assert_eq!(notice.severity, porthmos_core::Severity::Warning);
}

#[test]
fn a_bookmark_for_a_session_closed_meanwhile_warns_instead_of_listing() {
    let mut harness = remote_harness();
    feed_bookmarks(&mut harness, vec![marked("Web", "/var/www", Some("alpha"))]);
    ctrl(&mut harness, "b");
    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });
    harness.sent();

    harness.press(Message::BookmarkGoPressed);

    assert!(harness.app.tool().is_some());
    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.notices().last().unwrap().message, "Connect to alpha first");
}

#[test]
fn remove_sends_the_selected_index_and_keeps_the_list_open() {
    let mut harness = Harness::new();
    feed_bookmarks(&mut harness, vec![marked("a", "/a", None), marked("b", "/b", None)]);
    ctrl(&mut harness, "b");
    harness.sent();

    harness.press(Message::BookmarkRowPressed(1));
    harness.press(Message::BookmarkRemovePressed);

    assert_eq!(harness.sent(), vec![Command::RemoveBookmark { index: 1 }]);
    assert!(harness.app.tool().is_some());
}

#[test]
fn the_list_refreshes_when_bookmarks_arrive_while_open() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "b");
    assert!(bookmarks_state(&harness).items().is_empty());

    feed_bookmarks(&mut harness, vec![marked("a", "/a", None)]);

    assert_eq!(bookmarks_state(&harness).items().len(), 1);
    assert_eq!(selected_label(&harness).as_deref(), Some("a"));
}

#[test]
fn removing_the_last_row_selects_the_new_last_row() {
    let mut harness = Harness::new();
    feed_bookmarks(&mut harness, vec![marked("a", "/a", None), marked("b", "/b", None)]);
    ctrl(&mut harness, "b");
    harness.press(Message::BookmarkRowPressed(1));

    feed_bookmarks(&mut harness, vec![marked("a", "/a", None)]);

    assert_eq!(selected_label(&harness).as_deref(), Some("a"));
}

#[test]
fn go_and_remove_do_nothing_with_an_empty_list() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "b");
    harness.sent();

    harness.press(Message::BookmarkGoPressed);
    harness.press(Message::BookmarkRemovePressed);
    named(&mut harness, Named::Enter);

    assert!(harness.sent().is_empty());
    assert!(harness.app.tool().is_some());
}

#[test]
fn arrows_move_the_bookmark_selection_and_esc_closes_without_a_search_cancel() {
    let mut harness = Harness::new();
    feed_bookmarks(&mut harness, vec![marked("a", "/a", None), marked("b", "/b", None)]);
    ctrl(&mut harness, "b");
    harness.sent();

    named(&mut harness, Named::ArrowDown);
    assert_eq!(selected_label(&harness).as_deref(), Some("b"));
    named(&mut harness, Named::ArrowUp);
    assert_eq!(selected_label(&harness).as_deref(), Some("a"));
    named(&mut harness, Named::Escape);

    assert!(harness.app.tool().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn opening_a_tool_over_a_search_cancels_the_search() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();

    harness.app.open_bookmarks();

    assert!(matches!(harness.app.tool(), Some(Tool::Bookmarks(_))));
    assert_eq!(harness.sent(), vec![Command::CancelSearch]);
}

#[test]
fn search_events_do_not_disturb_the_bookmark_list() {
    let mut harness = Harness::new();
    ctrl(&mut harness, "b");

    found(&mut harness, "/x/a");
    harness.feed(Event::SearchDone { truncated: false });

    assert!(matches!(harness.app.tool(), Some(Tool::Bookmarks(_))));
}

#[test]
fn keyboard_selection_scrolls_the_bookmarks_into_view() {
    let mut harness = Harness::new();
    let items = (0..100).map(|index| marked(&format!("b{index}"), "/a", None)).collect();
    feed_bookmarks(&mut harness, items);
    ctrl(&mut harness, "b");

    for _ in 0..80 {
        named(&mut harness, Named::ArrowDown);
    }

    assert!(harness.app.tool_scroll() > 0.0);
}

fn past(label: &str, connection: &str) -> porthmos_core::history::HistoryEntry {
    use chrono::TimeZone;
    porthmos_core::history::HistoryEntry {
        finished_at: chrono::Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap(),
        connection: connection.to_string(),
        direction: porthmos_core::transfer::Direction::Upload,
        label: label.to_string(),
        local_path: "/l".to_string(),
        remote_path: "/r".to_string(),
        files_done: 1,
        files_total: 1,
        bytes: 10,
        result: porthmos_core::history::HistoryResult::Done,
        failed_count: 0,
        failed_files: Vec::new(),
    }
}

fn history_state(harness: &Harness) -> &crate::history_view::HistoryState {
    match harness.app.tool() {
        Some(Tool::History(state)) => state,
        other => panic!("expected the history tool, got {other:?}"),
    }
}

fn open_history(harness: &mut Harness, entries: Vec<porthmos_core::history::HistoryEntry>) {
    ctrl(harness, "y");
    harness.feed(Event::History(entries));
    harness.sent();
}

#[test]
fn ctrl_y_opens_history_and_requests_it() {
    let mut harness = Harness::new();
    harness.sent();

    ctrl(&mut harness, "y");

    assert!(matches!(harness.app.tool(), Some(Tool::History(_))));
    assert_eq!(harness.sent(), vec![Command::ListHistory]);
}

#[test]
fn the_more_menu_item_opens_history() {
    let mut harness = Harness::new();

    harness.press(Message::MoreItemChosen(MoreItem::History));

    assert!(matches!(harness.app.tool(), Some(Tool::History(_))));
}

#[test]
fn entries_arrive_and_the_first_is_selected() {
    let mut harness = Harness::new();

    open_history(&mut harness, vec![past("photos", "alpha"), past("music", "beta")]);

    assert_eq!(history_state(&harness).total(), 2);
    assert_eq!(history_state(&harness).selected().unwrap().label, "photos");
}

#[test]
fn history_events_without_the_tool_are_ignored() {
    let mut harness = Harness::new();

    harness.feed(Event::History(vec![past("photos", "alpha")]));

    assert!(harness.app.tool().is_none());
}

#[test]
fn typing_a_filter_narrows_the_list_without_firing_panel_shortcuts() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha"), past("music", "beta")]);

    harness.press(Message::HistoryFilterTyped("beta".to_string()));
    press(&mut harness, Key::Character("d".into()), Modifiers::empty());
    named(&mut harness, Named::Delete);

    assert_eq!(history_state(&harness).visible().len(), 1);
    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn arrows_move_the_history_selection_and_esc_closes() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha"), past("music", "beta")]);

    named(&mut harness, Named::ArrowDown);
    assert_eq!(history_state(&harness).selected().unwrap().label, "music");
    named(&mut harness, Named::Escape);

    assert!(harness.app.tool().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn enter_opens_details_in_a_dialog_over_the_list() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha")]);

    named(&mut harness, Named::Enter);

    match harness.app.modal() {
        Some(crate::modal::Modal::Info { title, body }) => {
            assert_eq!(title, "Transfer: photos");
            assert!(body.contains("Connection: alpha (Upload)"));
        }
        other => panic!("expected the details dialog, got {other:?}"),
    }
    assert!(harness.app.tool().is_some());
    named(&mut harness, Named::Enter);
    assert!(harness.app.modal().is_none());
    assert!(harness.app.tool().is_some());
}

#[test]
fn double_clicking_a_row_shows_its_details() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha"), past("music", "beta")]);

    harness.press(Message::HistoryRowActivated(1));

    assert!(matches!(harness.app.modal(), Some(crate::modal::Modal::Info { title, .. }) if title == "Transfer: music"));
}

#[test]
fn details_with_nothing_selected_do_nothing() {
    let mut harness = Harness::new();
    open_history(&mut harness, Vec::new());

    harness.press(Message::HistoryDetailsPressed);
    named(&mut harness, Named::Enter);

    assert!(harness.app.modal().is_none());
}

#[test]
fn clear_asks_before_sending() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha")]);

    harness.press(Message::HistoryClearPressed);
    assert!(matches!(harness.app.modal(), Some(crate::modal::Modal::ConfirmClearHistory)));
    assert!(harness.sent().is_empty());
    harness.press(Message::ModalAccepted);

    assert_eq!(harness.sent(), vec![Command::ClearHistory]);
}

#[test]
fn declining_clear_sends_nothing() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha")]);
    harness.press(Message::HistoryClearPressed);

    harness.press(Message::ModalDeclined);

    assert!(harness.sent().is_empty());
    assert!(harness.app.modal().is_none());
}

#[test]
fn clear_does_nothing_when_the_history_is_empty() {
    let mut harness = Harness::new();
    open_history(&mut harness, Vec::new());

    harness.press(Message::HistoryClearPressed);

    assert!(harness.app.modal().is_none());
}

#[test]
fn an_emptied_history_arriving_after_clear_empties_the_list() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha")]);

    harness.feed(Event::History(Vec::new()));

    assert_eq!(history_state(&harness).total(), 0);
}

#[test]
fn history_filter_text_survives_a_refresh() {
    let mut harness = Harness::new();
    open_history(&mut harness, vec![past("photos", "alpha"), past("music", "beta")]);
    harness.press(Message::HistoryFilterTyped("beta".to_string()));

    harness.feed(Event::History(vec![past("photos", "alpha"), past("music", "beta"), past("new", "beta")]));

    assert_eq!(history_state(&harness).visible().len(), 2);
}

#[test]
fn keyboard_selection_scrolls_the_history_into_view() {
    let mut harness = Harness::new();
    let entries = (0..300).map(|index| past(&format!("job{index}"), "alpha")).collect();
    open_history(&mut harness, entries);

    for _ in 0..200 {
        named(&mut harness, Named::ArrowDown);
    }

    assert!(harness.app.tool_scroll() > 0.0);
}
