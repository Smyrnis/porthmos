use iced::widget::{column, container, row, scrollable, space, tooltip};
use iced::{Alignment, Color, Element, Length, alignment};
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{TunnelInfo, TunnelStatus};

use super::button::{Look, compact_button, hinted, text_button};
use super::text_fit::fit;
use super::{bold_label, label};
use crate::app::{App, Message};
use crate::theme::Palette;
use crate::tunnels_view::{path_text, rate_text, total_text};
use crate::view::spacing::{MD, SM};

const ROW_HEIGHT: f32 = 32.0;
const DOT_WIDTH: f32 = 20.0;
const NAME_WIDTH: f32 = 140.0;
const NAME_CELLS: usize = 18;
const BADGE_WIDTH: f32 = 28.0;
const PATH_CELLS: usize = 44;
const VIA_CELLS: usize = 24;
const STATE_CELLS: usize = 28;
const TRAFFIC_WIDTH: f32 = 230.0;

pub fn start_enabled(status: &TunnelStatus) -> bool {
    matches!(status, TunnelStatus::Stopped | TunnelStatus::Lost | TunnelStatus::Failed(_))
}

pub fn stoppable(status: &TunnelStatus) -> bool {
    matches!(status, TunnelStatus::Running { .. } | TunnelStatus::Starting)
}

pub fn status_glyph(status: &TunnelStatus) -> &'static str {
    match status {
        TunnelStatus::Running { .. } => "●",
        TunnelStatus::Starting => "◐",
        TunnelStatus::Failed(_) => "!",
        TunnelStatus::Stopped | TunnelStatus::Offline | TunnelStatus::Lost => "○",
    }
}

pub fn status_color(status: &TunnelStatus, palette: &Palette) -> Color {
    match status {
        TunnelStatus::Running { .. } => palette.ok,
        TunnelStatus::Starting => palette.paused,
        TunnelStatus::Failed(_) => palette.err,
        TunnelStatus::Stopped | TunnelStatus::Offline | TunnelStatus::Lost => palette.fg_dim,
    }
}

pub fn status_text(status: &TunnelStatus) -> String {
    match status {
        TunnelStatus::Stopped => String::new(),
        TunnelStatus::Offline => tr("offline"),
        TunnelStatus::Starting => tr("Starting\u{2026}"),
        TunnelStatus::Running { .. } => tr("running"),
        TunnelStatus::Failed(message) => message.to_string(),
        TunnelStatus::Lost => tr("connection lost"),
    }
}

pub fn counts_text(info: &TunnelInfo) -> String {
    i18n::text_with(
        "gui.tunnels.counts",
        &[
            ("connections", &info.stats.connections.to_string()),
            ("errors", &info.stats.errors.to_string()),
            ("refused", &info.stats.refused.to_string()),
        ],
    )
}

pub fn traffic_text(up: u64, down: u64, info: &TunnelInfo) -> String {
    i18n::text_with(
        "gui.tunnels.traffic",
        &[
            ("up", &rate_text(up)),
            ("down", &rate_text(down)),
            ("total_up", &total_text(info.stats.bytes_up)),
            ("total_down", &total_text(info.stats.bytes_down)),
        ],
    )
}

fn actions<'a>(info: &TunnelInfo, palette: &Palette) -> Element<'a, Message> {
    let key = info.spec.key();
    let toggle = if stoppable(&info.status) {
        text_button(&tr("Stop"), Some(Message::TunnelStopRequested(key.clone())), Look::Normal, palette)
    } else {
        let press = start_enabled(&info.status).then(|| Message::TunnelStartRequested(key.clone()));
        text_button(&tr("Start"), press, Look::Normal, palette)
    };
    let running = matches!(info.status, TunnelStatus::Running { .. });
    let restart =
        compact_button(&tr("Restart"), running.then(|| Message::TunnelRestartRequested(key.clone())), palette);
    let edit = compact_button(&tr("Edit"), Some(Message::TunnelFormRequested(Some(key.clone()))), palette);
    let delete = compact_button(&tr("Delete"), Some(Message::TunnelDeleteRequested(key)), palette);
    row![toggle, restart, edit, delete].spacing(SM).align_y(Alignment::Center).into()
}

fn tunnel_row<'a>(app: &'a App, info: &'a TunnelInfo, palette: &Palette) -> Element<'a, Message> {
    let key = info.spec.key();
    let (up, down) = app.tunnels().rate(&key);
    let dot = hinted(
        label(status_glyph(&info.status), status_color(&info.status, palette)).width(Length::Fixed(DOT_WIDTH)).into(),
        counts_text(info),
        tooltip::Position::Top,
        palette,
    );
    let state = status_text(&info.status);
    let state_color = if matches!(info.status, TunnelStatus::Failed(_)) { palette.err } else { palette.fg_dim };
    let via = i18n::text_with("gui.tunnels.via", &[("connection", &fit(&info.spec.connection, VIA_CELLS))]);
    let line = row![
        dot,
        container(bold_label(fit(&info.spec.name, NAME_CELLS), palette.fg_bright)).width(Length::Fixed(NAME_WIDTH)),
        label(info.spec.kind.label(), palette.fg_muted).width(Length::Fixed(BADGE_WIDTH)),
        label(fit(&path_text(&info.spec), PATH_CELLS), palette.fg),
        label(via, palette.fg_dim),
        container(label(fit(&state, STATE_CELLS), state_color)).width(Length::Fill),
        container(label(traffic_text(up, down, info), palette.fg_muted).align_x(alignment::Horizontal::Right))
            .width(Length::Fixed(TRAFFIC_WIDTH)),
        actions(info, palette),
    ]
    .spacing(SM)
    .align_y(Alignment::Center);
    container(line)
        .padding([0.0, MD])
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let new = text_button(&tr("New tunnel\u{2026}"), Some(Message::TunnelFormRequested(None)), Look::Primary, palette);
    let top = container(row![space().width(Length::Fill), new].align_y(Alignment::Center))
        .padding([SM, MD])
        .width(Length::Fill);
    let rows = &app.tunnels().rows;
    let body: Element<Message> = if rows.is_empty() {
        container(label(i18n::text("gui.tunnels.empty"), palette.fg_dim))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let mut list = column![];
        for info in rows {
            list = list.push(tunnel_row(app, info, palette));
        }
        scrollable(list).style(super::scroll::style(palette)).width(Length::Fill).height(Length::Fill).into()
    };
    column![top, body].into()
}

#[cfg(test)]
mod tests;
