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

#[test]
fn a_move_to_the_same_cell_and_side_is_not_reported_twice() {
    let mut filter = MoveFilter::default();
    let here = ViewPoint { row: 2, column: 3 };

    assert!(filter.changed(here, Side::Left));
    assert!(!filter.changed(here, Side::Left));
    assert!(filter.changed(here, Side::Right));
    assert!(filter.changed(ViewPoint { row: 2, column: 4 }, Side::Right));

    filter.reset();
    assert!(filter.changed(ViewPoint { row: 2, column: 4 }, Side::Right));
}

#[test]
fn clicks_in_a_row_count_up_and_a_different_button_restarts() {
    use iced::Point;
    use iced::advanced::mouse::{Button, Click};

    let first = Click::new(Point::ORIGIN, Button::Left, None);
    let second = Click::new(Point::ORIGIN, Button::Left, Some(first));
    let third = Click::new(Point::ORIGIN, Button::Left, Some(second));
    let other = Click::new(Point::ORIGIN, Button::Middle, Some(third));
    let far = Click::new(Point::new(500.0, 500.0), Button::Left, Some(third));

    assert_eq!([first, second, third].map(click_count), [1, 2, 3]);
    assert_eq!(click_count(other), 1);
    assert_eq!(click_count(far), 1);
}

#[test]
fn mouse_buttons_map_and_unknown_ones_are_ignored() {
    use iced::advanced::mouse::Button;

    assert_eq!(term_button(Button::Left), Some(MouseButton::Left));
    assert_eq!(term_button(Button::Middle), Some(MouseButton::Middle));
    assert_eq!(term_button(Button::Right), Some(MouseButton::Right));
    assert_eq!(term_button(Button::Back), None);
}

#[test]
fn there_is_no_scroll_thumb_without_history_or_at_the_bottom() {
    assert_eq!(scroll_thumb(0, 0, 24, 432.0), None);
    assert_eq!(scroll_thumb(0, 5, 24, 432.0), None);
    assert_eq!(scroll_thumb(100, 0, 24, 432.0), None);
}

#[test]
fn the_scroll_thumb_is_proportional_to_the_visible_share() {
    let (_, height) = scroll_thumb(76, 10, 24, 400.0).unwrap();

    assert_eq!(height, 400.0 * 24.0 / 100.0);
}

#[test]
fn the_scroll_thumb_sits_at_the_top_when_scrolled_to_the_oldest_line() {
    let (top, _) = scroll_thumb(100, 100, 24, 496.0).unwrap();

    assert_eq!(top, 0.0);
}

#[test]
fn the_scroll_thumb_moves_down_as_the_view_returns_to_the_bottom() {
    let (far, _) = scroll_thumb(100, 90, 24, 496.0).unwrap();
    let (near, _) = scroll_thumb(100, 10, 24, 496.0).unwrap();

    assert!(near > far);
}

#[test]
fn the_scroll_thumb_never_leaves_its_track_nor_gets_smaller_than_the_minimum() {
    for history in [1usize, 10, 1_000, 1_000_000] {
        for offset in [1usize, history / 2 + 1, history] {
            let track = 300.0;
            let (top, height) = scroll_thumb(history, offset, 24, track).unwrap();

            assert!(height >= SCROLL_THUMB_MIN.min(track), "{history} {offset} {height}");
            assert!(top >= 0.0 && top + height <= track + 0.001, "{history} {offset} {top} {height}");
        }
    }
}

#[test]
fn degenerate_scroll_thumb_inputs_give_none() {
    assert_eq!(scroll_thumb(10, 5, 0, 400.0), None);
    assert_eq!(scroll_thumb(10, 5, 24, 0.0), None);
    assert_eq!(scroll_thumb(10, 5, 24, f32::NAN), None);
    assert_eq!(scroll_thumb(10, 5, 24, -3.0), None);
}

fn cell(row: u16, column: u16) -> (ViewPoint, Side) {
    (ViewPoint { row, column }, Side::Left)
}

#[test]
fn a_grab_releases_only_for_the_button_it_holds_and_at_the_last_known_cell() {
    let mut grab = Grab::default();
    grab.begin(MouseButton::Left, cell(0, 0).0, Side::Left);
    grab.moved(cell(3, 4).0, Side::Right);

    assert_eq!(grab.held(), Some(MouseButton::Left));
    assert_eq!(grab.finish(MouseButton::Middle), None);
    assert_eq!(grab.finish(MouseButton::Left), Some((ViewPoint { row: 3, column: 4 }, Side::Right)));
    assert_eq!(grab.held(), None);
    assert_eq!(grab.finish(MouseButton::Left), None);
}

#[test]
fn abandoning_a_grab_yields_a_release_once() {
    let mut grab = Grab::default();
    assert_eq!(grab.abandon(), None);
    grab.begin(MouseButton::Right, cell(1, 2).0, Side::Left);

    assert_eq!(grab.abandon(), Some((MouseButton::Right, ViewPoint { row: 1, column: 2 }, Side::Left)));
    assert_eq!(grab.abandon(), None);
}
