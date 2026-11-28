use std::ops::Range;

use std::collections::HashSet;

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::tooltip::Position;
use iced::widget::{column, container, responsive, row, scrollable, space, text, text_input};
use iced::{Alignment, Color, Element, Font, Length, alignment};
use porthmos_core::LogLevel;
use porthmos_core::edit::printable;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;

use super::button::{Look, compact_button, hinted, text_button};
use super::services::{session_picker, shows_session_picker};
use super::spacing::{MD, SM};
use super::text_fit::fit;
use super::{filled, hairline_across, label};
use crate::app::{App, Message};
use crate::geometry::{ROW_HEIGHT, visible_range};
use crate::logs_view::{LogStatus, LogTabs, LogsState, SourceKind, StoredLine, TabName, tabs_full_text};
use crate::theme::Palette;

pub const OVERSCAN: usize = 6;
const LINE_CELLS: usize = 400;
const FOOTER_HEIGHT: f32 = 28.0;
const TAB_TITLE_CELLS: usize = 24;

pub fn end_offset(total: usize, viewport: f32) -> f32 {
    (total as f32 * ROW_HEIGHT - viewport).max(0.0)
}

pub fn window(total: usize, offset: f32, viewport: f32, follow: bool) -> Range<usize> {
    let end = end_offset(total, viewport);
    let offset = if follow { end } else { offset.min(end) };
    visible_range(offset, viewport, ROW_HEIGHT, total, OVERSCAN)
}

pub fn line_color(stored: &StoredLine, palette: &Palette) -> Color {
    if stored.skipped.is_some() {
        return palette.fg_dim;
    }
    match stored.line.level {
        LogLevel::Error => palette.err,
        LogLevel::Warn => palette.warn,
        LogLevel::Info | LogLevel::Debug | LogLevel::Unknown => palette.fg,
    }
}

pub fn skipped_text(count: u64) -> String {
    i18n::plural("gui.logs.skipped", usize::try_from(count).unwrap_or(usize::MAX))
}

type Press = (Option<Message>, Option<String>);

pub fn start_press(state: &LogsState) -> Press {
    if state.source().is_some() {
        return (Some(Message::LogsStartRequested), None);
    }
    let reason = match state.kind {
        SourceKind::File => i18n::text("logs.empty_path"),
        SourceKind::Journal => i18n::text("gui.services.unsafe_name"),
    };
    (None, Some(reason))
}

fn not_running() -> Press {
    (None, Some(i18n::text("gui.logs.not_running")))
}

pub fn stop_press(state: &LogsState) -> Press {
    if state.running.is_some() { (Some(Message::LogsStopRequested), None) } else { not_running() }
}

pub fn pause_press(state: &LogsState) -> Press {
    if state.paused() || state.running.is_some() { (Some(Message::LogsPauseToggled), None) } else { not_running() }
}

pub fn clear_press(state: &LogsState) -> Press {
    if state.lines.is_empty() {
        (None, Some(i18n::text("gui.logs.nothing_to_clear")))
    } else {
        (Some(Message::LogsClearRequested), None)
    }
}

pub fn save_press(state: &LogsState) -> Press {
    if state.view_len() > 0 {
        (Some(Message::LogsSaveRequested), None)
    } else {
        (None, Some(i18n::text("gui.logs.save_nothing")))
    }
}

pub fn pause_title(paused: bool) -> String {
    if paused { tr("Resume") } else { tr("Pause") }
}

pub fn follow_text(paused: bool, follow: bool, running: bool, unseen: usize) -> Option<String> {
    if paused {
        return Some(if unseen == 0 { tr("Paused") } else { i18n::plural("gui.logs.paused_new", unseen) });
    }
    (follow && running).then(|| tr("Following"))
}

pub fn empty_text(status: &LogStatus) -> String {
    match status {
        LogStatus::Idle => i18n::text("gui.logs.idle"),
        LogStatus::Starting => i18n::text("gui.logs.starting"),
        LogStatus::Running => i18n::text("gui.logs.waiting"),
        LogStatus::Ended(message) | LogStatus::Unavailable(message) => message.clone(),
    }
}

pub fn empty_view_text(state: &LogsState) -> Option<String> {
    if state.shown_len() == 0 {
        Some(empty_text(&state.status))
    } else if state.view_len() == 0 {
        Some(i18n::text("gui.logs.no_match"))
    } else {
        None
    }
}

pub fn match_count_text(shown: usize, total: usize) -> String {
    i18n::text_with("gui.logs.match_count", &[("shown", &shown.to_string()), ("total", &total.to_string())])
}

pub fn level_title(level: LogLevel) -> String {
    match level {
        LogLevel::Error => i18n::text("gui.logs.level_error"),
        LogLevel::Warn => i18n::text("gui.logs.level_warn"),
        LogLevel::Info => i18n::text("gui.logs.level_info"),
        LogLevel::Debug => i18n::text("gui.logs.level_debug"),
        LogLevel::Unknown => String::new(),
    }
}

pub fn footer_text(status: &LogStatus) -> Option<String> {
    match status {
        LogStatus::Ended(message) | LogStatus::Unavailable(message) => Some(message.clone()),
        LogStatus::Idle | LogStatus::Starting | LogStatus::Running => None,
    }
}

pub fn tab_title(app: &App, index: usize) -> String {
    let tabs = app.log_tabs();
    let Some(tab) = tabs.tabs().get(index) else {
        return String::new();
    };
    let base = match tab.name() {
        TabName::Empty => i18n::text("gui.logs.new_tab"),
        TabName::File(name) => printable(&name),
        TabName::Journal(Some(unit)) => printable(&unit),
        TabName::Journal(None) => i18n::text("gui.logs.journal_title"),
    };
    let running: HashSet<_> = tabs.tabs().iter().filter_map(|tab| tab.running.map(|(_, session)| session)).collect();
    match tab.running.and_then(|(_, session)| app.sessions().by_id(session)) {
        Some(session) if running.len() > 1 => format!("{base} · {}", printable(&session.name)),
        _ => base,
    }
}

pub fn add_press(tabs: &LogTabs) -> Press {
    if tabs.can_add() { (Some(Message::LogsTabAdded), None) } else { (None, Some(tabs_full_text())) }
}

fn tab_strip<'a>(app: &App, palette: &Palette) -> Element<'a, Message> {
    let tabs = app.log_tabs();
    let mut line = row![].spacing(SM).align_y(Alignment::Center);
    for index in 0..tabs.tabs().len() {
        let look = if index == tabs.active_index() { Look::Chosen } else { Look::Normal };
        let title = fit(&tab_title(app, index), TAB_TITLE_CELLS);
        line = line.push(text_button(&title, Some(Message::LogsTabSelected(index)), look, palette));
        let close = compact_button("×", Some(Message::LogsTabClosed(index)), palette);
        line = line.push(hinted(close, i18n::text("gui.logs.close_tab"), Position::Bottom, palette));
    }
    let (press, reason) = add_press(tabs);
    let hint = reason.unwrap_or_else(|| i18n::text("gui.logs.add_tab"));
    line = line.push(hinted(compact_button("+", press, palette), hint, Position::Bottom, palette));
    let strip = scrollable(container(line).padding([SM, MD]))
        .direction(Direction::Horizontal(Scrollbar::default()))
        .style(super::scroll::style(palette));
    strip.width(Length::Fill).into()
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into()
}

fn with_reason<'a>(button: Element<'a, Message>, reason: Option<String>, palette: &Palette) -> Element<'a, Message> {
    match reason {
        Some(reason) => hinted(button, reason, iced::widget::tooltip::Position::Top, palette),
        None => button,
    }
}

fn action<'a>(title: &str, (press, reason): Press, palette: &Palette) -> Element<'a, Message> {
    with_reason(text_button(title, press, Look::Normal, palette), reason, palette)
}

fn kind_choice<'a>(title: String, kind: SourceKind, current: SourceKind, palette: &Palette) -> Element<'a, Message> {
    let look = if kind == current { Look::Chosen } else { Look::Normal };
    text_button(&title, Some(Message::LogsKindChosen(kind)), look, palette)
}

fn source_input<'a>(state: &LogsState, palette: &Palette) -> Element<'a, Message> {
    let input = match state.kind {
        SourceKind::File => text_input(&tr("Path of a log file"), &state.path).on_input(Message::LogsPathChanged),
        SourceKind::Journal => {
            text_input(&tr("Unit (empty for the whole journal)"), &state.unit).on_input(Message::LogsUnitChanged)
        }
    };
    input
        .on_submit(Message::LogsStartRequested)
        .style(super::input::style(palette))
        .size(13.0)
        .font(Font::DEFAULT)
        .width(Length::Fill)
        .into()
}

fn header<'a>(app: &App, palette: &Palette) -> Element<'a, Message> {
    let state = app.logs();
    let mut line = row![].spacing(SM).align_y(Alignment::Center);
    if shows_session_picker(app) {
        let shown = state.running.map(|(_, session)| session).or_else(|| app.health_target());
        line = line.push(session_picker(app, shown, Message::LogsSessionChosen, palette));
    }
    line = line
        .push(kind_choice(tr("File"), SourceKind::File, state.kind, palette))
        .push(kind_choice(tr("Journal"), SourceKind::Journal, state.kind, palette))
        .push(source_input(state, palette))
        .push(action(&tr("Start"), start_press(state), palette))
        .push(action(&tr("Stop"), stop_press(state), palette))
        .push(action(&pause_title(state.paused()), pause_press(state), palette))
        .push(action(&tr("Clear"), clear_press(state), palette))
        .push(action(&i18n::text("gui.logs.save"), save_press(state), palette));
    container(line).padding([SM, MD]).width(Length::Fill).into()
}

fn filter_row<'a>(state: &LogsState, palette: &Palette) -> Element<'a, Message> {
    let input = text_input(&i18n::text("gui.logs.search_hint"), &state.filter.text)
        .on_input(Message::LogsFilterChanged)
        .style(super::input::style(palette))
        .size(13.0)
        .font(Font::DEFAULT)
        .width(Length::Fill);
    let mut line = row![input].spacing(SM).align_y(Alignment::Center);
    for level in [LogLevel::Error, LogLevel::Warn, LogLevel::Info, LogLevel::Debug] {
        let look = if state.filter.level_on(level) { Look::Chosen } else { Look::Normal };
        line = line.push(text_button(&level_title(level), Some(Message::LogsLevelToggled(level)), look, palette));
    }
    if state.filtering() {
        line = line.push(label(match_count_text(state.view_len(), state.shown_len()), palette.fg_dim));
    }
    container(line).padding([SM, MD]).width(Length::Fill).into()
}

fn mono<'a>(content: String, color: Color) -> Element<'a, Message> {
    text(content).size(13.0).font(Font::MONOSPACE).color(color).wrapping(iced::widget::text::Wrapping::None).into()
}

fn line_row<'a>(stored: &StoredLine, palette: &Palette) -> Element<'a, Message> {
    let color = line_color(stored, palette);
    let shown = match stored.skipped {
        Some(count) => skipped_text(count),
        None => fit(&stored.line.text, LINE_CELLS),
    };
    container(mono(shown, color))
        .padding([0.0, SM])
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .into()
}

fn list<'a>(state: &'a LogsState, viewport: f32, palette: &Palette) -> Element<'a, Message> {
    let total = state.view_len();
    let range = window(total, state.scroll, viewport, state.follow);
    let mut content = column![space().height(Length::Fixed(range.start as f32 * ROW_HEIGHT))];
    for stored in state.view_lines(range.clone()) {
        content = content.push(line_row(stored, palette));
    }
    content = content.push(space().height(Length::Fixed(total.saturating_sub(range.end) as f32 * ROW_HEIGHT)));
    scrollable(content)
        .id(App::logs_list_id())
        .style(super::scroll::style(palette))
        .on_scroll(|viewport| Message::LogsScrolled {
            offset: viewport.absolute_offset().y,
            viewport: viewport.bounds().height,
            content: viewport.content_bounds().height,
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn body(app: &App) -> Element<'_, Message> {
    responsive(move |size| {
        let state = app.logs();
        let palette = app.palette();
        if let Some(text) = empty_view_text(state) {
            return centered(label(text, palette.fg_dim));
        }
        list(state, size.height, palette)
    })
    .into()
}

fn footer<'a>(state: &LogsState, palette: &Palette) -> Element<'a, Message> {
    let mut line = row![].spacing(SM).align_y(Alignment::Center);
    if let Some(text) = footer_text(&state.status) {
        let color = if matches!(state.status, LogStatus::Unavailable(_)) { palette.err } else { palette.fg_dim };
        line = line.push(label(text, color));
    }
    line = line.push(space().width(Length::Fill));
    if let Some(text) = follow_text(state.paused(), state.follow, state.running.is_some(), state.unseen_since_pause()) {
        line = line.push(label(text, palette.fg_dim));
    }
    if !state.follow && !state.paused() {
        let follow = text_button(&tr("Follow"), Some(Message::LogsFollowToggled(true)), Look::Normal, palette);
        line = line.push(follow);
    }
    container(line)
        .padding([0.0, MD])
        .width(Length::Fill)
        .height(FOOTER_HEIGHT)
        .align_y(alignment::Vertical::Center)
        .style(filled(palette.bg))
        .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    if !app.logs_shown() {
        return centered(label(tr("Connect to a server first"), palette.fg_dim));
    }
    column![
        tab_strip(app, palette),
        hairline_across(palette),
        header(app, palette),
        hairline_across(palette),
        filter_row(app.logs(), palette),
        hairline_across(palette),
        container(body(app)).width(Length::Fill).height(Length::Fill),
        hairline_across(palette),
        footer(app.logs(), palette),
    ]
    .into()
}

#[cfg(test)]
mod tests;
