use std::path::PathBuf;

use chrono::{DateTime, Local, TimeZone};
use porthmos_core::Entry;
use ratatui::{Terminal, backend::TestBackend};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_fg, find_text},
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap()
}

fn script() -> Entry {
    Entry {
        name: "deploy.sh".to_string(),
        path: PathBuf::from("/d/deploy.sh"),
        is_dir: false,
        size: 2457,
        permissions: Some(0o100755),
        modified: Some(Local.with_ymd_and_hms(2026, 10, 20, 14, 2, 0).unwrap().timestamp() as u64),
    }
}

fn draw(entry: Option<&Entry>, width: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let now = now();
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal.draw(|frame| render_info_line(frame, frame.area(), entry, &now, &theme)).unwrap();
    terminal
}

fn row(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    let mut x = 0;
    while x < buffer.area.width {
        let symbol = buffer[(x, 0)].symbol();
        text.push_str(symbol);
        x += symbol.width().max(1) as u16;
    }
    text
}

#[test]
fn a_file_shows_name_size_permissions_and_time_separated_by_three_cells() {
    let terminal = draw(Some(&script()), 80);

    assert_eq!(row(&terminal).trim_end(), "deploy.sh   2.4 KB   -rwxr-xr-x   today 14:02");
}

#[test]
fn the_name_is_in_the_text_colour_and_the_rest_dim() {
    let theme = indexed();
    let terminal = draw(Some(&script()), 80);

    let (x, y) = find_text(&terminal, "deploy.sh").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
    let (size_x, _) = find_text(&terminal, "2.4 KB").unwrap();
    assert_eq!(cell_fg(&terminal, size_x, y), theme.fg_dim);
    let (time_x, _) = find_text(&terminal, "today").unwrap();
    assert_eq!(cell_fg(&terminal, time_x, y), theme.fg_dim);
}

#[test]
fn a_directory_has_no_size() {
    let directory = Entry { is_dir: true, permissions: Some(0o040755), name: "src".to_string(), ..script() };

    let terminal = draw(Some(&directory), 80);

    assert_eq!(row(&terminal).trim_end(), "src   drwxr-xr-x   today 14:02");
}

#[test]
fn unknown_fields_are_left_out() {
    let bare = Entry { permissions: None, modified: None, ..script() };

    let terminal = draw(Some(&bare), 80);

    assert_eq!(row(&terminal).trim_end(), "deploy.sh   2.4 KB");
}

#[test]
fn no_entry_draws_nothing() {
    let terminal = draw(None, 40);

    assert_eq!(row(&terminal).trim(), "");
}

#[test]
fn a_long_name_is_cut_and_the_trailing_fields_are_dropped_before_the_name_is_lost() {
    let long = Entry { name: "n".repeat(100), ..script() };

    for width in [30, 20, 10, 5, 1, 0] {
        let terminal = draw(Some(&long), width);
        assert!(row(&terminal).width() <= usize::from(width), "width {width}");
    }
    let terminal = draw(Some(&long), 30);
    assert!(row(&terminal).contains('\u{2026}'));
    assert!(row(&terminal).contains("2.4 KB"));
}

#[test]
fn wide_names_are_measured_by_display_width() {
    let wide = Entry { name: "\u{65e5}\u{672c}\u{8a9e}".repeat(10), ..script() };

    for width in 1..60 {
        let terminal = draw(Some(&wide), width);
        assert!(row(&terminal).width() <= usize::from(width), "width {width}");
    }
}

#[test]
fn control_and_bidi_characters_in_the_name_are_replaced() {
    let hostile = Entry { name: "a\u{202e}b\nc".to_string(), ..script() };

    let terminal = draw(Some(&hostile), 80);

    assert!(row(&terminal).starts_with("a?b?c"));
}
