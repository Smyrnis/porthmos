use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::{Command, Entry, Event, Location, VfsPath};

use crate::menus::MoreItem;
use crate::search::SearchStatus;
use crate::shortcuts::KeyPress;
use crate::testing::Harness;

use super::super::*;

fn press_in(harness: &mut Harness, window: iced::window::Id, key: Key, modifiers: Modifiers) {
    let text = match &key {
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    };
    harness.press(Message::WindowKey(window, KeyPress { key: key.clone(), modified_key: key, modifiers, text }));
}

fn press(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    let window = harness.search_id().unwrap_or(harness.main_id());
    press_in(harness, window, key, modifiers);
}

fn named(harness: &mut Harness, key: Named) {
    press(harness, Key::Named(key), Modifiers::empty());
}

fn named_in_main(harness: &mut Harness, key: Named) {
    let window = harness.main_id();
    press_in(harness, window, Key::Named(key), Modifiers::empty());
}

fn ctrl(harness: &mut Harness, character: &str) {
    let window = harness.main_id();
    press_in(harness, window, Key::Character(character.into()), Modifiers::CTRL);
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
    harness.app.search_state().expect("the search window has state")
}

fn remote_harness() -> Harness {
    let mut harness = Harness::new();
    harness.feed(Event::Connected { session: 1, name: "alpha".to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: VfsPath::new("/srv"),
        entries: vec![hit("/srv/a.txt")],
    });
    named_in_main(&mut harness, Named::Tab);
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

    assert!(harness.app.search_state().is_some());
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
fn typing_goes_to_the_pattern_and_does_not_fire_panel_shortcuts() {
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
    assert_eq!(
        harness.sent(),
        vec![Command::Search { location: Location::Local, root: harness.start_folder(), pattern: "*d*".to_string() }]
    );
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
fn results_arrive_and_enter_reveals_the_result_and_keeps_the_window_open() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();
    found(&mut harness, "/home/tester/deep/a.txt");

    named(&mut harness, Named::Enter);

    assert!(harness.app.search_state().is_some());
    let sent = harness.sent();
    assert!(!sent.contains(&Command::CancelSearch), "{sent:?}");
    assert!(
        sent.contains(&Command::Reveal { location: Location::Local, path: VfsPath::new("/home/tester/deep/a.txt") })
    );
}

#[test]
fn enter_with_no_result_keeps_the_window_open_and_sends_nothing() {
    let mut harness = Harness::new();
    open_search(&mut harness);

    named(&mut harness, Named::Enter);

    assert!(harness.app.search_state().is_some());
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

    assert!(harness.app.search_state().is_some());
    assert!(
        harness
            .sent()
            .contains(&Command::Reveal { location: Location::Local, path: VfsPath::new("/home/tester/y/a2") })
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

    assert!(harness.app.search_state().is_some());
}

#[test]
fn esc_closes_and_cancels_the_search() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();

    named(&mut harness, Named::Escape);

    assert!(harness.app.search_state().is_none());
    assert!(harness.search_id().is_none());
    assert_eq!(harness.sent(), vec![Command::CancelSearch]);
}

#[test]
fn the_close_button_closes_and_cancels_the_search() {
    let mut harness = Harness::new();
    open_search(&mut harness);

    harness.press(Message::SearchClosed);

    assert!(harness.app.search_state().is_none());
    assert!(harness.search_id().is_none());
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

    assert!(harness.app.search_state().is_none());
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
            .contains(&Command::Reveal { location: Location::Session(1), path: VfsPath::new("/srv/sub/a.txt") })
    );
}

#[test]
fn disconnecting_the_searched_session_closes_the_window() {
    let mut harness = remote_harness();
    ctrl(&mut harness, "f");

    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    assert!(harness.app.search_state().is_none());
    assert!(harness.search_id().is_none());
}

#[test]
fn menus_and_row_presses_stay_available_while_the_search_window_is_open() {
    let mut harness = remote_harness();
    named_in_main(&mut harness, Named::Tab);
    harness.sent();
    open_search(&mut harness);

    assert!(harness.app.menus_allowed());
    harness.app.press_row(PanelSide::Remote, 0);

    assert_eq!(harness.app.active_side(), PanelSide::Remote);
}

#[test]
fn keyboard_selection_scrolls_the_results_into_view() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    for index in 0..200 {
        found(&mut harness, &format!("/home/tester/a{index}"));
    }
    assert_eq!(harness.app.search_scroll(), 0.0);

    for _ in 0..150 {
        named(&mut harness, Named::ArrowDown);
    }

    assert!(harness.app.search_scroll() > 0.0);
}

fn list_local_folder(harness: &mut Harness, folder: &str, names: &[&str]) {
    let folder = VfsPath::new(folder);
    let entries: Vec<Entry> = names
        .iter()
        .map(|name| Entry { name: name.to_string(), path: folder.join(*name), ..Entry::default() })
        .collect();
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries });
}

fn cursor_name(harness: &Harness) -> Option<String> {
    harness.app.local().panel.cursor_entry().map(|entry| entry.name)
}

#[test]
fn opening_a_result_in_another_folder_puts_the_cursor_on_the_file_once_core_reveals_it() {
    let mut harness = Harness::new();
    list_local_folder(&mut harness, "/home/tester", &["deep"]);
    open_search(&mut harness);
    search(&mut harness, "f19");
    found(&mut harness, "/home/tester/deep/f19.txt");
    named(&mut harness, Named::Enter);
    assert_eq!(cursor_name(&harness), None);

    list_local_folder(&mut harness, "/home/tester/deep", &["f1.txt", "f19.txt", "f198.txt"]);
    harness.feed(Event::Revealed {
        location: Location::Local,
        folder: VfsPath::new("/home/tester/deep"),
        name: "f19.txt".to_string(),
    });

    assert_eq!(cursor_name(&harness).as_deref(), Some("f19.txt"));
}

#[test]
fn opening_a_result_in_the_folder_already_shown_moves_the_cursor_once_core_reveals_it() {
    let mut harness = Harness::new();
    list_local_folder(&mut harness, "/home/tester", &["a.txt", "b.txt", "c.txt"]);
    open_search(&mut harness);
    search(&mut harness, "c");
    found(&mut harness, "/home/tester/c.txt");
    named(&mut harness, Named::Enter);

    list_local_folder(&mut harness, "/home/tester", &["a.txt", "b.txt", "c.txt"]);
    harness.feed(Event::Revealed {
        location: Location::Local,
        folder: VfsPath::new("/home/tester"),
        name: "c.txt".to_string(),
    });

    assert_eq!(cursor_name(&harness).as_deref(), Some("c.txt"));
}

#[test]
fn the_cursor_is_not_pulled_back_by_a_later_unrelated_listing() {
    let mut harness = Harness::new();
    list_local_folder(&mut harness, "/home/tester", &["a.txt", "b.txt"]);
    open_search(&mut harness);
    search(&mut harness, "b");
    found(&mut harness, "/home/tester/b.txt");
    named(&mut harness, Named::Enter);
    list_local_folder(&mut harness, "/home/tester", &["a.txt", "b.txt"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));

    list_local_folder(&mut harness, "/home/tester", &["a.txt", "b.txt"]);

    assert_eq!(cursor_name(&harness).as_deref(), Some("a.txt"));
}

fn previewing_harness() -> Harness {
    Harness::with_gui_settings(crate::settings_view::GuiSettings {
        search_preview: true,
        ..crate::settings_view::GuiSettings::default()
    })
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
fn the_first_result_is_hinted_when_the_setting_is_on() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();

    found(&mut harness, "/data/a.txt");

    let sent = hints(harness.sent());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, VfsPath::new("/data/a.txt"));
}

#[test]
fn no_hint_is_sent_when_the_setting_is_off() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    found(&mut harness, "/data/b.txt");
    named(&mut harness, Named::ArrowDown);
    harness.press(Message::SearchRowPressed(0));

    assert!(hints(harness.sent()).is_empty());
    assert!(search_state(&harness).preview().is_none());
}

#[test]
fn moving_the_selection_with_the_keyboard_hints_the_new_file() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    found(&mut harness, "/data/b.txt");
    let first = hints(harness.sent());

    named(&mut harness, Named::ArrowDown);

    let second = hints(harness.sent());
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].1, VfsPath::new("/data/b.txt"));
    assert!(second[0].0 > first[0].0);
}

#[test]
fn clicking_a_row_hints_that_row() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    found(&mut harness, "/data/b.txt");
    harness.sent();

    harness.press(Message::SearchRowPressed(1));

    let sent = hints(harness.sent());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, VfsPath::new("/data/b.txt"));
}

#[test]
fn clicking_the_row_that_is_already_shown_does_not_hint_again() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    harness.sent();

    harness.press(Message::SearchRowPressed(0));

    assert!(hints(harness.sent()).is_empty());
}

#[test]
fn typing_a_new_pattern_cancels_the_preview_and_clears_the_pane() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    harness.sent();

    search(&mut harness, "ab");

    assert!(harness.sent().contains(&Command::CancelPreview));
    assert!(search_state(&harness).preview().is_none());
}

#[test]
fn closing_the_search_cancels_a_running_preview() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    harness.sent();

    named(&mut harness, Named::Escape);

    let sent = harness.sent();
    assert!(sent.contains(&Command::CancelSearch), "{sent:?}");
    assert!(sent.contains(&Command::CancelPreview), "{sent:?}");
}

#[test]
fn an_answer_for_the_current_request_fills_the_pane() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    let (request_id, _) = hints(harness.sent()).remove(0);

    harness.feed(Event::PreviewReady { request_id, name: "a.txt".to_string(), result: Err("denied".to_string()) });

    assert!(matches!(
        search_state(&harness).preview(),
        Some(crate::search::SearchPreview::File(file)) if !file.is_loading()
    ));
}

#[test]
fn an_answer_for_a_file_the_selection_already_left_is_ignored() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    found(&mut harness, "/data/b.txt");
    let (old_id, _) = hints(harness.sent()).remove(0);
    named(&mut harness, Named::ArrowDown);

    harness.feed(Event::PreviewReady {
        request_id: old_id,
        name: "a.txt".to_string(),
        result: Err("stale".to_string()),
    });

    assert!(matches!(
        search_state(&harness).preview(),
        Some(crate::search::SearchPreview::File(file)) if file.is_loading()
    ));
}

#[test]
fn moving_the_selection_resets_the_pane_scroll() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    found(&mut harness, "/data/b.txt");
    harness.press(Message::SearchPreviewScrolled(240.0));
    assert_eq!(harness.app.search_preview_scroll(), 240.0);

    named(&mut harness, Named::ArrowDown);

    assert_eq!(harness.app.search_preview_scroll(), 0.0);
}

#[test]
fn the_search_view_builds_for_every_state_of_the_pane() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    drop(harness.app.view_of(harness.search_id().unwrap()));
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    drop(harness.app.view_of(harness.search_id().unwrap()));
    let (request_id, _) = hints(harness.sent()).remove(0);
    let text = std::sync::Arc::new(porthmos_core::preview::Preview {
        size: Some(12),
        shown: 12,
        truncated: false,
        content: porthmos_core::preview::PreviewContent::Text("hello\nworld\n".to_string()),
    });
    harness.feed(Event::PreviewReady { request_id, name: "a.txt".to_string(), result: Ok(text) });
    drop(harness.app.view_of(harness.search_id().unwrap()));
    harness.feed(Event::SearchFound(Entry { is_dir: true, ..hit("/data/sub") }));
    named(&mut harness, Named::ArrowDown);
    drop(harness.app.view_of(harness.search_id().unwrap()));
    harness.feed(Event::PreviewReady { request_id, name: "a.txt".to_string(), result: Err("denied".to_string()) });
    drop(harness.app.view_of(harness.search_id().unwrap()));
}

fn search_windows(harness: &Harness) -> usize {
    usize::from(harness.search_id().is_some())
}

fn resize_search(harness: &mut Harness, size: iced::Size) {
    let id = harness.search_id().expect("the search window is open");
    harness.press(Message::WindowSized(id, size));
}

#[test]
fn the_action_opens_one_window_with_state_for_the_active_panel() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::Search));

    assert_eq!(search_windows(&harness), 1);
    assert_eq!(search_state(&harness).location(), Location::Local);
    assert_eq!(search_state(&harness).root(), &harness.start_folder());
}

#[test]
fn opening_search_again_raises_the_window_and_keeps_the_pattern() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "abc");
    let id = harness.search_id();
    harness.sent();

    let units = harness.app.update(Message::ActionRequested(Action::Search)).units();

    assert_eq!(units, 2);
    assert_eq!(harness.search_id(), id);
    assert_eq!(search_state(&harness).pattern(), "abc");
    assert!(harness.sent().is_empty());
}

#[test]
fn opening_search_again_after_the_panel_moved_keeps_the_search() {
    let mut harness = remote_harness();
    ctrl(&mut harness, "f");
    search(&mut harness, "a");
    let id = harness.search_id();
    named_in_main(&mut harness, Named::Tab);
    harness.sent();

    ctrl(&mut harness, "f");

    assert_eq!(harness.search_id(), id);
    assert_eq!(search_state(&harness).location(), Location::Session(1));
    assert_eq!(search_state(&harness).pattern(), "a");
    assert!(harness.sent().is_empty());
}

#[test]
fn the_window_close_request_closes_it_like_escape() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();
    let id = harness.search_id().unwrap();

    harness.press(Message::WindowCloseRequested(id));

    assert!(harness.app.search_state().is_none());
    assert!(harness.search_id().is_none());
    assert_eq!(harness.sent(), vec![Command::CancelSearch]);
}

#[test]
fn reopening_after_a_close_starts_clean_in_a_new_window() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    let first = harness.search_id();
    named(&mut harness, Named::Escape);

    open_search(&mut harness);

    assert_ne!(harness.search_id(), first);
    assert_eq!(search_state(&harness).pattern(), "");
    assert_eq!(harness.app.search_scroll(), 0.0);
}

#[test]
fn keys_in_the_search_window_do_not_reach_the_main_window() {
    let mut harness = Harness::new();
    let folder = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: folder,
        entries: vec![hit("/home/tester/a.txt"), hit("/home/tester/b.txt")],
    });
    open_search(&mut harness);
    let cursor = harness.app.local().panel.cursor();

    named(&mut harness, Named::ArrowDown);
    named(&mut harness, Named::F7);

    assert_eq!(harness.app.local().panel.cursor(), cursor);
    assert!(harness.app.modal().is_none());
}

#[test]
fn keys_in_the_main_window_do_not_move_the_search_selection_or_close_it() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/home/tester/a1");
    found(&mut harness, "/home/tester/a2");

    named_in_main(&mut harness, Named::ArrowDown);
    named_in_main(&mut harness, Named::Escape);

    assert_eq!(search_state(&harness).selected().unwrap().name, "a1");
    assert_eq!(search_windows(&harness), 1);
}

#[test]
fn preview_answers_after_the_window_closed_are_ignored() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    let (request_id, _) = hints(harness.sent()).remove(0);
    named(&mut harness, Named::Escape);

    harness.feed(Event::PreviewReady { request_id, name: "a.txt".to_string(), result: Err("late".to_string()) });

    assert!(harness.app.search_state().is_none());
}

#[test]
fn opening_another_tool_leaves_search_open_and_sends_no_cancel() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    harness.sent();

    harness.app.open_bookmarks();

    assert!(matches!(harness.app.tool(), Some(crate::tools::Tool::Bookmarks(_))));
    assert!(harness.app.search_state().is_some());
    assert!(harness.sent().is_empty());
}

#[test]
fn opening_search_leaves_another_tool_open() {
    let mut harness = Harness::new();
    harness.app.open_bookmarks();

    harness.press(Message::ActionRequested(Action::Search));

    assert!(matches!(harness.app.tool(), Some(crate::tools::Tool::Bookmarks(_))));
    assert!(harness.app.search_state().is_some());
}

#[test]
fn the_list_viewport_follows_the_search_window_size_not_the_main_one() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    for index in 0..60 {
        found(&mut harness, &format!("/home/tester/a{index}"));
    }
    harness.resize(iced::Size::new(1600.0, 1600.0));
    resize_search(&mut harness, iced::Size::new(720.0, 360.0));

    for _ in 0..15 {
        named(&mut harness, Named::ArrowDown);
    }

    assert!(harness.app.search_scroll() > 0.0);
}

#[test]
fn a_tall_search_window_shows_more_rows_before_scrolling() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    for index in 0..60 {
        found(&mut harness, &format!("/home/tester/a{index}"));
    }
    resize_search(&mut harness, iced::Size::new(720.0, 1600.0));

    for _ in 0..15 {
        named(&mut harness, Named::ArrowDown);
    }

    assert_eq!(harness.app.search_scroll(), 0.0);
}

#[test]
fn a_new_search_window_focuses_its_input() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Search));
    let id = harness.search_id().unwrap();

    let units = harness.app.update(Message::WindowOpened(id)).units();

    assert_eq!(units, 1);
}

#[test]
fn opening_a_result_raises_the_main_window_and_keeps_the_search_window_selected() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/home/tester/deep/a.txt");
    let id = harness.search_id().unwrap();
    let press = KeyPress {
        key: Key::Named(Named::Enter),
        modified_key: Key::Named(Named::Enter),
        modifiers: Modifiers::empty(),
        text: None,
    };

    let units = harness.app.update(Message::WindowKey(id, press)).units();

    assert_eq!(units, 1);
    assert_eq!(search_state(&harness).selected().unwrap().name, "a.txt");
    assert_eq!(harness.search_id(), Some(id));
}

#[test]
fn the_preview_tool_and_the_search_pane_keep_their_own_text() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    found(&mut harness, "/data/a.txt");
    let (search_request, _) = hints(harness.sent()).remove(0);
    harness.app.open_bookmarks();

    harness.feed(Event::PreviewReady {
        request_id: search_request,
        name: "a.txt".to_string(),
        result: Err("for the pane".to_string()),
    });

    assert!(matches!(harness.app.tool(), Some(crate::tools::Tool::Bookmarks(_))));
    assert!(matches!(
        search_state(&harness).preview(),
        Some(crate::search::SearchPreview::File(file)) if !file.is_loading()
    ));
}

fn open_preview_tool(harness: &mut Harness) {
    harness
        .app
        .open_tool(crate::tools::Tool::Preview(crate::preview_view::PreviewState::loading(900, "x".to_string())));
    harness.sent();
}

fn selected_search(harness: &mut Harness) {
    open_search(harness);
    search(harness, "a");
    found(harness, "/data/a.txt");
    harness.sent();
}

#[test]
fn no_hint_is_sent_while_the_preview_tool_is_open() {
    let mut harness = previewing_harness();
    open_search(&mut harness);
    search(&mut harness, "a");
    open_preview_tool(&mut harness);

    found(&mut harness, "/data/a.txt");

    assert!(hints(harness.sent()).is_empty());
    assert!(search_state(&harness).preview().is_none());
}

#[test]
fn search_work_does_not_cancel_the_preview_tool() {
    let mut harness = previewing_harness();
    selected_search(&mut harness);
    open_preview_tool(&mut harness);

    search(&mut harness, "b");
    named(&mut harness, Named::Escape);

    assert!(!harness.sent().contains(&Command::CancelPreview));
}

#[test]
fn opening_the_preview_tool_clears_the_pane_and_closing_it_hints_the_result_again() {
    let mut harness = previewing_harness();
    selected_search(&mut harness);

    open_preview_tool(&mut harness);
    assert!(search_state(&harness).preview().is_none());
    harness.app.close_tool();

    let sent = hints(harness.sent());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, VfsPath::new("/data/a.txt"));
    assert!(search_state(&harness).preview().is_some());
}

fn typed_in_search(harness: &mut Harness, text: &str, modifiers: Modifiers) -> usize {
    let id = harness.search_id().unwrap();
    let key = Key::Character(text.into());
    let press = KeyPress { key: key.clone(), modified_key: key, modifiers, text: Some(text.to_string()) };
    harness.app.update(Message::WindowKey(id, press)).units()
}

#[test]
fn a_character_that_reaches_the_window_unfocused_is_added_to_the_pattern_and_refocuses_the_input() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "ab");
    harness.sent();

    let units = typed_in_search(&mut harness, "x", Modifiers::empty());

    assert_eq!(search_state(&harness).pattern(), "abx");
    assert_eq!(units, 2);
    assert_eq!(
        harness.sent(),
        vec![Command::Search { location: Location::Local, root: harness.start_folder(), pattern: "*abx*".to_string() }]
    );
}

#[test]
fn a_shortcut_in_the_search_window_adds_no_text() {
    let mut harness = Harness::new();
    open_search(&mut harness);
    search(&mut harness, "ab");
    harness.sent();

    let units = typed_in_search(&mut harness, "f", Modifiers::CTRL);

    assert_eq!(search_state(&harness).pattern(), "ab");
    assert_eq!(units, 0);
    assert!(harness.sent().is_empty());
}

#[test]
fn turning_the_search_preview_setting_off_clears_an_open_pane() {
    let mut harness = previewing_harness();
    selected_search(&mut harness);
    assert!(search_state(&harness).preview().is_some());

    harness.press(Message::SettingsSearchPreviewToggled(false));

    assert!(!search_state(&harness).preview_enabled());
    assert!(search_state(&harness).preview().is_none());
    assert!(harness.sent().contains(&Command::CancelPreview));
}

#[test]
fn turning_the_search_preview_setting_on_hints_the_selected_result() {
    let mut harness = Harness::new();
    selected_search(&mut harness);
    assert!(search_state(&harness).preview().is_none());

    harness.press(Message::SettingsSearchPreviewToggled(true));

    assert!(search_state(&harness).preview_enabled());
    assert_eq!(hints(harness.sent()).len(), 1);
}
