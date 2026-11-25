use iced::widget::{column, container, mouse_area, pick_list, row, scrollable, space, text_input};
use iced::{Alignment, Element, Font, Length, alignment};
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{HealthProcess, HealthSnapshot, MonitorState, ProcessSort, UNKNOWN_UID, arranged};

use super::button::{Look, hinted, text_button};
use super::spacing::{MD, SM};
use super::text_fit::fit;
use super::{bold_label, filled, hairline_down, label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::health_view::{
    ActionState, HealthState, action_state, disk_text, fraction, memory_text, memory_used, percent_text, swap_text,
    swap_used, uptime_text,
};
use crate::theme::Palette;
use crate::tunnels_view::rate_text;

const SUMMARY_WIDTH: f32 = 420.0;
const NAME_WIDTH: f32 = 72.0;
const METER_WIDTH: f32 = 96.0;
const METER_HEIGHT: f32 = 6.0;
const ALERT_FRACTION: f32 = 0.9;
const PICKER_WIDTH: f32 = 220.0;
const PID_WIDTH: f32 = 64.0;
const USER_WIDTH: f32 = 96.0;
const PERCENT_WIDTH: f32 = 64.0;
const HOST_CELLS: usize = 48;
const MOUNT_CELLS: usize = 10;
const INTERFACE_CELLS: usize = 10;
const USER_CELLS: usize = 12;
const COMMAND_CELLS: usize = 200;
const UNKNOWN: &str = "\u{2026}";

pub fn shows_session_picker(app: &App) -> bool {
    app.health_session_names().len() >= 2
}

pub fn shown<'a>(state: &HealthState, snapshot: &'a HealthSnapshot) -> Vec<&'a HealthProcess> {
    arranged(&snapshot.processes, state.sort, &state.filter, state.selected)
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into()
}

fn meter<'a>(used: f32, palette: &Palette) -> Element<'a, Message> {
    let used = used.clamp(0.0, 1.0);
    let color = if used >= ALERT_FRACTION { palette.err } else { palette.accent };
    let bar = container(space()).width(METER_WIDTH * used).height(METER_HEIGHT).style(filled(color));
    container(bar).width(METER_WIDTH).height(METER_HEIGHT).style(filled(palette.line)).into()
}

fn stat_row<'a>(name: String, used: Option<f32>, value: String, palette: &Palette) -> Element<'a, Message> {
    let bar: Element<Message> = match used {
        Some(used) => meter(used, palette),
        None => container(space()).width(METER_WIDTH).height(METER_HEIGHT).into(),
    };
    row![container(label(name, palette.fg_muted)).width(NAME_WIDTH).clip(true), bar, label(value, palette.fg)]
        .spacing(SM)
        .align_y(Alignment::Center)
        .into()
}

fn cores_text(cores: Option<u32>) -> String {
    cores.map_or_else(|| UNKNOWN.to_string(), |cores| i18n::plural("gui.health.cores", cores as usize))
}

fn summary<'a>(snapshot: &HealthSnapshot, palette: &Palette) -> Element<'a, Message> {
    let host = &snapshot.host;
    let name = host.hostname.clone().unwrap_or_else(|| UNKNOWN.to_string());
    let details: Vec<String> =
        [host.os.clone(), host.kernel.clone(), host.uptime_seconds.map(uptime_text)].into_iter().flatten().collect();
    let mut lines = column![bold_label(fit(&name, HOST_CELLS), palette.fg_bright)].spacing(SM);
    if !details.is_empty() {
        lines = lines.push(label(fit(&details.join(" \u{b7} "), HOST_CELLS), palette.fg_muted));
    }
    if !snapshot.problems.is_empty() {
        lines = lines.push(label(i18n::text("gui.health.partial"), palette.fg_dim));
    }
    let cpu_value = format!("{} \u{b7} {}", percent_text(snapshot.cpu.percent), cores_text(snapshot.cpu.cores));
    lines = lines.push(stat_row(tr("CPU"), snapshot.cpu.percent.map(|percent| percent / 100.0), cpu_value, palette));
    if let Some(load) = snapshot.load {
        let values = [
            ("one", format!("{:.2}", load.one)),
            ("five", format!("{:.2}", load.five)),
            ("fifteen", format!("{:.2}", load.fifteen)),
        ];
        let values: Vec<(&str, &str)> = values.iter().map(|(key, value)| (*key, value.as_str())).collect();
        lines = lines.push(label(i18n::text_with("gui.health.load", &values), palette.fg_muted));
    }
    if let Some(memory) = snapshot.memory {
        lines = lines.push(stat_row(
            tr("Memory"),
            Some(fraction(memory_used(&memory), memory.total)),
            memory_text(&memory),
            palette,
        ));
        if let Some(text) = swap_text(&memory) {
            lines =
                lines.push(stat_row(tr("Swap"), Some(fraction(swap_used(&memory), memory.swap_total)), text, palette));
        }
    }
    if !snapshot.disks.is_empty() {
        lines = lines.push(bold_label(tr("Disks"), palette.fg));
        for disk in &snapshot.disks {
            lines = lines.push(stat_row(
                fit(&disk.mount, MOUNT_CELLS).to_string(),
                Some(fraction(disk.used, disk.total)),
                disk_text(disk),
                palette,
            ));
        }
    }
    if !snapshot.interfaces.is_empty() {
        lines = lines.push(bold_label(tr("Network"), palette.fg));
        for interface in &snapshot.interfaces {
            let rate = |value: Option<u64>| value.map_or_else(|| UNKNOWN.to_string(), rate_text);
            let (down, up) = (rate(interface.rx_per_second), rate(interface.tx_per_second));
            let value = i18n::text_with("gui.health.rates", &[("down", &down), ("up", &up)]);
            lines = lines.push(stat_row(fit(&interface.name, INTERFACE_CELLS).to_string(), None, value, palette));
        }
    }
    scrollable(container(lines).padding(MD))
        .style(super::scroll::style(palette))
        .width(Length::Fixed(SUMMARY_WIDTH))
        .height(Length::Fill)
        .into()
}

fn user_text(process: &HealthProcess) -> String {
    match (&process.user, process.uid) {
        (Some(user), _) => user.clone(),
        (None, UNKNOWN_UID) => UNKNOWN.to_string(),
        (None, uid) => uid.to_string(),
    }
}

fn cell<'a>(text: String, width: f32, color: iced::Color) -> Element<'a, Message> {
    container(label(text, color)).width(width).clip(true).into()
}

fn process_row<'a>(process: &HealthProcess, selected: bool, palette: &Palette) -> Element<'a, Message> {
    let color = if selected { palette.fg_focus } else { palette.fg };
    let background = if selected { palette.row_focus } else { palette.bg };
    let line = row![
        cell(process.pid.to_string(), PID_WIDTH, color),
        cell(fit(&user_text(process), USER_CELLS).to_string(), USER_WIDTH, color),
        cell(percent_text(process.cpu_percent), PERCENT_WIDTH, color),
        cell(percent_text(Some(process.memory_percent)), PERCENT_WIDTH, color),
        container(label(fit(&process.command, COMMAND_CELLS), color)).width(Length::Fill).clip(true),
    ]
    .spacing(SM)
    .align_y(Alignment::Center);
    mouse_area(
        container(line)
            .padding([0.0, SM])
            .width(Length::Fill)
            .height(ROW_HEIGHT)
            .align_y(alignment::Vertical::Center)
            .style(filled(background)),
    )
    .on_press(Message::HealthProcessSelected(process.pid))
    .into()
}

fn sort_button<'a>(title: String, sort: ProcessSort, current: ProcessSort, palette: &Palette) -> Element<'a, Message> {
    let look = if sort == current { Look::Chosen } else { Look::Normal };
    text_button(&title, Some(Message::HealthSortChosen(sort)), look, palette)
}

fn actions<'a>(app: &App, snapshot: &HealthSnapshot, palette: &Palette) -> Element<'a, Message> {
    let state = action_state(snapshot, app.health().selected);
    let allowed = state == ActionState::Allowed;
    let press = |message: Message| allowed.then_some(message);
    let buttons = row![
        text_button(&tr("Terminate"), press(Message::HealthTerminateRequested), Look::Normal, palette),
        text_button(&tr("Force kill"), press(Message::HealthKillRequested), Look::Normal, palette),
        text_button(&tr("Renice\u{2026}"), press(Message::HealthReniceRequested), Look::Normal, palette),
    ]
    .spacing(SM);
    if state == ActionState::NeedsRoot {
        hinted(buttons.into(), tr("Needs root to change this process"), iced::widget::tooltip::Position::Top, palette)
    } else {
        buttons.into()
    }
}

fn processes<'a>(app: &'a App, snapshot: &'a HealthSnapshot, palette: &Palette) -> Element<'a, Message> {
    let state = app.health();
    let filter = text_input(&tr("Filter processes"), &state.filter)
        .style(super::input::style(palette))
        .on_input(Message::HealthFilterChanged)
        .size(13.0)
        .font(Font::DEFAULT)
        .width(Length::Fill);
    let bar = row![
        filter,
        sort_button(tr("CPU"), ProcessSort::Cpu, state.sort, palette),
        sort_button(tr("Memory"), ProcessSort::Memory, state.sort, palette),
    ]
    .spacing(SM)
    .align_y(Alignment::Center);
    let head = row![
        cell(tr("PID"), PID_WIDTH, palette.fg_dim),
        cell(tr("User"), USER_WIDTH, palette.fg_dim),
        cell(tr("CPU"), PERCENT_WIDTH, palette.fg_dim),
        cell(tr("Memory"), PERCENT_WIDTH, palette.fg_dim),
        container(label(tr("Command"), palette.fg_dim)).width(Length::Fill),
    ]
    .spacing(SM);
    let mut rows = column![];
    for process in shown(state, snapshot) {
        rows = rows.push(process_row(process, state.selected == Some(process.pid), palette));
    }
    column![
        container(bar).padding([SM, MD]).width(Length::Fill),
        container(actions(app, snapshot, palette)).padding([0.0, MD]).width(Length::Fill),
        container(head).padding([0.0, MD]).width(Length::Fill),
        scrollable(rows).style(super::scroll::style(palette)).width(Length::Fill).height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}

fn body<'a>(app: &'a App, palette: &Palette) -> Element<'a, Message> {
    match &app.health().state {
        None => centered(label(i18n::text("gui.health.loading"), palette.fg_dim)),
        Some(MonitorState::Unavailable(message)) => centered(label(message.to_string(), palette.fg_dim)),
        Some(MonitorState::Failing(message)) => centered(
            column![
                label(message.to_string(), palette.err),
                text_button(&tr("Retry"), Some(Message::HealthRetry), Look::Primary, palette),
            ]
            .spacing(MD)
            .align_x(Alignment::Center),
        ),
        Some(MonitorState::Snapshot(snapshot)) => {
            row![summary(snapshot, palette), hairline_down(palette), processes(app, snapshot, palette)].into()
        }
    }
}

fn picker<'a>(app: &App, palette: &Palette) -> Element<'a, Message> {
    let current = app.health_target().and_then(|id| app.sessions().by_id(id)).map(|session| session.name.clone());
    let list = pick_list(app.health_session_names(), current, Message::HealthSessionChosen)
        .text_size(13.0)
        .padding([6.0, 8.0])
        .style(super::dropdown::style(palette))
        .menu_style(super::dropdown::menu_style(palette))
        .width(Length::Fixed(PICKER_WIDTH));
    container(list).padding([SM, MD]).width(Length::Fill).into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    if app.health_target().is_none() {
        return centered(label(tr("Connect to a server first"), palette.fg_dim));
    }
    let mut page = column![];
    if shows_session_picker(app) {
        page = page.push(picker(app, palette));
    }
    page.push(container(body(app, palette)).width(Length::Fill).height(Length::Fill)).into()
}

#[cfg(test)]
mod tests;
