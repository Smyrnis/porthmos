use iced::mouse::ScrollDelta;
use iced::{Point, Size};
use porthmos_term::{Modes, MouseButton, MouseMode, SelectionKind, Side, TermSize, ViewPoint};

use crate::view::terminal_grid::CellMetrics;

pub const WHEEL_LINES_PER_NOTCH: i32 = 3;
pub const MAX_WHEEL_LINES: i32 = 64;
const MAX_CLICKS: u8 = 3;

pub fn wheel_lines(delta: ScrollDelta, cell_height: f32) -> i32 {
    let lines = match delta {
        ScrollDelta::Lines { y, .. } => y * WHEEL_LINES_PER_NOTCH as f32,
        ScrollDelta::Pixels { y, .. } => {
            if !cell_height.is_finite() || cell_height <= 0.0 {
                return 0;
            }
            y / cell_height
        }
    };
    if lines.is_nan() {
        return 0;
    }
    lines.round().clamp(-(MAX_WHEEL_LINES as f32), MAX_WHEEL_LINES as f32) as i32
}

fn cell_index(offset: f32, cell: f32, count: u16) -> (u16, f32) {
    if !offset.is_finite() || !cell.is_finite() || cell <= 0.0 || offset <= 0.0 {
        return (0, 0.0);
    }
    let cells = offset / cell;
    let last = f32::from(count.max(1) - 1);
    if cells >= last + 1.0 {
        return (count.max(1) - 1, 1.0);
    }
    (cells.floor().min(last) as u16, cells.fract())
}

pub fn cell_at(position: Point, size: Size, metrics: CellMetrics, grid: TermSize) -> (ViewPoint, Side) {
    let bounded = |value: f32, limit: f32| if value.is_nan() { 0.0 } else { value.min(limit) };
    let x = bounded(position.x, size.width);
    let y = bounded(position.y, size.height);
    let (column, across) = cell_index(x, metrics.width, grid.columns);
    let (row, _) = cell_index(y, metrics.height, grid.rows);
    let side = if across < 0.5 { Side::Left } else { Side::Right };
    (ViewPoint { row, column }, side)
}

pub fn selection_kind(clicks: u8, alt: bool) -> SelectionKind {
    match clicks.min(MAX_CLICKS) {
        2 => SelectionKind::Words,
        3 => SelectionKind::Lines,
        _ if alt => SelectionKind::Block,
        _ => SelectionKind::Simple,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Press(MouseButton),
    Release(MouseButton),
    Move { dragging: bool },
    Wheel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Report,
    Select,
    Scroll,
    AlternateScroll,
    Paste,
    FocusOnly,
    Ignore,
}

pub fn route(kind: PointerKind, modes: Modes, shift: bool, focused: bool) -> Route {
    if !focused {
        return match kind {
            PointerKind::Press(_) => Route::FocusOnly,
            _ => Route::Ignore,
        };
    }
    if modes.mouse != MouseMode::Off && !shift {
        return Route::Report;
    }
    match kind {
        PointerKind::Press(MouseButton::Left)
        | PointerKind::Release(MouseButton::Left)
        | PointerKind::Move { dragging: true } => Route::Select,
        PointerKind::Press(MouseButton::Middle) => Route::Paste,
        PointerKind::Wheel if modes.alternate_screen => Route::AlternateScroll,
        PointerKind::Wheel => Route::Scroll,
        _ => Route::Ignore,
    }
}

#[cfg(test)]
mod tests;
