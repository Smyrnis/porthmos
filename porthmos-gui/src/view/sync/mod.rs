use iced::widget::{column, container, mouse_area, row, scrollable, space, text_input};
use iced::{Alignment, Border, Color, Element, Font, Length};
use porthmos_core::edit::printable;
use porthmos_core::sync::{SyncAction, SyncReason};

use super::button::{Look, text_button};
use super::format::size;
use super::tools::{frame_size, sync_list_height};
use super::{bold_label, filled, hairline_across, label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::preview_view::visible_window;
use crate::sync_view::{SyncRow, SyncView, reason_text};
use crate::theme::Palette;
use crate::tools::Tool;

pub const SYNC_TIMES_MIN_WIDTH: f32 = 820.0;
pub const SYNC_STACK_BELOW: f32 = 560.0;
pub const SYNC_STACK_EXTRA: f32 = 40.0;
const TICK_SIZE: f32 = 14.0;
const GLYPH_WIDTH: f32 = 18.0;
const LEFT_WIDTH: f32 = 58.0;
const REASON_WIDTH: f32 = 150.0;
const SIZE_WIDTH: f32 = 70.0;
const TIME_WIDTH: f32 = 128.0;
const GAP: f32 = 10.0;
const EDGE: f32 = 8.0;

pub struct RowTexts {
    pub glyph: &'static str,
    pub reason: &'static str,
    pub path: String,
    pub size: String,
    pub local: String,
    pub remote: String,
}

pub fn glyph(action: SyncAction) -> &'static str {
    match action {
        SyncAction::Upload => "\u{2191}",
        SyncAction::Download => "\u{2193}",
        SyncAction::Skip => "\u{b7}",
    }
}

pub fn row_texts(row: &SyncRow) -> RowTexts {
    RowTexts {
        glyph: glyph(row.action),
        reason: row.reason,
        path: printable(&row.path),
        size: size(row.size, false),
        local: row.local_time.clone(),
        remote: row.remote_time.clone(),
    }
}

pub fn window_for(scroll: f32, viewport: f32, rows: usize) -> (usize, usize) {
    visible_window(scroll, viewport, rows, ROW_HEIGHT)
}

pub fn show_times(frame_width: f32) -> bool {
    frame_width >= SYNC_TIMES_MIN_WIDTH
}

pub fn stacked_buttons(frame_width: f32) -> bool {
    frame_width < SYNC_STACK_BELOW
}

pub fn run_enabled(view: &SyncView) -> bool {
    !view.visible_choices().is_empty()
}

fn cell<'a>(content: String, width: Length, color: Color) -> Element<'a, Message> {
    container(label(content, color)).width(width).clip(true).into()
}

fn tick_box<'a>(ticked: bool, palette: &Palette) -> Element<'a, Message> {
    let fill = if ticked { palette.accent } else { Color::TRANSPARENT };
    let border =
        Border { color: if ticked { palette.accent } else { palette.line_strong }, width: 1.0, radius: 3.0.into() };
    container(space())
        .width(Length::Fixed(TICK_SIZE))
        .height(Length::Fixed(TICK_SIZE))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(fill)),
            border,
            ..container::Style::default()
        })
        .into()
}

fn text_color(row: &SyncRow, palette: &Palette) -> Color {
    if row.reason == reason_text(SyncReason::KindMismatch) {
        palette.err
    } else if row.ticked {
        palette.fg
    } else if row.flippable {
        palette.fg_bright
    } else {
        palette.fg_dim
    }
}

fn header<'a>(times: bool, palette: &Palette) -> Element<'a, Message> {
    let dim = palette.fg_dim;
    let mut line = row![
        space().width(Length::Fixed(LEFT_WIDTH)),
        cell("reason".to_string(), Length::Fixed(REASON_WIDTH), dim),
        cell("path".to_string(), Length::Fill, dim),
        cell("size".to_string(), Length::Fixed(SIZE_WIDTH), dim),
    ]
    .spacing(GAP);
    if times {
        line = line.push(cell("local".to_string(), Length::Fixed(TIME_WIDTH), dim)).push(cell(
            "remote".to_string(),
            Length::Fixed(TIME_WIDTH),
            dim,
        ));
    }
    line.push(space().width(Length::Fixed(EDGE))).into()
}

fn list_row<'a>(position: usize, row: &SyncRow, focused: bool, times: bool, palette: &Palette) -> Element<'a, Message> {
    let texts = row_texts(row);
    let color = text_color(row, palette);
    let accent = if focused { palette.accent } else { Color::TRANSPARENT };
    let left = container(
        row![
            container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(accent)),
            space().width(Length::Fixed(EDGE - 2.0)),
            tick_box(row.ticked, palette),
            space().width(Length::Fixed(GAP)),
            cell(texts.glyph.to_string(), Length::Fixed(GLYPH_WIDTH), palette.fg_bright),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fixed(LEFT_WIDTH));
    let mut line = row![
        left,
        cell(texts.reason.to_string(), Length::Fixed(REASON_WIDTH), color),
        cell(texts.path, Length::Fill, color),
        cell(texts.size, Length::Fixed(SIZE_WIDTH), palette.fg_dim),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center);
    if times {
        line = line.push(cell(texts.local, Length::Fixed(TIME_WIDTH), palette.fg_dim)).push(cell(
            texts.remote,
            Length::Fixed(TIME_WIDTH),
            palette.fg_dim,
        ));
    }
    let background = if focused { palette.row_focus } else { Color::TRANSPARENT };
    let line = container(line.push(space().width(Length::Fixed(EDGE))))
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .clip(true)
        .style(filled(background));
    mouse_area(line).on_press(Message::SyncRowPressed(position)).into()
}

fn rows_view<'a>(app: &'a App, view: &'a SyncView, times: bool, palette: &Palette) -> Element<'a, Message> {
    let viewport = sync_list_height(app.window_size());
    let total = view.matched();
    let (first, count) = window_for(app.tool_scroll(), viewport, total);
    let mut lines = column![space().height(Length::Fixed(first as f32 * ROW_HEIGHT))];
    for position in first..first + count {
        if let Some(row) = view.row(position) {
            lines = lines.push(list_row(position, &row, position == view.cursor(), times, palette));
        }
    }
    let below = total.saturating_sub(first + count);
    lines = lines.push(space().height(Length::Fixed(below as f32 * ROW_HEIGHT)));
    scrollable(lines)
        .id(App::tool_list_id())
        .on_scroll(|viewport| Message::SyncScrolled(viewport.absolute_offset().y))
        .width(Length::Fill)
        .height(Length::Fixed(viewport))
        .into()
}

fn hint<'a>(text: &str, height: f32, palette: &Palette) -> Element<'a, Message> {
    container(label(text.to_string(), palette.fg_dim))
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .into()
}

pub fn body<'a>(app: &'a App, view: &'a SyncView) -> Element<'a, Message> {
    let palette = app.palette();
    let times = show_times(frame_size(&Tool::Sync, app.window_size()).width);
    let input = text_input("Filter rows", view.filter().unwrap_or_default())
        .id(App::sync_filter_id())
        .on_input(Message::SyncFilterTyped)
        .size(13.0)
        .font(Font::DEFAULT);
    let counts = match view.filter() {
        Some(_) => format!("{} of {}", view.matched(), view.total()),
        None => format!("{} rows", view.total()),
    };
    let viewport = sync_list_height(app.window_size());
    let rows: Element<'a, Message> = match (view.matched(), view.total()) {
        (0, 0) => hint("Nothing to synchronize.", viewport, palette),
        (0, _) => hint("No rows match.", viewport, palette),
        _ => rows_view(app, view, times, palette),
    };
    let content: Element<'a, Message> = column![header(times, palette), rows].spacing(4).into();
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    let boxed = container(content)
        .width(Length::Fill)
        .style(move |_| container::Style { border, ..container::Style::default() });
    let selection = row![
        text_button("Tick all", Some(Message::SyncTickAllPressed), Look::Normal, palette),
        text_button("Untick all", Some(Message::SyncUntickAllPressed), Look::Normal, palette),
        text_button("Switch direction", view.can_flip().then_some(Message::SyncFlipPressed), Look::Normal, palette),
    ]
    .spacing(8);
    let actions = row![
        text_button("Cancel", Some(Message::ToolClosed), Look::Normal, palette),
        text_button("Run", run_enabled(view).then_some(Message::SyncRunPressed), Look::Primary, palette),
    ]
    .spacing(8);
    let buttons: Element<'a, Message> = if stacked_buttons(frame_size(&Tool::Sync, app.window_size()).width) {
        column![selection, row![space().width(Length::Fill), actions]].spacing(8).into()
    } else {
        row![selection, space().width(Length::Fill), actions].spacing(8).into()
    };
    column![
        bold_label(app.sync_title(), palette.fg_bright),
        row![input, label(counts, palette.fg_dim)].spacing(10).align_y(Alignment::Center),
        boxed,
        label(view.top_text(), palette.fg_dim),
        hairline_across(palette),
        buttons,
    ]
    .spacing(10)
    .into()
}

#[cfg(test)]
mod tests;
