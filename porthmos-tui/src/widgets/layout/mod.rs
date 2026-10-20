use ratatui::layout::{Constraint, Direction, Layout, Rect};

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

pub const PANEL_GUTTER: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesLayout {
    pub local: Rect,
    pub remote: Rect,
    pub divider: Option<Rect>,
    pub rule: Rect,
    pub info: Rect,
    pub transfer: Rect,
}

pub const TWO_PANELS_MIN_BODY_WIDTH: u16 = 96;
pub const FULL_COLUMNS_MIN_BODY_WIDTH: u16 = 56;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesMode {
    TwoPanels,
    SinglePanel { show_modified: bool },
}

pub fn files_mode(body_width: u16) -> FilesMode {
    if body_width >= TWO_PANELS_MIN_BODY_WIDTH {
        FilesMode::TwoPanels
    } else {
        FilesMode::SinglePanel { show_modified: body_width >= FULL_COLUMNS_MIN_BODY_WIDTH }
    }
}

pub fn files_layout(body: Rect, mode: FilesMode, transfer_line: bool) -> FilesLayout {
    let transfer_height = if transfer_line { body.height.min(1) } else { 0 };
    let info_height = (body.height - transfer_height).min(1);
    let rule_height = (body.height - transfer_height - info_height).min(1);
    let panels_height = body.height - transfer_height - info_height - rule_height;

    let transfer = Rect::new(body.x, body.bottom() - transfer_height, body.width, transfer_height);
    let info = Rect::new(body.x, transfer.y - info_height, body.width, info_height);
    let rule = Rect::new(body.x, info.y - rule_height, body.width, rule_height);

    if matches!(mode, FilesMode::SinglePanel { .. }) {
        let panel = Rect::new(body.x, body.y, body.width, panels_height);
        return FilesLayout { local: panel, remote: panel, divider: None, rule, info, transfer };
    }

    let divider_x = body.x + body.width / 2;
    let local_width = (divider_x - body.x).saturating_sub(PANEL_GUTTER);
    let remote_x = (divider_x + 1 + PANEL_GUTTER).min(body.right());
    let local = Rect::new(body.x, body.y, local_width, panels_height);
    let remote = Rect::new(remote_x, body.y, body.right() - remote_x, panels_height);
    let divider = (body.width >= 3).then(|| Rect::new(divider_x, body.y, 1, panels_height));

    FilesLayout { local, remote, divider, rule, info, transfer }
}

pub const DETAILS_WIDTH: u16 = 42;
pub const DETAILS_MIN_BODY_WIDTH: u16 = 96;
const DETAILS_GUTTER_BEFORE_DIVIDER: u16 = 2;
const DETAILS_GUTTER_AFTER_DIVIDER: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionsLayout {
    pub list: Rect,
    pub divider: Option<Rect>,
    pub details: Option<Rect>,
}

pub fn connections_layout(body: Rect) -> ConnectionsLayout {
    if body.width < DETAILS_MIN_BODY_WIDTH {
        return ConnectionsLayout { list: body, divider: None, details: None };
    }
    let details_x = body.right() - DETAILS_WIDTH;
    let divider_x = details_x - DETAILS_GUTTER_AFTER_DIVIDER - 1;
    let list_width = divider_x - DETAILS_GUTTER_BEFORE_DIVIDER - body.x;
    ConnectionsLayout {
        list: Rect::new(body.x, body.y, list_width, body.height),
        divider: Some(Rect::new(divider_x, body.y, 1, body.height)),
        details: Some(Rect::new(details_x, body.y, DETAILS_WIDTH, body.height)),
    }
}

#[cfg(test)]
mod tests;
