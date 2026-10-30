use iced::widget::{button, column, container, mouse_area, row, space, stack};
use iced::{Background, Border, Element, Length, alignment};

use super::button::{Look, flat, tool_button};
use super::{filled, label};
use crate::app::{App, Message};
use crate::geometry::{HAIRLINE, TAB_HEIGHT, TOOLBAR_HEIGHT, labels_visible};
use crate::icons::Icon;
use crate::menus::{MoreState, more_items};
use crate::shortcuts::Action;
use crate::theme::Palette;
use crate::update_view::badge_text;

const MORE_MENU_WIDTH: f32 = 220.0;
const SEPARATOR_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolbarState {
    pub copy: bool,
    pub move_: bool,
    pub new_folder: bool,
    pub rename: bool,
    pub delete: bool,
    pub refresh: bool,
    pub filter: bool,
    pub transfers: bool,
}

pub fn show_titles(window_width: f32) -> bool {
    labels_visible(window_width)
}

pub fn badge_label(app: &App) -> Option<String> {
    app.update_badge().map(|badge| badge_text(&badge, app.window_size().width))
}

fn badge_button(app: &App) -> Option<Element<'_, Message>> {
    let text = badge_label(app)?;
    let tip = app.update_announcement().unwrap_or_default();
    Some(tool_button(None, &text, true, tip, Some(Message::UpdateBadgePressed), Look::Primary, app.palette()))
}

fn hint(app: &App, title: &str, action: Action) -> String {
    match app.shortcuts().label_for(action) {
        Some(keys) => format!("{title} ({keys})"),
        None => title.to_string(),
    }
}

fn action_button<'a>(app: &'a App, icon: Icon, title: &str, action: Action, enabled: bool) -> Element<'a, Message> {
    let press = enabled.then_some(Message::ActionRequested(action));
    let titles = show_titles(app.window_size().width);
    tool_button(Some(icon), title, titles, hint(app, title, action), press, Look::Normal, app.palette())
}

fn separator<'a>(palette: &Palette) -> Element<'a, Message> {
    container(space())
        .width(Length::Fixed(HAIRLINE))
        .height(Length::Fixed(SEPARATOR_HEIGHT))
        .style(filled(palette.line_strong))
        .into()
}

pub fn bar(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let state = app.toolbar_state();
    let titles = show_titles(app.window_size().width);
    let running = app.running_transfer_count();
    let transfers_title = if running > 0 { format!("Transfers · {running}") } else { "Transfers".to_string() };
    let transfers_look = if running > 0 { Look::Primary } else { Look::Normal };
    let line = row![
        action_button(app, Icon::Copy, "Copy", Action::Copy, state.copy),
        action_button(app, Icon::Move, "Move", Action::Move, state.move_),
        action_button(app, Icon::NewFolder, "New folder", Action::NewFolder, state.new_folder),
        action_button(app, Icon::Rename, "Rename", Action::Rename, state.rename),
        action_button(app, Icon::Delete, "Delete", Action::Delete, state.delete),
        separator(palette),
        action_button(app, Icon::Refresh, "Refresh", Action::Refresh, state.refresh),
        action_button(app, Icon::Search, "Filter", Action::StartFilter, state.filter),
        space().width(Length::Fill),
        badge_button(app).unwrap_or_else(|| space().width(Length::Fixed(0.0)).into()),
        tool_button(
            Some(Icon::Transfers),
            &transfers_title,
            titles,
            hint(app, "Transfers", Action::ToggleQueue),
            state.transfers.then_some(Message::ActionRequested(Action::ToggleQueue)),
            transfers_look,
            palette,
        ),
        tool_button(
            Some(Icon::More),
            "More",
            titles,
            "More".to_string(),
            Some(Message::MoreMenuToggled),
            Look::Normal,
            palette,
        ),
        tool_button(
            Some(Icon::Settings),
            "Settings",
            titles,
            hint(app, "Settings", Action::Settings),
            Some(Message::ActionRequested(Action::Settings)),
            Look::Normal,
            palette
        ),
    ]
    .spacing(6)
    .align_y(alignment::Vertical::Center);
    container(line)
        .padding([0.0, 10.0])
        .width(Length::Fill)
        .height(Length::Fixed(TOOLBAR_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .style(filled(palette.surface))
        .into()
}

pub fn more_menu(app: &App) -> Element<'_, Message> {
    let palette = *app.palette();
    let mut items = column![];
    for (item, title, enabled) in more_items(MoreState {
        hidden_shown: app.hidden_shown(),
        file: app.single_file(),
        terminal: app.terminal_possible(),
        sync: app.sync_possible(),
    }) {
        let color = if enabled { palette.fg_bright } else { palette.fg_dim };
        let row_button = button(label(title, color))
            .padding([6.0, 12.0])
            .width(Length::Fixed(MORE_MENU_WIDTH))
            .on_press_maybe(enabled.then_some(Message::MoreItemChosen(item)))
            .style(flat(color, enabled));
        items = items.push(row_button);
    }
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let surface = palette.surface;
    let panel = container(items).padding(4).style(move |_| container::Style {
        background: Some(Background::Color(surface)),
        border,
        ..container::Style::default()
    });
    let backdrop =
        mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::MoreMenuToggled);
    let placed = container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: TAB_HEIGHT + HAIRLINE + TOOLBAR_HEIGHT - 6.0,
            right: 56.0,
            ..iced::Padding::ZERO
        })
        .align_x(alignment::Horizontal::Right)
        .align_y(alignment::Vertical::Top);
    stack![backdrop, placed].into()
}

#[cfg(test)]
mod tests;
