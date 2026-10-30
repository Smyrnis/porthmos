use iced::Size;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};
use porthmos_core::{Command, Entry, Event, Location, VfsPath};

use crate::geometry::ROW_HEIGHT;
use crate::menus::MoreItem;
use crate::modal::Modal;
use crate::palette::{PaletteState, Target};
use crate::shortcuts::{Action, KeyPress};
use crate::testing::Harness;
use crate::tools::Tool;
use crate::view::tools::list_height;

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

fn profile(name: &str) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: "h".to_string(),
        port: 22,
        username: "u".to_string(),
        password: None,
        options: Default::default(),
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
        source: ConnectionSource::Profile,
    }
}

fn file(folder: &VfsPath, name: &str) -> Entry {
    Entry { name: name.to_string(), path: folder.join(name), ..Entry::default() }
}

fn folder(parent: &VfsPath, name: &str) -> Entry {
    Entry { is_dir: true, ..file(parent, name) }
}

fn list_local(harness: &mut Harness, entries: Vec<Entry>) {
    let path = harness.start_folder();
    harness.feed(Event::Listed { location: Location::Local, path, entries });
}

fn connect(harness: &mut Harness, session: u64, name: &str, shell: bool) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: shell });
}

fn list_session(harness: &mut Harness, session: u64, entries: Vec<Entry>) {
    harness.feed(Event::Listed { location: Location::Session(session), path: VfsPath::new("/srv"), entries });
}

fn open(harness: &mut Harness) {
    ctrl(harness, "p");
    harness.sent();
}

fn typed(harness: &mut Harness, text: &str) {
    harness.press(Message::PaletteTyped(text.to_string()));
}

fn state(harness: &Harness) -> &PaletteState {
    match harness.app.tool() {
        Some(Tool::Palette(state)) => state,
        other => panic!("expected the palette, got {other:?}"),
    }
}

fn labels(harness: &Harness) -> Vec<String> {
    state(harness).hits().iter().map(|entry| entry.label.clone()).collect()
}

fn hints(harness: &Harness) -> Vec<Option<String>> {
    state(harness).hits().iter().map(|entry| entry.hint.clone()).collect()
}

fn palette_open(harness: &Harness) -> bool {
    matches!(harness.app.tool(), Some(Tool::Palette(_)))
}

#[test]
fn ctrl_p_opens_the_palette_with_commands_and_hosts() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![profile("alpha")]));
    let start = harness.start_folder();
    list_local(&mut harness, vec![file(&start, "notes.txt")]);
    harness.sent();

    ctrl(&mut harness, "p");

    let shown = labels(&harness);
    assert!(shown.contains(&"Search…".to_string()), "{shown:?}");
    assert!(shown.contains(&"Connect alpha".to_string()), "{shown:?}");
    assert!(!shown.contains(&"notes.txt".to_string()), "{shown:?}");
    assert_eq!(state(&harness).query(), "");
    assert_eq!(harness.sent(), [Command::ListProfiles]);
}

#[test]
fn profiles_arriving_while_the_palette_is_open_add_the_hosts_and_keep_the_query() {
    let mut harness = Harness::new();
    open(&mut harness);
    typed(&mut harness, "@alp");
    assert!(labels(&harness).is_empty());

    harness.feed(Event::Profiles(vec![profile("alpha"), profile("beta")]));

    assert_eq!(labels(&harness), ["Connect alpha"]);
    assert_eq!(state(&harness).query(), "@alp");
}

#[test]
fn profiles_arriving_while_the_palette_is_closed_do_not_open_it() {
    let mut harness = Harness::new();

    harness.feed(Event::Profiles(vec![profile("alpha")]));

    assert!(harness.app.tool().is_none());
}

#[test]
fn profiles_arriving_while_another_tool_is_open_leave_it_alone() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Search));

    harness.feed(Event::Profiles(vec![profile("alpha")]));

    assert!(matches!(harness.app.tool(), Some(Tool::Search(_))));
}

#[test]
fn ctrl_p_is_ignored_behind_a_modal() {
    let mut harness = Harness::new();
    named(&mut harness, Named::F7);
    assert!(matches!(harness.app.modal(), Some(Modal::NewFolder { .. })));

    ctrl(&mut harness, "p");
    harness.press(Message::ActionRequested(Action::Palette));
    harness.press(Message::MoreItemChosen(MoreItem::Palette));

    assert!(harness.app.tool().is_none());
}

#[test]
fn ctrl_p_again_closes_it() {
    let mut harness = Harness::new();
    open(&mut harness);

    ctrl(&mut harness, "p");

    assert!(harness.app.tool().is_none());
}

#[test]
fn escape_closes_it() {
    let mut harness = Harness::new();
    open(&mut harness);

    named(&mut harness, Named::Escape);

    assert!(harness.app.tool().is_none());
}

#[test]
fn typing_filters_the_list_and_selects_the_first_hit() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::PaletteRowPressed(3));

    typed(&mut harness, "hist");

    assert_eq!(labels(&harness).first().map(String::as_str), Some("Transfer history…"));
    assert_eq!(state(&harness).selected(), 0);
    assert_eq!(state(&harness).query(), "hist");
}

#[test]
fn a_hash_prefix_lists_the_files_of_both_panels() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    list_local(&mut harness, vec![file(&start, "a.txt"), folder(&start, "docs")]);
    connect(&mut harness, 1, "alpha", false);
    list_session(&mut harness, 1, vec![file(&VfsPath::new("/srv"), "b.txt")]);
    open(&mut harness);

    typed(&mut harness, "#");

    assert_eq!(labels(&harness), ["docs/", "a.txt", "b.txt"]);
    assert_eq!(hints(&harness), [Some("local".to_string()), Some("local".to_string()), Some("alpha".to_string())]);
}

#[test]
fn file_names_with_control_characters_are_printable_and_one_line() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    let hostile = "bad\nname\u{1b}[31m.txt";
    list_local(&mut harness, vec![file(&start, hostile)]);
    open(&mut harness);

    typed(&mut harness, "#bad");

    assert_eq!(labels(&harness), ["bad?name?[31m.txt"]);
    let selected = state(&harness).selected_entry().expect("a hit");
    assert_eq!(selected.target, Target::File { location: Location::Local, name: hostile.to_string() });
}

#[test]
fn enter_runs_a_command_after_closing_the_palette() {
    let mut harness = Harness::new();
    open(&mut harness);
    typed(&mut harness, ">new folder");

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    assert!(matches!(harness.app.modal(), Some(Modal::NewFolder { .. })));
}

#[test]
fn a_command_that_opens_a_tool_replaces_the_palette() {
    let mut harness = Harness::new();
    open(&mut harness);
    typed(&mut harness, ">search");

    named(&mut harness, Named::Enter);

    assert!(matches!(harness.app.tool(), Some(Tool::Search(_))));
}

#[test]
fn the_connection_manager_command_opens_the_manager() {
    let mut harness = Harness::new();
    open(&mut harness);
    typed(&mut harness, ">connection manager");

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    assert!(harness.app.manager_open());
    assert!(harness.sent().contains(&Command::ListProfiles));
}

#[test]
fn commands_that_cannot_act_are_not_listed() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    list_local(&mut harness, vec![file(&start, "a.txt")]);
    open(&mut harness);
    let without = labels(&harness);
    for missing in ["Edit", "Preview", "Synchronize…", "Open terminal here", "Copy to other panel", "Rename"] {
        assert!(!without.contains(&missing.to_string()), "{missing} listed in {without:?}");
    }

    named(&mut harness, Named::Escape);
    connect(&mut harness, 1, "alpha", true);
    list_session(&mut harness, 1, vec![file(&VfsPath::new("/srv"), "b.txt")]);
    harness.press(Message::PanelClicked(PanelSide::Local, 1));
    open(&mut harness);
    let with = labels(&harness);

    for present in ["Edit", "Preview", "Synchronize…", "Open terminal here", "Copy to other panel", "Rename"] {
        assert!(with.contains(&present.to_string()), "{present} missing from {with:?}");
    }
}

#[test]
fn enter_on_a_host_sends_connect() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![profile("alpha")]));
    open(&mut harness);
    typed(&mut harness, "@alpha");

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.sent(), [Command::Connect { profile: "alpha".to_string() }]);
}

#[test]
fn enter_on_a_connected_host_disconnect_sends_disconnect() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha", false);
    harness.sent();
    open(&mut harness);
    typed(&mut harness, "@disconnect");

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.sent(), [Command::Disconnect { session: 1 }]);
}

#[test]
fn enter_on_a_file_puts_the_cursor_on_it() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    list_local(&mut harness, vec![file(&start, "a.txt"), file(&start, "b.txt"), file(&start, "c.txt")]);
    open(&mut harness);
    typed(&mut harness, "#c.txt");

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.app.active_side(), PanelSide::Local);
    assert_eq!(harness.app.local().panel.cursor_entry().map(|entry| entry.name), Some("c.txt".to_string()));
}

#[test]
fn enter_on_a_file_of_the_connection_shows_that_panel() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "alpha", false);
    list_session(&mut harness, 1, vec![file(&VfsPath::new("/srv"), "a.txt"), file(&VfsPath::new("/srv"), "b.txt")]);
    assert_eq!(harness.app.active_side(), PanelSide::Local);
    open(&mut harness);
    typed(&mut harness, "#b.txt");

    named(&mut harness, Named::Enter);

    assert_eq!(harness.app.active_side(), PanelSide::Remote);
    let panel = harness.app.right_panel().expect("the connection has a panel");
    assert_eq!(panel.cursor_entry().map(|entry| entry.name), Some("b.txt".to_string()));
}

#[test]
fn enter_on_a_file_that_vanished_does_nothing() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    list_local(&mut harness, vec![file(&start, "a.txt")]);
    open(&mut harness);
    typed(&mut harness, "#a.txt");
    list_local(&mut harness, Vec::new());

    named(&mut harness, Named::Enter);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.app.local().panel.cursor(), 0);
}

#[test]
fn enter_with_no_hits_does_nothing() {
    let mut harness = Harness::new();
    open(&mut harness);
    typed(&mut harness, "zzzzqqqq");

    named(&mut harness, Named::Enter);

    assert!(palette_open(&harness));
    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn arrow_page_keys_move_the_selection_and_scroll() {
    let mut harness = Harness::new();
    open(&mut harness);
    let total = state(&harness).hit_count();
    let tool = harness.app.tool().expect("open");
    let page = ((list_height(tool, harness.app.window_size()) / ROW_HEIGHT) as usize).saturating_sub(1).max(1);
    assert!(total > page, "{total} commands should be more than a page of {page}");

    named(&mut harness, Named::ArrowDown);
    assert_eq!(state(&harness).selected(), 1);
    named(&mut harness, Named::ArrowUp);
    named(&mut harness, Named::ArrowUp);
    assert_eq!(state(&harness).selected(), 0);
    named(&mut harness, Named::PageDown);
    assert_eq!(state(&harness).selected(), page);
    named(&mut harness, Named::PageDown);
    named(&mut harness, Named::PageDown);
    assert_eq!(state(&harness).selected(), (3 * page).min(total - 1));
    assert!(harness.app.tool_scroll() > 0.0);
    named(&mut harness, Named::PageUp);
    named(&mut harness, Named::PageUp);
    named(&mut harness, Named::PageUp);
    named(&mut harness, Named::PageUp);
    assert_eq!(state(&harness).selected(), 0);
    assert_eq!(harness.app.tool_scroll(), 0.0);
}

#[test]
fn clicking_selects_and_double_clicking_runs() {
    let mut harness = Harness::new();
    open(&mut harness);

    harness.press(Message::PaletteRowPressed(2));
    assert_eq!(state(&harness).selected(), 2);
    assert!(palette_open(&harness));

    typed(&mut harness, ">new folder");
    harness.press(Message::PaletteRowActivated(0));

    assert!(harness.app.tool().is_none());
    assert!(matches!(harness.app.modal(), Some(Modal::NewFolder { .. })));
}

#[test]
fn typing_does_not_fire_panel_shortcuts() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    list_local(&mut harness, vec![file(&start, "a.txt"), file(&start, "b.txt")]);
    open(&mut harness);
    let cursor = harness.app.local().panel.cursor();

    for key in ["j", "k", "/", "e", "p", "a"] {
        press(&mut harness, Key::Character(key.into()), Modifiers::empty());
    }
    named(&mut harness, Named::Space);
    named(&mut harness, Named::Backspace);
    named(&mut harness, Named::Delete);
    named(&mut harness, Named::F7);
    named(&mut harness, Named::F5);

    assert!(palette_open(&harness));
    assert!(harness.app.modal().is_none());
    assert_eq!(harness.app.local().panel.cursor(), cursor);
    assert!(harness.sent().is_empty());
}

#[test]
fn hints_show_the_bound_shortcut() {
    let mut harness = Harness::new();
    open(&mut harness);

    let entries = state(&harness).hits();
    let hint_of = |label: &str| entries.iter().find(|entry| entry.label == label).map(|entry| entry.hint.clone());

    assert_eq!(hint_of("Bookmarks…"), Some(Some("Ctrl+B".to_string())));
    assert_eq!(hint_of("Search…"), Some(Some("Ctrl+F".to_string())));
    assert_eq!(hint_of("Connection manager"), Some(None));
}

#[test]
fn the_more_item_opens_the_palette() {
    let mut harness = Harness::new();
    harness.press(Message::MoreMenuToggled);

    harness.press(Message::MoreItemChosen(MoreItem::Palette));

    assert!(palette_open(&harness));
    assert!(!harness.app.more_open());
}

#[test]
fn fifty_thousand_files_cap_the_hits() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    let entries = (0..50_000).map(|index| file(&start, &format!("file-{index}.txt"))).collect();
    list_local(&mut harness, entries);
    open(&mut harness);

    typed(&mut harness, "#file");

    assert_eq!(state(&harness).hit_count(), crate::palette::MAX_HITS);
    assert_eq!(state(&harness).total_matches(), 50_000);
}

#[test]
fn the_palette_survives_a_window_resize() {
    let mut harness = Harness::new();
    open(&mut harness);

    harness.press(Message::WindowResized(Size::new(480.0, 320.0)));

    assert!(palette_open(&harness));
    assert!(state(&harness).hit_count() > 0);
}
