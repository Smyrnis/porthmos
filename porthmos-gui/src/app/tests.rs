use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::listing::{SortKey, SortOrder};
use porthmos_core::{Command, Event, Location, Severity};

use crate::geometry::{ROW_HEIGHT, list_viewport};
use crate::shortcuts::KeyPress;
use crate::testing::Harness;

use super::*;

#[test]
fn starting_the_app_asks_core_to_list_the_local_start_folder() {
    let mut harness = Harness::new();

    let sent = harness.sent();

    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(matches!(
        &sent[0],
        Command::List { location: Location::Local, path: Some(path) } if path == &harness.start_folder()
    ));
}

#[test]
fn a_core_event_is_applied_by_update() {
    let mut harness = Harness::new();

    harness.feed(Event::Notice { severity: Severity::Info, message: "hello".to_string() });

    assert_eq!(harness.app.notices().len(), 1);
    assert_eq!(harness.app.notices()[0].message, "hello");
}

#[test]
fn closing_the_window_marks_the_app_as_closing() {
    let mut harness = Harness::new();
    assert!(!harness.app.is_closing());

    harness.press(Message::CloseRequested);

    assert!(harness.app.is_closing());
}

fn entry(folder: &VfsPath, name: &str) -> porthmos_core::Entry {
    porthmos_core::Entry { name: name.to_string(), path: folder.join(name), ..Default::default() }
}

fn connect(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
}

fn list_session(harness: &mut Harness, session: u64, folder: &str, names: &[&str]) {
    let path = VfsPath::new(folder);
    let entries = names.iter().map(|name| entry(&path, name)).collect();
    harness.feed(Event::Listed { location: Location::Session(session), path, entries });
}

fn tab_names(harness: &Harness) -> Vec<String> {
    harness.app.sessions().names().into_iter().map(|tab| tab.name).collect()
}

fn active_name(harness: &Harness) -> Option<String> {
    harness.app.sessions().names().into_iter().find(|tab| tab.active).map(|tab| tab.name)
}

#[test]
fn a_connected_event_adds_a_tab_and_makes_it_active_and_lists_its_start_folder() {
    let mut harness = Harness::new();
    harness.sent();

    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv/start", &["a.txt", "b.txt"]);

    assert_eq!(tab_names(&harness), ["alpha"]);
    assert_eq!(active_name(&harness).as_deref(), Some("alpha"));
    let right = harness.app.right_panel().expect("the active session has a panel");
    assert_eq!(right.path(), &VfsPath::new("/srv/start"));
    assert_eq!(right.rows().len(), 3);
    assert!(harness.sent().is_empty());
}

#[test]
fn a_second_connection_becomes_the_active_tab() {
    let mut harness = Harness::new();

    connect(&mut harness, 1, "alpha");
    connect(&mut harness, 2, "beta");

    assert_eq!(tab_names(&harness), ["alpha", "beta"]);
    assert_eq!(active_name(&harness).as_deref(), Some("beta"));
}

#[test]
fn connecting_to_an_open_session_again_selects_its_tab_without_a_second_one() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    connect(&mut harness, 2, "beta");

    connect(&mut harness, 1, "alpha");

    assert_eq!(tab_names(&harness), ["alpha", "beta"]);
    assert_eq!(active_name(&harness).as_deref(), Some("alpha"));
}

#[test]
fn a_listing_for_the_local_location_replaces_the_left_panel() {
    let mut harness = Harness::new();
    let folder = harness.start_folder();

    harness.feed(Event::Listed {
        location: Location::Local,
        path: folder.clone(),
        entries: vec![entry(&folder, "notes.txt")],
    });

    assert_eq!(harness.app.local().panel.path(), &folder);
    assert_eq!(harness.app.local().panel.rows().len(), 2);
}

#[test]
fn a_listing_for_a_session_that_is_gone_is_dropped() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    list_session(&mut harness, 1, "/late", &["x"]);

    assert!(harness.app.sessions().is_empty());
    assert!(harness.app.right_panel().is_none());
}

#[test]
fn a_listing_for_a_folder_the_panel_already_left_is_dropped() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    harness.app.list(Location::Local, Some(VfsPath::new("/first")));
    harness.app.list(Location::Local, Some(VfsPath::new("/second")));

    harness.feed(Event::Listed { location: Location::Local, path: VfsPath::new("/first"), entries: Vec::new() });
    assert_eq!(harness.app.local().panel.path(), &start);

    harness.feed(Event::Listed { location: Location::Local, path: VfsPath::new("/second"), entries: Vec::new() });
    assert_eq!(harness.app.local().panel.path(), &VfsPath::new("/second"));
}

#[test]
fn listing_a_folder_asks_core_and_remembers_the_request() {
    let mut harness = Harness::new();
    harness.sent();
    connect(&mut harness, 1, "alpha");

    harness.app.list(Location::Session(1), Some(VfsPath::new("/x")));

    assert_eq!(harness.sent(), vec![Command::List { location: Location::Session(1), path: Some(VfsPath::new("/x")) }]);
}

#[test]
fn disconnecting_the_active_tab_activates_its_neighbour() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    connect(&mut harness, 2, "beta");
    connect(&mut harness, 3, "gamma");
    harness.press(Message::TabClicked(1));

    harness.feed(Event::Disconnected { session: 2, name: "beta".to_string() });

    assert_eq!(tab_names(&harness), ["alpha", "gamma"]);
    assert_eq!(active_name(&harness).as_deref(), Some("gamma"));
}

#[test]
fn disconnecting_the_last_tab_empties_the_right_panel() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &["a"]);
    assert!(harness.app.right_panel().is_some());

    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    assert!(harness.app.right_panel().is_none());
    assert!(harness.app.sessions().is_empty());
}

#[test]
fn a_location_changed_event_relists_that_location() {
    let mut harness = Harness::new();
    harness.sent();
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &[]);

    harness.feed(Event::LocationChanged { location: Location::Session(1) });
    harness.feed(Event::LocationChanged { location: Location::Local });
    harness.feed(Event::LocationChanged { location: Location::Session(9) });

    assert_eq!(
        harness.sent(),
        vec![
            Command::List { location: Location::Session(1), path: Some(VfsPath::new("/srv")) },
            Command::List { location: Location::Local, path: Some(harness.start_folder()) },
        ]
    );
}

#[test]
fn free_space_for_an_old_path_is_ignored() {
    let mut harness = Harness::new();
    let folder = harness.start_folder();

    harness.feed(Event::FreeSpace { location: Location::Local, path: VfsPath::new("/old"), bytes: Some(5) });
    assert_eq!(harness.app.local().panel.free_bytes(), None);

    harness.feed(Event::FreeSpace { location: Location::Local, path: folder, bytes: Some(9) });
    assert_eq!(harness.app.local().panel.free_bytes(), Some(9));
}

#[test]
fn a_failed_connection_adds_an_error_notice_and_no_tab() {
    let mut harness = Harness::new();
    harness.feed(Event::Connecting { name: "alpha".to_string() });
    assert_eq!(harness.app.connecting(), Some("alpha"));

    harness.feed(Event::ConnectFailed { name: "alpha".to_string(), message: "refused".to_string() });

    assert!(harness.app.sessions().is_empty());
    assert_eq!(harness.app.connecting(), None);
    let notice = harness.app.notices().last().unwrap();
    assert_eq!((notice.severity, notice.message.as_str()), (Severity::Error, "refused"));
}

#[test]
fn connecting_is_cleared_when_the_connection_succeeds() {
    let mut harness = Harness::new();
    harness.feed(Event::Connecting { name: "alpha".to_string() });

    connect(&mut harness, 1, "alpha");

    assert_eq!(harness.app.connecting(), None);
}

#[test]
fn notices_keep_the_newest_fifty() {
    let mut harness = Harness::new();

    for number in 0..60 {
        harness.feed(Event::Notice { severity: Severity::Info, message: format!("n{number}") });
    }

    let notices = harness.app.notices();
    assert_eq!(notices.len(), 50);
    assert_eq!(notices.first().unwrap().message, "n10");
    assert_eq!(notices.last().unwrap().message, "n59");
}

#[test]
fn session_health_marks_a_tab_unhealthy() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");

    harness.feed(Event::SessionHealth { session: 1, healthy: false });
    assert!(!harness.app.sessions().names()[0].healthy);

    harness.feed(Event::SessionHealth { session: 1, healthy: true });
    assert!(harness.app.sessions().names()[0].healthy);
}

#[test]
fn session_capabilities_are_recorded() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");

    harness.feed(Event::SessionCapabilities { session: 1, preserves_times: true });

    assert!(harness.app.sessions().by_id(1).unwrap().preserves_times);
}

fn fill_local(harness: &mut Harness, names: &[&str]) {
    let folder = harness.start_folder();
    let entries = names.iter().map(|name| entry(&folder, name)).collect();
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries });
}

fn selected_local(harness: &Harness) -> Vec<String> {
    harness.app.local().panel.selected().into_iter().map(|entry| entry.name).collect()
}

#[test]
fn clicking_a_row_selects_it_and_makes_that_panel_active() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c"]);
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &["x", "y"]);

    harness.press(Message::PanelClicked(PanelSide::Remote, 2));
    assert_eq!(harness.app.active_side(), PanelSide::Remote);
    assert_eq!(harness.app.right_panel().unwrap().selection_count(), 1);

    harness.press(Message::PanelClicked(PanelSide::Local, 3));
    assert_eq!(harness.app.active_side(), PanelSide::Local);
    assert_eq!(selected_local(&harness), ["c"]);
}

#[test]
fn clicking_the_right_panel_without_a_connection_does_nothing() {
    let mut harness = Harness::new();

    harness.press(Message::PanelClicked(PanelSide::Remote, 0));

    assert_eq!(harness.app.active_side(), PanelSide::Local);
}

#[test]
fn a_click_with_the_command_key_held_toggles_instead_of_replacing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));

    harness.press(Message::ModifiersChanged(Modifiers::COMMAND));
    harness.press(Message::PanelClicked(PanelSide::Local, 3));
    assert_eq!(selected_local(&harness), ["a", "c"]);

    harness.press(Message::ModifiersChanged(Modifiers::empty()));
    harness.press(Message::PanelClicked(PanelSide::Local, 2));
    assert_eq!(selected_local(&harness), ["b"]);
}

#[test]
fn a_click_with_shift_held_selects_a_range() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c", "d"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));

    harness.press(Message::ModifiersChanged(Modifiers::SHIFT));
    harness.press(Message::PanelClicked(PanelSide::Local, 3));

    assert_eq!(selected_local(&harness), ["a", "b", "c"]);
}

#[test]
fn the_sort_header_click_cycles_direction_on_the_second_click() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);

    harness.press(Message::HeaderClicked(PanelSide::Local, SortKey::Size));
    let spec = harness.app.local().panel.sort_spec();
    assert_eq!((spec.key, spec.order), (SortKey::Size, SortOrder::Ascending));

    harness.press(Message::HeaderClicked(PanelSide::Local, SortKey::Size));
    let spec = harness.app.local().panel.sort_spec();
    assert_eq!((spec.key, spec.order), (SortKey::Size, SortOrder::Descending));
}

#[test]
fn clicking_a_tab_activates_that_connection() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    connect(&mut harness, 2, "beta");

    harness.press(Message::TabClicked(0));
    assert_eq!(active_name(&harness).as_deref(), Some("alpha"));

    harness.press(Message::TabClicked(9));
    assert_eq!(active_name(&harness).as_deref(), Some("alpha"));
}

#[test]
fn scrolling_is_remembered_per_panel() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    connect(&mut harness, 2, "beta");

    harness.press(Message::ScrollChanged(PanelSide::Local, 480.0));
    harness.press(Message::ScrollChanged(PanelSide::Remote, 96.0));
    harness.press(Message::TabClicked(0));
    harness.press(Message::ScrollChanged(PanelSide::Remote, 24.0));
    harness.press(Message::TabClicked(1));

    assert_eq!(harness.app.local().scroll, 480.0);
    assert_eq!(harness.app.sessions().by_id(2).unwrap().pane.scroll, 96.0);
    assert_eq!(harness.app.sessions().by_id(1).unwrap().pane.scroll, 24.0);
}

#[test]
fn a_nonsense_scroll_offset_is_clamped_to_the_top() {
    let mut harness = Harness::new();

    harness.press(Message::ScrollChanged(PanelSide::Local, f32::NAN));
    assert_eq!(harness.app.local().scroll, 0.0);

    harness.press(Message::ScrollChanged(PanelSide::Local, -30.0));
    assert_eq!(harness.app.local().scroll, 0.0);
}

fn key(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    let press = KeyPress { key: key.clone(), modified_key: key, modifiers, text: None };
    harness.press(Message::KeyPressed(press));
}

fn plain(harness: &mut Harness, named: Named) {
    key(harness, Key::Named(named), Modifiers::empty());
}

fn letter(harness: &mut Harness, text: &str) {
    let key = Key::Character(text.into());
    let press =
        KeyPress { key: key.clone(), modified_key: key, modifiers: Modifiers::empty(), text: Some(text.to_string()) };
    harness.press(Message::KeyPressed(press));
}

fn type_text(harness: &mut Harness, text: &str) {
    for character in text.chars() {
        letter(harness, &character.to_string());
    }
}

fn folder_entry(folder: &VfsPath, name: &str) -> porthmos_core::Entry {
    porthmos_core::Entry { is_dir: true, ..entry(folder, name) }
}

fn fill_local_with(harness: &mut Harness, entries: Vec<porthmos_core::Entry>) {
    let folder = harness.start_folder();
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries });
}

fn cursor_name(harness: &Harness, side: PanelSide) -> String {
    let panel = match side {
        PanelSide::Local => &harness.app.local().panel,
        PanelSide::Remote => harness.app.right_panel().unwrap(),
    };
    match &panel.rows()[panel.cursor()] {
        porthmos_core::listing::Row::Parent => "..".to_string(),
        porthmos_core::listing::Row::Entry(entry) => entry.name.clone(),
    }
}

fn list_of(location: Location, path: &str) -> Command {
    Command::List { location, path: Some(VfsPath::new(path)) }
}

#[test]
fn double_clicking_a_folder_asks_core_to_list_it() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    fill_local_with(&mut harness, vec![folder_entry(&start, "sub"), entry(&start, "note")]);
    harness.sent();

    harness.press(Message::PanelDoubleClicked(PanelSide::Local, 1));

    assert_eq!(harness.sent(), vec![list_of(Location::Local, "/home/tester/sub")]);
}

#[test]
fn double_clicking_a_file_does_nothing_in_stage_one() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    fill_local_with(&mut harness, vec![entry(&start, "note")]);
    harness.sent();

    harness.press(Message::PanelDoubleClicked(PanelSide::Local, 1));

    assert!(harness.sent().is_empty());
}

#[test]
fn the_panel_shows_the_new_folder_only_when_the_listing_arrives() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    fill_local_with(&mut harness, vec![folder_entry(&start, "sub")]);

    harness.press(Message::PanelDoubleClicked(PanelSide::Local, 1));
    assert_eq!(harness.app.local().panel.path(), &start);

    harness.feed(Event::Listed {
        location: Location::Local,
        path: VfsPath::new("/home/tester/sub"),
        entries: Vec::new(),
    });
    assert_eq!(harness.app.local().panel.path(), &VfsPath::new("/home/tester/sub"));
}

#[test]
fn enter_on_the_parent_row_lists_the_parent() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.sent();

    plain(&mut harness, Named::Enter);

    assert_eq!(harness.sent(), vec![list_of(Location::Local, "/home")]);
}

#[test]
fn enter_on_a_folder_lists_it_and_on_a_file_does_nothing_in_stage_one() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    fill_local_with(&mut harness, vec![folder_entry(&start, "sub"), entry(&start, "note")]);
    harness.sent();

    plain(&mut harness, Named::ArrowDown);
    plain(&mut harness, Named::Enter);
    assert_eq!(harness.sent(), vec![list_of(Location::Local, "/home/tester/sub")]);

    plain(&mut harness, Named::ArrowDown);
    plain(&mut harness, Named::Enter);
    assert!(harness.sent().is_empty());
}

#[test]
fn backspace_lists_the_parent_folder() {
    let mut harness = Harness::new();
    harness.sent();

    plain(&mut harness, Named::Backspace);

    assert_eq!(harness.sent(), vec![list_of(Location::Local, "/home")]);
}

#[test]
fn backspace_at_the_root_does_nothing() {
    let mut harness = Harness::new();
    harness.feed(Event::Listed { location: Location::Local, path: VfsPath::new("/home/tester"), entries: Vec::new() });
    harness.app.list(Location::Local, Some(VfsPath::root()));
    harness.feed(Event::Listed { location: Location::Local, path: VfsPath::root(), entries: Vec::new() });
    harness.sent();

    plain(&mut harness, Named::Backspace);

    assert!(harness.sent().is_empty());
}

#[test]
fn arrows_and_vim_keys_move_the_cursor_inside_the_rows() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c"]);

    plain(&mut harness, Named::ArrowDown);
    assert_eq!(cursor_name(&harness, PanelSide::Local), "a");
    letter(&mut harness, "j");
    assert_eq!(cursor_name(&harness, PanelSide::Local), "b");
    letter(&mut harness, "k");
    assert_eq!(cursor_name(&harness, PanelSide::Local), "a");
    plain(&mut harness, Named::End);
    assert_eq!(cursor_name(&harness, PanelSide::Local), "c");
    plain(&mut harness, Named::ArrowDown);
    assert_eq!(cursor_name(&harness, PanelSide::Local), "c");
    plain(&mut harness, Named::Home);
    assert_eq!(cursor_name(&harness, PanelSide::Local), "..");
    plain(&mut harness, Named::ArrowUp);
    assert_eq!(cursor_name(&harness, PanelSide::Local), "..");
}

#[test]
fn page_keys_move_by_about_a_screen() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..200).map(|index| format!("f{index:03}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_local(&mut harness, &names);
    let page = (list_viewport(700.0) / ROW_HEIGHT) as usize - 1;

    plain(&mut harness, Named::PageDown);
    assert_eq!(harness.app.local().panel.cursor(), page);

    plain(&mut harness, Named::PageUp);
    assert_eq!(harness.app.local().panel.cursor(), 0);
}

#[test]
fn moving_the_cursor_past_the_bottom_scrolls_the_list() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..100).map(|index| format!("f{index:03}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_local(&mut harness, &names);

    plain(&mut harness, Named::End);

    let expected = 101.0 * ROW_HEIGHT - list_viewport(700.0);
    assert_eq!(harness.app.local().scroll, expected);

    plain(&mut harness, Named::Home);
    assert_eq!(harness.app.local().scroll, 0.0);
}

#[test]
fn resizing_the_window_changes_how_far_the_list_scrolls() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..100).map(|index| format!("f{index:03}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_local(&mut harness, &names);
    harness.press(Message::WindowResized(iced::Size::new(800.0, 334.0)));

    plain(&mut harness, Named::End);

    assert_eq!(harness.app.local().scroll, 101.0 * ROW_HEIGHT - list_viewport(334.0));
}

#[test]
fn tab_switches_the_active_panel() {
    let mut harness = Harness::new();

    plain(&mut harness, Named::Tab);
    assert_eq!(harness.app.active_side(), PanelSide::Local);

    connect(&mut harness, 1, "alpha");
    plain(&mut harness, Named::Tab);
    assert_eq!(harness.app.active_side(), PanelSide::Remote);

    plain(&mut harness, Named::Tab);
    assert_eq!(harness.app.active_side(), PanelSide::Local);
}

#[test]
fn keys_act_on_the_active_panel() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &["x", "y", "z"]);
    plain(&mut harness, Named::Tab);

    plain(&mut harness, Named::End);

    assert_eq!(cursor_name(&harness, PanelSide::Remote), "z");
    assert_eq!(cursor_name(&harness, PanelSide::Local), "..");
}

#[test]
fn ctrl_h_toggles_hidden_in_the_active_panel_only() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &[".dot", "a"]);
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &[".dot", "x"]);
    assert_eq!(harness.app.local().panel.rows().len(), 2);

    key(&mut harness, Key::Character("h".into()), Modifiers::CTRL);

    assert!(harness.app.local().panel.show_hidden());
    assert_eq!(harness.app.local().panel.rows().len(), 3);
    assert!(!harness.app.right_panel().unwrap().show_hidden());
    assert_eq!(harness.app.right_panel().unwrap().rows().len(), 2);
}

#[test]
fn the_sort_key_cycles_the_active_panel() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);

    key(&mut harness, Key::Character("s".into()), Modifiers::CTRL);

    let spec = harness.app.local().panel.sort_spec();
    assert_eq!((spec.key, spec.order), (SortKey::Name, SortOrder::Descending));
}

#[test]
fn space_toggles_the_row_under_the_cursor_and_select_all_takes_every_entry() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c"]);
    plain(&mut harness, Named::ArrowDown);

    plain(&mut harness, Named::Space);
    assert_eq!(selected_local(&harness), ["a"]);
    plain(&mut harness, Named::Space);
    assert!(selected_local(&harness).is_empty());

    key(&mut harness, Key::Character("a".into()), Modifiers::COMMAND);
    assert_eq!(selected_local(&harness), ["a", "b", "c"]);
}

#[test]
fn typing_in_the_filter_narrows_the_active_panel() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["alpha", "beta", "alpine"]);

    letter(&mut harness, "/");
    assert!(harness.app.filter_editing_side().is_some());
    type_text(&mut harness, "alp");

    assert_eq!(harness.app.local().panel.filter(), Some("alp"));
    assert_eq!(harness.app.local().panel.match_count(), (2, 3));

    plain(&mut harness, Named::Backspace);
    assert_eq!(harness.app.local().panel.filter(), Some("al"));
}

#[test]
fn while_filtering_letters_are_typed_instead_of_navigating() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["jam", "kit"]);
    harness.sent();

    letter(&mut harness, "/");
    letter(&mut harness, "j");

    assert_eq!(harness.app.local().panel.filter(), Some("j"));
    assert_eq!(cursor_name(&harness, PanelSide::Local), "..");

    plain(&mut harness, Named::Enter);
    assert!(!harness.app.filter_editing_side().is_some());
    assert_eq!(harness.app.local().panel.filter(), Some("j"));
    assert!(harness.sent().is_empty());

    letter(&mut harness, "j");
    assert_eq!(cursor_name(&harness, PanelSide::Local), "jam");
}

#[test]
fn escape_clears_the_filter_first_and_then_nothing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["alpha", "beta"]);
    letter(&mut harness, "/");
    type_text(&mut harness, "al");

    plain(&mut harness, Named::Escape);
    assert!(!harness.app.filter_editing_side().is_some());
    assert_eq!(harness.app.local().panel.filter(), None);
    assert_eq!(harness.app.local().panel.match_count(), (2, 2));

    letter(&mut harness, "/");
    plain(&mut harness, Named::Enter);
    type_text(&mut harness, "zz");
    assert_eq!(harness.app.local().panel.filter(), None);
}

#[test]
fn escape_after_confirming_a_filter_clears_it() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["alpha", "beta"]);
    letter(&mut harness, "/");
    type_text(&mut harness, "al");
    plain(&mut harness, Named::Enter);

    plain(&mut harness, Named::Escape);

    assert_eq!(harness.app.local().panel.filter(), None);
}

#[test]
fn control_characters_and_shortcuts_are_not_typed_into_the_filter() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["alpha"]);
    letter(&mut harness, "/");

    let press = KeyPress {
        key: Key::Character("h".into()),
        modified_key: Key::Character("h".into()),
        modifiers: Modifiers::CTRL,
        text: Some("\u{8}".to_string()),
    };
    harness.press(Message::KeyPressed(press));
    let tab = KeyPress {
        key: Key::Named(Named::Tab),
        modified_key: Key::Named(Named::Tab),
        modifiers: Modifiers::empty(),
        text: Some("\t".to_string()),
    };
    harness.press(Message::KeyPressed(tab));

    assert_eq!(harness.app.local().panel.filter(), None);
    assert!(!harness.app.local().panel.show_hidden());
}

#[test]
fn a_new_filter_starts_from_the_text_already_applied() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["alpha", "beta"]);
    letter(&mut harness, "/");
    type_text(&mut harness, "al");
    plain(&mut harness, Named::Enter);

    letter(&mut harness, "/");
    type_text(&mut harness, "p");

    assert_eq!(harness.app.local().panel.filter(), Some("alp"));
}

#[test]
fn refresh_relists_the_active_location() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &[]);
    harness.sent();

    key(&mut harness, Key::Character("r".into()), Modifiers::CTRL);
    assert_eq!(harness.sent(), vec![list_of(Location::Local, "/home/tester")]);

    plain(&mut harness, Named::Tab);
    key(&mut harness, Key::Character("r".into()), Modifiers::CTRL);
    assert_eq!(harness.sent(), vec![list_of(Location::Session(1), "/srv")]);
}

#[test]
fn the_tab_keys_walk_through_the_connections() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    connect(&mut harness, 2, "beta");

    key(&mut harness, Key::Character("n".into()), Modifiers::CTRL);
    assert_eq!(active_name(&harness).as_deref(), Some("alpha"));
}

#[test]
fn the_quit_shortcut_closes_like_the_window_button() {
    let mut harness = Harness::new();

    key(&mut harness, Key::Character("q".into()), Modifiers::COMMAND);

    assert!(harness.app.is_closing());
}

#[test]
fn keys_are_ignored_once_the_app_is_closing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::CloseRequested);

    plain(&mut harness, Named::End);

    assert_eq!(harness.app.local().panel.cursor(), 0);
}

#[test]
fn disconnecting_the_last_session_returns_focus_to_the_local_panel() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    plain(&mut harness, Named::Tab);
    assert_eq!(harness.app.active_side(), PanelSide::Remote);

    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    assert_eq!(harness.app.active_side(), PanelSide::Local);
}

fn question(request_id: u64, question: porthmos_core::Question) -> Event {
    Event::Question { request_id, question }
}

fn password_question() -> porthmos_core::Question {
    porthmos_core::Question::Password { username: "me".to_string(), name: "alpha".to_string() }
}

fn host_question() -> porthmos_core::Question {
    porthmos_core::Question::TrustHostKey {
        name: "alpha".to_string(),
        host: "h".to_string(),
        port: 22,
        key_type: "ssh-ed25519".to_string(),
        fingerprint: "SHA256:x".to_string(),
    }
}

#[test]
fn a_password_question_is_answered_with_the_typed_text() {
    let mut harness = Harness::new();
    harness.sent();
    harness.feed(question(9, password_question()));
    assert!(harness.app.dialog().is_some());

    harness.press(Message::DialogTyped(crate::dialogs::Typed::new("s3cret".to_string())));
    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![Command::Answer {
            request_id: 9,
            answer: Some(porthmos_core::Answer::Password("s3cret".to_string())),
            save: false
        }]
    );
    assert!(harness.app.dialog().is_none());
}

#[test]
fn a_yes_no_question_answers_confirmed_or_none() {
    let mut harness = Harness::new();
    harness.sent();

    harness.feed(question(1, host_question()));
    harness.press(Message::ModalAccepted);
    harness.feed(question(2, host_question()));
    harness.press(Message::ModalDeclined);

    assert_eq!(
        harness.sent(),
        vec![
            Command::Answer { request_id: 1, answer: Some(porthmos_core::Answer::Confirmed), save: false },
            Command::Answer { request_id: 2, answer: None, save: false },
        ]
    );
}

#[test]
fn cancelling_a_password_question_answers_none() {
    let mut harness = Harness::new();
    harness.feed(question(3, password_question()));
    harness.sent();

    plain(&mut harness, Named::Escape);

    assert_eq!(harness.sent(), vec![Command::Answer { request_id: 3, answer: None, save: false }]);
}

#[test]
fn enter_confirms_a_yes_no_question_but_not_a_password_prompt() {
    let mut harness = Harness::new();
    harness.feed(question(1, host_question()));
    harness.sent();
    plain(&mut harness, Named::Enter);
    assert_eq!(
        harness.sent(),
        vec![Command::Answer { request_id: 1, answer: Some(porthmos_core::Answer::Confirmed), save: false }]
    );

    harness.feed(question(2, password_question()));
    harness.sent();
    plain(&mut harness, Named::Enter);
    assert!(harness.sent().is_empty());
    assert!(harness.app.dialog().is_some());
}

#[test]
fn a_second_question_waits_for_the_first_to_be_answered() {
    let mut harness = Harness::new();
    harness.feed(question(1, host_question()));
    harness.feed(question(2, password_question()));
    assert_eq!(harness.app.dialog().unwrap().request_id(), 1);
    harness.sent();

    harness.press(Message::ModalDeclined);

    assert_eq!(harness.app.dialog().unwrap().request_id(), 2);
}

#[test]
fn keys_do_not_navigate_while_a_question_is_open() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    harness.feed(question(1, host_question()));

    plain(&mut harness, Named::End);
    letter(&mut harness, "/");

    assert_eq!(harness.app.local().panel.cursor(), 0);
    assert!(!harness.app.filter_editing_side().is_some());
}

#[test]
fn typed_text_goes_nowhere_without_a_password_dialog() {
    let mut harness = Harness::new();
    harness.feed(question(1, host_question()));

    harness.press(Message::DialogTyped(crate::dialogs::Typed::new("x".to_string())));

    assert!(matches!(harness.app.dialog(), Some(crate::dialogs::Dialog::Confirm { .. })));
}

fn open_modal(harness: &Harness) -> &crate::modal::Modal {
    harness.app.modal().expect("a modal is open")
}

fn type_into_prompt(harness: &mut Harness, text: &str) {
    harness.press(Message::ModalTyped(text.to_string()));
}

fn select_rows(harness: &mut Harness, rows: &[usize]) {
    harness.press(Message::ModifiersChanged(Modifiers::empty()));
    harness.press(Message::PanelClicked(PanelSide::Local, rows[0]));
    harness.press(Message::ModifiersChanged(Modifiers::COMMAND));
    for row in &rows[1..] {
        harness.press(Message::PanelClicked(PanelSide::Local, *row));
    }
    harness.press(Message::ModifiersChanged(Modifiers::empty()));
}

#[test]
fn the_new_folder_key_opens_a_prompt_for_the_active_panel() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");

    plain(&mut harness, Named::F7);
    assert!(matches!(open_modal(&harness), crate::modal::Modal::NewFolder { side: PanelSide::Local, .. }));
    plain(&mut harness, Named::Escape);

    plain(&mut harness, Named::Tab);
    plain(&mut harness, Named::F7);
    assert!(matches!(open_modal(&harness), crate::modal::Modal::NewFolder { side: PanelSide::Remote, .. }));
}

#[test]
fn accepting_a_new_folder_name_sends_create_dir_in_the_panel_folder() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &[]);
    harness.sent();

    plain(&mut harness, Named::F7);
    type_into_prompt(&mut harness, "docs");
    harness.press(Message::ModalAccepted);
    plain(&mut harness, Named::Tab);
    plain(&mut harness, Named::F7);
    type_into_prompt(&mut harness, "new");
    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![
            Command::CreateDir { location: Location::Local, path: VfsPath::new("/home/tester/docs") },
            Command::CreateDir { location: Location::Session(1), path: VfsPath::new("/srv/new") },
        ]
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_bad_name_keeps_the_prompt_open_with_the_reason_and_sends_nothing() {
    let mut harness = Harness::new();
    harness.sent();
    plain(&mut harness, Named::F7);

    type_into_prompt(&mut harness, "a/b");
    harness.press(Message::ModalAccepted);

    match open_modal(&harness) {
        crate::modal::Modal::NewFolder { error: Some(reason), text, .. } => {
            assert!(reason.contains('/'), "{reason}");
            assert_eq!(text, "a/b");
        }
        other => panic!("{other:?}"),
    }
    assert!(harness.sent().is_empty());

    type_into_prompt(&mut harness, "ab");
    assert!(matches!(open_modal(&harness), crate::modal::Modal::NewFolder { error: None, .. }));
}

#[test]
fn a_name_that_already_exists_in_the_panel_is_refused_before_core_is_asked() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["docs"]);
    harness.sent();
    plain(&mut harness, Named::F7);

    type_into_prompt(&mut harness, "docs");
    harness.press(Message::ModalAccepted);

    assert!(
        matches!(open_modal(&harness), crate::modal::Modal::NewFolder { error: Some(reason), .. } if reason.contains("already exists"))
    );
    assert!(harness.sent().is_empty());
}

#[test]
fn rename_starts_with_the_cursor_name_and_sends_rename() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    harness.sent();
    plain(&mut harness, Named::ArrowDown);

    plain(&mut harness, Named::F2);
    match open_modal(&harness) {
        crate::modal::Modal::Rename { original, text, .. } => {
            assert_eq!(original.name, "a");
            assert_eq!(text, "a");
        }
        other => panic!("{other:?}"),
    }
    type_into_prompt(&mut harness, "z");
    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![Command::Rename {
            location: Location::Local,
            from: VfsPath::new("/home/tester/a"),
            to: VfsPath::new("/home/tester/z")
        }]
    );
}

#[test]
fn rename_on_the_parent_row_does_nothing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);

    plain(&mut harness, Named::F2);

    assert!(harness.app.modal().is_none());
}

#[test]
fn rename_to_the_same_name_closes_the_prompt_without_a_command() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    plain(&mut harness, Named::ArrowDown);
    harness.sent();
    plain(&mut harness, Named::F2);

    harness.press(Message::ModalAccepted);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn renaming_onto_another_existing_name_is_refused() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    plain(&mut harness, Named::ArrowDown);
    harness.sent();
    plain(&mut harness, Named::F2);

    type_into_prompt(&mut harness, "b");
    harness.press(Message::ModalAccepted);

    assert!(matches!(open_modal(&harness), crate::modal::Modal::Rename { error: Some(_), .. }));
    assert!(harness.sent().is_empty());
}

#[test]
fn delete_asks_for_confirmation_listing_a_count_and_at_most_five_names() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..8).map(|n| format!("f{n}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_local(&mut harness, &names);
    key(&mut harness, Key::Character("a".into()), Modifiers::COMMAND);

    plain(&mut harness, Named::Delete);

    match open_modal(&harness) {
        crate::modal::Modal::ConfirmDelete { entries, .. } => {
            assert_eq!(entries.len(), 8);
            let lines = crate::modal::delete_summary(entries);
            assert_eq!(lines.len(), 1 + 5 + 1);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn confirming_delete_sends_one_delete_with_every_selected_path() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c"]);
    select_rows(&mut harness, &[1, 3]);
    harness.sent();

    plain(&mut harness, Named::F8);
    plain(&mut harness, Named::Enter);

    assert_eq!(
        harness.sent(),
        vec![Command::Delete {
            location: Location::Local,
            paths: vec![VfsPath::new("/home/tester/a"), VfsPath::new("/home/tester/c")]
        }]
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn declining_delete_sends_nothing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    plain(&mut harness, Named::ArrowDown);
    harness.sent();

    plain(&mut harness, Named::Delete);
    plain(&mut harness, Named::Escape);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn delete_with_nothing_selected_uses_the_cursor_row() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    plain(&mut harness, Named::ArrowDown);
    plain(&mut harness, Named::ArrowDown);

    plain(&mut harness, Named::Delete);

    match open_modal(&harness) {
        crate::modal::Modal::ConfirmDelete { entries, .. } => {
            assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["b"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn delete_never_includes_the_parent_row() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);

    plain(&mut harness, Named::Delete);

    assert!(harness.app.modal().is_none());
}

#[test]
fn a_prompt_blocks_navigation_keys_and_escape_cancels_it() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    plain(&mut harness, Named::F7);

    plain(&mut harness, Named::End);
    letter(&mut harness, "/");
    assert_eq!(harness.app.local().panel.cursor(), 0);
    assert!(!harness.app.filter_editing_side().is_some());

    plain(&mut harness, Named::Escape);
    assert!(harness.app.modal().is_none());
}

#[test]
fn core_questions_and_prompts_queue_in_arrival_order() {
    let mut harness = Harness::new();
    plain(&mut harness, Named::F7);

    harness.feed(question(4, host_question()));
    assert!(matches!(open_modal(&harness), crate::modal::Modal::NewFolder { .. }));

    plain(&mut harness, Named::Escape);
    assert_eq!(harness.app.dialog().unwrap().request_id(), 4);
}

#[test]
fn file_actions_follow_focus_back_to_the_local_panel_when_the_connection_closes() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    plain(&mut harness, Named::Tab);
    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    plain(&mut harness, Named::F7);

    assert!(matches!(open_modal(&harness), crate::modal::Modal::NewFolder { side: PanelSide::Local, .. }));
}

#[test]
fn fifty_thousand_selected_entries_become_one_delete_command() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..50_000).map(|n| format!("f{n:05}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_local(&mut harness, &names);
    key(&mut harness, Key::Character("a".into()), Modifiers::COMMAND);
    harness.sent();

    plain(&mut harness, Named::Delete);
    plain(&mut harness, Named::Enter);

    let sent = harness.sent();
    assert_eq!(sent.len(), 1);
    assert!(matches!(&sent[0], Command::Delete { paths, .. } if paths.len() == 50_000));
}

#[test]
fn the_action_bar_buttons_run_the_same_actions_as_the_keys() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    plain(&mut harness, Named::ArrowDown);

    harness.press(Message::ActionRequested(crate::shortcuts::Action::Rename));

    assert!(matches!(open_modal(&harness), crate::modal::Modal::Rename { .. }));
}

fn upload_target(harness: &mut Harness) {
    connect(harness, 1, "alpha");
    list_session(harness, 1, "/srv/in", &["old"]);
}

#[test]
fn copy_from_local_goes_to_the_active_session_folder() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Local, 2));
    harness.sent();

    plain(&mut harness, Named::F5);

    let folder = harness.start_folder();
    assert_eq!(
        harness.sent(),
        vec![Command::Copy {
            from: Location::Local,
            entries: vec![entry(&folder, "b")],
            to: Location::Session(1),
            dest_dir: VfsPath::new("/srv/in"),
        }]
    );
}

#[test]
fn copy_from_the_session_goes_to_the_local_folder() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    list_session(&mut harness, 1, "/srv", &["x", "y"]);
    harness.press(Message::PanelClicked(PanelSide::Remote, 1));
    harness.sent();

    plain(&mut harness, Named::F5);

    assert_eq!(
        harness.sent(),
        vec![Command::Copy {
            from: Location::Session(1),
            entries: vec![entry(&VfsPath::new("/srv"), "x")],
            to: Location::Local,
            dest_dir: harness.start_folder(),
        }]
    );
}

#[test]
fn move_uses_the_move_command() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    harness.sent();

    plain(&mut harness, Named::F6);

    assert!(matches!(
        harness.sent().as_slice(),
        [Command::Move { from: Location::Local, to: Location::Session(1), .. }]
    ));
}

#[test]
fn copy_without_a_connection_says_connect_first_and_sends_nothing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    harness.sent();

    plain(&mut harness, Named::F5);

    assert!(harness.sent().is_empty());
    let notice = harness.app.notices().last().expect("a notice");
    assert_eq!(notice.severity, Severity::Info);
    assert!(notice.message.contains("Connect"), "{}", notice.message);
}

#[test]
fn copy_with_an_empty_selection_uses_the_cursor_row() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    upload_target(&mut harness);
    plain(&mut harness, Named::ArrowDown);
    plain(&mut harness, Named::ArrowDown);
    harness.sent();

    plain(&mut harness, Named::F5);

    match harness.sent().as_slice() {
        [Command::Copy { entries, .. }] => {
            assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["b"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn copy_never_sends_the_parent_row() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.sent();

    plain(&mut harness, Named::F5);

    assert!(harness.sent().is_empty());
}

#[test]
fn copy_sends_every_selected_entry() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c"]);
    upload_target(&mut harness);
    select_rows(&mut harness, &[1, 3]);
    harness.sent();

    plain(&mut harness, Named::F5);

    match harness.sent().as_slice() {
        [Command::Copy { entries, .. }] => {
            assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_toolbar_copy_and_move_buttons_run_the_same_actions_as_the_keys() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    harness.sent();

    harness.press(Message::ActionRequested(Action::Copy));
    harness.press(Message::ActionRequested(Action::Move));

    assert!(matches!(harness.sent().as_slice(), [Command::Copy { .. }, Command::Move { .. }]));
}

#[test]
fn a_transfers_event_replaces_the_snapshot() {
    use porthmos_core::transfer::TransferSnapshot;
    use porthmos_core::transfer::rows::{QueueRow, RowKind, RowState};
    let mut harness = Harness::new();
    assert_eq!(harness.app.transfer_summary(), None);
    let row = QueueRow {
        kind: RowKind::Single(1),
        label: "a".to_string(),
        direction: porthmos_core::transfer::Direction::Upload,
        files_done: 0,
        files_total: 1,
        bytes_done: 25,
        bytes_total: 100,
        state: RowState::Running,
        paused: false,
        session_id: Some(1),
        failure: None,
    };

    harness.feed(Event::TransfersChanged(TransferSnapshot { rows: vec![row], ..TransferSnapshot::default() }));
    assert_eq!(harness.app.transfer_summary().as_deref(), Some("1 transfer · 25%"));

    harness.feed(Event::TransfersChanged(TransferSnapshot::default()));
    assert_eq!(harness.app.transfer_summary(), None);
}

#[test]
fn fifty_thousand_entries_become_one_copy_command() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..50_000).map(|n| format!("f{n:05}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_local(&mut harness, &names);
    upload_target(&mut harness);
    key(&mut harness, Key::Character("a".into()), Modifiers::COMMAND);
    harness.sent();

    plain(&mut harness, Named::F5);

    let sent = harness.sent();
    assert_eq!(sent.len(), 1);
    assert!(matches!(&sent[0], Command::Copy { entries, .. } if entries.len() == 50_000));
}

fn queue_row(
    kind: porthmos_core::transfer::rows::RowKind, state: porthmos_core::transfer::rows::RowState,
) -> porthmos_core::transfer::rows::QueueRow {
    porthmos_core::transfer::rows::QueueRow {
        kind,
        label: "file".to_string(),
        direction: porthmos_core::transfer::Direction::Upload,
        files_done: 0,
        files_total: 1,
        bytes_done: 0,
        bytes_total: 10,
        state,
        paused: false,
        session_id: Some(1),
        failure: None,
    }
}

fn feed_rows(harness: &mut Harness, rows: Vec<porthmos_core::transfer::rows::QueueRow>) {
    harness.feed(Event::TransfersChanged(porthmos_core::transfer::TransferSnapshot {
        rows,
        ..porthmos_core::transfer::TransferSnapshot::default()
    }));
}

fn ctrl_t(harness: &mut Harness) {
    key(harness, Key::Character("t".into()), Modifiers::CTRL);
}

#[test]
fn the_queue_opens_when_a_transfer_starts_and_stays_closed_after_the_user_closes_it() {
    use porthmos_core::transfer::rows::{RowKind, RowState};
    let mut harness = Harness::new();
    assert!(!harness.app.queue().is_open());

    feed_rows(&mut harness, vec![queue_row(RowKind::Single(1), RowState::Running)]);
    assert!(harness.app.queue().is_open());

    ctrl_t(&mut harness);
    assert!(!harness.app.queue().is_open());
    feed_rows(&mut harness, vec![queue_row(RowKind::Single(1), RowState::Running)]);
    assert!(!harness.app.queue().is_open());

    feed_rows(&mut harness, vec![queue_row(RowKind::Single(1), RowState::Done)]);
    feed_rows(&mut harness, vec![queue_row(RowKind::Single(2), RowState::Queued)]);
    assert!(harness.app.queue().is_open());
}

#[test]
fn toggling_with_the_key_opens_and_closes_the_pane() {
    let mut harness = Harness::new();

    ctrl_t(&mut harness);
    assert!(harness.app.queue().is_open());

    ctrl_t(&mut harness);
    assert!(!harness.app.queue().is_open());
}

#[test]
fn the_action_bar_transfers_button_toggles_the_pane() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::ToggleQueue));

    assert!(harness.app.queue().is_open());
}

#[test]
fn an_open_queue_pane_shortens_the_list_viewport() {
    let mut harness = Harness::new();
    let closed = harness.app.list_height();

    ctrl_t(&mut harness);

    assert_eq!(harness.app.list_height(), closed - crate::transfers::QUEUE_HEIGHT - 1.0);
}

#[test]
fn pause_resume_cancel_retry_and_reorder_send_the_row_commands_with_the_row_kind() {
    use crate::transfers::RowButton;
    use porthmos_core::transfer::rows::RowKind;
    let mut harness = Harness::new();
    harness.sent();
    let kind = RowKind::Batch(3);

    for button in [
        RowButton::Pause,
        RowButton::Resume,
        RowButton::Cancel,
        RowButton::Retry,
        RowButton::MoveUp,
        RowButton::MoveDown,
    ] {
        harness.press(Message::QueueRowPressed(kind, button));
    }

    assert_eq!(
        harness.sent(),
        vec![
            Command::PauseRow { kind },
            Command::ResumeRow { kind },
            Command::CancelRow { kind },
            Command::RetryRow { kind },
            Command::ReorderRow { kind, earlier: true },
            Command::ReorderRow { kind, earlier: false },
        ]
    );
}

#[test]
fn clear_finished_and_cancel_all_send_their_commands() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::QueueClearFinished);
    harness.press(Message::QueueCancelAll);

    assert_eq!(harness.sent(), vec![Command::ClearFinished, Command::CancelAllTransfers]);
}

#[test]
fn the_speed_limit_is_applied_on_enter_and_echoes_from_the_snapshot() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::SpeedLimitTyped("100K".to_string()));
    assert_eq!(harness.app.queue().speed_text(), Some("100K"));
    assert!(harness.sent().is_empty());
    harness.press(Message::SpeedLimitSubmitted);

    assert_eq!(harness.sent(), vec![Command::SetSpeedLimit { bytes_per_second: Some(100 * 1024) }]);
    assert_eq!(harness.app.queue().speed_text(), None);
    harness.feed(Event::TransfersChanged(porthmos_core::transfer::TransferSnapshot {
        speed_limit: Some(100 * 1024),
        ..porthmos_core::transfer::TransferSnapshot::default()
    }));
    assert_eq!(harness.app.transfer_speed_limit(), Some(100 * 1024));
}

#[test]
fn a_bad_speed_limit_shows_a_hint_and_sends_nothing() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::SpeedLimitTyped("fast".to_string()));
    harness.press(Message::SpeedLimitSubmitted);

    assert!(harness.sent().is_empty());
    assert!(harness.app.queue().speed_error().is_some());
    harness.press(Message::SpeedLimitTyped("fas".to_string()));
    assert!(harness.app.queue().speed_error().is_none());
}

#[test]
fn off_removes_the_speed_limit() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::SpeedLimitTyped("off".to_string()));
    harness.press(Message::SpeedLimitSubmitted);

    assert_eq!(harness.sent(), vec![Command::SetSpeedLimit { bytes_per_second: None }]);
}

#[test]
fn submitting_an_untouched_speed_limit_sends_nothing() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::SpeedLimitSubmitted);

    assert!(harness.sent().is_empty());
}

#[test]
fn a_nonsense_queue_scroll_offset_is_clamped_to_the_top() {
    let mut harness = Harness::new();

    harness.press(Message::QueueScrolled(-40.0));

    assert_eq!(harness.app.queue().scroll(), 0.0);
}

#[test]
fn keys_do_not_leak_into_the_panels_while_the_queue_speed_field_has_typed_text() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    harness.press(Message::SpeedLimitTyped("5".to_string()));

    plain(&mut harness, Named::ArrowDown);

    assert_eq!(harness.app.local().panel.cursor(), 0);
    plain(&mut harness, Named::Escape);
    assert_eq!(harness.app.queue().speed_text(), None);
    plain(&mut harness, Named::ArrowDown);
    assert_eq!(harness.app.local().panel.cursor(), 1);
}

fn conflict_info(name: &str) -> porthmos_core::transfer::conflicts::ConflictInfo {
    porthmos_core::transfer::conflicts::ConflictInfo {
        display_name: name.to_string(),
        existing: Some(porthmos_core::transfer::plan::ExistingFile { size: 5, modified: Some(1_000), is_dir: false }),
        partial: None,
        new_size: 9,
        new_modified: Some(2_000),
    }
}

fn partial_info(name: &str) -> porthmos_core::transfer::conflicts::ConflictInfo {
    porthmos_core::transfer::conflicts::ConflictInfo {
        existing: None,
        partial: Some(porthmos_core::transfer::plan::ExistingFile { size: 3, modified: None, is_dir: false }),
        ..conflict_info(name)
    }
}

fn found(batch_id: u64, files: Vec<porthmos_core::transfer::conflicts::ConflictInfo>) -> Event {
    Event::ConflictsFound { batch_id, files }
}

fn shown_conflict(harness: &Harness) -> Option<String> {
    match harness.app.modal() {
        Some(crate::modal::Modal::Conflict { view, .. }) => Some(view.file_name.clone()),
        _ => None,
    }
}

fn resolve_command(batch_id: u64, answers: &[porthmos_core::transfer::conflicts::Resolution]) -> Command {
    Command::ResolveConflicts { batch_id, answers: Some(answers.to_vec()) }
}

#[test]
fn conflicts_found_open_the_dialog_for_the_first_file() {
    let mut harness = Harness::new();

    harness.feed(found(1, vec![conflict_info("a"), conflict_info("b")]));

    assert_eq!(shown_conflict(&harness).as_deref(), Some("a"));
}

#[test]
fn a_conflict_dialog_waits_behind_a_prompt_that_is_already_open() {
    let mut harness = Harness::new();
    plain(&mut harness, Named::F7);

    harness.feed(found(1, vec![conflict_info("a")]));
    assert!(matches!(open_modal(&harness), crate::modal::Modal::NewFolder { .. }));

    plain(&mut harness, Named::Escape);
    assert_eq!(shown_conflict(&harness).as_deref(), Some("a"));
}

#[test]
fn answering_through_the_buttons_sends_one_resolve_conflicts_for_the_batch() {
    use porthmos_core::transfer::conflicts::Resolution;
    let mut harness = Harness::new();
    harness.feed(found(3, vec![conflict_info("a"), conflict_info("b")]));
    harness.sent();

    harness.press(Message::ConflictAnswered(Some(Resolution::Overwrite)));
    assert!(harness.sent().is_empty());
    assert_eq!(shown_conflict(&harness).as_deref(), Some("b"));
    harness.press(Message::ConflictAnswered(Some(Resolution::Rename)));

    assert_eq!(harness.sent(), vec![resolve_command(3, &[Resolution::Overwrite, Resolution::Rename])]);
    assert!(harness.app.modal().is_none());
}

#[test]
fn the_apply_to_the_rest_checkbox_answers_the_following_files_that_fit() {
    use porthmos_core::transfer::conflicts::Resolution;
    let mut harness = Harness::new();
    harness.feed(found(3, vec![conflict_info("a"), conflict_info("b"), partial_info("c")]));
    harness.sent();

    harness.press(Message::ConflictApplyToRestToggled);
    harness.press(Message::ConflictAnswered(Some(Resolution::Overwrite)));

    assert_eq!(shown_conflict(&harness).as_deref(), Some("c"));
    assert!(matches!(open_modal(&harness), crate::modal::Modal::Conflict { apply_to_rest: false, .. }));
    harness.press(Message::ConflictAnswered(Some(Resolution::Resume)));
    assert_eq!(
        harness.sent(),
        vec![resolve_command(3, &[Resolution::Overwrite, Resolution::Overwrite, Resolution::Resume])]
    );
}

#[test]
fn escape_cancels_the_batch_and_enter_decides_nothing() {
    let mut harness = Harness::new();
    harness.feed(found(5, vec![conflict_info("a"), conflict_info("b")]));
    harness.sent();

    plain(&mut harness, Named::Enter);
    assert_eq!(shown_conflict(&harness).as_deref(), Some("a"));
    assert!(harness.sent().is_empty());

    plain(&mut harness, Named::Escape);

    assert_eq!(harness.sent(), vec![Command::ResolveConflicts { batch_id: 5, answers: None }]);
    assert!(harness.app.modal().is_none());
}

#[test]
fn the_cancel_batch_button_sends_none() {
    let mut harness = Harness::new();
    harness.feed(found(5, vec![conflict_info("a")]));
    harness.sent();

    harness.press(Message::ConflictAnswered(None));

    assert_eq!(harness.sent(), vec![Command::ResolveConflicts { batch_id: 5, answers: None }]);
}

#[test]
fn letter_keys_answer_the_conflict_dialog() {
    use porthmos_core::transfer::conflicts::Resolution;
    let mut harness = Harness::new();
    harness.feed(found(1, vec![conflict_info("a"), conflict_info("b"), conflict_info("c"), partial_info("d")]));
    harness.sent();

    letter(&mut harness, "o");
    letter(&mut harness, "s");
    letter(&mut harness, "r");
    letter(&mut harness, "u");

    assert_eq!(
        harness.sent(),
        vec![resolve_command(1, &[Resolution::Overwrite, Resolution::Skip, Resolution::Rename, Resolution::Resume])]
    );
}

#[test]
fn the_resume_key_is_ignored_for_a_file_without_a_partial() {
    let mut harness = Harness::new();
    harness.feed(found(1, vec![conflict_info("a")]));
    harness.sent();

    letter(&mut harness, "u");

    assert_eq!(shown_conflict(&harness).as_deref(), Some("a"));
    assert!(harness.sent().is_empty());
}

#[test]
fn the_apply_to_the_rest_key_toggles_the_checkbox() {
    let mut harness = Harness::new();
    harness.feed(found(1, vec![conflict_info("a")]));

    letter(&mut harness, "a");
    assert!(matches!(open_modal(&harness), crate::modal::Modal::Conflict { apply_to_rest: true, .. }));
    letter(&mut harness, "a");
    assert!(matches!(open_modal(&harness), crate::modal::Modal::Conflict { apply_to_rest: false, .. }));
}

#[test]
fn a_withdrawn_batch_is_dropped_and_closes_its_dialog() {
    let mut harness = Harness::new();
    harness.feed(found(1, vec![conflict_info("a")]));
    harness.feed(found(2, vec![conflict_info("x")]));

    harness.feed(Event::ConflictsWithdrawn { batch_ids: vec![1] });

    assert_eq!(shown_conflict(&harness).as_deref(), Some("x"));
    harness.feed(Event::ConflictsWithdrawn { batch_ids: vec![2] });
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_withdrawn_batch_that_is_not_showing_leaves_the_current_dialog_alone() {
    let mut harness = Harness::new();
    harness.feed(found(1, vec![conflict_info("a")]));
    harness.feed(found(2, vec![conflict_info("x")]));

    harness.feed(Event::ConflictsWithdrawn { batch_ids: vec![2] });

    assert_eq!(shown_conflict(&harness).as_deref(), Some("a"));
}

#[test]
fn a_withdrawal_while_a_prompt_is_open_removes_the_waiting_batch() {
    let mut harness = Harness::new();
    plain(&mut harness, Named::F7);
    harness.feed(found(1, vec![conflict_info("a")]));

    harness.feed(Event::ConflictsWithdrawn { batch_ids: vec![1] });
    plain(&mut harness, Named::Escape);

    assert!(harness.app.modal().is_none());
}

#[test]
fn ten_thousand_conflicts_answer_with_apply_to_rest_in_one_command() {
    use porthmos_core::transfer::conflicts::Resolution;
    let mut harness = Harness::new();
    harness.feed(found(1, (0..10_000).map(|n| conflict_info(&format!("f{n}"))).collect()));
    harness.sent();

    harness.press(Message::ConflictApplyToRestToggled);
    harness.press(Message::ConflictAnswered(Some(Resolution::Skip)));

    let sent = harness.sent();
    assert_eq!(sent.len(), 1);
    assert!(matches!(&sent[0], Command::ResolveConflicts { answers: Some(answers), .. } if answers.len() == 10_000));
}

#[test]
fn other_keys_do_nothing_while_a_conflict_is_open() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    harness.feed(found(1, vec![conflict_info("a")]));
    harness.sent();

    plain(&mut harness, Named::ArrowDown);
    plain(&mut harness, Named::F7);

    assert_eq!(harness.app.local().panel.cursor(), 0);
    assert_eq!(shown_conflict(&harness).as_deref(), Some("a"));
    assert!(harness.sent().is_empty());
}

fn drag_point(harness: &Harness, side: PanelSide, row: usize) -> iced::Point {
    let width = harness.app.window_size().width;
    let x = match side {
        PanelSide::Local => 100.0,
        PanelSide::Remote => width - 100.0,
    };
    iced::Point::new(x, crate::geometry::LIST_TOP + crate::geometry::ROW_HEIGHT * row as f32 + 5.0)
}

fn press_and_drag(harness: &mut Harness, from: (PanelSide, usize), to: iced::Point) {
    let start = drag_point(harness, from.0, from.1);
    harness.press(Message::RowPressed(from.0, from.1));
    harness.press(Message::CursorMoved(start));
    harness.press(Message::CursorMoved(iced::Point::new(start.x + 20.0, start.y)));
    harness.press(Message::CursorMoved(to));
}

fn drop_ready(harness: &mut Harness, local: &[&str]) {
    fill_local(harness, local);
    upload_target(harness);
    harness.sent();
}

#[test]
fn a_drag_between_panels_sends_one_copy_on_release() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b"]);
    let target = drag_point(&harness, PanelSide::Remote, 10);

    press_and_drag(&mut harness, (PanelSide::Local, 1), target);
    assert!(harness.sent().is_empty());
    harness.press(Message::MouseReleased);

    let folder = harness.start_folder();
    assert_eq!(
        harness.sent(),
        vec![Command::Copy {
            from: Location::Local,
            entries: vec![entry(&folder, "a")],
            to: Location::Session(1),
            dest_dir: VfsPath::new("/srv/in"),
        }]
    );
    assert!(!harness.app.drag().is_tracking());
}

#[test]
fn a_drop_on_a_folder_row_goes_into_that_folder() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    connect(&mut harness, 1, "alpha");
    let remote = VfsPath::new("/srv/in");
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: remote.clone(),
        entries: vec![folder_entry(&remote, "sub")],
    });
    harness.sent();
    let target = drag_point(&harness, PanelSide::Remote, 1);

    press_and_drag(&mut harness, (PanelSide::Local, 1), target);
    harness.press(Message::MouseReleased);

    assert!(matches!(
        harness.sent().as_slice(),
        [Command::Copy { dest_dir, .. }] if dest_dir == &VfsPath::new("/srv/in/sub")
    ));
}

#[test]
fn shift_turns_the_drop_into_a_move() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a"]);
    let target = drag_point(&harness, PanelSide::Remote, 10);

    press_and_drag(&mut harness, (PanelSide::Local, 1), target);
    harness.press(Message::ModifiersChanged(Modifiers::SHIFT));
    harness.press(Message::MouseReleased);

    assert!(matches!(harness.sent().as_slice(), [Command::Move { .. }]));
}

#[test]
fn a_refused_drop_adds_a_notice_and_sends_nothing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.sent();
    let target = drag_point(&harness, PanelSide::Remote, 3);

    press_and_drag(&mut harness, (PanelSide::Local, 1), target);
    harness.press(Message::MouseReleased);

    assert!(harness.sent().is_empty());
    let notice = harness.app.notices().last().expect("a notice");
    assert_eq!(notice.severity, Severity::Warning);
    assert!(notice.message.contains("Connect"), "{}", notice.message);
}

#[test]
fn a_drop_within_the_same_panel_is_refused_with_a_notice() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b"]);
    let target = drag_point(&harness, PanelSide::Local, 10);

    press_and_drag(&mut harness, (PanelSide::Local, 1), target);
    harness.press(Message::MouseReleased);

    assert!(harness.sent().is_empty());
    assert!(harness.app.notices().last().is_some());
}

#[test]
fn a_drop_outside_both_lists_does_nothing_quietly() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a"]);
    let before = harness.app.notices().len();

    press_and_drag(&mut harness, (PanelSide::Local, 1), iced::Point::new(300.0, 5.0));
    harness.press(Message::MouseReleased);

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.notices().len(), before);
}

#[test]
fn a_drag_does_not_change_the_selection_until_it_starts() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b", "c"]);
    select_rows(&mut harness, &[1, 2]);

    harness.press(Message::RowPressed(PanelSide::Local, 1));
    assert_eq!(selected_local(&harness), ["a", "b"]);
    let start = drag_point(&harness, PanelSide::Local, 1);
    harness.press(Message::CursorMoved(start));
    harness.press(Message::CursorMoved(iced::Point::new(start.x + 30.0, start.y)));

    assert!(harness.app.drag().is_active());
    assert_eq!(selected_local(&harness), ["a", "b"]);
}

#[test]
fn dragging_a_selected_row_sends_the_whole_selection() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b", "c"]);
    select_rows(&mut harness, &[1, 3]);
    let target = drag_point(&harness, PanelSide::Remote, 10);

    press_and_drag(&mut harness, (PanelSide::Local, 1), target);
    harness.press(Message::MouseReleased);

    match harness.sent().as_slice() {
        [Command::Copy { entries, .. }] => {
            assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn dragging_an_unselected_row_selects_and_sends_only_that_row() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b", "c"]);
    select_rows(&mut harness, &[1, 2]);
    let target = drag_point(&harness, PanelSide::Remote, 10);

    press_and_drag(&mut harness, (PanelSide::Local, 3), target);
    assert_eq!(selected_local(&harness), ["c"]);
    harness.press(Message::MouseReleased);

    match harness.sent().as_slice() {
        [Command::Copy { entries, .. }] => {
            assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["c"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn releasing_the_mouse_without_moving_still_selects_the_row() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b", "c"]);
    select_rows(&mut harness, &[1, 2]);

    harness.press(Message::RowPressed(PanelSide::Local, 1));
    harness.press(Message::MouseReleased);

    assert_eq!(selected_local(&harness), ["a"]);
    assert!(harness.sent().is_empty());
}

#[test]
fn a_small_wobble_before_the_release_is_still_a_click() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b"]);
    select_rows(&mut harness, &[1, 2]);
    let start = drag_point(&harness, PanelSide::Local, 1);

    harness.press(Message::RowPressed(PanelSide::Local, 1));
    harness.press(Message::CursorMoved(start));
    harness.press(Message::CursorMoved(iced::Point::new(start.x + 3.0, start.y + 2.0)));
    harness.press(Message::MouseReleased);

    assert_eq!(selected_local(&harness), ["a"]);
}

#[test]
fn pressing_a_row_with_a_modifier_still_toggles_it() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    harness.press(Message::ModifiersChanged(Modifiers::COMMAND));

    harness.press(Message::RowPressed(PanelSide::Local, 2));
    harness.press(Message::MouseReleased);

    assert_eq!(selected_local(&harness), ["a", "b"]);
}

#[test]
fn escape_cancels_the_drag_and_the_release_drops_nothing() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a"]);
    let target = drag_point(&harness, PanelSide::Remote, 10);
    press_and_drag(&mut harness, (PanelSide::Local, 1), target);

    plain(&mut harness, Named::Escape);
    harness.press(Message::MouseReleased);

    assert!(harness.sent().is_empty());
    assert!(!harness.app.drag().is_tracking());
    assert!(harness.app.drag_hint().is_none());
}

#[test]
fn the_parent_row_is_never_dragged() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a"]);
    let target = drag_point(&harness, PanelSide::Remote, 10);

    press_and_drag(&mut harness, (PanelSide::Local, 0), target);
    harness.press(Message::MouseReleased);

    assert!(harness.sent().is_empty());
    assert!(!harness.app.drag().is_tracking());
}

#[test]
fn keys_do_nothing_while_a_drag_is_in_progress_except_escape() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b"]);
    let target = drag_point(&harness, PanelSide::Remote, 10);
    press_and_drag(&mut harness, (PanelSide::Local, 1), target);

    plain(&mut harness, Named::F8);

    assert!(harness.app.modal().is_none());
    assert!(harness.app.drag().is_active());
}

#[test]
fn the_hint_names_copy_or_move_and_the_count() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b", "c"]);
    select_rows(&mut harness, &[1, 2, 3]);
    let target = drag_point(&harness, PanelSide::Remote, 10);
    press_and_drag(&mut harness, (PanelSide::Local, 1), target);

    let copy = harness.app.drag_hint().expect("a hint");
    assert_eq!((copy.glyph, copy.text.as_str(), copy.refused), ("+", "Copy 3 items", false));

    harness.press(Message::ModifiersChanged(Modifiers::SHIFT));
    let moving = harness.app.drag_hint().expect("a hint");
    assert_eq!((moving.glyph, moving.text.as_str()), ("→", "Move 3 items"));
}

#[test]
fn the_hint_is_singular_for_one_item_and_explains_a_refusal() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a", "b"]);
    let same_panel = drag_point(&harness, PanelSide::Local, 10);
    press_and_drag(&mut harness, (PanelSide::Local, 1), same_panel);

    let refused = harness.app.drag_hint().expect("a hint");
    assert!(refused.refused);
    assert_eq!(refused.glyph, "✕");

    harness.press(Message::CursorMoved(drag_point(&harness, PanelSide::Remote, 10)));
    assert_eq!(harness.app.drag_hint().expect("a hint").text, "Copy 1 item");
}

#[test]
fn the_drop_target_follows_the_pointer_during_a_drag() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a"]);
    assert_eq!(harness.app.drop_target(), None);
    let target = drag_point(&harness, PanelSide::Remote, 10);

    press_and_drag(&mut harness, (PanelSide::Local, 1), target);

    assert_eq!(harness.app.drop_target(), Some(crate::geometry::Hit { side: PanelSide::Remote, row: None }));
}

#[test]
fn an_open_queue_pane_moves_the_drop_area() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a"]);
    let height = harness.app.window_size().height;
    let low = iced::Point::new(100.0, height - crate::geometry::CHROME_HEIGHT + crate::geometry::LIST_TOP - 10.0);
    ctrl_t(&mut harness);

    press_and_drag(&mut harness, (PanelSide::Local, 1), low);

    assert_eq!(harness.app.drop_target(), None);
}

#[test]
fn a_press_with_shift_held_starts_no_drag() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &["a"]);
    harness.press(Message::ModifiersChanged(Modifiers::SHIFT));

    harness.press(Message::RowPressed(PanelSide::Local, 1));
    let start = drag_point(&harness, PanelSide::Local, 1);
    harness.press(Message::CursorMoved(start));
    harness.press(Message::CursorMoved(iced::Point::new(start.x + 40.0, start.y)));

    assert!(!harness.app.drag().is_active());
}

fn desktop_files(names: &[&str]) -> (tempfile::TempDir, Vec<std::path::PathBuf>) {
    let root = tempfile::tempdir().unwrap();
    let paths = names
        .iter()
        .map(|name| {
            let path = root.path().join(name);
            std::fs::write(&path, b"data").unwrap();
            path
        })
        .collect();
    (root, paths)
}

fn desktop_drop(harness: &mut Harness, paths: Vec<std::path::PathBuf>, at: Option<iced::Point>) {
    harness.press(Message::Desktop(crate::native::DesktopEvent::Dropped { paths, at }));
    harness.press(Message::DesktopDropFlush);
}

#[test]
fn dropping_files_on_the_remote_panel_uploads_them_into_its_folder() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    let (_keep, paths) = desktop_files(&["a.txt", "b.txt"]);
    let at = drag_point(&harness, PanelSide::Remote, 10);

    desktop_drop(&mut harness, paths.clone(), Some(at));

    match harness.sent().as_slice() {
        [Command::Copy { from: Location::Local, entries, to: Location::Session(1), dest_dir }] => {
            assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["a.txt", "b.txt"]);
            assert_eq!(dest_dir, &VfsPath::new("/srv/in"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn dropping_files_on_a_folder_row_copies_into_that_folder() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    let remote = VfsPath::new("/srv/in");
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: remote.clone(),
        entries: vec![folder_entry(&remote, "sub")],
    });
    harness.sent();
    let (_keep, paths) = desktop_files(&["a.txt"]);
    let at = drag_point(&harness, PanelSide::Remote, 1);

    desktop_drop(&mut harness, paths, Some(at));

    assert!(matches!(
        harness.sent().as_slice(),
        [Command::Copy { dest_dir, .. }] if dest_dir == &VfsPath::new("/srv/in/sub")
    ));
}

#[test]
fn several_drop_events_in_one_batch_become_one_command() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    let (_keep, paths) = desktop_files(&["a.txt", "b.txt", "c.txt"]);
    let at = drag_point(&harness, PanelSide::Remote, 10);

    for path in paths {
        harness.press(Message::Desktop(crate::native::DesktopEvent::Dropped { paths: vec![path], at: Some(at) }));
    }
    harness.press(Message::DesktopDropFlush);

    match harness.sent().as_slice() {
        [Command::Copy { entries, .. }] => assert_eq!(entries.len(), 3),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dropping_with_no_connection_on_the_right_panel_says_connect_first() {
    let mut harness = Harness::new();
    harness.sent();
    let (_keep, paths) = desktop_files(&["a.txt"]);
    let at = drag_point(&harness, PanelSide::Remote, 3);

    desktop_drop(&mut harness, paths, Some(at));

    assert!(harness.sent().is_empty());
    let notice = harness.app.notices().last().expect("a notice");
    assert_eq!(notice.severity, Severity::Warning);
    assert!(notice.message.contains("Connect"), "{}", notice.message);
}

#[test]
fn dropping_on_the_local_panel_is_refused_with_a_notice() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    let (_keep, paths) = desktop_files(&["a.txt"]);
    let at = drag_point(&harness, PanelSide::Local, 3);

    desktop_drop(&mut harness, paths, Some(at));

    assert!(harness.sent().is_empty());
    assert!(harness.app.notices().last().unwrap().message.contains("right panel"));
}

#[test]
fn dropping_nothing_readable_sends_nothing_and_notes_each_problem() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    let at = drag_point(&harness, PanelSide::Remote, 10);
    let missing = vec![std::path::PathBuf::from("/nonexistent/one"), std::path::PathBuf::from("/nonexistent/two")];

    desktop_drop(&mut harness, missing, Some(at));

    assert!(harness.sent().is_empty());
    let notices: Vec<&str> = harness.app.notices().iter().map(|notice| notice.message.as_str()).collect();
    assert!(notices.iter().any(|message| message.contains("one")));
    assert!(notices.iter().any(|message| message.contains("two")));
}

#[test]
fn readable_files_are_still_sent_when_another_one_is_missing() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    let (_keep, mut paths) = desktop_files(&["a.txt"]);
    paths.push(std::path::PathBuf::from("/nonexistent/gone"));
    let at = drag_point(&harness, PanelSide::Remote, 10);

    desktop_drop(&mut harness, paths, Some(at));

    assert!(matches!(harness.sent().as_slice(), [Command::Copy { entries, .. }] if entries.len() == 1));
    assert!(harness.app.notices().last().unwrap().message.contains("gone"));
}

#[test]
fn a_desktop_drop_on_the_tab_strip_does_nothing() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    let before = harness.app.notices().len();
    let (_keep, paths) = desktop_files(&["a.txt"]);

    desktop_drop(&mut harness, paths, Some(iced::Point::new(300.0, 10.0)));

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.notices().len(), before);
}

#[test]
fn a_drop_whose_position_is_unknown_goes_to_the_remote_panel() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    let (_keep, paths) = desktop_files(&["a.txt"]);

    desktop_drop(&mut harness, paths, None);

    assert!(matches!(
        harness.sent().as_slice(),
        [Command::Copy { to: Location::Session(1), dest_dir, .. }] if dest_dir == &VfsPath::new("/srv/in")
    ));
}

#[test]
fn the_last_pointer_position_is_used_when_the_drop_has_none() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    harness.press(Message::Desktop(crate::native::DesktopEvent::Hover(Vec::new())));
    harness.press(Message::CursorMoved(drag_point(&harness, PanelSide::Local, 3)));
    let (_keep, paths) = desktop_files(&["a.txt"]);

    desktop_drop(&mut harness, paths, None);

    assert!(harness.sent().is_empty());
    assert!(harness.app.notices().last().unwrap().message.contains("right panel"));
}

#[test]
fn hovering_files_highlights_the_target_and_leaving_clears_it() {
    use crate::native::DesktopEvent;
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    assert_eq!(harness.app.drop_target(), None);

    harness.press(Message::Desktop(DesktopEvent::Hover(vec![std::path::PathBuf::from("/x/a")])));
    harness.press(Message::Desktop(DesktopEvent::Moved(drag_point(&harness, PanelSide::Remote, 10))));
    assert_eq!(harness.app.drop_target(), Some(crate::geometry::Hit { side: PanelSide::Remote, row: None }));
    let hint = harness.app.drag_hint().expect("a hint while hovering");
    assert_eq!((hint.glyph, hint.text.as_str(), hint.refused), ("+", "Upload 1 item", false));

    harness.press(Message::Desktop(DesktopEvent::Left));
    assert_eq!(harness.app.drop_target(), None);
    assert!(harness.app.drag_hint().is_none());
}

#[test]
fn hovering_over_the_local_panel_warns_in_the_hint() {
    use crate::native::DesktopEvent;
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);

    harness.press(Message::Desktop(DesktopEvent::Hover(Vec::new())));
    harness.press(Message::Desktop(DesktopEvent::Moved(drag_point(&harness, PanelSide::Local, 3))));

    let hint = harness.app.drag_hint().expect("a hint");
    assert!(hint.refused);
    assert!(hint.text.contains("right panel"));
}

#[test]
fn a_hover_without_a_known_count_says_files() {
    use crate::native::DesktopEvent;
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);

    harness.press(Message::Desktop(DesktopEvent::Hover(Vec::new())));
    harness.press(Message::Desktop(DesktopEvent::Moved(drag_point(&harness, PanelSide::Remote, 10))));

    assert_eq!(harness.app.drag_hint().unwrap().text, "Upload files");
}

#[test]
fn attaching_nothing_is_harmless() {
    let mut harness = Harness::new();

    harness.press(Message::NativeAttached(None));

    assert!(harness.app.notices().is_empty());
}

fn local_path(harness: &Harness, name: &str) -> std::path::PathBuf {
    porthmos_core::to_os_path(&harness.start_folder().join(name))
}

fn outside_the_window() -> iced::Point {
    iced::Point::new(-20.0, 200.0)
}

fn leave_ready(harness: &mut Harness, local: &[&str]) -> crate::testing::NativeProbe {
    fill_local(harness, local);
    harness.sent();
    harness.attach_native()
}

#[test]
fn the_dragged_local_paths_are_passed_to_start_drag_in_row_order() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a", "b", "c", "d"]);
    select_rows(&mut harness, &[4, 2]);

    press_and_drag(&mut harness, (PanelSide::Local, 4), outside_the_window());

    assert_eq!(probe.started(), vec![vec![local_path(&harness, "b"), local_path(&harness, "d")]]);
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Native);
    assert!(harness.app.drag_hint().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn the_native_drag_starts_once_however_far_the_pointer_goes() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);

    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());
    harness.press(Message::CursorMoved(iced::Point::new(-300.0, 400.0)));
    harness.press(Message::WindowLeft);

    assert_eq!(probe.started().len(), 1);
}

#[test]
fn the_cursor_leaving_the_window_event_also_hands_off() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);
    let inside = drag_point(&harness, PanelSide::Remote, 3);
    press_and_drag(&mut harness, (PanelSide::Local, 1), inside);
    assert!(probe.started().is_empty());

    harness.press(Message::WindowLeft);

    assert_eq!(probe.started(), vec![vec![local_path(&harness, "a")]]);
}

#[test]
fn a_failed_start_adds_a_notice_and_returns_to_idle() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);
    probe.refuse_with("the pointer grab was refused");

    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());

    let notice = harness.app.notices().last().unwrap();
    assert_eq!(notice.severity, Severity::Warning);
    assert!(notice.message.contains("the pointer grab was refused"));
    assert!(!harness.app.drag().is_tracking());
    assert!(probe.started().is_empty());
}

#[test]
fn re_entering_the_window_before_leaving_never_starts_a_native_drag() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);
    let inside = drag_point(&harness, PanelSide::Remote, 5);

    press_and_drag(&mut harness, (PanelSide::Local, 1), inside);
    harness.press(Message::CursorMoved(iced::Point::new(1.0, 1.0)));
    harness.press(Message::CursorMoved(inside));

    assert!(probe.started().is_empty());
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Internal);
}

#[test]
fn leaving_without_a_native_backend_keeps_the_internal_drag() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.sent();

    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());

    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Internal);
    harness.press(Message::MouseReleased);
    assert!(!harness.app.drag().is_tracking());
    assert!(harness.sent().is_empty());
}

#[test]
fn leaving_with_the_parent_row_starts_nothing() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);

    press_and_drag(&mut harness, (PanelSide::Local, 0), outside_the_window());

    assert!(probe.started().is_empty());
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Pressed);
}

#[test]
fn escape_during_the_native_drag_calls_cancel() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);
    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());

    plain(&mut harness, Named::Escape);

    assert_eq!(probe.cancels(), 1);
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Native);
    probe.emit(crate::native::NativeEvent::Cancelled);
    harness.press(Message::NativePoll);
    assert!(!harness.app.drag().is_tracking());
}

#[test]
fn the_native_answer_returns_the_drag_to_idle_and_later_events_are_ignored() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);
    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());

    probe.emit(crate::native::NativeEvent::Started);
    harness.press(Message::NativePoll);
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Native);

    probe.emit(crate::native::NativeEvent::Finished);
    probe.emit(crate::native::NativeEvent::Cancelled);
    harness.press(Message::NativePoll);

    assert!(!harness.app.drag().is_tracking());
    assert_eq!(probe.cancels(), 0);
}

#[test]
fn a_release_inside_the_window_during_the_native_drag_sends_nothing() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);
    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());

    harness.press(Message::MouseReleased);

    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Native);
    assert!(harness.sent().is_empty());
    assert_eq!(probe.started().len(), 1);
}

#[test]
fn a_new_press_after_the_native_drag_ended_drags_normally() {
    let mut harness = Harness::new();
    let probe = leave_ready(&mut harness, &["a"]);
    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());
    probe.emit(crate::native::NativeEvent::Finished);
    harness.press(Message::NativePoll);
    harness.press(Message::MouseReleased);

    press_and_drag(&mut harness, (PanelSide::Local, 1), outside_the_window());

    assert_eq!(probe.started().len(), 2);
}

fn staging_rows(state: porthmos_core::transfer::rows::RowState) -> Vec<porthmos_core::transfer::rows::QueueRow> {
    vec![queue_row(porthmos_core::transfer::rows::RowKind::Batch(5), state)]
}

fn remote_ready(harness: &mut Harness, names: &[&str]) -> crate::testing::NativeProbe {
    connect(harness, 1, "alpha");
    let remote = VfsPath::new("/srv/in");
    let entries = names.iter().map(|name| entry(&remote, name)).collect();
    harness.feed(Event::Listed { location: Location::Session(1), path: remote, entries });
    let probe = harness.attach_native();
    harness.sent();
    probe
}

fn leave_remote(harness: &mut Harness, row: usize) -> Vec<Command> {
    press_and_drag(harness, (PanelSide::Remote, row), outside_the_window());
    harness.sent()
}

fn copy_into_staging(sent: &[Command]) -> (Vec<porthmos_core::Entry>, std::path::PathBuf) {
    match sent {
        [Command::Copy { from: Location::Session(1), entries, to: Location::Local, dest_dir }] => {
            (entries.clone(), porthmos_core::to_os_path(dest_dir))
        }
        other => panic!("expected one copy into the staging folder, got {other:?}"),
    }
}

fn deliver(root: &std::path::Path, names: &[&str]) {
    for name in names {
        std::fs::write(root.join(name), "data").unwrap();
    }
}

#[test]
fn leaving_with_remote_entries_sends_one_copy_into_the_staging_folder() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a", "b"]);

    let sent = leave_remote(&mut harness, 2);

    let (entries, root) = copy_into_staging(&sent);
    assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), vec!["b"]);
    assert!(root.starts_with(harness.scratch()));
    assert!(root.file_name().unwrap().to_string_lossy().starts_with(crate::staging::PREFIX));
    assert!(root.is_dir());
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Staging);
    assert!(probe.started().is_empty());
    feed_rows(&mut harness, staging_rows(RowState::Running));
    assert!(harness.sent().is_empty());
}

#[test]
fn the_native_drag_starts_only_after_staging_finishes() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a", "b", "c"]);
    select_remote(&mut harness, &[1, 3]);
    let sent = leave_remote(&mut harness, 3);
    let (_, root) = copy_into_staging(&sent);

    feed_rows(&mut harness, staging_rows(RowState::Running));
    assert!(probe.started().is_empty());

    deliver(&root, &["a", "c"]);
    feed_rows(&mut harness, staging_rows(RowState::Done));

    assert_eq!(probe.started(), vec![vec![root.join("a"), root.join("c")]]);
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Native);
    assert!(root.is_dir());
}

#[test]
fn a_done_row_without_the_files_does_not_start_the_native_drag() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a"]);
    leave_remote(&mut harness, 1);

    feed_rows(&mut harness, staging_rows(RowState::Done));

    assert!(probe.started().is_empty());
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Staging);
}

#[test]
fn releasing_the_button_while_staging_cancels_the_rows_and_removes_the_folder() {
    use porthmos_core::transfer::rows::{RowKind, RowState};
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    feed_rows(&mut harness, staging_rows(RowState::Running));

    harness.press(Message::MouseReleased);

    assert_eq!(harness.sent(), vec![Command::CancelRow { kind: RowKind::Batch(5) }]);
    assert!(!harness.app.drag().is_tracking());
    feed_rows(&mut harness, staging_rows(RowState::Cancelled));
    assert!(!root.exists());
    assert!(probe.started().is_empty());
    assert!(harness.app.notices().is_empty());
}

#[test]
fn releasing_before_any_row_appeared_waits_for_the_rows_before_removing_the_folder() {
    use porthmos_core::transfer::rows::{RowKind, RowState};
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));

    harness.press(Message::MouseReleased);
    assert!(harness.sent().is_empty());
    assert!(root.is_dir());

    feed_rows(&mut harness, staging_rows(RowState::Running));
    assert_eq!(harness.sent(), vec![Command::CancelRow { kind: RowKind::Batch(5) }]);
    assert!(root.is_dir());

    feed_rows(&mut harness, staging_rows(RowState::Cancelled));
    assert!(!root.exists());
}

#[test]
fn staging_that_finishes_after_the_release_offers_nothing() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    feed_rows(&mut harness, staging_rows(RowState::Running));
    harness.press(Message::MouseReleased);

    deliver(&root, &["a"]);
    feed_rows(&mut harness, staging_rows(RowState::Done));

    assert!(probe.started().is_empty());
    assert!(!root.exists());
}

#[test]
fn escape_while_staging_cancels_the_same_way() {
    use porthmos_core::transfer::rows::{RowKind, RowState};
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    feed_rows(&mut harness, staging_rows(RowState::Running));

    plain(&mut harness, Named::Escape);

    assert_eq!(harness.sent(), vec![Command::CancelRow { kind: RowKind::Batch(5) }]);
    feed_rows(&mut harness, staging_rows(RowState::Cancelled));
    assert!(!root.exists());
    harness.press(Message::MouseReleased);
    assert!(!harness.app.drag().is_tracking());
    assert!(probe.started().is_empty());
}

#[test]
fn rows_queued_before_the_drag_are_never_cancelled() {
    use porthmos_core::transfer::rows::{RowKind, RowState};
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a"]);
    feed_rows(&mut harness, vec![queue_row(RowKind::Single(9), RowState::Running)]);
    harness.sent();
    leave_remote(&mut harness, 1);
    feed_rows(
        &mut harness,
        vec![queue_row(RowKind::Single(9), RowState::Running), queue_row(RowKind::Batch(5), RowState::Running)],
    );

    harness.press(Message::MouseReleased);

    assert_eq!(harness.sent(), vec![Command::CancelRow { kind: RowKind::Batch(5) }]);
}

#[test]
fn disconnecting_the_source_session_while_staging_cancels() {
    use porthmos_core::transfer::rows::{RowKind, RowState};
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    feed_rows(&mut harness, staging_rows(RowState::Running));

    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    assert_eq!(harness.sent(), vec![Command::CancelRow { kind: RowKind::Batch(5) }]);
    assert!(!harness.app.drag().is_tracking());
    feed_rows(&mut harness, staging_rows(RowState::Failed));
    assert!(!root.exists());
}

#[test]
fn a_failed_staging_row_cancels_the_drag_and_tells_the_user() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    let mut failed = staging_rows(RowState::Failed);
    failed[0].failure = Some(porthmos_core::transfer::rows::RowFailure {
        file: "a".to_string(),
        message: "permission denied".to_string(),
        attempts: 1,
    });

    feed_rows(&mut harness, failed);

    let notice = harness.app.notices().last().unwrap();
    assert_eq!(notice.severity, Severity::Warning);
    assert!(notice.message.contains("permission denied"), "{}", notice.message);
    assert!(!harness.app.drag().is_tracking());
    assert!(!root.exists());
    assert!(probe.started().is_empty());
}

#[test]
fn closing_the_app_removes_the_staging_folder() {
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    assert!(root.is_dir());

    harness.press(Message::CloseRequested);

    assert!(!root.exists());
}

#[test]
fn the_end_of_the_native_drag_removes_the_staging_folder() {
    use porthmos_core::transfer::rows::RowState;
    for event in [crate::native::NativeEvent::Finished, crate::native::NativeEvent::Cancelled] {
        let mut harness = Harness::new();
        let probe = remote_ready(&mut harness, &["a"]);
        let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
        deliver(&root, &["a"]);
        feed_rows(&mut harness, staging_rows(RowState::Done));
        assert_eq!(probe.started().len(), 1);
        assert!(root.is_dir());

        probe.emit(event);
        harness.press(Message::NativePoll);

        assert!(!root.exists(), "{event:?}");
        assert!(!harness.app.drag().is_tracking());
    }
}

#[test]
fn escape_during_the_native_drag_of_staged_files_cancels_through_the_native_layer() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    deliver(&root, &["a"]);
    feed_rows(&mut harness, staging_rows(RowState::Done));

    plain(&mut harness, Named::Escape);

    assert_eq!(probe.cancels(), 1);
    assert!(root.is_dir());
}

#[test]
fn a_refused_native_start_after_staging_removes_the_folder_and_tells_the_user() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    let probe = remote_ready(&mut harness, &["a"]);
    let (_, root) = copy_into_staging(&leave_remote(&mut harness, 1));
    probe.refuse_with("no serial");
    deliver(&root, &["a"]);

    feed_rows(&mut harness, staging_rows(RowState::Done));

    assert!(harness.app.notices().last().unwrap().message.contains("no serial"));
    assert!(!root.exists());
    assert!(!harness.app.drag().is_tracking());
}

#[test]
fn a_staging_folder_that_cannot_be_created_ends_the_drag_with_a_notice() {
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a"]);
    harness.app.set_staging_parent(harness.scratch().join("missing").join("deeper"));

    let sent = leave_remote(&mut harness, 1);

    assert!(sent.is_empty());
    assert_eq!(harness.app.notices().last().unwrap().severity, Severity::Warning);
    assert!(!harness.app.drag().is_tracking());
}

#[test]
fn leaving_with_remote_entries_without_a_native_backend_keeps_the_internal_drag() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    let remote = VfsPath::new("/srv/in");
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: remote.clone(),
        entries: vec![entry(&remote, "r")],
    });
    harness.sent();

    let sent = leave_remote(&mut harness, 1);

    assert!(sent.is_empty());
    assert_eq!(harness.app.drag().phase(), crate::drag::Phase::Internal);
}

fn select_remote(harness: &mut Harness, rows: &[usize]) {
    harness.press(Message::ModifiersChanged(Modifiers::empty()));
    harness.press(Message::PanelClicked(PanelSide::Remote, rows[0]));
    harness.press(Message::ModifiersChanged(Modifiers::COMMAND));
    for row in &rows[1..] {
        harness.press(Message::PanelClicked(PanelSide::Remote, *row));
    }
    harness.press(Message::ModifiersChanged(Modifiers::empty()));
}

#[test]
fn a_drag_carries_only_the_entries_of_the_panel_it_started_in() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["x", "y"]);
    select_rows(&mut harness, &[1, 2]);
    remote_ready(&mut harness, &["a", "b"]);

    let sent = leave_remote(&mut harness, 2);

    let (entries, _) = copy_into_staging(&sent);
    assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), vec!["b"]);
}

#[test]
fn the_progress_label_follows_the_snapshot() {
    use porthmos_core::transfer::rows::RowState;
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a", "b", "c"]);
    select_remote(&mut harness, &[1, 2, 3]);
    leave_remote(&mut harness, 2);

    assert_eq!(harness.app.drag_hint().unwrap().text, "Preparing 3 files…");

    let mut rows = staging_rows(RowState::Running);
    rows[0].bytes_done = 41;
    rows[0].bytes_total = 100;
    feed_rows(&mut harness, rows);

    let hint = harness.app.drag_hint().unwrap();
    assert_eq!(hint.text, "Preparing 3 files… 41%");
    assert!(!hint.refused);
}

#[test]
fn the_progress_label_says_items_when_a_folder_is_included() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    let remote = VfsPath::new("/srv/in");
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: remote.clone(),
        entries: vec![folder_entry(&remote, "dir"), entry(&remote, "f")],
    });
    harness.attach_native();
    select_remote(&mut harness, &[1, 2]);
    leave_remote(&mut harness, 2);

    assert_eq!(harness.app.drag_hint().unwrap().text, "Preparing 2 items…");
}

#[test]
fn the_label_turns_to_taking_long_after_fifteen_seconds() {
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a"]);
    leave_remote(&mut harness, 1);

    harness.press(Message::StagingTick(std::time::Instant::now() + std::time::Duration::from_secs(10)));
    assert_eq!(harness.app.drag_hint().unwrap().text, "Preparing 1 file…");

    harness.press(Message::StagingTick(std::time::Instant::now() + std::time::Duration::from_secs(16)));
    assert_eq!(harness.app.drag_hint().unwrap().text, "Taking long — keep holding or Esc");
}

#[test]
fn the_hint_anchor_stays_inside_the_window_while_staging() {
    let mut harness = Harness::new();
    remote_ready(&mut harness, &["a"]);
    leave_remote(&mut harness, 1);
    harness.press(Message::CursorMoved(iced::Point::new(-300.0, 9000.0)));

    let anchor = harness.app.hint_anchor().unwrap();

    let size = harness.app.window_size();
    assert!(anchor.x >= 0.0 && anchor.x <= size.width);
    assert!(anchor.y >= 0.0 && anchor.y <= size.height);
}

#[test]
fn toolbar_copy_is_disabled_without_a_connection() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));

    let state = harness.app.toolbar_state();

    assert!(!state.copy && !state.move_);
    assert!(state.delete && state.rename);
}

#[test]
fn toolbar_copy_enables_with_a_selection_and_a_connected_remote() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));

    let state = harness.app.toolbar_state();

    assert!(state.copy && state.move_ && state.delete);
}

#[test]
fn toolbar_file_actions_are_disabled_on_the_parent_row() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Local, 0));

    let state = harness.app.toolbar_state();

    assert!(!state.copy && !state.move_ && !state.delete && !state.rename);
}

#[test]
fn toolbar_new_folder_refresh_filter_and_transfers_need_only_the_local_panel() {
    let harness = Harness::new();

    let state = harness.app.toolbar_state();

    assert!(state.new_folder && state.refresh && state.filter && state.transfers);
}

#[test]
fn transfer_possible_needs_the_other_panel_and_a_target() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    assert!(!harness.app.transfer_possible(PanelSide::Local));

    upload_target(&mut harness);

    assert!(harness.app.transfer_possible(PanelSide::Local));
    assert!(!harness.app.transfer_possible(PanelSide::Remote));
}

#[test]
fn running_transfer_count_counts_unfinished_rows() {
    use porthmos_core::transfer::rows::{RowKind, RowState};
    let mut harness = Harness::new();
    assert_eq!(harness.app.running_transfer_count(), 0);

    feed_rows(
        &mut harness,
        vec![
            queue_row(RowKind::Single(1), RowState::Running),
            queue_row(RowKind::Single(2), RowState::Queued),
            queue_row(RowKind::Single(3), RowState::Done),
        ],
    );

    assert_eq!(harness.app.running_transfer_count(), 2);
}

#[test]
fn the_more_menu_toggles_and_select_all_selects_every_entry() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    assert!(!harness.app.more_open());

    harness.press(Message::MoreMenuToggled);
    assert!(harness.app.more_open());
    harness.press(Message::MoreItemChosen(crate::menus::MoreItem::SelectAll));

    assert!(!harness.app.more_open());
    assert_eq!(selected_local(&harness), ["a", "b"]);
}

#[test]
fn choosing_hidden_files_from_the_more_menu_toggles_them() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    assert!(!harness.app.hidden_shown());

    harness.press(Message::MoreItemChosen(crate::menus::MoreItem::ToggleHidden));

    assert!(harness.app.hidden_shown());
}

fn list_local_at(harness: &mut Harness, folder: &str, names: &[&str]) {
    let path = VfsPath::new(folder);
    let entries = names.iter().map(|name| entry(&path, name)).collect();
    harness.feed(Event::Listed { location: Location::Local, path, entries });
}

fn local_folder(harness: &Harness) -> VfsPath {
    harness.app.local().panel.path().clone()
}

fn listed_paths(sent: &[Command]) -> Vec<String> {
    sent.iter()
        .filter_map(|command| match command {
            Command::List { path: Some(path), .. } => Some(path.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn navigate_up_lists_the_parent() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.sent();

    harness.press(Message::NavigateUp(PanelSide::Local));

    assert_eq!(listed_paths(&harness.sent()), ["/home"]);
}

#[test]
fn history_starts_empty() {
    let harness = Harness::new();

    assert!(!harness.app.can_go_back(PanelSide::Local));
    assert!(!harness.app.can_go_forward(PanelSide::Local));
}

#[test]
fn back_and_forward_walk_the_visited_folders_without_recording_themselves() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::NavigateUp(PanelSide::Local));
    list_local_at(&mut harness, "/home", &["tester"]);
    assert!(harness.app.can_go_back(PanelSide::Local));
    harness.sent();

    harness.press(Message::NavigateBack(PanelSide::Local));
    assert_eq!(listed_paths(&harness.sent()), ["/home/tester"]);
    list_local_at(&mut harness, "/home/tester", &["a"]);

    assert!(!harness.app.can_go_back(PanelSide::Local));
    assert!(harness.app.can_go_forward(PanelSide::Local));
    harness.press(Message::NavigateForward(PanelSide::Local));
    assert_eq!(listed_paths(&harness.sent()), ["/home"]);
}

#[test]
fn a_crumb_press_lists_that_folder() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.sent();

    harness.press(Message::CrumbPressed(PanelSide::Local, VfsPath::new("/home")));

    assert_eq!(listed_paths(&harness.sent()), ["/home"]);
}

#[test]
fn submitting_a_path_lists_it_and_closes_the_editor() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.sent();

    harness.press(Message::PathEditStarted(PanelSide::Local));
    assert!(harness.app.path_edit().is_some());
    harness.press(Message::PathTyped("/tmp".to_string()));
    harness.press(Message::PathSubmitted);

    assert_eq!(listed_paths(&harness.sent()), ["/tmp"]);
    assert!(harness.app.path_edit().is_none());
}

#[test]
fn a_leading_tilde_means_the_local_start_folder() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::PathEditStarted(PanelSide::Local));
    harness.press(Message::PathTyped("~/docs".to_string()));
    harness.press(Message::PathSubmitted);

    assert_eq!(listed_paths(&harness.sent()), ["/home/tester/docs"]);
}

#[test]
fn an_empty_path_submission_just_closes_the_editor() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::PathEditStarted(PanelSide::Local));
    harness.press(Message::PathTyped("   ".to_string()));
    harness.press(Message::PathSubmitted);

    assert!(harness.sent().is_empty());
    assert!(harness.app.path_edit().is_none());
}

#[test]
fn a_path_that_fails_to_list_leaves_the_panel_and_its_history_alone() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::PathEditStarted(PanelSide::Local));
    harness.press(Message::PathTyped("/nope".to_string()));
    harness.press(Message::PathSubmitted);
    harness.feed(Event::Notice { severity: Severity::Error, message: "no such folder".to_string() });

    assert_eq!(local_folder(&harness), harness.start_folder());
    assert!(!harness.app.can_go_back(PanelSide::Local));
    assert_eq!(harness.app.notices().last().unwrap().message, "no such folder");
}

#[test]
fn typing_in_the_path_field_does_not_trigger_shortcuts() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    let cursor = harness.app.local().panel.cursor();
    harness.press(Message::PathEditStarted(PanelSide::Local));

    key(&mut harness, Key::Character("j".into()), Modifiers::empty());

    assert_eq!(harness.app.local().panel.cursor(), cursor);
    assert!(harness.app.path_edit().is_some());
}

#[test]
fn escape_cancels_the_path_edit() {
    let mut harness = Harness::new();
    harness.press(Message::PathEditStarted(PanelSide::Local));

    plain(&mut harness, Named::Escape);

    assert!(harness.app.path_edit().is_none());
}

#[test]
fn escape_closes_the_more_menu() {
    let mut harness = Harness::new();
    harness.press(Message::MoreMenuToggled);

    plain(&mut harness, Named::Escape);

    assert!(!harness.app.more_open());
}

#[test]
fn up_is_unavailable_at_the_root() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    assert!(harness.app.can_go_up(PanelSide::Local));

    harness.press(Message::CrumbPressed(PanelSide::Local, VfsPath::root()));
    list_local_at(&mut harness, "/", &["home"]);

    assert!(!harness.app.can_go_up(PanelSide::Local));
}

#[test]
fn closing_a_tab_asks_core_to_disconnect_that_session() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    connect(&mut harness, 2, "beta");
    harness.sent();

    harness.press(Message::TabClosed(0));

    assert!(matches!(harness.sent().as_slice(), [Command::Disconnect { session: 1 }]));
}

#[test]
fn closing_a_tab_does_not_remove_it_before_core_confirms() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");

    harness.press(Message::TabClosed(0));

    assert_eq!(tab_names(&harness), ["alpha"]);
    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });
    assert!(tab_names(&harness).is_empty());
}

#[test]
fn closing_an_out_of_range_tab_does_nothing() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha");
    harness.sent();

    harness.press(Message::TabClosed(5));

    assert!(harness.sent().is_empty());
}

fn copied(sent: &[Command]) -> Vec<(Location, Vec<String>, Location, VfsPath)> {
    sent.iter()
        .filter_map(|command| match command {
            Command::Copy { from, entries, to, dest_dir } => {
                Some((*from, entries.iter().map(|entry| entry.name.clone()).collect(), *to, dest_dir.clone()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_right_arrow_copies_the_local_selection_to_the_remote_folder() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    harness.sent();

    harness.press(Message::TransferArrowPressed(PanelSide::Local));

    let expected = vec![(Location::Local, vec!["a".to_string()], Location::Session(1), VfsPath::new("/srv/in"))];
    assert_eq!(copied(&harness.sent()), expected);
}

#[test]
fn the_left_arrow_copies_the_remote_selection_even_when_the_local_panel_is_active() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Remote, 1));
    harness.press(Message::PanelClicked(PanelSide::Local, 0));
    assert_eq!(harness.app.active_side(), PanelSide::Local);
    harness.sent();

    harness.press(Message::TransferArrowPressed(PanelSide::Remote));

    let expected = vec![(Location::Session(1), vec!["old".to_string()], Location::Local, harness.start_folder())];
    assert_eq!(copied(&harness.sent()), expected);
}

#[test]
fn an_arrow_without_a_selection_sends_nothing() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.press(Message::PanelClicked(PanelSide::Local, 0));
    harness.sent();

    harness.press(Message::TransferArrowPressed(PanelSide::Local));

    assert!(harness.sent().is_empty());
}

#[test]
fn an_arrow_without_a_connection_sends_nothing_and_adds_no_notice() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    harness.sent();
    let notices = harness.app.notices().len();

    harness.press(Message::TransferArrowPressed(PanelSide::Local));

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.notices().len(), notices);
}

fn context_target(harness: &Harness) -> Option<crate::menus::MenuTarget> {
    harness.app.context_menu().map(|menu| menu.target.clone())
}

#[test]
fn right_clicking_an_unselected_row_selects_only_it_and_opens_the_menu() {
    use crate::menus::MenuTarget;
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b"]);

    harness.press(Message::RowRightPressed(PanelSide::Local, 2));

    assert_eq!(selected_local(&harness), ["b"]);
    assert_eq!(context_target(&harness), Some(MenuTarget::Entries { count: 1, only_parent: false, file: true }));
}

#[test]
fn right_clicking_inside_the_selection_keeps_the_selection() {
    use crate::menus::MenuTarget;
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a", "b", "c"]);
    select_rows(&mut harness, &[1, 2]);

    harness.press(Message::RowRightPressed(PanelSide::Local, 1));

    assert_eq!(selected_local(&harness), ["a", "b"]);
    assert_eq!(harness.app.local().panel.cursor(), 1);
    assert_eq!(context_target(&harness), Some(MenuTarget::Entries { count: 2, only_parent: false, file: false }));
}

#[test]
fn right_clicking_the_parent_row_builds_a_parent_only_target() {
    use crate::menus::MenuTarget;
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);

    harness.press(Message::RowRightPressed(PanelSide::Local, 0));

    assert_eq!(context_target(&harness), Some(MenuTarget::Entries { count: 1, only_parent: true, file: false }));
}

#[test]
fn right_clicking_empty_space_opens_a_background_menu_for_that_panel() {
    use crate::menus::MenuTarget;
    let mut harness = Harness::new();
    upload_target(&mut harness);

    harness.press(Message::BackgroundRightPressed(PanelSide::Remote));

    assert_eq!(context_target(&harness), Some(MenuTarget::Background));
    assert_eq!(harness.app.active_side(), PanelSide::Remote);
}

#[test]
fn the_menu_opens_at_the_last_pointer_position() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::PointerMoved(Point::new(120.0, 80.0)));

    harness.press(Message::RowRightPressed(PanelSide::Local, 1));

    assert_eq!(harness.app.context_menu().unwrap().at, Point::new(120.0, 80.0));
}

#[test]
fn choosing_copy_from_the_menu_sends_the_same_command_as_f5() {
    use crate::menus::MenuItem;
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    upload_target(&mut harness);
    harness.press(Message::RowRightPressed(PanelSide::Local, 1));
    harness.sent();

    harness.press(Message::MenuItemChosen(MenuItem::CopyToOther));

    let expected = vec![(Location::Local, vec!["a".to_string()], Location::Session(1), VfsPath::new("/srv/in"))];
    assert_eq!(copied(&harness.sent()), expected);
    assert!(harness.app.context_menu().is_none());
}

#[test]
fn choosing_delete_opens_the_confirm_modal() {
    use crate::menus::MenuItem;
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::RowRightPressed(PanelSide::Local, 1));

    harness.press(Message::MenuItemChosen(MenuItem::Delete));

    assert!(matches!(harness.app.modal(), Some(Modal::ConfirmDelete { .. })));
}

#[test]
fn no_menu_opens_while_a_modal_is_open() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::ActionRequested(Action::NewFolder));
    assert!(harness.app.modal().is_some());

    harness.press(Message::RowRightPressed(PanelSide::Local, 1));
    harness.press(Message::BackgroundRightPressed(PanelSide::Local));

    assert!(harness.app.context_menu().is_none());
}

#[test]
fn dismissing_and_escape_close_the_menu() {
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::RowRightPressed(PanelSide::Local, 1));
    harness.press(Message::ContextMenuDismissed);
    assert!(harness.app.context_menu().is_none());

    harness.press(Message::RowRightPressed(PanelSide::Local, 1));
    plain(&mut harness, Named::Escape);

    assert!(harness.app.context_menu().is_none());
}

#[test]
fn a_disabled_menu_item_does_nothing_when_delivered() {
    use crate::menus::MenuItem;
    let mut harness = Harness::new();
    fill_local(&mut harness, &["a"]);
    harness.press(Message::RowRightPressed(PanelSide::Local, 0));
    harness.sent();

    harness.press(Message::MenuItemChosen(MenuItem::Delete));

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}
