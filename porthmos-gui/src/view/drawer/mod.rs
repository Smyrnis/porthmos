use iced::widget::{column, container, row, space, stack, text_input};
use iced::{Color, Element, Length, alignment};
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;

use super::button::{compact_button, hinted};
use super::terminal_grid::terminal_grid;
use super::text_fit::fit;
use super::{filled, floating, hairline_across, label};
use crate::app::{App, Message};
use crate::drawer::search::search_status;
use crate::geometry::{DRAWER_HEADER_HEIGHT, drawer_height};
use crate::shortcuts::Action;
use crate::theme::Palette;
use crate::view::spacing::{MD, SM};

const EDGE_WIDTH: f32 = 2.0;
const TITLE_CELLS: usize = 48;
const SEARCH_WIDTH: f32 = 320.0;
const SEARCH_HEIGHT: f32 = 32.0;
const SEARCH_INPUT_WIDTH: f32 = 160.0;
const SEARCH_STATUS_CELLS: usize = 12;

pub fn tab_title(title: &str) -> String {
    if title.is_empty() { tr("Terminal") } else { title.to_string() }
}

pub fn search_bar(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let search = &app.drawer().search;
    let input = text_input(&tr("Search in terminal"), &search.query)
        .id(App::terminal_search_id())
        .on_input(Message::TerminalSearchTyped)
        .on_submit(Message::TerminalSearchSubmitted)
        .style(crate::view::input::style(palette))
        .size(13.0)
        .font(iced::Font::DEFAULT)
        .width(Length::Fixed(SEARCH_INPUT_WIDTH));
    let status =
        search_status(search.missed, &search.query).map(|text| label(fit(&text, SEARCH_STATUS_CELLS), palette.err));
    let previous = hinted(
        compact_button("↑", Some(Message::TerminalSearchPrevious), palette),
        tr("Previous match"),
        iced::widget::tooltip::Position::Bottom,
        palette,
    );
    let next = hinted(
        compact_button("↓", Some(Message::TerminalSearchNext), palette),
        tr("Next match"),
        iced::widget::tooltip::Position::Bottom,
        palette,
    );
    let close = hinted(
        compact_button("×", Some(Message::TerminalSearchClosed), palette),
        tr("Close search"),
        iced::widget::tooltip::Position::Bottom,
        palette,
    );
    let mut line = row![input].spacing(SM).align_y(alignment::Vertical::Center);
    if let Some(status) = status {
        line = line.push(status);
    }
    line = line.push(previous).push(next).push(close);
    container(line)
        .padding([0.0, SM])
        .height(Length::Fixed(SEARCH_HEIGHT))
        .max_width(SEARCH_WIDTH)
        .align_y(alignment::Vertical::Center)
        .style(floating(palette))
        .into()
}

pub fn exit_text(exited: Option<Option<u32>>) -> Option<String> {
    match exited? {
        None => Some(tr("Exited")),
        Some(status) => Some(i18n::text_with("gui.terminal.exited_with", &[("status", &status.to_string())])),
    }
}

pub fn edge_color(focused: bool, palette: &Palette) -> Color {
    if focused { palette.accent } else { palette.line_strong }
}

pub fn hint_text(key: &str) -> String {
    if key.is_empty() {
        i18n::text("gui.terminal.empty_unbound")
    } else {
        i18n::text_with("gui.terminal.empty", &[("key", key)])
    }
}

pub fn empty_hint(app: &App) -> String {
    hint_text(&app.shortcuts().label_for(Action::Terminal).unwrap_or_default())
}

fn header<'a>(app: &'a App, title: String, status: Option<String>) -> Element<'a, Message> {
    let palette = app.palette();
    let focused = app.drawer().shows_focus();
    let edge = container(space()).width(EDGE_WIDTH).height(Length::Fill).style(filled(edge_color(focused, palette)));
    let title_color = if focused { palette.fg_bright } else { palette.fg_muted };
    let mut line = row![label(fit(&title, TITLE_CELLS), title_color)].spacing(SM).align_y(alignment::Vertical::Center);
    if let Some(status) = status {
        line = line.push(label(status, palette.fg_dim));
    }
    let close = hinted(
        compact_button("×", Some(Message::TerminalClosed), palette),
        tr("Close terminal"),
        iced::widget::tooltip::Position::Left,
        palette,
    );
    row![
        edge,
        container(line).padding([0.0, MD]).width(Length::Fill).align_y(alignment::Vertical::Center),
        container(close).padding([0.0, SM]).align_y(alignment::Vertical::Center),
    ]
    .height(DRAWER_HEADER_HEIGHT)
    .into()
}

pub fn view(app: &App) -> Option<Element<'_, Message>> {
    let drawer = app.drawer();
    if !drawer.open {
        return None;
    }
    let palette = app.palette();
    let body: Element<Message> = match &drawer.tab {
        Some(tab) => match tab.snapshot() {
            Some(snapshot) => {
                let id = tab.id;
                terminal_grid(
                    id,
                    snapshot,
                    palette,
                    drawer.shows_focus(),
                    move |size| Message::TerminalResized(id, size),
                    move |event| Message::TerminalPointer(id, event),
                )
                .into()
            }
            None => container(space()).into(),
        },
        None => container(label(empty_hint(app), palette.fg_dim))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
    };
    let (title, status) = match &drawer.tab {
        Some(tab) => (tab_title(&tab.title), exit_text(tab.exited)),
        None => (tab_title(""), None),
    };
    let body: Element<Message> = if drawer.search.open {
        let bar = container(search_bar(app)).width(Length::Fill).align_x(alignment::Horizontal::Right).padding(SM);
        stack![body, bar].into()
    } else {
        body
    };
    let page = column![header(app, title, status), hairline_across(palette), body];
    Some(
        container(page)
            .width(Length::Fill)
            .height(Length::Fixed(drawer_height(app.window_size().height)))
            .style(filled(palette.bg))
            .into(),
    )
}

#[cfg(test)]
mod tests;
