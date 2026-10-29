use iced::widget::{column, container, mouse_area, row, scrollable, space, stack, svg, text_input, tooltip};
use iced::{Alignment, Border, Element, Font, Length, Size, alignment};
use porthmos_core::connections_tree::TreeRow;
use porthmos_core::profiles::ConnectionSource;

use super::button::{Look, flat, hinted, text_button};
use super::{bold_label, filled, hairline_across, hairline_down, label};
use crate::app::{App, Message};
use crate::connections::{badge, details_lines, subtitle};
use crate::geometry::ROW_HEIGHT;
use crate::icons::{self, Icon};
use crate::theme::Palette;

pub const MANAGER_WIDTH: f32 = 760.0;
pub const MANAGER_HEIGHT: f32 = 460.0;
const MARGIN: f32 = 16.0;
const INDENT: f32 = 16.0;
const ICON_SIZE: f32 = 16.0;
const BADGE_WIDTH: f32 = 70.0;
const DETAILS_WIDTH: f32 = 240.0;
pub const DETAILS_MIN_PANEL: f32 = 560.0;
const LABEL_WIDTH: f32 = 104.0;

pub fn panel_size(window: Size) -> Size {
    Size::new(
        MANAGER_WIDTH.min((window.width - 2.0 * MARGIN).max(0.0)),
        MANAGER_HEIGHT.min((window.height - 2.0 * MARGIN).max(0.0)),
    )
}

pub fn details_visible(panel_width: f32) -> bool {
    panel_width >= DETAILS_MIN_PANEL
}

pub fn group_text(name: &str, count: usize) -> String {
    format!("{name} ({count})")
}

pub fn empty_hint(matches: usize, filtering: bool) -> Option<&'static str> {
    match (matches, filtering) {
        (0, true) => Some("No connections match"),
        (0, false) => Some("No saved connections. Click Add to create one."),
        _ => None,
    }
}

pub fn button<'a>(palette: &Palette) -> Element<'a, Message> {
    let plus = iced::widget::button(label("＋", palette.fg_bright))
        .padding([6.0, 12.0])
        .on_press(Message::ManagerToggled)
        .style(flat(palette.fg_bright, true));
    hinted(plus.into(), "Connections".to_string(), tooltip::Position::Left, palette)
}

fn group_row<'a>(
    app: &App, row_index: usize, path: &str, name: &str, depth: usize, count: usize, expanded: bool,
) -> Element<'a, Message> {
    let palette = app.palette();
    let selected = app.manager().selected_row() == Some(row_index);
    let chevron = if expanded { "▾" } else { "▸" };
    let line = row![
        space().width(Length::Fixed(depth as f32 * INDENT)),
        label(chevron, palette.fg_muted).width(Length::Fixed(16.0)),
        svg(icons::handle(Icon::Folder)).width(ICON_SIZE).height(ICON_SIZE),
        space().width(Length::Fixed(6.0)),
        bold_label(group_text(name, count), palette.fg_bright),
    ]
    .align_y(Alignment::Center);
    let background = if selected { palette.row_focus } else { iced::Color::TRANSPARENT };
    let line = container(line)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .padding([0.0, 8.0])
        .style(filled(background));
    mouse_area(line).on_press(Message::ManagerGroupToggled(path.to_string())).into()
}

fn connection_row<'a>(app: &'a App, row_index: usize, entry_index: usize, depth: usize) -> Element<'a, Message> {
    let palette = app.palette();
    let entry = &app.manager().entries()[entry_index];
    let selected = app.manager().selected_row() == Some(row_index);
    let orphan = entry.source.is_orphan_labels();
    let name_color = match (selected, orphan) {
        (_, true) => palette.fg_dim,
        (true, false) => palette.fg_focus,
        (false, false) => palette.fg,
    };
    let dot = if app.is_connected(&entry.name) { label("●", palette.ok) } else { label(" ", palette.fg_dim) };
    let name = if orphan { format!("⚠ {}", entry.name) } else { entry.name.clone() };
    let line = row![
        container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(if selected {
            palette.accent
        } else {
            iced::Color::TRANSPARENT
        })),
        space().width(Length::Fixed(6.0 + depth as f32 * INDENT)),
        dot.width(Length::Fixed(14.0)),
        container(label(name, name_color)).width(Length::FillPortion(2)).clip(true),
        label(badge(entry), palette.fg_dim).width(Length::Fixed(BADGE_WIDTH)),
        container(label(subtitle(entry), palette.fg_dim)).width(Length::FillPortion(3)).clip(true),
    ]
    .align_y(Alignment::Center);
    let background = if selected { palette.row_focus } else { iced::Color::TRANSPARENT };
    let line =
        container(line).width(Length::Fill).height(Length::Fixed(ROW_HEIGHT)).clip(true).style(filled(background));
    mouse_area(line)
        .on_press(Message::ManagerRowPressed(row_index))
        .on_double_click(Message::ManagerRowActivated(row_index))
        .into()
}

fn list<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    let manager = app.manager();
    let rows = manager.rows();
    let filtering = !manager.filter().trim().trim_start_matches('#').trim().is_empty();
    if let Some(hint) = empty_hint(manager.match_count(), filtering) {
        return container(label(hint, palette.fg_dim)).padding(12).into();
    }
    let mut lines = column![];
    for (index, tree_row) in rows.iter().enumerate() {
        lines = lines.push(match tree_row {
            TreeRow::Group { path, name, depth, count, expanded } => {
                group_row(app, index, path, name, *depth, *count, *expanded)
            }
            TreeRow::Connection { index: entry, depth } => connection_row(app, index, *entry, *depth),
        });
    }
    scrollable(lines).width(Length::Fill).height(Length::Fill).into()
}

fn details<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    let Some(entry) = app.manager().selected_entry() else {
        return container(label("Select a connection to see its details", palette.fg_dim)).padding(12).into();
    };
    let mut lines = column![bold_label(entry.name.clone(), palette.fg_bright)].spacing(6);
    if entry.source == ConnectionSource::SshConfig {
        lines = lines.push(label("From ~/.ssh/config", palette.fg_dim));
    }
    if entry.source.is_orphan_labels() {
        lines = lines.push(label("This host is not in ~/.ssh/config", palette.fg_muted));
    } else {
        let protocol = app.protocol_name(&entry.protocol);
        for (name, value) in details_lines(entry, &protocol, app.is_connected(&entry.name)) {
            lines = lines.push(
                row![label(name, palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)), label(value, palette.fg)]
                    .spacing(6),
            );
        }
    }
    container(lines).padding(12).width(Length::Fill).into()
}

fn footer<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let buttons = app.manager_buttons();
    row![
        text_button("Add", Some(Message::ManagerAddPressed), Look::Normal, palette),
        text_button("Edit", buttons.edit.then_some(Message::ManagerEditPressed), Look::Normal, palette),
        text_button("Delete", buttons.delete.then_some(Message::ManagerDeletePressed), Look::Normal, palette),
        space().width(Length::Fill),
        text_button("Close", Some(Message::ManagerClosed), Look::Normal, palette),
        text_button("Connect", buttons.connect.then_some(Message::ManagerConnect), Look::Primary, palette),
    ]
    .spacing(8)
    .into()
}

pub fn overlay(app: &App) -> Option<Element<'_, Message>> {
    if !app.manager_open() {
        return None;
    }
    let palette = app.palette();
    let size = panel_size(app.window_size());
    let filter = text_input("Filter connections", app.manager().filter())
        .id(App::filter_input_id())
        .on_input(Message::ManagerFilterTyped)
        .size(13.0)
        .font(Font::DEFAULT);
    let mut body = row![container(list(app)).width(Length::Fill).height(Length::Fill)].height(Length::Fill);
    if details_visible(size.width) {
        body = body
            .push(hairline_down(palette))
            .push(container(details(app)).width(Length::Fixed(DETAILS_WIDTH)).height(Length::Fill));
    }
    let content = column![
        bold_label("Connections", palette.fg_bright),
        filter,
        container(body).height(Length::Fill).style(bordered(palette)),
        hairline_across(palette),
        footer(app),
    ]
    .spacing(10);
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
    let blocked = mouse_area(boxed).on_press(Message::ManagerIgnored);
    let backdrop =
        mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::ManagerIgnored);
    let placed = container(blocked)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);
    Some(stack![backdrop, placed].into())
}

fn bordered<'a>(palette: &Palette) -> impl Fn(&iced::Theme) -> container::Style + 'a {
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    move |_| container::Style { border, ..container::Style::default() }
}

#[cfg(test)]
mod tests;
