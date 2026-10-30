use porthmos_core::listing::SortSpec;
use porthmos_core::{Entry, VfsPath};

use super::{connection_text, keyring_text, panel_summary, selection_text};
use crate::panel::{Modifiers, Panel};
use crate::testing::Harness;
use porthmos_core::Event;

fn panel() -> Panel {
    let folder = VfsPath::new("/data");
    let entries = ["alpha", "beta", "gamma"]
        .into_iter()
        .map(|name| Entry { name: name.to_string(), path: folder.join(name), ..Entry::default() })
        .collect();
    let mut panel = Panel::new(folder.clone(), SortSpec::default(), false);
    panel.replace_listing(folder, entries);
    panel
}

#[test]
fn the_summary_counts_the_shown_items() {
    assert_eq!(panel_summary(&panel()), "3 items");
}

#[test]
fn the_summary_names_the_filter_and_the_match_count() {
    let mut panel = panel();
    panel.set_filter(Some("a"));

    assert_eq!(panel_summary(&panel), "3 of 3 match \"a\"");

    panel.set_filter(Some("ga"));
    assert_eq!(panel_summary(&panel), "1 of 3 match \"ga\"");
}

#[test]
fn the_summary_adds_the_free_space_when_it_is_known() {
    let mut panel = panel();
    panel.set_free_space(VfsPath::new("/data"), Some(3 * 1024 * 1024 * 1024));

    assert_eq!(panel_summary(&panel), "3 items  ·  3.0G free");
}

fn sized_panel() -> Panel {
    let folder = VfsPath::new("/data");
    let mut entries: Vec<Entry> = [("a", 1024u64), ("b", 3072)]
        .into_iter()
        .map(|(name, size)| Entry { name: name.to_string(), path: folder.join(name), size, ..Entry::default() })
        .collect();
    entries.push(Entry { name: "dir".to_string(), path: folder.join("dir"), is_dir: true, ..Entry::default() });
    let mut panel = Panel::new(folder.clone(), SortSpec::default(), false);
    panel.replace_listing(folder, entries);
    panel
}

#[test]
fn nothing_selected_has_no_selection_text() {
    assert_eq!(selection_text(&sized_panel()), None);
}

#[test]
fn the_selection_text_reports_count_and_file_size() {
    let mut panel = sized_panel();
    panel.click(2, Modifiers::default());
    panel.click(3, Modifiers { ctrl: true, shift: false });

    assert_eq!(selection_text(&panel), Some("2 selected  ·  4.0K".to_string()));
}

#[test]
fn a_selected_folder_alone_has_no_size() {
    let mut panel = sized_panel();
    panel.click(1, Modifiers::default());

    assert_eq!(selection_text(&panel), Some("1 selected".to_string()));
}

#[test]
fn connection_text_says_no_connection_when_the_right_panel_is_empty() {
    let harness = Harness::new();

    assert_eq!(connection_text(&harness.app), (false, "No connection".to_string()));
}

#[test]
fn connection_text_names_the_active_connection() {
    let mut harness = Harness::new();
    harness.feed(Event::Connected { session: 1, name: "alpha".to_string(), shell_available: false });
    harness.feed(Event::Connected { session: 2, name: "beta".to_string(), shell_available: false });

    assert_eq!(connection_text(&harness.app), (true, "beta connected".to_string()));
}

#[test]
fn the_summary_says_when_hidden_entries_are_shown() {
    let mut panel = panel();
    panel.toggle_hidden();

    assert_eq!(panel_summary(&panel), "3 items  ·  hidden shown");
}

#[test]
fn small_selections_show_bytes_with_a_unit() {
    let folder = VfsPath::new("/data");
    let entry = Entry { name: "tiny".to_string(), path: folder.join("tiny"), size: 3, ..Entry::default() };
    let mut panel = Panel::new(folder.clone(), SortSpec::default(), false);
    panel.replace_listing(folder, vec![entry]);
    panel.click(1, Modifiers::default());

    assert_eq!(selection_text(&panel), Some("1 selected  ·  3 B".to_string()));
}

#[test]
fn the_keyring_text_shows_only_while_waiting() {
    assert_eq!(keyring_text(true), Some("Waiting for the keyring\u{2026}"));
    assert_eq!(keyring_text(false), None);
}
