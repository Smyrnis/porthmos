use porthmos_core::Entry;
use porthmos_core::listing::Row;

use super::{RowState, columns, edge_color, icon_of, line_cells, link_note, row_background};
use crate::icons::Icon;
use crate::theme::{Palette, ThemeSettings};

#[test]
fn narrow_panels_drop_columns_in_order_owner_modified_size() {
    let wide = columns(700.0);
    assert!(wide.size && wide.modified && wide.owner);

    let medium = columns(500.0);
    assert!(medium.size && medium.modified && !medium.owner);

    let narrow = columns(380.0);
    assert!(narrow.size && !narrow.modified && !narrow.owner);

    let tiny = columns(200.0);
    assert!(!tiny.size && !tiny.modified && !tiny.owner);
}

#[test]
fn a_full_width_line_gets_the_panel_width_minus_its_padding() {
    assert_eq!(line_cells(400.0), 49);
    assert_eq!(line_cells(10.0), 1);
    assert_eq!(line_cells(0.0), 1);
}

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

fn entry(name: &str, is_dir: bool) -> Row {
    Row::Entry(Entry { name: name.to_string(), is_dir, ..Entry::default() })
}

fn state(cursor: bool, selected: bool, focused: bool, drop: bool) -> RowState {
    RowState { cursor, selected, focused, drop }
}

#[test]
fn icon_of_picks_parent_folder_and_file_icons() {
    assert_eq!(icon_of(&Row::Parent), Icon::Parent);
    assert_eq!(icon_of(&entry("src", true)), Icon::Folder);
    assert_eq!(icon_of(&entry("a.json", false)), Icon::Json);
}

#[test]
fn selected_rows_are_marked_by_fill_not_by_a_dot() {
    let palette = palette();

    assert_eq!(row_background(state(false, true, true, false), &palette), palette.row_focus);
    assert_eq!(row_background(state(false, true, false, false), &palette), palette.row_focus_inactive);
    assert_eq!(row_background(state(false, false, true, false), &palette), palette.bg);
}

#[test]
fn the_cursor_and_drop_rows_keep_their_fills() {
    let palette = palette();

    assert_eq!(row_background(state(true, false, true, false), &palette), palette.row_focus);
    assert_eq!(row_background(state(true, false, false, false), &palette), palette.row_focus_inactive);
    assert_eq!(row_background(state(false, false, false, true), &palette), palette.row_focus);
}

#[test]
fn the_accent_edge_marks_selection_cursor_and_drop_in_the_active_panel() {
    let palette = palette();

    assert_eq!(edge_color(state(false, true, true, false), &palette), palette.accent);
    assert_eq!(edge_color(state(true, false, true, false), &palette), palette.accent);
    assert_eq!(edge_color(state(false, false, false, true), &palette), palette.accent);
    assert_eq!(
        edge_color(state(false, true, false, false), &palette),
        row_background(state(false, true, false, false), &palette)
    );
    assert_eq!(edge_color(state(false, false, true, false), &palette), palette.bg);
}

fn link(name: &str, target: &str) -> Row {
    Row::Entry(Entry { name: name.to_string(), link_target: Some(target.to_string()), ..Entry::default() })
}

#[test]
fn a_symlink_has_the_link_icon_whatever_its_name_says() {
    assert_eq!(icon_of(&link("config.json", "../etc/c.json")), Icon::Link);
    assert_eq!(icon_of(&link("alias", "real")), Icon::Link);
}

#[test]
fn a_file_without_a_link_target_keeps_its_own_icon() {
    assert_eq!(icon_of(&entry("config.json", false)), Icon::Json);
}

#[test]
fn a_link_note_shows_the_target_as_written() {
    let Row::Entry(entry) = link("alias", "../real/file.txt") else { unreachable!() };
    assert_eq!(link_note(&entry).as_deref(), Some("\u{2192} ../real/file.txt"));
}

#[test]
fn a_link_with_an_unreadable_target_says_so() {
    let Row::Entry(entry) = link("alias", "") else { unreachable!() };
    assert_eq!(link_note(&entry).as_deref(), Some("\u{2192} unreadable target"));
}

#[test]
fn an_ordinary_entry_has_no_link_note() {
    let Row::Entry(entry) = entry("a.txt", false) else { unreachable!() };
    assert_eq!(link_note(&entry), None);
}
