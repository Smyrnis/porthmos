use std::ops::Range;

use iced::{Point, Size};

use crate::app::PanelSide;

pub const ROW_HEIGHT: f32 = 22.0;
pub const TAB_HEIGHT: f32 = 32.0;
pub const TOOLBAR_HEIGHT: f32 = 44.0;
pub const PATH_BAR_HEIGHT: f32 = 36.0;
pub const COLUMN_HEADER_HEIGHT: f32 = 24.0;
pub const STATUS_HEIGHT: f32 = 24.0;
pub const TRANSFER_COLUMN_WIDTH: f32 = 44.0;
pub const HAIRLINE: f32 = 1.0;
pub const LABELS_MIN_WIDTH: f32 = 900.0;
pub const LIST_TOP: f32 =
    TAB_HEIGHT + HAIRLINE + TOOLBAR_HEIGHT + HAIRLINE + PATH_BAR_HEIGHT + HAIRLINE + COLUMN_HEADER_HEIGHT + HAIRLINE;
pub const CHROME_HEIGHT: f32 = LIST_TOP + HAIRLINE + STATUS_HEIGHT + HAIRLINE + STATUS_HEIGHT;

pub fn labels_visible(window_width: f32) -> bool {
    window_width >= LABELS_MIN_WIDTH
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub side: PanelSide,
    pub row: Option<usize>,
}

pub fn hit_test(
    window: Size, point: Point, scrolls: (f32, f32), rows: (usize, usize), queue_height: f32,
) -> Option<Hit> {
    if !point.x.is_finite() || !point.y.is_finite() || point.x < 0.0 || point.x >= window.width {
        return None;
    }
    let list_height = (window.height - CHROME_HEIGHT - queue_height).max(ROW_HEIGHT);
    if point.y < LIST_TOP || point.y >= LIST_TOP + list_height {
        return None;
    }
    let panel_width = (window.width - TRANSFER_COLUMN_WIDTH - 2.0 * HAIRLINE) / 2.0;
    let right_start = panel_width + HAIRLINE + TRANSFER_COLUMN_WIDTH + HAIRLINE;
    let (side, scroll, total) = if point.x < panel_width {
        (PanelSide::Local, scrolls.0, rows.0)
    } else if point.x >= right_start {
        (PanelSide::Remote, scrolls.1, rows.1)
    } else {
        return None;
    };
    let row = ((point.y - LIST_TOP + scroll.max(0.0)) / ROW_HEIGHT).floor() as usize;
    Some(Hit { side, row: (row < total).then_some(row) })
}

pub fn visible_range(offset: f32, viewport: f32, row_height: f32, total: usize, overscan: usize) -> Range<usize> {
    if row_height <= 0.0 || total == 0 {
        return 0..0;
    }
    let offset = offset.max(0.0);
    let viewport = viewport.max(0.0);
    let first = ((offset / row_height).floor() as usize).saturating_sub(overscan);
    let last = (((offset + viewport) / row_height).ceil() as usize).saturating_add(overscan);
    first.min(total)..last.min(total)
}

pub fn scroll_to_show(row: usize, offset: f32, viewport: f32, row_height: f32) -> f32 {
    let top = row as f32 * row_height;
    let bottom = top + row_height;
    let viewport = viewport.max(0.0);
    if top < offset {
        top
    } else if bottom > offset + viewport {
        (bottom - viewport).min(top)
    } else {
        offset
    }
}

pub fn list_viewport(window_height: f32) -> f32 {
    (window_height - CHROME_HEIGHT).max(ROW_HEIGHT)
}

#[cfg(test)]
mod tests;
