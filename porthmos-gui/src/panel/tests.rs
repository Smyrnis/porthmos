use std::time::{Duration, Instant};

use porthmos_core::listing::{Row, SortKey, SortOrder, SortSpec};
use porthmos_core::{Entry, VfsPath};

use super::{Modifiers, Panel};

const PLAIN: Modifiers = Modifiers { ctrl: false, shift: false };
const CTRL: Modifiers = Modifiers { ctrl: true, shift: false };
const SHIFT: Modifiers = Modifiers { ctrl: false, shift: true };

fn folder() -> VfsPath {
    VfsPath::new("/data")
}

fn file(name: &str) -> Entry {
    Entry { name: name.to_string(), path: folder().join(name), size: 10, ..Entry::default() }
}

fn dir(name: &str) -> Entry {
    Entry { is_dir: true, ..file(name) }
}

fn panel_with(entries: Vec<Entry>) -> Panel {
    let mut panel = Panel::new(folder(), SortSpec::default(), false);
    panel.replace_listing(folder(), entries);
    panel
}

fn abc_panel() -> Panel {
    panel_with(vec![file("a"), file("b"), file("c"), file("d")])
}

fn row_names(panel: &Panel) -> Vec<String> {
    panel
        .rows()
        .iter()
        .map(|row| match row {
            Row::Parent => "..".to_string(),
            Row::Entry(entry) => entry.name.clone(),
        })
        .collect()
}

fn selected_names(panel: &Panel) -> Vec<String> {
    panel.selected().into_iter().map(|entry| entry.name).collect()
}

#[test]
fn a_plain_click_selects_only_that_row() {
    let mut panel = abc_panel();
    panel.click(1, PLAIN);
    panel.click(2, PLAIN);

    assert_eq!(selected_names(&panel), ["b"]);
    assert_eq!(panel.cursor(), 2);
    assert_eq!(panel.selection_count(), 1);
}

#[test]
fn ctrl_click_toggles_one_row() {
    let mut panel = abc_panel();
    panel.click(1, PLAIN);
    panel.click(3, CTRL);
    assert_eq!(selected_names(&panel), ["a", "c"]);

    panel.click(1, CTRL);
    assert_eq!(selected_names(&panel), ["c"]);
    assert_eq!(panel.cursor(), 1);
}

#[test]
fn shift_click_selects_the_range_from_the_anchor() {
    let mut panel = abc_panel();
    panel.click(2, PLAIN);
    panel.click(4, SHIFT);
    assert_eq!(selected_names(&panel), ["b", "c", "d"]);

    panel.click(1, SHIFT);
    assert_eq!(selected_names(&panel), ["a", "b"]);
    assert_eq!(panel.cursor(), 1);
}

#[test]
fn the_parent_row_cannot_be_selected() {
    let mut panel = abc_panel();
    assert_eq!(panel.rows()[0], Row::Parent);

    panel.click(0, CTRL);
    assert_eq!(panel.selection_count(), 0);

    panel.click(3, PLAIN);
    panel.click(0, SHIFT);
    assert_eq!(selected_names(&panel), ["a", "b", "c"]);
    assert_eq!(panel.selection_count(), 3);

    panel.click(0, PLAIN);
    assert_eq!(panel.selection_count(), 0);
    assert_eq!(panel.cursor(), 0);
}

#[test]
fn clicking_past_the_last_row_changes_nothing() {
    let mut panel = abc_panel();
    panel.click(2, PLAIN);
    panel.click(40, PLAIN);

    assert_eq!(selected_names(&panel), ["b"]);
    assert_eq!(panel.cursor(), 2);
}

#[test]
fn refreshing_the_same_folder_keeps_the_cursor_on_the_same_name() {
    let mut panel = abc_panel();
    panel.click(3, PLAIN);

    panel.replace_listing(folder(), vec![file("0"), file("a"), file("b"), file("c"), file("d")]);

    assert_eq!(row_names(&panel)[panel.cursor()], "c");
}

#[test]
fn refreshing_when_the_cursor_name_is_gone_stays_inside_the_rows() {
    let mut panel = abc_panel();
    panel.click(4, PLAIN);

    panel.replace_listing(folder(), vec![file("a")]);

    assert_eq!(panel.cursor(), 1);
}

#[test]
fn refreshing_keeps_the_selection_of_entries_that_still_exist() {
    let mut panel = abc_panel();
    panel.click(1, PLAIN);
    panel.click(3, CTRL);
    panel.click(4, CTRL);

    panel.replace_listing(folder(), vec![file("a"), file("c"), file("e")]);

    assert_eq!(selected_names(&panel), ["a", "c"]);
    assert_eq!(panel.selection_count(), 2);
}

#[test]
fn the_anchor_survives_a_refresh_of_the_same_folder() {
    let mut panel = abc_panel();
    panel.click(2, PLAIN);

    panel.replace_listing(folder(), vec![file("0"), file("a"), file("b"), file("c"), file("d")]);
    panel.click(5, SHIFT);

    assert_eq!(selected_names(&panel), ["b", "c", "d"]);
}

#[test]
fn entering_another_folder_resets_cursor_and_selection() {
    let mut panel = abc_panel();
    panel.click(2, PLAIN);
    panel.click(3, CTRL);

    let other = VfsPath::new("/data/sub");
    panel.replace_listing(other.clone(), vec![Entry { path: other.join("a"), ..file("a") }]);

    assert_eq!(panel.path(), &other);
    assert_eq!(panel.cursor(), 0);
    assert_eq!(panel.selection_count(), 0);
}

#[test]
fn the_cursor_never_leaves_the_rows() {
    let mut panel = abc_panel();
    panel.move_cursor(-5);
    assert_eq!(panel.cursor(), 0);

    panel.move_cursor(2);
    assert_eq!(panel.cursor(), 2);

    panel.move_cursor(100);
    assert_eq!(panel.cursor(), 4);

    panel.set_cursor(900);
    assert_eq!(panel.cursor(), 4);
}

#[test]
fn the_cursor_is_zero_in_an_empty_folder_at_the_root() {
    let mut panel = Panel::new(VfsPath::root(), SortSpec::default(), false);
    panel.replace_listing(VfsPath::root(), Vec::new());

    panel.move_cursor(3);
    panel.set_cursor(7);
    panel.click(2, CTRL);

    assert!(panel.rows().is_empty());
    assert_eq!(panel.cursor(), 0);
    assert_eq!(panel.selection_count(), 0);
}

#[test]
fn a_filter_narrows_the_rows_and_keeps_the_match_count() {
    let mut panel = panel_with(vec![file("alpha.txt"), file("beta.txt"), file("alpine.md")]);

    panel.set_filter(Some("alp"));

    assert_eq!(row_names(&panel), ["..", "alpha.txt", "alpine.md"]);
    assert_eq!(panel.match_count(), (2, 3));

    panel.set_filter(None);
    assert_eq!(panel.match_count(), (3, 3));
}

#[test]
fn a_filter_drops_the_selection_of_rows_it_hides() {
    let mut panel = panel_with(vec![file("alpha.txt"), file("beta.txt")]);
    panel.click(1, PLAIN);
    panel.click(2, CTRL);

    panel.set_filter(Some("beta"));

    assert_eq!(selected_names(&panel), ["beta.txt"]);
    assert_eq!(panel.selection_count(), 1);
}

#[test]
fn hiding_dotfiles_keeps_the_cursor_on_a_visible_row() {
    let mut panel = Panel::new(folder(), SortSpec::default(), true);
    panel.replace_listing(folder(), vec![file(".env"), file("a"), file("b")]);
    panel.click(1, PLAIN);
    assert_eq!(row_names(&panel)[panel.cursor()], ".env");

    panel.toggle_hidden();

    assert!(!panel.show_hidden());
    assert_eq!(row_names(&panel), ["..", "a", "b"]);
    assert!(panel.cursor() < panel.rows().len());
    assert_eq!(panel.selection_count(), 0);
}

#[test]
fn cycling_the_sort_keeps_the_cursor_on_the_same_name() {
    let mut panel = panel_with(vec![file("a"), file("b"), file("c")]);
    panel.click(1, PLAIN);

    panel.cycle_sort();

    assert_eq!(panel.sort_spec(), SortSpec { key: SortKey::Name, order: SortOrder::Descending });
    assert_eq!(row_names(&panel), ["..", "c", "b", "a"]);
    assert_eq!(row_names(&panel)[panel.cursor()], "a");
}

#[test]
fn open_target_of_parent_is_the_parent_folder() {
    let panel = panel_with(vec![dir("sub"), file("note")]);

    assert_eq!(panel.open_target(0), Some(VfsPath::new("/")));
    assert_eq!(panel.open_target(1), Some(folder().join("sub")));
    assert_eq!(panel.open_target(2), None);
    assert_eq!(panel.open_target(9), None);
}

#[test]
fn free_space_belongs_to_the_folder_it_was_reported_for() {
    let mut panel = abc_panel();
    assert_eq!(panel.free_bytes(), None);

    panel.set_free_space(folder(), Some(4096));
    assert_eq!(panel.free_bytes(), Some(4096));

    panel.set_free_space(VfsPath::new("/elsewhere"), Some(1));
    assert_eq!(panel.free_bytes(), None);

    panel.set_free_space(folder(), None);
    assert_eq!(panel.free_bytes(), None);
}

#[test]
fn free_space_reported_before_the_listing_arrives_is_used_for_that_folder() {
    let other = VfsPath::new("/data/sub");
    let mut panel = abc_panel();

    panel.set_free_space(other.clone(), Some(77));
    assert_eq!(panel.free_bytes(), None);

    panel.replace_listing(other, Vec::new());
    assert_eq!(panel.free_bytes(), Some(77));
}

#[test]
fn fifty_thousand_entries_are_listed_and_sorted_once() {
    let entries: Vec<Entry> = (0..50_000).rev().map(|index| file(&format!("file-{index:05}"))).collect();
    let started = Instant::now();

    let mut panel = Panel::new(folder(), SortSpec::default(), false);
    panel.replace_listing(folder(), entries);
    panel.cycle_sort();
    panel.cycle_sort();

    assert!(started.elapsed() < Duration::from_secs(1), "took {:?}", started.elapsed());
    assert_eq!(panel.rows().len(), 50_001);
    assert_eq!(panel.match_count(), (50_000, 50_000));
}

#[test]
fn sorting_by_a_new_key_starts_ascending() {
    let mut panel = abc_panel();

    panel.sort_by(SortKey::Size);

    assert_eq!(panel.sort_spec(), SortSpec { key: SortKey::Size, order: SortOrder::Ascending });
}

#[test]
fn sorting_by_the_same_key_twice_flips_the_direction() {
    let mut panel = abc_panel();
    panel.click(2, PLAIN);

    panel.sort_by(SortKey::Name);
    assert_eq!(panel.sort_spec(), SortSpec { key: SortKey::Name, order: SortOrder::Descending });
    assert_eq!(row_names(&panel), ["..", "d", "c", "b", "a"]);
    assert_eq!(row_names(&panel)[panel.cursor()], "b");

    panel.sort_by(SortKey::Name);
    assert_eq!(panel.sort_spec(), SortSpec { key: SortKey::Name, order: SortOrder::Ascending });
}

#[test]
fn select_all_selects_every_visible_entry_but_not_the_parent_row() {
    let mut panel = Panel::new(folder(), SortSpec::default(), false);
    panel.replace_listing(folder(), vec![file("a"), file(".hidden"), file("b")]);

    panel.select_all();

    assert_eq!(selected_names(&panel), ["a", "b"]);
    assert_eq!(panel.selection_count(), 2);
}

#[test]
fn select_all_respects_the_filter() {
    let mut panel = panel_with(vec![file("alpha"), file("beta"), file("alpine")]);
    panel.set_filter(Some("alp"));

    panel.select_all();

    assert_eq!(selected_names(&panel), ["alpha", "alpine"]);
}
