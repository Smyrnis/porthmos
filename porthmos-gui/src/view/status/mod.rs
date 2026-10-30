use iced::widget::{container, row};
use iced::{Element, Length, alignment};
use porthmos_core::Severity;

use super::file_list::PADDING;
use super::text_fit::fit;
use super::{filled, format, label};
use crate::app::{App, Message, PanelSide};
use crate::geometry::STATUS_HEIGHT as BAR_HEIGHT;
use crate::panel::Panel;
use crate::theme::Palette;

const SEPARATOR: &str = "  ·  ";

pub fn panel_summary(panel: &Panel) -> String {
    let (matched, shown) = panel.match_count();
    let count = match panel.filter() {
        Some(filter) => format!("{matched} of {shown} match \"{filter}\""),
        None => format!("{shown} items"),
    };
    let free = panel.free_bytes().map(|bytes| format!("{} free", format::size(bytes, false)));
    let hidden = panel.show_hidden().then(|| "hidden shown".to_string());
    [Some(count), hidden, free].into_iter().flatten().collect::<Vec<_>>().join(SEPARATOR)
}

pub fn selection_text(panel: &Panel) -> Option<String> {
    let count = panel.selection_count();
    if count == 0 {
        return None;
    }
    let files: Vec<u64> = panel.selected().iter().filter(|entry| !entry.is_dir).map(|entry| entry.size).collect();
    if files.is_empty() {
        return Some(format!("{count} selected"));
    }
    let total: u64 = files.iter().sum();
    let size = if total < 1024 { format!("{total} B") } else { format::size(total, false) };
    Some(format!("{count} selected{SEPARATOR}{size}"))
}

pub fn connection_text(app: &App) -> (bool, String) {
    match app.sessions().names().into_iter().find(|tab| tab.active) {
        Some(tab) if tab.healthy => (true, format!("{} connected", tab.name)),
        Some(tab) => (false, format!("{} disconnected", tab.name)),
        None => (false, "No connection".to_string()),
    }
}

pub fn panel_line<'a>(panel: &Panel, palette: &Palette, cells: usize, editing: bool) -> Element<'a, Message> {
    let mut summary = panel_summary(panel);
    if editing {
        summary = format!("Filter: {}▏  (Enter keeps, Esc clears)", panel.filter().unwrap_or(""));
    }
    container(label(fit(&summary, cells), palette.fg_dim))
        .padding([0.0, PADDING])
        .width(Length::Fill)
        .height(Length::Fixed(BAR_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .style(filled(palette.bg))
        .into()
}

pub fn keyring_text(waiting: bool) -> Option<&'static str> {
    waiting.then_some("Waiting for the keyring\u{2026}")
}

pub fn bar(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let content = match (app.connecting(), app.notices().last()) {
        _ if app.is_closing() => label("Closing…", palette.fg_muted),
        (Some(name), _) => label(format!("Connecting to {name}…"), palette.fg_muted),
        (None, Some(notice)) => match notice.severity {
            Severity::Error => label(format!("Error: {}", one_line(&notice.message)), palette.err),
            Severity::Warning => label(format!("Warning: {}", one_line(&notice.message)), palette.fg_bright),
            Severity::Info => label(one_line(&notice.message), palette.fg_muted),
        },
        (None, None) => label("Ready", palette.fg_dim),
    };
    let left: Element<'_, Message> = match app.transfer_summary() {
        Some(summary) => row![content, label(format!("{SEPARATOR}{summary}"), palette.accent)].into(),
        None => content.into(),
    };
    let active_panel = match app.active_side() {
        PanelSide::Local => Some(&app.local().panel),
        PanelSide::Remote => app.right_panel(),
    };
    let mut right = row![].spacing(14).align_y(alignment::Vertical::Center);
    if let Some(text) = active_panel.and_then(selection_text) {
        right = right.push(label(text, palette.accent));
    }
    if let Some(text) = keyring_text(app.keyring_waiting()) {
        right = right.push(label(text, palette.accent));
    }
    let (connected, text) = connection_text(app);
    let (dot, color) = if connected { ("●", palette.ok) } else { ("○", palette.fg_dim) };
    right = right.push(label(dot, color)).push(label(text, palette.fg_muted));
    let line =
        row![container(left).width(Length::Fill).clip(true), right].spacing(12).align_y(alignment::Vertical::Center);
    container(line)
        .padding([0.0, PADDING])
        .width(Length::Fill)
        .height(Length::Fixed(BAR_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .style(filled(palette.bg))
        .into()
}

fn one_line(message: &str) -> String {
    super::text_fit::fit(message, 400)
}

#[cfg(test)]
mod tests;
