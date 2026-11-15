use iced::widget::{column, container, mouse_area, row, scrollable, space, svg, text_input};
use iced::{Alignment, Element, Font, Length, Size};
use porthmos_core::Entry;

use super::button::{Look, text_button};
use super::spacing::{DIALOG_GAP, DIALOG_PADDING, SM};
use super::{bold_label, filled, hairline_across, label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::icons;
use crate::search::{SearchState, SearchStatus};
use crate::theme::Palette;

const SEARCH_CHROME: f32 = 207.0;
const ICON_SIZE: f32 = 16.0;

pub fn results_height(window: Size) -> f32 {
    (window.height - SEARCH_CHROME).max(ROW_HEIGHT)
}

const PREVIEW_PANE_SHARE: f32 = 0.4;
const PREVIEW_PANE_MIN_ROWS: f32 = 4.0;
const PREVIEW_PANE_GAP: f32 = 2.0 * DIALOG_GAP + 1.0;
const PREVIEW_LIST_MIN_ROWS: f32 = 3.0;

pub fn search_preview_pane_height(window: Size, enabled: bool) -> Option<f32> {
    if !enabled {
        return None;
    }
    let base = results_height(window);
    let pane = (base * PREVIEW_PANE_SHARE).max(PREVIEW_PANE_MIN_ROWS * ROW_HEIGHT);
    (base - pane - PREVIEW_PANE_GAP >= PREVIEW_LIST_MIN_ROWS * ROW_HEIGHT).then_some(pane)
}

pub fn search_list_height(window: Size, enabled: bool) -> f32 {
    let base = results_height(window);
    match search_preview_pane_height(window, enabled) {
        Some(pane) => base - pane - PREVIEW_PANE_GAP,
        None => base,
    }
}

fn result_row<'a>(app: &App, state: &SearchState, index: usize, entry: &Entry, selected: bool) -> Element<'a, Message> {
    let palette = app.palette();
    let name_color = if selected { palette.fg_focus } else { palette.fg };
    let icon = icons::icon_for_entry(&entry.name, entry.is_dir);
    let accent = if selected { palette.accent } else { iced::Color::TRANSPARENT };
    let line = row![
        container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(accent)),
        space().width(Length::Fixed(6.0)),
        svg(icons::handle(icon, palette.light)).width(ICON_SIZE).height(ICON_SIZE),
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

fn body<'a>(app: &'a App, state: &'a SearchState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input("Name to search for", state.pattern())
        .style(crate::view::input::style(palette))
        .id(App::search_input_id())
        .on_input(Message::SearchTyped)
        .size(13.0)
        .font(Font::DEFAULT);
    let mut rows = column![];
    for (index, entry) in state.results().iter().enumerate() {
        rows = rows.push(result_row(app, state, index, entry, state.selected_row() == Some(index)));
    }
    let results = scrollable(rows)
        .style(crate::view::scroll::style(palette))
        .id(App::search_list_id())
        .width(Length::Fill)
        .height(Length::Fixed(search_list_height(app.search_window_size(), state.preview_enabled())));
    let open = state.selected().is_some().then_some(Message::SearchOpenPressed);
    let title = row![
        bold_label("Search", palette.fg_bright),
        container(label(state.scope_text(&app.search_place()), palette.fg_dim)).width(Length::Fill).clip(true)
    ]
    .spacing(DIALOG_GAP)
    .align_y(Alignment::Center);
    let mut body = column![title, hairline_across(palette), input, results].spacing(DIALOG_GAP);
    if let Some(height) = search_preview_pane_height(app.search_window_size(), state.preview_enabled()) {
        let pane = match state.preview() {
            Some(preview) => super::preview::pane(app, preview, height),
            None => container(space()).width(Length::Fill).height(Length::Fixed(height)).into(),
        };
        body = body.push(hairline_across(palette)).push(pane);
    }
    body.push(container(label(state.status_text(), status_color(state, palette))).width(Length::Fill).clip(true))
        .push(hairline_across(palette))
        .push(
            row![
                space().width(Length::Fill),
                text_button("Close", Some(Message::SearchClosed), Look::Normal, palette),
                text_button("Open", open, Look::Primary, palette),
            ]
            .spacing(SM),
        )
        .into()
}

pub fn window(app: &App) -> Option<Element<'_, Message>> {
    let state = app.search_state()?;
    let ground = app.palette().bg;
    let content = container(body(app, state))
        .padding(DIALOG_PADDING)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(filled(ground));
    Some(content.into())
}

#[cfg(test)]
mod tests;
