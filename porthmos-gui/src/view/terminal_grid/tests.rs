use iced::Size;
use porthmos_core::theme::ColorScheme;
use porthmos_term::{TermSize, Terminal, TerminalConfig};

use super::*;
use crate::theme::ThemeSettings;

const METRICS: CellMetrics = CellMetrics { width: 8.0, height: 16.0 };

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default().with_scheme(ColorScheme::dark()))
}

fn screen(columns: u16, rows: u16, input: &str) -> porthmos_term::Snapshot {
    let mut terminal = Terminal::new(TermSize::clamped(columns, rows), TerminalConfig::default());
    terminal.feed(input.as_bytes());
    terminal.snapshot()
}

fn first_row(snapshot: &porthmos_term::Snapshot) -> &[porthmos_term::Cell] {
    &snapshot.cells[..usize::from(snapshot.size.columns)]
}

#[test]
fn grid_size_floors_to_whole_cells() {
    assert_eq!(grid_size(Size::new(645.0, 400.0), METRICS), TermSize::clamped(80, 25));
}

#[test]
fn a_zero_or_negative_or_nan_size_is_one_by_one() {
    for size in [Size::new(0.0, 0.0), Size::new(-5.0, -5.0), Size::new(f32::NAN, f32::NAN), Size::new(3.0, 3.0)] {
        assert_eq!(grid_size(size, METRICS), TermSize::clamped(1, 1), "{size:?}");
    }
}

#[test]
fn a_huge_size_is_clamped_to_the_maximum_grid() {
    assert_eq!(
        grid_size(Size::new(1.0e9, 1.0e9), METRICS),
        TermSize::clamped(TermSize::MAX_COLUMNS, TermSize::MAX_ROWS)
    );
}

#[test]
fn degenerate_cell_metrics_do_not_divide_by_zero() {
    let broken = CellMetrics { width: 0.0, height: 0.0 };

    assert_eq!(grid_size(Size::new(100.0, 100.0), broken), TermSize::clamped(1, 1));
}

#[test]
fn resize_is_reported_once_per_distinct_size() {
    let mut gate = ResizeGate::default();
    let small = TermSize::clamped(80, 24);
    let large = TermSize::clamped(100, 30);

    assert_eq!(gate.changed(small), Some(small));
    assert_eq!(gate.changed(small), None);
    assert_eq!(gate.changed(large), Some(large));
    assert_eq!(gate.changed(large), None);
    assert_eq!(gate.changed(small), Some(small));
}

#[test]
fn neighbours_with_the_same_look_share_a_run() {
    let snapshot = screen(20, 2, "hello \x1b[31mred\x1b[0m end");

    let runs = row_runs(first_row(&snapshot), &palette());

    let texts: Vec<(u16, &str)> = runs.iter().map(|run| (run.column, run.text.trim_end())).collect();
    assert_eq!(texts, vec![(0, "hello"), (6, "red"), (10, "end")]);
    assert_ne!(runs[0].color, runs[1].color);
}

#[test]
fn bold_and_underline_are_carried_to_the_run() {
    let snapshot = screen(20, 2, "\x1b[1;4mhi\x1b[0m");

    let runs = row_runs(first_row(&snapshot), &palette());

    assert!(runs[0].bold && runs[0].underline && !runs[0].italic && !runs[0].strikeout);
}

#[test]
fn wide_and_non_ascii_characters_get_their_own_run_at_their_column() {
    let snapshot = screen(20, 2, "a你é b");

    let runs = row_runs(first_row(&snapshot), &palette());

    let placed: Vec<(u16, String)> = runs.iter().map(|run| (run.column, run.text.clone())).collect();
    assert_eq!(placed, vec![(0, "a".into()), (1, "你".into()), (3, "é".into()), (5, "b".into())]);
}

#[test]
fn inverse_swaps_the_foreground_and_background() {
    let palette = palette();
    let snapshot = screen(20, 2, "\x1b[7mx");

    let runs = row_runs(first_row(&snapshot), &palette);
    let backgrounds = row_backgrounds(first_row(&snapshot), &palette);

    assert_eq!(runs[0].color, palette.bg);
    assert_eq!(backgrounds[0].color, palette.fg);
}

#[test]
fn hidden_text_is_not_drawn() {
    let snapshot = screen(20, 2, "\x1b[8msecret\x1b[0m");

    assert!(row_runs(first_row(&snapshot), &palette()).is_empty());
}

#[test]
fn only_cells_with_a_non_default_background_get_a_span_and_neighbours_merge() {
    let snapshot = screen(20, 2, "a\x1b[44mbc\x1b[0m d\x1b[42me");

    let spans = row_backgrounds(first_row(&snapshot), &palette());

    let placed: Vec<(u16, u16)> = spans.iter().map(|span| (span.column, span.width)).collect();
    assert_eq!(placed, vec![(1, 2), (5, 1)]);
}

#[test]
fn selected_cells_use_the_selection_colours() {
    let palette = palette();
    let mut terminal = Terminal::new(TermSize::clamped(20, 2), TerminalConfig::default());
    terminal.feed(b"pick");
    terminal.start_selection(
        porthmos_term::SelectionKind::Simple,
        porthmos_term::ViewPoint { row: 0, column: 0 },
        porthmos_term::Side::Left,
    );
    terminal.update_selection(porthmos_term::ViewPoint { row: 0, column: 3 }, porthmos_term::Side::Right);
    let snapshot = terminal.snapshot();

    let spans = row_backgrounds(first_row(&snapshot), &palette);

    assert_eq!(spans[0].color, palette.row_focus);
}

#[test]
fn a_new_terminal_is_told_its_size_even_when_the_size_did_not_change() {
    let mut gate = ResizeGate::default();
    let size = TermSize::clamped(140, 14);

    assert_eq!(gate.changed_for(1, size), Some(size));
    assert_eq!(gate.changed_for(1, size), None);
    assert_eq!(gate.changed_for(2, size), Some(size));
    assert_eq!(gate.changed_for(2, size), None);
}
