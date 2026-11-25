use iced::mouse::ScrollDelta;
use iced::{Point, Size};
use porthmos_term::{Modes, MouseButton, MouseMode, SelectionKind, Side, TermSize, ViewPoint};

use super::*;
use crate::view::terminal_grid::CellMetrics;

const METRICS: CellMetrics = CellMetrics { width: 8.0, height: 18.0 };

fn grid() -> TermSize {
    TermSize::clamped(80, 24)
}

fn reporting() -> Modes {
    Modes { mouse: MouseMode::Click, ..Modes::default() }
}

#[test]
fn wheel_lines_converts_notches_and_pixels() {
    assert_eq!(wheel_lines(ScrollDelta::Lines { x: 0.0, y: 1.0 }, 18.0), 3);
    assert_eq!(wheel_lines(ScrollDelta::Lines { x: 0.0, y: -1.0 }, 18.0), -3);
    assert_eq!(wheel_lines(ScrollDelta::Pixels { x: 0.0, y: 36.0 }, 18.0), 2);
    assert_eq!(wheel_lines(ScrollDelta::Pixels { x: 0.0, y: -18.0 }, 18.0), -1);
}

#[test]
fn degenerate_wheel_deltas_are_bounded() {
    for delta in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, f32::MIN, 0.0] {
        let lines = wheel_lines(ScrollDelta::Lines { x: 0.0, y: delta }, 18.0);
        assert!(lines.abs() <= MAX_WHEEL_LINES, "{delta}: {lines}");
        let pixels = wheel_lines(ScrollDelta::Pixels { x: 0.0, y: delta }, 18.0);
        assert!(pixels.abs() <= MAX_WHEEL_LINES, "{delta}: {pixels}");
    }
    assert_eq!(wheel_lines(ScrollDelta::Lines { x: 0.0, y: f32::NAN }, 18.0), 0);
    assert_eq!(wheel_lines(ScrollDelta::Pixels { x: 0.0, y: 36.0 }, 0.0), 0);
    assert_eq!(wheel_lines(ScrollDelta::Pixels { x: 0.0, y: 36.0 }, -4.0), 0);
    assert_eq!(wheel_lines(ScrollDelta::Pixels { x: 0.0, y: 36.0 }, f32::NAN), 0);
    assert_eq!(wheel_lines(ScrollDelta::Lines { x: 0.0, y: f32::MAX }, 18.0), MAX_WHEEL_LINES);
}

#[test]
fn cell_at_floors_into_the_grid_and_picks_the_side() {
    let size = Size::new(640.0, 432.0);

    let (left, side_left) = cell_at(Point::new(2.4 * 8.0, 1.9 * 18.0), size, METRICS, grid());
    let (right, side_right) = cell_at(Point::new(2.6 * 8.0, 1.9 * 18.0), size, METRICS, grid());

    assert_eq!(left, ViewPoint { row: 1, column: 2 });
    assert_eq!(side_left, Side::Left);
    assert_eq!(right, ViewPoint { row: 1, column: 2 });
    assert_eq!(side_right, Side::Right);
}

#[test]
fn cell_at_clamps_outside_positions() {
    let size = Size::new(640.0, 432.0);

    let (first, _) = cell_at(Point::new(-50.0, -9.0), size, METRICS, grid());
    let (last, _) = cell_at(Point::new(9999.0, 9999.0), size, METRICS, grid());
    let (nan, _) = cell_at(Point::new(f32::NAN, f32::NAN), size, METRICS, grid());

    assert_eq!(first, ViewPoint { row: 0, column: 0 });
    assert_eq!(last, ViewPoint { row: 23, column: 79 });
    assert_eq!(nan, ViewPoint { row: 0, column: 0 });
}

#[test]
fn cell_at_survives_degenerate_metrics() {
    let size = Size::new(640.0, 432.0);
    let broken = CellMetrics { width: 0.0, height: f32::NAN };

    let (point, _) = cell_at(Point::new(10.0, 10.0), size, broken, grid());

    assert_eq!(point, ViewPoint { row: 0, column: 0 });
}

#[test]
fn selection_kind_follows_the_click_count_and_alt() {
    assert_eq!(selection_kind(1, false), SelectionKind::Simple);
    assert_eq!(selection_kind(1, true), SelectionKind::Block);
    assert_eq!(selection_kind(2, false), SelectionKind::Words);
    assert_eq!(selection_kind(3, false), SelectionKind::Lines);
    assert_eq!(selection_kind(9, true), SelectionKind::Lines);
    assert_eq!(selection_kind(0, false), SelectionKind::Simple);
}

#[test]
fn an_unfocused_drawer_only_focuses_on_press() {
    let press = PointerKind::Press(MouseButton::Left);

    assert_eq!(route(press, reporting(), false, false), Route::FocusOnly);
    assert_eq!(route(PointerKind::Release(MouseButton::Left), reporting(), false, false), Route::Ignore);
    assert_eq!(route(PointerKind::Wheel, Modes::default(), false, false), Route::Ignore);
}

#[test]
fn a_program_that_asked_for_the_mouse_gets_it() {
    for kind in [
        PointerKind::Press(MouseButton::Left),
        PointerKind::Release(MouseButton::Left),
        PointerKind::Move { dragging: true },
        PointerKind::Wheel,
        PointerKind::Press(MouseButton::Right),
    ] {
        assert_eq!(route(kind, reporting(), false, true), Route::Report, "{kind:?}");
    }
}

#[test]
fn shift_bypasses_reporting_and_selects() {
    assert_eq!(route(PointerKind::Press(MouseButton::Left), reporting(), true, true), Route::Select);
    assert_eq!(route(PointerKind::Move { dragging: true }, reporting(), true, true), Route::Select);
    assert_eq!(route(PointerKind::Release(MouseButton::Left), reporting(), true, true), Route::Select);
}

#[test]
fn middle_press_pastes_when_not_reporting() {
    assert_eq!(route(PointerKind::Press(MouseButton::Middle), Modes::default(), false, true), Route::Paste);
}

#[test]
fn the_wheel_scrolls_history_on_the_main_screen() {
    assert_eq!(route(PointerKind::Wheel, Modes::default(), false, true), Route::Scroll);
}

#[test]
fn the_wheel_sends_arrows_on_the_alternate_screen_without_reporting() {
    let modes = Modes { alternate_screen: true, ..Modes::default() };

    assert_eq!(route(PointerKind::Wheel, modes, false, true), Route::AlternateScroll);
    assert_eq!(route(PointerKind::Wheel, Modes { mouse: MouseMode::Click, ..modes }, false, true), Route::Report);
}

#[test]
fn right_clicks_and_undragged_moves_are_ignored_when_not_reporting() {
    assert_eq!(route(PointerKind::Press(MouseButton::Right), Modes::default(), false, true), Route::Ignore);
    assert_eq!(route(PointerKind::Move { dragging: false }, Modes::default(), false, true), Route::Ignore);
    assert_eq!(route(PointerKind::Release(MouseButton::Middle), Modes::default(), false, true), Route::Ignore);
}
