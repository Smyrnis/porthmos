use std::path::PathBuf;

use chrono::{DateTime, Local, TimeZone};
use porthmos_core::Entry;
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text},
    theme::{ColorDepth, ThemeSettings},
    widgets::panel_view::PanelView,
};

const THREE_CHARS_SIX_COLUMNS: &str = "\u{65e5}\u{672c}\u{8a9e}";

fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap()
}

fn seconds_ago(days: i64) -> Option<u64> {
    Some((now() - chrono::Duration::days(days)).timestamp() as u64)
}

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn file(name: &str, size: u64) -> Entry {
    Entry {
        name: name.to_string(),
        path: PathBuf::from("/d").join(name),
        is_dir: false,
        size,
        permissions: Some(0o100644),
        modified: None,
        ..Default::default()
    }
}

fn directory(name: &str) -> Entry {
    Entry { is_dir: true, permissions: Some(0o040755), size: 4096, ..file(name, 0) }
}

fn panel_of(entries: Vec<Entry>) -> PanelView {
    PanelView::from_listing(PathBuf::from("/d"), entries)
}

fn cursor_on(panel: &mut PanelView, name: &str) {
    panel.cursor = panel
        .rows()
        .iter()
        .position(|row| matches!(row, Row::Entry(entry) if entry.name == name))
        .expect("entry is listed");
}

fn draw(
    panel: &PanelView, width: u16, height: u16, active: bool, show_modified: bool, theme: &Theme,
) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let now = now();
    terminal
        .draw(|frame| {
            let context = ListContext { active, show_modified, now: &now };
            render_file_list(frame, frame.area(), panel, &context, theme);
        })
        .unwrap();
    terminal
}

fn modifier_at(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> Modifier {
    terminal.backend().buffer()[(x, y)].modifier
}

fn symbol_at(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> String {
    terminal.backend().buffer()[(x, y)].symbol().to_string()
}

fn column_of(terminal: &Terminal<TestBackend>, y: u16, needle: &str) -> Option<u16> {
    let buffer = terminal.backend().buffer();
    let letters: Vec<char> = needle.chars().collect();
    (0..buffer.area.width).find(|&x| {
        letters
            .iter()
            .enumerate()
            .all(|(offset, letter)| symbol_at(terminal, x + offset as u16, y) == letter.to_string())
    })
}

fn row_of(terminal: &Terminal<TestBackend>, needle: &str) -> u16 {
    find_text(terminal, needle).unwrap_or_else(|| panic!("{needle:?} is drawn")).1
}

#[test]
fn a_file_row_uses_the_fg_colour_without_bold() {
    let theme = indexed();
    let panel = panel_of(vec![file("a.txt", 5)]);

    let terminal = draw(&panel, 60, 6, true, true, &theme);

    let (x, y) = find_text(&terminal, "a.txt").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
    assert!(!modifier_at(&terminal, x, y).contains(Modifier::BOLD));
}

#[test]
fn a_directory_row_is_bright_bold_with_a_slash() {
    let theme = indexed();
    let panel = panel_of(vec![directory("src")]);

    let terminal = draw(&panel, 60, 6, true, true, &theme);

    let (x, y) = find_text(&terminal, "src/").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    assert!(modifier_at(&terminal, x, y).contains(Modifier::BOLD));
}

#[test]
fn the_parent_row_is_dim_when_it_is_not_focused() {
    let theme = indexed();
    let mut panel = panel_of(vec![file("a.txt", 5)]);
    cursor_on(&mut panel, "a.txt");

    let terminal = draw(&panel, 60, 6, true, true, &theme);

    let (x, y) = find_text(&terminal, "..").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    assert!(!modifier_at(&terminal, x, y).contains(Modifier::BOLD));
}

#[test]
fn a_selected_row_has_an_accent_plus_marker_and_unchanged_name_colour() {
    let theme = indexed();
    let mut panel = panel_of(vec![file("a.txt", 5)]);
    panel.selected.insert(PathBuf::from("/d/a.txt"));

    let terminal = draw(&panel, 60, 6, true, true, &theme);

    let (x, y) = find_text(&terminal, "a.txt").unwrap();
    assert_eq!(symbol_at(&terminal, 0, y), "+");
    assert_eq!(cell_fg(&terminal, 0, y), theme.accent);
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
}

#[test]
fn the_focused_row_in_the_active_panel_has_an_accent_chevron_focus_colour_bold_name_and_row_focus_background() {
    let theme = indexed();
    let mut panel = panel_of(vec![file("a.txt", 5)]);
    cursor_on(&mut panel, "a.txt");

    let terminal = draw(&panel, 60, 6, true, true, &theme);

    let (x, y) = find_text(&terminal, "a.txt").unwrap();
    assert_eq!(symbol_at(&terminal, 0, y), "\u{203a}");
    assert_eq!(cell_fg(&terminal, 0, y), theme.accent);
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_focus);
    assert!(modifier_at(&terminal, x, y).contains(Modifier::BOLD));
    assert_eq!(cell_bg(&terminal, x, y), theme.row_focus);
    assert_eq!(cell_bg(&terminal, 59, y), theme.row_focus);
}

#[test]
fn the_focused_row_in_the_inactive_panel_has_no_marker_and_the_inactive_background() {
    let theme = indexed();
    let mut panel = panel_of(vec![file("a.txt", 5)]);
    cursor_on(&mut panel, "a.txt");

    let terminal = draw(&panel, 60, 6, false, true, &theme);

    let (x, y) = find_text(&terminal, "a.txt").unwrap();
    assert_eq!(symbol_at(&terminal, 0, y), " ");
    assert_eq!(cell_bg(&terminal, x, y), theme.row_focus_inactive);
    assert_eq!(cell_fg(&terminal, x, y), theme.inactive_file);
}

#[test]
fn inactive_panel_rows_use_the_inactive_file_and_directory_colours() {
    let theme = indexed();
    let panel = panel_of(vec![file("a.txt", 5), directory("src")]);

    let terminal = draw(&panel, 60, 7, false, true, &theme);

    let (file_x, file_y) = find_text(&terminal, "a.txt").unwrap();
    let (dir_x, dir_y) = find_text(&terminal, "src/").unwrap();
    assert_eq!(cell_fg(&terminal, file_x, file_y), theme.inactive_file);
    assert_eq!(cell_fg(&terminal, dir_x, dir_y), theme.inactive_dir);
    assert!(modifier_at(&terminal, dir_x, dir_y).contains(Modifier::BOLD));
}

#[test]
fn a_focused_and_selected_row_shows_the_chevron() {
    let theme = indexed();
    let mut panel = panel_of(vec![file("a.txt", 5)]);
    panel.selected.insert(PathBuf::from("/d/a.txt"));
    cursor_on(&mut panel, "a.txt");

    let terminal = draw(&panel, 60, 6, true, true, &theme);

    assert_eq!(symbol_at(&terminal, 0, row_of(&terminal, "a.txt")), "\u{203a}");
}

#[test]
fn executable_files_get_a_star_suffix_and_directories_do_not() {
    let theme = indexed();
    let script = Entry { permissions: Some(0o100755), ..file("run.sh", 10) };
    let panel = panel_of(vec![script, directory("bin"), file("plain", 1)]);

    let terminal = draw(&panel, 60, 8, true, true, &theme);

    assert!(find_text(&terminal, "run.sh*").is_some());
    assert!(find_text(&terminal, "bin/").is_some());
    assert!(find_text(&terminal, "bin*").is_none());
    assert!(find_text(&terminal, "plain*").is_none());
}

#[test]
fn size_and_modified_cells_are_dim_and_right_aligned_in_their_columns() {
    let theme = indexed();
    let small = Entry { modified: seconds_ago(0), ..file("a.bin", 812) };
    let medium = Entry { modified: seconds_ago(1), ..file("b.bin", 2457) };
    let large = Entry { modified: seconds_ago(30), ..file("c.bin", 13_312_000) };
    let panel = panel_of(vec![small, medium, large]);

    let terminal = draw(&panel, 60, 8, true, true, &theme);

    assert_eq!(find_text(&terminal, "812 B").unwrap().0, 43);
    assert_eq!(find_text(&terminal, "2.4 KB").unwrap().0, 42);
    assert_eq!(find_text(&terminal, "12.7 MB").unwrap().0, 41);
    let (today_x, today_y) = find_text(&terminal, "today").unwrap();
    assert_eq!(today_x, 49);
    assert_eq!(cell_fg(&terminal, today_x, today_y), theme.fg_dim);
    assert!(find_text(&terminal, "yesterday").is_some());
    assert!(find_text(&terminal, "Sep 20").is_some());
    let (size_x, size_y) = find_text(&terminal, "812 B").unwrap();
    assert_eq!(cell_fg(&terminal, size_x, size_y), theme.fg_dim);
}

#[test]
fn directories_show_no_size() {
    let panel = panel_of(vec![Entry { modified: seconds_ago(0), ..directory("src") }]);

    let terminal = draw(&panel, 60, 6, true, true, &indexed());

    let row: String = (0..60).map(|x| symbol_at(&terminal, x, row_of(&terminal, "src/"))).collect();
    assert!(!row.contains("KB") && !row.contains(" B") && !row.contains('\u{2014}'), "{row:?}");
    assert!(row.contains("today"));
}

#[test]
fn an_unknown_modified_time_leaves_the_cell_empty() {
    let panel = panel_of(vec![file("a.txt", 5)]);

    let terminal = draw(&panel, 60, 6, true, true, &indexed());

    let row: String = (0..60).map(|x| symbol_at(&terminal, x, row_of(&terminal, "a.txt"))).collect();
    assert!(row.trim_end().ends_with("5 B"), "{row:?}");
}

#[test]
fn the_column_header_names_name_size_and_modified_in_dim() {
    let theme = indexed();
    let panel = panel_of(vec![file("a.txt", 5)]);

    let terminal = draw(&panel, 60, 6, true, true, &theme);

    assert_eq!(column_of(&terminal, 0, "name"), Some(2));
    assert_eq!(column_of(&terminal, 0, "size"), Some(44));
    assert_eq!(column_of(&terminal, 0, "modified"), Some(49));
    assert_eq!(cell_fg(&terminal, 2, 0), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, 49, 0), theme.fg_dim);
}

#[test]
fn hiding_the_modified_column_hides_the_cell_and_the_header() {
    let panel = panel_of(vec![Entry { modified: seconds_ago(0), ..file("a.txt", 5) }]);

    let terminal = draw(&panel, 60, 6, true, false, &indexed());

    assert!(find_text(&terminal, "modified").is_none());
    assert!(find_text(&terminal, "today").is_none());
    assert_eq!(column_of(&terminal, 0, "size"), Some(56));
}

#[test]
fn a_narrow_panel_hides_the_size_column() {
    let panel = panel_of(vec![file("a.txt", 123_456)]);

    let terminal = draw(&panel, 20, 6, true, false, &indexed());

    assert!(find_text(&terminal, "a.txt").is_some());
    assert!(find_text(&terminal, "KB").is_none());
    assert!(find_text(&terminal, "size").is_none());
}

#[test]
fn size_label_uses_bytes_then_one_decimal_units() {
    assert_eq!(size_label(0), "0 B");
    assert_eq!(size_label(1023), "1023 B");
    assert_eq!(size_label(1024), "1.0 KB");
    assert_eq!(size_label(2457), "2.4 KB");
    assert_eq!(size_label(13_312_000), "12.7 MB");
    assert_eq!(size_label(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    assert_eq!(size_label(2 * 1024u64.pow(4)), "2.0 TB");
    assert!(!size_label(u64::MAX).is_empty());
}

#[test]
fn a_five_hundred_character_name_is_cut_inside_the_name_column() {
    let long = "x".repeat(500);
    let panel = panel_of(vec![Entry { modified: seconds_ago(0), ..file(&long, 5) }]);

    let terminal = draw(&panel, 60, 6, true, true, &indexed());

    let y = row_of(&terminal, "xxxx");
    let row: String = (0..60).map(|x| symbol_at(&terminal, x, y)).collect();
    assert!(row.contains('\u{2026}'), "{row:?}");
    assert_eq!(column_of(&terminal, y, "today"), Some(49));
}

#[test]
fn wide_names_keep_the_size_column_aligned() {
    let wide = Entry { size: 2048, ..file("\u{65e5}\u{672c}\u{8a9e}.txt", 0) };
    let narrow = Entry { size: 4096, ..file("a.txt", 0) };
    let panel = panel_of(vec![wide, narrow]);

    let terminal = draw(&panel, 60, 7, true, true, &indexed());

    let wide_row = row_of(&terminal, "2.0 KB");
    let narrow_row = row_of(&terminal, "4.0 KB");
    assert_eq!(
        column_of(&terminal, wide_row, "KB").map(|x| x + 2),
        column_of(&terminal, narrow_row, "KB").map(|x| x + 2)
    );
    assert_eq!(column_of(&terminal, wide_row, "KB"), Some(46));
}

#[test]
fn control_and_bidi_characters_in_names_are_replaced() {
    let panel = panel_of(vec![file("a\u{202e}b\nc", 5)]);

    let terminal = draw(&panel, 60, 6, true, true, &indexed());

    assert!(find_text(&terminal, "a?b?c").is_some());
}

#[test]
fn an_empty_listing_draws_only_the_column_header() {
    let panel = PanelView::new(PathBuf::from("/"), Default::default(), false);

    let terminal = draw(&panel, 40, 5, true, true, &indexed());

    assert!(find_text(&terminal, "name").is_some());
    assert_eq!(row_of(&terminal, "name"), 0);
}

#[test]
fn scrolling_keeps_the_cursor_row_visible() {
    let entries: Vec<Entry> = (0..60).map(|index| file(&format!("file{index:02}"), 1)).collect();
    let mut panel = panel_of(entries);
    cursor_on(&mut panel, "file40");

    let terminal = draw(&panel, 60, 7, true, true, &indexed());

    assert!(find_text(&terminal, "file40").is_some());
    assert!(find_text(&terminal, "file00").is_none());
}

#[test]
fn tiny_areas_do_not_panic() {
    let panel = panel_of(vec![file("a.txt", 5), directory("src")]);

    for width in 0..=12 {
        for height in 0..=3 {
            let _ = draw(&panel, width, height, true, true, &indexed());
            let _ = draw(&panel, width, height, false, false, &indexed());
        }
    }
}

#[test]
fn an_indexed_theme_keeps_the_focus_row_distinguishable() {
    let theme = indexed();
    let mut panel = panel_of(vec![file("a.txt", 5), file("b.txt", 5)]);
    cursor_on(&mut panel, "a.txt");

    let active = draw(&panel, 60, 6, true, true, &theme);
    let inactive = draw(&panel, 60, 6, false, true, &theme);

    let focused_y = row_of(&active, "a.txt");
    let other_y = row_of(&active, "b.txt");
    assert_ne!(cell_bg(&active, 30, focused_y), cell_bg(&active, 30, other_y));
    assert_ne!(cell_bg(&active, 30, focused_y), cell_bg(&inactive, 30, focused_y));
    assert_ne!(cell_bg(&inactive, 30, focused_y), cell_bg(&inactive, 30, other_y));
}

#[test]
fn the_list_no_longer_uses_the_reversed_modifier() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
}

#[test]
fn name_cell_appends_a_slash_to_directories_and_a_star_to_executables() {
    assert_eq!(name_cell(&directory("src")), "src/");
    assert_eq!(name_cell(&Entry { permissions: Some(0o100755), ..file("run", 1) }), "run*");
    assert_eq!(name_cell(&file("plain", 1)), "plain");
    assert_eq!(name_cell(&Entry { permissions: None, ..file("unknown", 1) }), "unknown");
}

#[test]
fn permissions_string_adds_the_entry_type_in_front_of_the_triplets() {
    assert_eq!(
        permissions_string(&Entry { permissions: Some(0o100755), ..file("a", 1) }).as_deref(),
        Some("-rwxr-xr-x")
    );
    assert_eq!(
        permissions_string(&Entry { permissions: Some(0o040750), ..directory("d") }).as_deref(),
        Some("drwxr-x---")
    );
    assert_eq!(permissions_string(&Entry { permissions: None, ..file("a", 1) }), None);
}

#[test]
fn format_size_shows_raw_bytes_under_1024() {
    assert_eq!(format_size(0, false), "0");
    assert_eq!(format_size(999, false), "999");
}

#[test]
fn format_size_uses_kilobyte_units_above_1024() {
    assert_eq!(format_size(1024, false), "1.0K");
    assert_eq!(format_size(1536, false), "1.5K");
}

#[test]
fn format_size_uses_megabyte_and_gigabyte_units() {
    assert_eq!(format_size(2 * 1024 * 1024, false), "2.0M");
    assert_eq!(format_size(3 * 1024 * 1024 * 1024, false), "3.0G");
}

#[test]
fn format_size_shows_an_em_dash_for_directories() {
    assert_eq!(format_size(4096, true), "\u{2014}");
}

#[test]
fn truncate_name_measures_display_width_not_char_count() {
    let name = "\u{65e5}\u{672c}\u{8a9e}\u{30d5}\u{30a1}\u{30a4}\u{30eb}";

    let truncated = truncate_name(name, 6);

    assert!(UnicodeWidthStr::width(truncated.as_str()) <= 6);
    assert!(truncated.ends_with('\u{2026}'));
}

#[test]
fn truncate_name_leaves_a_wide_name_that_already_fits_untouched() {
    let name = THREE_CHARS_SIX_COLUMNS;

    assert_eq!(truncate_name(name, 10), name);
}

#[test]
fn size_label_rolls_over_before_it_needs_nine_cells() {
    assert_eq!(size_label(1_022_976), "999.0 KB");
    assert_eq!(size_label(1_023_999), "1.0 MB");
    assert_eq!(size_label(1_048_575), "1.0 MB");
    assert_eq!(size_label(1_073_741_000), "1.0 GB");
}

#[test]
fn a_file_of_about_one_megabyte_keeps_the_modified_column_aligned() {
    let near_a_megabyte = Entry { modified: seconds_ago(0), ..file("big.bin", 1_030_000) };
    let small = Entry { modified: seconds_ago(0), ..file("small.bin", 812) };
    let panel = panel_of(vec![near_a_megabyte, small]);

    let terminal = draw(&panel, 60, 7, true, true, &indexed());

    let big_row = row_of(&terminal, "big.bin");
    let small_row = row_of(&terminal, "small.bin");
    assert_eq!(column_of(&terminal, big_row, "today"), Some(49));
    assert_eq!(column_of(&terminal, small_row, "today"), Some(49));
}

#[test]
fn the_focus_and_inactive_focus_backgrounds_differ_from_the_screen_background() {
    let theme = indexed();

    assert_ne!(theme.row_focus, theme.bg);
    assert_ne!(theme.row_focus_inactive, theme.bg);
    assert_ne!(theme.row_focus, theme.row_focus_inactive);
}

#[test]
fn permissions_string_renders_mixed_triplets() {
    assert_eq!(
        permissions_string(&Entry { permissions: Some(0o100640), ..file("a", 1) }).as_deref(),
        Some("-rw-r-----")
    );
}

#[test]
fn name_cell_marks_a_symlink_with_an_at_sign() {
    let link = Entry { link_target: Some("target".to_string()), ..file("current", 1) };

    assert_eq!(name_cell(&link), "current@");
}

#[test]
fn name_cell_marks_a_symlink_to_a_folder_with_an_at_sign_too() {
    let link = Entry { link_target: Some("releases".to_string()), ..directory("current") };

    assert_eq!(name_cell(&link), "current@");
}

#[test]
fn name_cell_marks_an_executable_symlink_with_an_at_sign_only() {
    let link = Entry { link_target: Some("run".to_string()), permissions: Some(0o100755), ..file("go", 1) };

    assert_eq!(name_cell(&link), "go@");
}

#[test]
fn permissions_string_marks_a_symlink_with_an_l() {
    let link = Entry { link_target: Some("t".to_string()), permissions: Some(0o120777), ..file("l", 1) };

    assert_eq!(permissions_string(&link).as_deref(), Some("lrwxrwxrwx"));
}
