use chrono::Local;
use iced::widget::{column, container, mouse_area, row, scrollable, space, svg};
use iced::{Alignment, Color, Element, Length, Size, alignment};
use porthmos_core::listing::Row;

use super::{BOLD, filled, format, label};
use crate::app::{App, Message, PanelSide};
use crate::geometry::{ROW_HEIGHT, visible_range};
use crate::icons::{self, Icon, icon_for_entry};
use crate::panel::Panel;
use crate::theme::Palette;

pub const TEXT_SIZE: f32 = 13.0;
pub const CELL_WIDTH: f32 = 7.8;
pub const OVERSCAN: usize = 6;
pub const EDGE_WIDTH: f32 = 2.0;
pub const ICON_SLOT: f32 = 24.0;
const ICON_SIZE: f32 = 16.0;
const INACTIVE_ICON_OPACITY: f32 = 0.7;
pub const SIZE_WIDTH: f32 = 80.0;
pub const MODIFIED_WIDTH: f32 = 150.0;
pub const OWNER_WIDTH: f32 = 110.0;
pub const PADDING: f32 = 8.0;
const NAME_MIN_WIDTH: f32 = 220.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    pub size: bool,
    pub modified: bool,
    pub owner: bool,
}

pub fn columns(width: f32) -> Columns {
    let size = width >= NAME_MIN_WIDTH + SIZE_WIDTH;
    let modified = size && width >= NAME_MIN_WIDTH + SIZE_WIDTH + MODIFIED_WIDTH;
    let owner = modified && width >= NAME_MIN_WIDTH + SIZE_WIDTH + MODIFIED_WIDTH + OWNER_WIDTH;
    Columns { size, modified, owner }
}

pub fn line_cells(width: f32) -> usize {
    (((width - 2.0 * PADDING) / CELL_WIDTH).floor() as usize).max(1)
}

#[derive(Debug, Clone, Copy)]
pub struct RowState {
    cursor: bool,
    selected: bool,
    focused: bool,
    drop: bool,
}

pub fn list<'a>(app: &'a App, side: PanelSide, panel: &'a Panel, shown: Columns, size: Size) -> Element<'a, Message> {
    let palette = app.palette();
    let rows = panel.rows();
    let range = visible_range(app.scroll_offset(side), size.height, ROW_HEIGHT, rows.len(), OVERSCAN);
    let focused = app.active_side() == side;
    let selection = panel.selected_paths();
    let drop_row = app.drop_target().filter(|hit| hit.side == side).and_then(|hit| hit.row);
    let mut content = column![space().height(Length::Fixed(range.start as f32 * ROW_HEIGHT))];
    for index in range.clone() {
        let state = RowState {
            cursor: index == panel.cursor(),
            selected: matches!(&rows[index], Row::Entry(entry) if selection.contains(&entry.path)),
            focused,
            drop: drop_row == Some(index),
        };
        content = content.push(row_view(side, index, &rows[index], state, shown, palette));
    }
    content = content.push(space().height(Length::Fixed(rows.len().saturating_sub(range.end) as f32 * ROW_HEIGHT)));
    let list = scrollable(content)
        .id(side.list_id())
        .on_scroll(move |viewport| Message::ScrollChanged(side, viewport.absolute_offset().y))
        .width(Length::Fill)
        .height(Length::Fill);
    mouse_area(list).on_right_press(Message::BackgroundRightPressed(side)).into()
}

pub fn icon_of(row: &Row) -> Icon {
    match row {
        Row::Parent => Icon::Parent,
        Row::Entry(entry) => icon_for_entry(&entry.name, entry.is_dir),
    }
}

pub fn row_background(state: RowState, palette: &Palette) -> Color {
    match (state.drop, state.cursor, state.selected, state.focused) {
        (true, ..) | (_, true, _, true) | (_, _, true, true) => palette.row_focus,
        (_, true, _, false) | (_, _, true, false) => palette.row_focus_inactive,
        _ => palette.bg,
    }
}

pub fn edge_color(state: RowState, palette: &Palette) -> Color {
    if state.drop || (state.focused && (state.cursor || state.selected)) {
        palette.accent
    } else {
        row_background(state, palette)
    }
}

fn row_view<'a>(
    side: PanelSide, index: usize, row_data: &Row, state: RowState, shown: Columns, palette: &Palette,
) -> Element<'a, Message> {
    let (name, is_dir, entry) = match row_data {
        Row::Parent => ("..".to_string(), true, None),
        Row::Entry(entry) if entry.is_dir => (format!("{}/", entry.name), true, Some(entry)),
        Row::Entry(entry) => (entry.name.clone(), false, Some(entry)),
    };
    let color = match (state.focused, is_dir, state.cursor) {
        (true, _, true) => palette.fg_focus,
        (true, true, false) => palette.fg_bright,
        (true, false, false) => palette.fg,
        (false, true, _) => palette.inactive_dir,
        (false, false, _) => palette.inactive_file,
    };
    let background = row_background(state, palette);
    let opacity = if state.focused { 1.0 } else { INACTIVE_ICON_OPACITY };
    let icon = svg(icons::handle(icon_of(row_data))).width(ICON_SIZE).height(ICON_SIZE).opacity(opacity);
    let name = label(name, color);
    let name = if is_dir || state.cursor || state.drop { name.font(BOLD) } else { name };
    let mut line = row![
        container(space())
            .width(Length::Fixed(EDGE_WIDTH))
            .height(Length::Fill)
            .style(filled(edge_color(state, palette))),
        container(icon)
            .width(Length::Fixed(ICON_SLOT))
            .height(Length::Fill)
            .center_x(Length::Fixed(ICON_SLOT))
            .center_y(Length::Fill),
        container(name).width(Length::Fill).clip(true),
    ]
    .align_y(Alignment::Center);
    if let Some(entry) = entry {
        if shown.size {
            let size = format::size(entry.size, entry.is_dir);
            line = line.push(
                label(size, palette.fg_muted).width(Length::Fixed(SIZE_WIDTH)).align_x(alignment::Horizontal::Right),
            );
        }
        if shown.modified {
            let modified = format!("  {}", format::modified(entry.modified, &Local));
            line = line.push(label(modified, palette.fg_dim).width(Length::Fixed(MODIFIED_WIDTH)));
        }
        if shown.owner {
            let owner = format!("  {}", entry.owner.as_deref().unwrap_or(""));
            line = line.push(container(label(owner, palette.fg_dim)).width(Length::Fixed(OWNER_WIDTH)).clip(true));
        }
    }
    let line = container(line.push(space().width(Length::Fixed(PADDING))))
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .clip(true)
        .style(filled(background));
    mouse_area(line)
        .on_press(Message::RowPressed(side, index))
        .on_double_click(Message::PanelDoubleClicked(side, index))
        .on_right_press(Message::RowRightPressed(side, index))
        .into()
}

#[cfg(test)]
mod tests;
