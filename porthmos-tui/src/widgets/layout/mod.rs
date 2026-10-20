use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn split_panels(area: Rect) -> (Rect, Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    (columns[0], columns[1])
}

pub const MIN_HEIGHT: u16 = 12;
pub const SIDE_PADDING: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameAreas {
    pub top_bar: Rect,
    pub top_rule: Rect,
    pub body: Rect,
    pub bottom_rule: Rect,
    pub hint_bar: Rect,
}

pub fn split_frame(area: Rect) -> Option<FrameAreas> {
    if area.height < MIN_HEIGHT {
        return None;
    }
    let padding = if area.width > 2 * SIDE_PADDING { SIDE_PADDING } else { 0 };
    let inset = |row: Rect| Rect::new(row.x + padding, row.y, row.width - 2 * padding, row.height);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    Some(FrameAreas {
        top_bar: inset(rows[0]),
        top_rule: rows[1],
        body: inset(rows[2]),
        bottom_rule: rows[3],
        hint_bar: inset(rows[4]),
    })
}

#[cfg(test)]
mod tests;
