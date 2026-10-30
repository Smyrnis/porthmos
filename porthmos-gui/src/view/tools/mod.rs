use iced::widget::{column, container, mouse_area, row, scrollable, space, stack, svg, text_input};
use iced::{Alignment, Border, Element, Font, Length, Size, alignment, mouse};
use porthmos_core::Entry;

use super::button::{Look, text_button};
use super::{bold_label, filled, hairline_across, label};
use crate::app::{App, Message};
use crate::bookmarks::{BookmarksState, row_text};
use crate::geometry::ROW_HEIGHT;
use crate::history_view::{HistoryState, row_text as history_row_text};
use crate::icons::{self, Icon};
use crate::search::{SearchState, SearchStatus};
use crate::theme::Palette;
use crate::tools::Tool;

pub const PREVIEW_WIDTH: f32 = 960.0;
pub const PREVIEW_HEIGHT: f32 = 640.0;
pub const PREVIEW_CHROME: f32 = 120.0;
pub const SYNC_WIDTH: f32 = 960.0;
pub const SYNC_HEIGHT: f32 = 640.0;
const SYNC_CHROME: f32 = 190.0;
pub const SETTINGS_WIDTH: f32 = 760.0;
pub const SETTINGS_HEIGHT: f32 = 560.0;
pub const SETTINGS_CHROME: f32 = 140.0;
pub const PALETTE_WIDTH: f32 = 640.0;
pub const PALETTE_HEIGHT: f32 = 420.0;
pub const PALETTE_CHROME: f32 = 76.0;
pub const TOOL_WIDTH: f32 = 640.0;
pub const TOOL_HEIGHT: f32 = 440.0;
pub const BOOKMARKS_WIDTH: f32 = 560.0;
pub const BOOKMARKS_HEIGHT: f32 = 380.0;
const BOOKMARKS_CHROME: f32 = 110.0;
pub const HISTORY_WIDTH: f32 = 760.0;
pub const HISTORY_HEIGHT: f32 = 460.0;
const HISTORY_CHROME: f32 = 146.0;
const MARGIN: f32 = 16.0;
const CHROME_HEIGHT: f32 = 172.0;
const ICON_SIZE: f32 = 16.0;

fn fit(window: Size, width: f32, height: f32) -> Size {
    Size::new(width.min((window.width - 2.0 * MARGIN).max(0.0)), height.min((window.height - 2.0 * MARGIN).max(0.0)))
}

pub fn panel_size(window: Size) -> Size {
    fit(window, TOOL_WIDTH, TOOL_HEIGHT)
}

pub fn frame_size(tool: &Tool, window: Size) -> Size {
    match tool {
        Tool::Search(_) => panel_size(window),
        Tool::Bookmarks(_) => fit(window, BOOKMARKS_WIDTH, BOOKMARKS_HEIGHT),
        Tool::History(_) => fit(window, HISTORY_WIDTH, HISTORY_HEIGHT),
        Tool::Preview(_) => fit(window, PREVIEW_WIDTH, PREVIEW_HEIGHT),
        Tool::Palette(_) => fit(window, PALETTE_WIDTH, PALETTE_HEIGHT),
        Tool::Settings(_) => fit(window, SETTINGS_WIDTH, SETTINGS_HEIGHT),
        Tool::Sync => fit(window, SYNC_WIDTH, SYNC_HEIGHT),
    }
}

pub fn list_height(tool: &Tool, window: Size) -> f32 {
    match tool {
        Tool::Search(_) => results_height(window),
        Tool::Bookmarks(_) => bookmarks_list_height(window),
        Tool::History(_) => history_list_height(window),
        Tool::Preview(_) => preview_list_height(window),
        Tool::Palette(_) => palette_list_height(window),
        Tool::Settings(_) => settings_list_height(window),
        Tool::Sync => sync_list_height(window),
    }
}

pub fn sync_list_height(window: Size) -> f32 {
    let frame = fit(window, SYNC_WIDTH, SYNC_HEIGHT);
    let stacked = if super::sync::stacked_buttons(frame.width) { super::sync::SYNC_STACK_EXTRA } else { 0.0 };
    (frame.height - SYNC_CHROME - stacked).max(ROW_HEIGHT)
}

pub fn settings_list_height(window: Size) -> f32 {
    (fit(window, SETTINGS_WIDTH, SETTINGS_HEIGHT).height - SETTINGS_CHROME).max(ROW_HEIGHT)
}

pub fn palette_list_height(window: Size) -> f32 {
    (fit(window, PALETTE_WIDTH, PALETTE_HEIGHT).height - PALETTE_CHROME).max(ROW_HEIGHT)
}

pub fn preview_list_height(window: Size) -> f32 {
    (fit(window, PREVIEW_WIDTH, PREVIEW_HEIGHT).height - PREVIEW_CHROME).max(ROW_HEIGHT)
}

fn history_list_height(window: Size) -> f32 {
    (fit(window, HISTORY_WIDTH, HISTORY_HEIGHT).height - HISTORY_CHROME).max(ROW_HEIGHT)
}

fn bookmarks_list_height(window: Size) -> f32 {
    (fit(window, BOOKMARKS_WIDTH, BOOKMARKS_HEIGHT).height - BOOKMARKS_CHROME).max(ROW_HEIGHT)
}

pub fn history_hint(visible: usize, total: usize) -> Option<&'static str> {
    match (visible, total) {
        (0, 0) => Some("No transfers yet."),
        (0, _) => Some("No transfers match."),
        _ => None,
    }
}

pub fn bookmarks_hint(count: usize) -> Option<&'static str> {
    (count == 0).then_some("No bookmarks yet. Use Bookmark this folder.")
}

pub fn results_height(window: Size) -> f32 {
    (panel_size(window).height - CHROME_HEIGHT).max(ROW_HEIGHT)
}

fn result_row<'a>(app: &App, state: &SearchState, index: usize, entry: &Entry, selected: bool) -> Element<'a, Message> {
    let palette = app.palette();
    let name_color = if selected { palette.fg_focus } else { palette.fg };
    let icon = icons::icon_for_entry(&entry.name, entry.is_dir);
    let accent = if selected { palette.accent } else { iced::Color::TRANSPARENT };
    let line = row![
        container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(accent)),
        space().width(Length::Fixed(6.0)),
        svg(icons::handle(icon)).width(ICON_SIZE).height(ICON_SIZE),
        space().width(Length::Fixed(6.0)),
        container(label(entry.name.clone(), name_color)).width(Length::FillPortion(2)).clip(true),
        container(label(state.shown_parent(entry), palette.fg_dim)).width(Length::FillPortion(3)).clip(true),
    ]
    .align_y(Alignment::Center);
    let background = if selected { palette.row_focus } else { iced::Color::TRANSPARENT };
    let line =
        container(line).width(Length::Fill).height(Length::Fixed(ROW_HEIGHT)).clip(true).style(filled(background));
    mouse_area(line)
        .on_press(Message::SearchRowPressed(index))
        .on_double_click(Message::SearchRowActivated(index))
        .into()
}

fn status_color(state: &SearchState, palette: &Palette) -> iced::Color {
    match state.status() {
        SearchStatus::Failed(_) => palette.err,
        _ => palette.fg_dim,
    }
}

fn search_body<'a>(app: &'a App, state: &'a SearchState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input("Name to search for", state.pattern())
        .id(App::search_input_id())
        .on_input(Message::SearchTyped)
        .size(13.0)
        .font(Font::DEFAULT);
    let mut rows = column![];
    for (index, entry) in state.results().iter().enumerate() {
        rows = rows.push(result_row(app, state, index, entry, state.selected_row() == Some(index)));
    }
    let results = scrollable(rows)
        .id(App::tool_list_id())
        .width(Length::Fill)
        .height(Length::Fixed(results_height(app.window_size())));
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    let boxed = container(results).style(move |_| container::Style { border, ..container::Style::default() });
    let open = state.selected().is_some().then_some(Message::SearchOpenPressed);
    column![
        bold_label("Search", palette.fg_bright),
        input,
        boxed,
        label(state.status_text(), status_color(state, palette)),
        hairline_across(palette),
        row![
            space().width(Length::Fill),
            text_button("Close", Some(Message::ToolClosed), Look::Normal, palette),
            text_button("Open", open, Look::Primary, palette),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .into()
}

fn bookmark_row<'a>(
    app: &App, index: usize, bookmark: &porthmos_core::config::bookmarks::Bookmark, selected: bool,
) -> Element<'a, Message> {
    let palette = app.palette();
    let (title, place) = row_text(bookmark);
    let name_color = if selected { palette.fg_focus } else { palette.fg };
    let accent = if selected { palette.accent } else { iced::Color::TRANSPARENT };
    let line = row![
        container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(accent)),
        space().width(Length::Fixed(6.0)),
        svg(icons::handle(Icon::Folder)).width(ICON_SIZE).height(ICON_SIZE),
        space().width(Length::Fixed(6.0)),
        container(bold_label(title, name_color)).width(Length::FillPortion(2)).clip(true),
        container(label(place, palette.fg_dim)).width(Length::FillPortion(3)).clip(true),
    ]
    .align_y(Alignment::Center);
    let background = if selected { palette.row_focus } else { iced::Color::TRANSPARENT };
    let line =
        container(line).width(Length::Fill).height(Length::Fixed(ROW_HEIGHT)).clip(true).style(filled(background));
    mouse_area(line)
        .on_press(Message::BookmarkRowPressed(index))
        .on_double_click(Message::BookmarkRowActivated(index))
        .into()
}

fn bookmarks_body<'a>(app: &'a App, state: &'a BookmarksState) -> Element<'a, Message> {
    let palette = app.palette();
    let selected = state.selected().map(|(row, _)| row);
    let mut rows = column![];
    for (index, bookmark) in state.items().iter().enumerate() {
        rows = rows.push(bookmark_row(app, index, bookmark, selected == Some(index)));
    }
    if let Some(hint) = bookmarks_hint(state.items().len()) {
        rows = rows.push(container(label(hint, palette.fg_dim)).padding(12));
    }
    let list = scrollable(rows)
        .id(App::tool_list_id())
        .width(Length::Fill)
        .height(Length::Fixed(bookmarks_list_height(app.window_size())));
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    let boxed = container(list).style(move |_| container::Style { border, ..container::Style::default() });
    let chosen = selected.is_some();
    column![
        bold_label("Bookmarks", palette.fg_bright),
        boxed,
        hairline_across(palette),
        row![
            text_button("Remove", chosen.then_some(Message::BookmarkRemovePressed), Look::Normal, palette),
            space().width(Length::Fill),
            text_button("Close", Some(Message::ToolClosed), Look::Normal, palette),
            text_button("Go", chosen.then_some(Message::BookmarkGoPressed), Look::Primary, palette),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .into()
}

fn history_row<'a>(
    app: &App, index: usize, entry: &porthmos_core::history::HistoryEntry, selected: bool,
) -> Element<'a, Message> {
    let palette = app.palette();
    let (title, summary) = history_row_text(entry);
    let name_color = if selected { palette.fg_focus } else { palette.fg };
    let accent = if selected { palette.accent } else { iced::Color::TRANSPARENT };
    let line = row![
        container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(accent)),
        space().width(Length::Fixed(8.0)),
        container(label(title, name_color)).width(Length::FillPortion(2)).clip(true),
        container(label(summary, palette.fg_dim)).width(Length::FillPortion(3)).clip(true),
    ]
    .align_y(Alignment::Center);
    let background = if selected { palette.row_focus } else { iced::Color::TRANSPARENT };
    let line =
        container(line).width(Length::Fill).height(Length::Fixed(ROW_HEIGHT)).clip(true).style(filled(background));
    mouse_area(line)
        .on_press(Message::HistoryRowPressed(index))
        .on_double_click(Message::HistoryRowActivated(index))
        .into()
}

fn history_body<'a>(app: &'a App, state: &'a HistoryState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input("Filter transfers", state.filter())
        .id(App::history_filter_id())
        .on_input(Message::HistoryFilterTyped)
        .size(13.0)
        .font(Font::DEFAULT);
    let visible = state.visible();
    let mut rows = column![];
    for (index, entry) in visible.iter().enumerate() {
        rows = rows.push(history_row(app, index, entry, state.selected_row() == Some(index)));
    }
    if let Some(hint) = history_hint(visible.len(), state.total()) {
        rows = rows.push(container(label(hint, palette.fg_dim)).padding(12));
    }
    let list = scrollable(rows)
        .id(App::tool_list_id())
        .width(Length::Fill)
        .height(Length::Fixed(history_list_height(app.window_size())));
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    let boxed = container(list).style(move |_| container::Style { border, ..container::Style::default() });
    let chosen = state.selected().is_some();
    column![
        bold_label("Transfer history", palette.fg_bright),
        input,
        boxed,
        hairline_across(palette),
        row![
            text_button("Clear", (state.total() > 0).then_some(Message::HistoryClearPressed), Look::Normal, palette),
            space().width(Length::Fill),
            text_button("Close", Some(Message::ToolClosed), Look::Normal, palette),
            text_button("Details", chosen.then_some(Message::HistoryDetailsPressed), Look::Primary, palette),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .into()
}

pub fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let tool = app.tool()?;
    let palette = app.palette();
    let size = frame_size(tool, app.window_size());
    let content = match tool {
        Tool::Search(state) => search_body(app, state),
        Tool::Bookmarks(state) => bookmarks_body(app, state),
        Tool::History(state) => history_body(app, state),
        Tool::Preview(state) => super::preview::body(app, state),
        Tool::Palette(state) => super::palette::body(app, state),
        Tool::Settings(state) => super::settings::body(app, state),
        Tool::Sync => super::sync::body(app, app.sync_view()),
    };
    let surface = palette.surface;
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let boxed =
        container(content).padding(16).width(Length::Fixed(size.width)).height(Length::Fixed(size.height)).style(
            move |_| container::Style {
                background: Some(iced::Background::Color(surface)),
                border,
                ..container::Style::default()
            },
        );
    let blocked = mouse_area(boxed).on_press(Message::ToolIgnored).interaction(mouse::Interaction::Idle);
    let backdrop = mouse_area(container(space()).width(Length::Fill).height(Length::Fill))
        .on_press(Message::ToolIgnored)
        .interaction(mouse::Interaction::Idle);
    let placed = container(blocked)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);
    Some(stack![backdrop, placed].into())
}

#[cfg(test)]
mod tests;
