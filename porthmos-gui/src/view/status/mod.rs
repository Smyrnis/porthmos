use iced::widget::{container, row, tooltip};
use iced::{Element, Length, alignment};
use porthmos_core::Severity;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::transfer::limit::speed_limit_note;

use super::button::hinted;
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
        Some(filter) => i18n::text_with(
            "gui.status.match_count",
            &[("matched", &matched.to_string()), ("shown", &shown.to_string()), ("filter", filter)],
        ),
        None => i18n::plural("list.items", shown),
    };
    let free =
        panel.free_bytes().map(|bytes| i18n::text_with("gui.status.free", &[("size", &format::size(bytes, false))]));
    let hidden = panel.show_hidden().then(|| i18n::text("gui.status.hidden_shown"));
    [Some(count), hidden, free].into_iter().flatten().collect::<Vec<_>>().join(SEPARATOR)
}

pub fn selection_text(panel: &Panel) -> Option<String> {
    let count = panel.selection_count();
    if count == 0 {
        return None;
    }
    let files: Vec<u64> = panel.selected().iter().filter(|entry| !entry.is_dir).map(|entry| entry.size).collect();
    if files.is_empty() {
        return Some(i18n::text_with("list.selected", &[("count", &count.to_string())]));
    }
    let total: u64 = files.iter().sum();
    let size = format::size(total, false);
    Some(format!("{}{SEPARATOR}{size}", i18n::text_with("list.selected", &[("count", &count.to_string())])))
}

pub fn connection_text(app: &App) -> (bool, String) {
    match app.sessions().names().into_iter().find(|tab| tab.active) {
        Some(tab) if tab.healthy => (true, i18n::text_with("gui.status.connected", &[("name", &tab.name)])),
        Some(tab) => (false, i18n::text_with("gui.status.disconnected", &[("name", &tab.name)])),
        None => (false, tr("No connection")),
    }
}

pub fn panel_line<'a>(panel: &Panel, palette: &Palette, cells: usize, editing: bool) -> Element<'a, Message> {
    let mut summary = panel_summary(panel);
    if editing {
        summary = i18n::text_with("gui.status.filter_editing", &[("text", panel.filter().unwrap_or(""))]);
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

pub fn limit_text(app: &App) -> Option<String> {
    speed_limit_note(app.transfer_speed_limit())
}

pub fn keyring_text(waiting: bool) -> Option<String> {
    waiting.then(|| i18n::text("gui.status.keyring_waiting"))
}

pub fn notice_text(severity: Severity, message: &str) -> String {
    match severity {
        Severity::Error => i18n::text_with("gui.status.error", &[("message", message)]),
        Severity::Warning => i18n::text_with("gui.status.warning", &[("message", message)]),
        Severity::Info => message.to_string(),
    }
}

pub fn bar(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let notice = app.notices().last();
    let content = match (app.connecting(), notice) {
        _ if app.is_closing() => label(tr("Closing…"), palette.fg_muted),
        (Some(name), _) => label(i18n::text_with("gui.status.connecting", &[("name", name)]), palette.fg_muted),
        (None, Some(notice)) => {
            let color = match notice.severity {
                Severity::Error => palette.err,
                Severity::Warning => palette.fg_bright,
                Severity::Info => palette.fg_muted,
            };
            label(one_line(&notice_text(notice.severity, &notice.message.one_line())), color)
        }
        (None, None) => label(tr("Ready"), palette.fg_dim),
    };
    let content: Element<'_, Message> = match (app.connecting(), notice) {
        (None, Some(notice)) if !app.is_closing() => hinted(
            content.into(),
            notice_text(notice.severity, &notice.message.to_string()),
            tooltip::Position::Top,
            palette,
        ),
        _ => content.into(),
    };
    let left: Element<'_, Message> = match app.transfer_summary() {
        Some(summary) => row![content, label(format!("{SEPARATOR}{summary}"), palette.accent)].into(),
        None => content,
    };
    let active_panel = match app.active_side() {
        PanelSide::Local => Some(&app.local().panel),
        PanelSide::Remote => app.right_panel(),
    };
    let mut right = row![].spacing(14).align_y(alignment::Vertical::Center);
    if let Some(text) = active_panel.and_then(selection_text) {
        right = right.push(label(text, palette.accent));
    }
    if let Some(text) = limit_text(app) {
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
