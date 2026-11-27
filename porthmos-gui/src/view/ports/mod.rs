use iced::widget::{column, container, pick_list, row, scrollable, text_input};
use iced::{Alignment, Color, Element, Font, Length, alignment};
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{NetInterface, PortsSnapshot, PortsState as PortsEvent, Socket, arranged_sockets, size_text};

use super::button::{Look, hinted, hinted_if, text_button};
use super::spacing::{MD, SM};
use super::text_fit::{fit, full_text_hint};
use super::{hairline_across, label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::ports_view::{
    PortsPane, PortsState, addresses_text, interface_state_text, mac_text, mtu_text, owner_hint, owner_text,
    protocol_text, socket_state_text,
};
use crate::theme::Palette;

const PICKER_WIDTH: f32 = 220.0;
const PROTOCOL_WIDTH: f32 = 64.0;
const ADDRESS_WIDTH: f32 = 224.0;
const STATE_WIDTH: f32 = 112.0;
const NAME_WIDTH: f32 = 96.0;
const LINK_WIDTH: f32 = 72.0;
const MAC_WIDTH: f32 = 152.0;
const MTU_WIDTH: f32 = 64.0;
const TRAFFIC_WIDTH: f32 = 96.0;
const ADDRESS_CELLS: usize = 28;
const NAME_CELLS: usize = 12;
const OWNER_CELLS: usize = 80;
const ADDRESSES_CELLS: usize = 120;

pub fn shows_session_picker(app: &App) -> bool {
    app.health_session_names().len() >= 2
}

pub fn shows_filter(pane: PortsPane) -> bool {
    pane != PortsPane::Interfaces
}

pub fn shown_listening<'a>(state: &PortsState, snapshot: &'a PortsSnapshot) -> Vec<&'a Socket> {
    arranged_sockets(&snapshot.listening, &state.filter)
}

pub fn shown_connections<'a>(state: &PortsState, snapshot: &'a PortsSnapshot) -> Vec<&'a Socket> {
    arranged_sockets(&snapshot.connections, &state.filter)
}

pub fn row_color(dimmed: bool, palette: &Palette) -> Color {
    if dimmed { palette.fg_dim } else { palette.fg }
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into()
}

fn cell<'a>(text: String, width: f32, color: Color) -> Element<'a, Message> {
    container(label(text, color)).width(width).clip(true).into()
}

fn wide_cell<'a>(text: String, color: Color) -> Element<'a, Message> {
    container(label(text, color)).width(Length::Fill).clip(true).into()
}

fn table_row<'a>(line: iced::widget::Row<'a, Message>) -> Element<'a, Message> {
    container(line.spacing(SM).align_y(Alignment::Center))
        .padding([0.0, SM])
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .align_y(alignment::Vertical::Center)
        .into()
}

pub fn owner_tooltip(uid: u32, socket: &Socket) -> Option<String> {
    owner_hint(uid, socket).or_else(|| full_text_hint(&owner_text(socket), OWNER_CELLS))
}

pub fn address_tooltip(address: &str) -> Option<String> {
    full_text_hint(address, ADDRESS_CELLS)
}

fn address_cell<'a>(address: &str, color: Color, palette: &Palette) -> Element<'a, Message> {
    let cell = cell(fit(address, ADDRESS_CELLS), ADDRESS_WIDTH, color);
    hinted_if(cell, address_tooltip(address), iced::widget::tooltip::Position::Top, palette)
}

fn owner_cell<'a>(uid: u32, socket: &Socket, color: Color, palette: &Palette) -> Element<'a, Message> {
    let cell = wide_cell(fit(&owner_text(socket), OWNER_CELLS), color);
    hinted_if(cell, owner_tooltip(uid, socket), iced::widget::tooltip::Position::Top, palette)
}

fn socket_head<'a>(connections: bool, palette: &Palette) -> Element<'a, Message> {
    let dim = palette.fg_dim;
    let mut line = row![cell(tr("Protocol"), PROTOCOL_WIDTH, dim), cell(tr("Local address"), ADDRESS_WIDTH, dim)];
    if connections {
        line = line.push(cell(tr("Remote address"), ADDRESS_WIDTH, dim)).push(cell(tr("State"), STATE_WIDTH, dim));
    }
    table_row(line.push(wide_cell(tr("Process"), dim)))
}

fn socket_row<'a>(
    socket: &Socket, connections: bool, uid: u32, dimmed: bool, palette: &Palette,
) -> Element<'a, Message> {
    let color = row_color(dimmed, palette);
    let mut line = row![
        cell(protocol_text(socket.protocol).to_string(), PROTOCOL_WIDTH, color),
        address_cell(&socket.local, color, palette),
    ];
    if connections {
        line = line.push(address_cell(&socket.remote, color, palette)).push(cell(
            socket_state_text(socket.state).to_string(),
            STATE_WIDTH,
            color,
        ));
    }
    table_row(line.push(owner_cell(uid, socket, color, palette)))
}

fn interface_head<'a>(palette: &Palette) -> Element<'a, Message> {
    let dim = palette.fg_dim;
    table_row(row![
        cell(tr("Name"), NAME_WIDTH, dim),
        cell(tr("State"), LINK_WIDTH, dim),
        wide_cell(tr("Addresses"), dim),
        cell(tr("MAC"), MAC_WIDTH, dim),
        cell(tr("MTU"), MTU_WIDTH, dim),
        cell(tr("Received"), TRAFFIC_WIDTH, dim),
        cell(tr("Sent"), TRAFFIC_WIDTH, dim),
    ])
}

fn interface_row<'a>(interface: &NetInterface, dimmed: bool, palette: &Palette) -> Element<'a, Message> {
    let color = row_color(dimmed, palette);
    table_row(row![
        cell(fit(&interface.name, NAME_CELLS), NAME_WIDTH, color),
        cell(interface_state_text(interface.up), LINK_WIDTH, color),
        wide_cell(fit(&addresses_text(interface), ADDRESSES_CELLS), color),
        cell(mac_text(interface), MAC_WIDTH, color),
        cell(mtu_text(interface), MTU_WIDTH, color),
        cell(size_text(interface.rx_bytes), TRAFFIC_WIDTH, color),
        cell(size_text(interface.tx_bytes), TRAFFIC_WIDTH, color),
    ])
}

fn nothing<'a>(palette: &Palette) -> Element<'a, Message> {
    container(label(i18n::text("gui.ports.nothing"), palette.fg_dim)).padding(MD).into()
}

fn table<'a>(head: Element<'a, Message>, rows: Vec<Element<'a, Message>>, palette: &Palette) -> Element<'a, Message> {
    let body: Element<Message> =
        if rows.is_empty() { nothing(palette) } else { column(rows).width(Length::Fill).into() };
    column![
        container(head).padding([0.0, MD]).width(Length::Fill),
        scrollable(container(body).padding([0.0, MD]).width(Length::Fill))
            .style(super::scroll::style(palette))
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}

fn tables<'a>(state: &PortsState, snapshot: &PortsSnapshot, dimmed: bool, palette: &Palette) -> Element<'a, Message> {
    let uid = snapshot.uid;
    match state.pane {
        PortsPane::Listening => {
            let rows = shown_listening(state, snapshot)
                .into_iter()
                .map(|socket| socket_row(socket, false, uid, dimmed, palette))
                .collect();
            table(socket_head(false, palette), rows, palette)
        }
        PortsPane::Connections => {
            let rows = shown_connections(state, snapshot)
                .into_iter()
                .map(|socket| socket_row(socket, true, uid, dimmed, palette))
                .collect();
            table(socket_head(true, palette), rows, palette)
        }
        PortsPane::Interfaces => {
            let rows = snapshot.interfaces.iter().map(|interface| interface_row(interface, dimmed, palette)).collect();
            table(interface_head(palette), rows, palette)
        }
    }
}

fn failing_bar<'a>(message: String, palette: &Palette) -> Element<'a, Message> {
    let notice = hinted(
        label(i18n::text("gui.ports.updating_failed"), palette.err).into(),
        message,
        iced::widget::tooltip::Position::Bottom,
        palette,
    );
    let line = row![notice, text_button(&tr("Retry"), Some(Message::PortsRetry), Look::Primary, palette)]
        .spacing(SM)
        .align_y(Alignment::Center);
    container(line).padding([SM, MD]).width(Length::Fill).into()
}

fn partial<'a>(snapshot: &PortsSnapshot, palette: &Palette) -> Option<Element<'a, Message>> {
    (!snapshot.problems.is_empty()).then(|| {
        container(label(i18n::text("gui.ports.partial"), palette.fg_dim)).padding([0.0, MD]).width(Length::Fill).into()
    })
}

fn snapshot_page<'a>(
    state: &PortsState, snapshot: &PortsSnapshot, failure: Option<String>, palette: &Palette,
) -> Element<'a, Message> {
    let mut page = column![];
    if let Some(message) = failure {
        page = page.push(failing_bar(message, palette));
    }
    if let Some(partial) = partial(snapshot, palette) {
        page = page.push(partial);
    }
    page.push(tables(state, snapshot, state.failing(), palette)).into()
}

fn body<'a>(app: &'a App, palette: &Palette) -> Element<'a, Message> {
    let state = app.ports();
    match &state.state {
        None => centered(label(i18n::text("gui.ports.loading"), palette.fg_dim)),
        Some(PortsEvent::Unavailable(message)) => centered(label(message.to_string(), palette.fg_dim)),
        Some(PortsEvent::Snapshot(snapshot)) => snapshot_page(state, snapshot, None, palette),
        Some(PortsEvent::Failing(message)) => match state.snapshot() {
            Some(snapshot) => snapshot_page(state, snapshot, Some(message.to_string()), palette),
            None => centered(
                column![
                    label(message.to_string(), palette.err),
                    text_button(&tr("Retry"), Some(Message::PortsRetry), Look::Primary, palette),
                ]
                .spacing(MD)
                .align_x(Alignment::Center),
            ),
        },
    }
}

fn pane_button<'a>(title: String, pane: PortsPane, current: PortsPane, palette: &Palette) -> Element<'a, Message> {
    let look = if pane == current { Look::Chosen } else { Look::Normal };
    text_button(&title, Some(Message::PortsViewChosen(pane)), look, palette)
}

fn picker<'a>(app: &App, palette: &Palette) -> Element<'a, Message> {
    let current = app.health_target().and_then(|id| app.sessions().by_id(id)).map(|session| session.name.clone());
    pick_list(app.health_session_names(), current, Message::HealthSessionChosen)
        .text_size(13.0)
        .padding([6.0, 8.0])
        .style(super::dropdown::style(palette))
        .menu_style(super::dropdown::menu_style(palette))
        .width(Length::Fixed(PICKER_WIDTH))
        .into()
}

fn header<'a>(app: &App, palette: &Palette) -> Element<'a, Message> {
    let state = app.ports();
    let mut line = row![].spacing(SM).align_y(Alignment::Center);
    if shows_session_picker(app) {
        line = line.push(picker(app, palette));
    }
    line = line
        .push(pane_button(tr("Listening"), PortsPane::Listening, state.pane, palette))
        .push(pane_button(tr("Connections"), PortsPane::Connections, state.pane, palette))
        .push(pane_button(tr("Interfaces"), PortsPane::Interfaces, state.pane, palette));
    if shows_filter(state.pane) {
        line = line.push(
            text_input(&tr("Filter sockets"), &state.filter)
                .style(super::input::style(palette))
                .on_input(Message::PortsFilterChanged)
                .size(13.0)
                .font(Font::DEFAULT)
                .width(Length::Fill),
        );
    } else {
        line = line.push(container(iced::widget::space()).width(Length::Fill));
    }
    let refresh = text_button(&tr("Refresh"), state.started.map(|_| Message::PortsRefresh), Look::Normal, palette);
    container(line.push(refresh)).padding([SM, MD]).width(Length::Fill).into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    if app.health_target().is_none() {
        return centered(label(tr("Connect to a server first"), palette.fg_dim));
    }
    column![
        header(app, palette),
        hairline_across(palette),
        container(body(app, palette)).width(Length::Fill).height(Length::Fill),
    ]
    .into()
}

#[cfg(test)]
mod tests;
