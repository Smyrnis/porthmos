use std::sync::Arc;

use iced::Size;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::preview::{Preview, PreviewContent};
use porthmos_core::{Command, Entry, Event, Location, VfsPath};

use crate::menus::MoreItem;
use crate::preview_view::PreviewStatus;
use crate::shortcuts::KeyPress;
use crate::testing::Harness;
use crate::tools::Tool;
use crate::view::preview::ROW_HEIGHT;
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

fn letter(harness: &mut Harness, character: &str) {
    press(harness, Key::Character(character.into()), Modifiers::empty());
}

fn file(folder: &VfsPath, name: &str) -> Entry {
    Entry { name: name.to_string(), path: folder.join(name), size: 10, ..Entry::default() }
}

fn folder_entry(folder: &VfsPath, name: &str) -> Entry {
    Entry { is_dir: true, ..file(folder, name) }
}

fn local_harness() -> Harness {
    let mut harness = Harness::new();
    harness.app.set_window_size(Size::new(1100.0, 700.0));
    let folder = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: folder.clone(),
        entries: vec![folder_entry(&folder, "docs"), file(&folder, "a.txt"), file(&folder, "b.txt")],
    });
    harness.sent();
    harness
}

fn on_row(harness: &mut Harness, row: usize) {
    harness.app.local.panel.set_cursor(row);
}

fn text_preview(lines: usize) -> Arc<Preview> {
    let text: String = (0..lines).map(|line| format!("line {line}\n")).collect();
    Arc::new(Preview {
        size: Some(text.len() as u64),
        shown: text.len(),
        truncated: false,
        content: PreviewContent::Text(text),
    })
}

fn ready(harness: &mut Harness, request_id: u64, lines: usize) {
    harness.feed(Event::PreviewReady { request_id, name: "a.txt".to_string(), result: Ok(text_preview(lines)) });
}

fn preview_state(harness: &Harness) -> &crate::preview_view::PreviewState {
    match harness.app.tool() {
        Some(Tool::Preview(state)) => state,
        other => panic!("expected the preview tool, got {other:?}"),
    }
}

fn open_a(harness: &mut Harness) -> u64 {
    on_row(harness, 2);
    letter(harness, "p");
    match harness.sent().as_slice() {
        [Command::Preview { request_id, .. }] => *request_id,
        other => panic!("expected one Preview command, got {other:?}"),
    }
}

fn viewport(harness: &Harness) -> f32 {
    list_height(harness.app.tool().unwrap(), harness.app.window_size())
}

#[test]
fn p_on_a_file_opens_a_loading_preview_and_asks_core() {
    let mut harness = local_harness();
    on_row(&mut harness, 2);

    letter(&mut harness, "p");

    let folder = harness.start_folder();
    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [Command::Preview { location: Location::Local, path, .. }] if *path == folder.join("a.txt")),
        "{sent:?}"
    );
    assert!(preview_state(&harness).is_loading());
    assert_eq!(preview_state(&harness).title(), "a.txt");
}

#[test]
fn p_on_a_folder_or_the_parent_row_does_nothing() {
    let mut harness = local_harness();

    on_row(&mut harness, 0);
    letter(&mut harness, "p");
    on_row(&mut harness, 1);
    letter(&mut harness, "p");

    assert!(harness.app.tool().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn p_with_two_files_selected_does_nothing() {
    let mut harness = local_harness();
    harness.press(Message::ActionRequested(Action::SelectAll));
    harness.sent();

    letter(&mut harness, "p");

    assert!(harness.app.tool().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn previewing_a_remote_file_names_its_session() {
    let mut harness = local_harness();
    harness.feed(Event::Connected { session: 4, name: "alpha".to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(4),
        path: VfsPath::new("/srv"),
        entries: vec![file(&VfsPath::new("/srv"), "r.log")],
    });
    named(&mut harness, Named::Tab);
    harness.sent();
    harness.app.side_mut(PanelSide::Remote).unwrap().panel.set_cursor(1);

    letter(&mut harness, "p");

    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [Command::Preview { location: Location::Session(4), path, .. }] if *path == VfsPath::new("/srv/r.log")),
        "{sent:?}"
    );
}

#[test]
fn the_more_menu_opens_the_preview_of_the_cursor_file() {
    let mut harness = local_harness();
    on_row(&mut harness, 3);

    harness.press(Message::MoreItemChosen(MoreItem::Preview));

    assert_eq!(preview_state(&harness).title(), "b.txt");
}

#[test]
fn each_preview_request_gets_a_fresh_id() {
    let mut harness = local_harness();
    let first = open_a(&mut harness);
    named(&mut harness, Named::Escape);
    harness.sent();

    let second = open_a(&mut harness);

    assert_ne!(first, second);
}

#[test]
fn a_matching_result_fills_the_preview() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);

    ready(&mut harness, id, 3);

    assert!(matches!(preview_state(&harness).status(), PreviewStatus::Ready(_)));
    assert_eq!(preview_state(&harness).row_count(), 3);
}

#[test]
fn a_failed_result_shows_the_message() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);

    harness.feed(Event::PreviewReady { request_id: id, name: "a.txt".to_string(), result: Err("denied".to_string()) });

    assert_eq!(preview_state(&harness).status(), &PreviewStatus::Failed("denied".to_string()));
}

#[test]
fn a_result_for_an_older_request_is_ignored() {
    let mut harness = local_harness();
    let first = open_a(&mut harness);
    named(&mut harness, Named::Escape);
    harness.sent();
    let second = open_a(&mut harness);

    ready(&mut harness, first, 3);

    assert!(preview_state(&harness).is_loading());
    ready(&mut harness, second, 3);
    assert!(!preview_state(&harness).is_loading());
}

#[test]
fn a_result_after_the_preview_closed_is_ignored() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 3);
    named(&mut harness, Named::Escape);

    ready(&mut harness, id, 3);

    assert!(harness.app.tool().is_none());
}

#[test]
fn closing_a_loading_preview_cancels_it() {
    let mut harness = local_harness();
    open_a(&mut harness);

    named(&mut harness, Named::Escape);

    assert!(harness.app.tool().is_none());
    assert_eq!(harness.sent(), vec![Command::CancelPreview]);
}

#[test]
fn closing_a_finished_preview_sends_nothing() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 3);

    named(&mut harness, Named::Escape);

    assert!(harness.app.tool().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn opening_another_tool_cancels_a_loading_preview() {
    let mut harness = local_harness();
    open_a(&mut harness);

    harness.press(Message::MoreItemChosen(MoreItem::Bookmarks));

    assert!(harness.sent().contains(&Command::CancelPreview));
}

#[test]
fn arrow_keys_scroll_by_one_row_and_stop_at_the_ends() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 200);

    named(&mut harness, Named::ArrowUp);
    assert_eq!(harness.app.tool_scroll(), 0.0);
    named(&mut harness, Named::ArrowDown);
    named(&mut harness, Named::ArrowDown);

    assert_eq!(harness.app.tool_scroll(), 2.0 * ROW_HEIGHT);
}

#[test]
fn page_keys_scroll_by_a_viewport_less_one_row() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 500);
    let page = (viewport(&harness) / ROW_HEIGHT).floor() * ROW_HEIGHT - ROW_HEIGHT;

    named(&mut harness, Named::PageDown);
    assert_eq!(harness.app.tool_scroll(), page);
    press(&mut harness, Key::Named(Named::Space), Modifiers::empty());
    assert_eq!(harness.app.tool_scroll(), 2.0 * page);
    named(&mut harness, Named::PageUp);

    assert_eq!(harness.app.tool_scroll(), page);
}

#[test]
fn home_and_end_jump_to_the_ends() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 500);
    let last = 500.0 * ROW_HEIGHT - viewport(&harness);

    named(&mut harness, Named::End);
    assert_eq!(harness.app.tool_scroll(), last);
    named(&mut harness, Named::Home);

    assert_eq!(harness.app.tool_scroll(), 0.0);
}

#[test]
fn a_short_file_never_scrolls() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 3);

    named(&mut harness, Named::ArrowDown);
    named(&mut harness, Named::End);

    assert_eq!(harness.app.tool_scroll(), 0.0);
}

#[test]
fn pointer_scrolling_is_tracked() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 200);

    harness.press(Message::PreviewScrolled(123.0));

    assert_eq!(harness.app.tool_scroll(), 123.0);
}

#[test]
fn a_new_preview_starts_at_the_top() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 200);
    named(&mut harness, Named::End);
    named(&mut harness, Named::Escape);

    open_a(&mut harness);

    assert_eq!(harness.app.tool_scroll(), 0.0);
}

#[test]
fn the_close_button_closes_the_preview() {
    let mut harness = local_harness();
    let id = open_a(&mut harness);
    ready(&mut harness, id, 3);

    harness.press(Message::ToolClosed);

    assert!(harness.app.tool().is_none());
}
