use iced::widget::{column, container, mouse_area, row, space, stack, text_input};
use iced::{Color, Element, Length, alignment};
use porthmos_core::TerminalId;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;

use super::button::{compact_button, hinted};
use super::terminal_grid::terminal_grid;
use super::text_fit::fit;
use super::{bold_label, filled, floating, hairline_across, label};
use crate::app::{App, Message};
use crate::drawer::layout::{Direction, Layout};
use crate::drawer::links::Link;
use crate::drawer::search::search_status;
use crate::drawer::{Drawer, DrawerView, TerminalPane, TerminalTab};
use crate::geometry::{DRAWER_HEADER_HEIGHT, drawer_height};
use crate::shortcuts::Action;
use crate::theme::Palette;
use crate::view::spacing::{MD, SM};

const EDGE_WIDTH: f32 = 2.0;
pub const RING_WIDTH: f32 = 2.0;
const TITLE_CELLS: usize = 48;
const TARGET_CELLS: usize = 64;
const SEARCH_WIDTH: f32 = 320.0;
const SEARCH_HEIGHT: f32 = 32.0;
const SEARCH_INPUT_WIDTH: f32 = 160.0;
const SEARCH_STATUS_CELLS: usize = 12;

pub fn tab_title(title: &str) -> String {
    if title.is_empty() { tr("Terminal") } else { title.to_string() }
}

pub fn search_bar(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let search = app.drawer().search();
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

pub struct TabEntry {
    pub id: TerminalId,
    pub label: String,
    pub active: bool,
}

pub fn tab_label(pane: &TerminalPane, pane_count: usize) -> String {
    let title = tab_title(&pane.title);
    let label = match exit_text(pane.exited) {
        Some(status) => format!("{title} · {status}"),
        None => title,
    };
    if pane_count > 1 { format!("{label} [{pane_count}]") } else { label }
}

pub fn ring_width(pane_count: usize) -> f32 {
    if pane_count > 1 { RING_WIDTH } else { 0.0 }
}

pub fn ring_color(focused: bool, palette: &Palette) -> Color {
    if focused { palette.accent } else { palette.line_strong }
}

pub fn tab_entries(drawer: &Drawer) -> Vec<TabEntry> {
    drawer
        .tabs
        .iter()
        .enumerate()
        .filter_map(|(index, tab)| {
            let pane = drawer.pane(tab.focus)?;
            Some(TabEntry {
                id: tab.id,
                label: tab_label(pane, tab.layout.leaves().len()),
                active: index == drawer.active,
            })
        })
        .collect()
}

pub fn tab_title_color(active: bool, drawer_focused: bool, palette: &Palette) -> Color {
    if active && drawer_focused { palette.fg_bright } else { palette.fg_muted }
}

fn tab_button<'a>(entry: TabEntry, focused: bool, palette: &Palette) -> Element<'a, Message> {
    let color = tab_title_color(entry.active, focused, palette);
    let title = fit(&entry.label, TITLE_CELLS);
    let text = if entry.active { bold_label(title, color) } else { label(title, color) };
    let underline = if entry.active { edge_color(focused, palette) } else { palette.bg };
    let close = hinted(
        compact_button("×", Some(Message::TerminalTabClosed(entry.id)), palette),
        tr("Close terminal tab"),
        iced::widget::tooltip::Position::Bottom,
        palette,
    );
    let body = row![container(text).padding([0.0, MD]), close].spacing(SM).align_y(alignment::Vertical::Center);
    let tab = column![
        container(body).height(Length::Fill).align_y(alignment::Vertical::Center),
        container(space()).width(Length::Fill).height(EDGE_WIDTH).style(filled(underline)),
    ];
    mouse_area(tab).on_press(Message::TerminalTabSelected(entry.id)).into()
}

#[cfg(test)]
pub fn strip_labels(app: &App) -> Vec<String> {
    let mut labels = vec![
        app.tunnels_tab_label(),
        app.health_tab_label(),
        app.services_tab_label(),
        app.ports_tab_label(),
        app.logs_tab_label(),
    ];
    labels.extend(tab_entries(app.drawer()).into_iter().map(|entry| entry.label));
    labels
}

fn view_entry<'a>(title: String, active: bool, press: Message, palette: &Palette) -> Element<'a, Message> {
    let color = tab_title_color(active, true, palette);
    let title = fit(&title, TITLE_CELLS);
    let text = if active { bold_label(title, color) } else { label(title, color) };
    let underline = if active { palette.accent } else { palette.bg };
    let tab = column![
        container(container(text).padding([0.0, MD])).height(Length::Fill).align_y(alignment::Vertical::Center),
        container(space()).width(Length::Fill).height(EDGE_WIDTH).style(filled(underline)),
    ];
    mouse_area(tab).on_press(press).into()
}

fn view_entries<'a>(app: &App, palette: &Palette) -> iced::widget::Row<'a, Message> {
    let view = app.drawer().view;
    row![
        view_entry(app.tunnels_tab_label(), view == DrawerView::Tunnels, Message::TunnelsViewSelected, palette),
        view_entry(app.health_tab_label(), view == DrawerView::Health, Message::HealthViewSelected, palette),
        view_entry(app.services_tab_label(), view == DrawerView::Services, Message::ServicesViewSelected, palette),
        view_entry(app.ports_tab_label(), view == DrawerView::Ports, Message::PortsViewSelected, palette),
        view_entry(app.logs_tab_label(), view == DrawerView::Logs, Message::LogsViewSelected, palette),
    ]
}

pub fn tab_strip(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let focused = app.drawer().shows_focus();
    let mut tabs = view_entries(app, palette).align_y(alignment::Vertical::Center);
    for mut entry in tab_entries(app.drawer()) {
        entry.active &= app.drawer().view == DrawerView::Terminals;
        tabs = tabs.push(tab_button(entry, focused, palette));
    }
    let new_tab = hinted(
        compact_button("+", Some(Message::TerminalNewTab), palette),
        tr("New terminal tab"),
        iced::widget::tooltip::Position::Bottom,
        palette,
    );
    row![container(tabs).clip(true).width(Length::Fill), container(new_tab).padding([0.0, SM])]
        .align_y(alignment::Vertical::Center)
        .into()
}

fn header(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let focused = app.drawer().shows_focus();
    let edge = container(space()).width(EDGE_WIDTH).height(Length::Fill).style(filled(edge_color(focused, palette)));
    let strip: Element<Message> = if app.drawer().tabs.is_empty() {
        let tunnels = view_entries(app, palette);
        let hint =
            container(label(tab_title(""), palette.fg_dim)).padding([0.0, MD]).align_y(alignment::Vertical::Center);
        row![tunnels, hint].align_y(alignment::Vertical::Center).into()
    } else {
        tab_strip(app)
    };
    let target = hovered_link(app.drawer()).map(|link| {
        let shown = fit(&link.target, TARGET_CELLS);
        label(i18n::text_with("gui.terminal.link_target", &[("target", &shown)]), palette.fg_dim)
    });
    let mut line = row![edge, container(strip).width(Length::Fill).height(Length::Fill)];
    if let Some(target) = target {
        line = line.push(container(target).padding([0.0, MD]).align_y(alignment::Vertical::Center));
    }
    line.height(DRAWER_HEADER_HEIGHT).into()
}

pub fn hovered_link(drawer: &Drawer) -> Option<&Link> {
    let tab = drawer.active_tab()?;
    tab.layout.leaves().into_iter().find_map(|id| drawer.pane(id)?.hover.as_ref())
}

pub fn pane_has_focus(drawer: &Drawer, tab: &TerminalTab, id: TerminalId) -> bool {
    drawer.open && drawer.focused && tab.focus == id
}

fn layout_view<'a>(app: &'a App, tab: &'a TerminalTab, layout: &'a Layout, count: usize) -> Element<'a, Message> {
    let drawer = app.drawer();
    let palette = app.palette();
    match layout {
        Layout::Leaf(id) => {
            let id = *id;
            let Some(pane) = drawer.pane(id) else {
                return container(space()).into();
            };
            let body: Element<Message> = match pane.snapshot() {
                Some(snapshot) => terminal_grid(
                    id,
                    snapshot,
                    palette,
                    drawer.shows_focus() && tab.focus == id,
                    move |size| Message::TerminalResized(id, size),
                    move |event| Message::TerminalPointer(id, event),
                )
                .link(pane.hover.as_ref().map_or_else(Vec::new, |link| link.segments.clone()))
                .into(),
                None => container(space()).into(),
            };
            let width = ring_width(count);
            if width == 0.0 {
                return body;
            }
            let ring = ring_color(pane_has_focus(drawer, tab, id), palette);
            container(body).padding(width).width(Length::Fill).height(Length::Fill).style(filled(ring)).into()
        }
        Layout::Split { direction, first, second } => {
            let half = |child: &'a Layout, direction: Direction| {
                let child = layout_view(app, tab, child, count);
                match direction {
                    Direction::Right => container(child).width(Length::FillPortion(1)).height(Length::Fill),
                    Direction::Down => container(child).width(Length::Fill).height(Length::FillPortion(1)),
                }
            };
            match direction {
                Direction::Right => row![half(first, *direction), half(second, *direction)].into(),
                Direction::Down => column![half(first, *direction), half(second, *direction)].into(),
            }
        }
    }
}

pub fn view(app: &App) -> Option<Element<'_, Message>> {
    let drawer = app.drawer();
    if !drawer.open {
        return None;
    }
    let palette = app.palette();
    let body: Element<Message> = match drawer.active_tab().filter(|_| drawer.view == DrawerView::Terminals) {
        Some(tab) => layout_view(app, tab, &tab.layout, tab.layout.leaves().len()),
        None if drawer.view == DrawerView::Tunnels => super::tunnel_list::view(app),
        None if drawer.view == DrawerView::Health => super::health::view(app),
        None if drawer.view == DrawerView::Services => super::services::view(app),
        None if drawer.view == DrawerView::Ports => super::ports::view(app),
        None if drawer.view == DrawerView::Logs => super::logs::view(app),
        None => container(label(empty_hint(app), palette.fg_dim))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
    };
    let body: Element<Message> = if drawer.search().open && drawer.view == DrawerView::Terminals {
        let bar = container(search_bar(app)).width(Length::Fill).align_x(alignment::Horizontal::Right).padding(SM);
        stack![body, bar].into()
    } else {
        body
    };
    let page = column![header(app), hairline_across(palette), body];
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
