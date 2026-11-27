use iced::widget::{column, container, mouse_area, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Color, Element, Font, Length, alignment};
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{
    ServiceVerb, ServicesSnapshot, ServicesState as ServicesEvent, SessionId, StatusFilter, Unit, UnitName,
};

use super::button::{Look, hinted, hinted_if, text_button};
use super::spacing::{MD, SM};
use super::text_fit::{fit, full_text_hint};
use super::{bold_label, filled, hairline_across, label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::services_view::{
    ALL_VERBS, ServicesState, active_text, boot_text, row_color, shown_units, status_blocked, verb_blocked,
};
use crate::theme::Palette;

const PICKER_WIDTH: f32 = 220.0;
const NAME_WIDTH: f32 = 280.0;
const STATE_WIDTH: f32 = 112.0;
const NAME_CELLS: usize = 36;
const DESCRIPTION_CELLS: usize = 200;
const STATUS_PANEL_HEIGHT: f32 = 168.0;

pub fn shows_session_picker(app: &App) -> bool {
    app.health_session_names().len() >= 2
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

fn head<'a>(palette: &Palette) -> Element<'a, Message> {
    let dim = palette.fg_dim;
    container(
        row![
            cell(tr("Name"), NAME_WIDTH, dim),
            cell(tr("Active"), STATE_WIDTH, dim),
            cell(tr("Boot"), STATE_WIDTH, dim),
            wide_cell(tr("Description"), dim),
        ]
        .spacing(SM),
    )
    .padding([0.0, MD + SM])
    .width(Length::Fill)
    .into()
}

pub fn name_tooltip(name: &str) -> Option<String> {
    full_text_hint(name, NAME_CELLS)
}

fn name_cell<'a>(name: &str, color: Color, palette: &Palette) -> Element<'a, Message> {
    let cell = cell(fit(name, NAME_CELLS), NAME_WIDTH, color);
    hinted_if(cell, name_tooltip(name), iced::widget::tooltip::Position::Top, palette)
}

fn unit_row<'a>(unit: &Unit, selected: bool, dimmed: bool, palette: &Palette) -> Element<'a, Message> {
    let color = row_color(unit, selected, dimmed, palette);
    let background = if selected { palette.row_focus } else { palette.bg };
    let line = row![
        name_cell(&unit.name, color, palette),
        cell(active_text(unit.active), STATE_WIDTH, color),
        cell(boot_text(unit.boot), STATE_WIDTH, color),
        wide_cell(fit(&unit.description, DESCRIPTION_CELLS), color),
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
    .on_press(Message::ServiceSelected(unit.name.clone()))
    .into()
}

fn nothing<'a>(palette: &Palette) -> Element<'a, Message> {
    container(label(i18n::text("gui.services.nothing"), palette.fg_dim)).padding(MD).into()
}

fn table<'a>(
    state: &ServicesState, snapshot: &ServicesSnapshot, dimmed: bool, palette: &Palette,
) -> Element<'a, Message> {
    let units = shown_units(state, snapshot);
    let body: Element<Message> = if units.is_empty() {
        nothing(palette)
    } else {
        column(units.into_iter().map(|unit| {
            let selected = state.selected.as_deref() == Some(unit.name.as_str());
            unit_row(unit, selected, dimmed, palette)
        }))
        .width(Length::Fill)
        .into()
    };
    column![
        head(palette),
        scrollable(container(body).padding([0.0, MD]).width(Length::Fill))
            .style(super::scroll::style(palette))
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn verb_title(verb: ServiceVerb) -> String {
    match verb {
        ServiceVerb::Start => tr("Start"),
        ServiceVerb::Stop => tr("Stop"),
        ServiceVerb::Restart => tr("Restart"),
        ServiceVerb::Enable => tr("Enable"),
        ServiceVerb::Disable => tr("Disable"),
    }
}

pub fn verb_press(
    state: &ServicesState, snapshot: &ServicesSnapshot, verb: ServiceVerb,
) -> (Option<Message>, Option<String>) {
    let reason = verb_blocked(state, snapshot, verb);
    (reason.is_none().then_some(Message::ServiceActionRequested(verb)), reason)
}

pub fn status_press(state: &ServicesState, snapshot: &ServicesSnapshot) -> (Option<Message>, Option<String>) {
    let reason = status_blocked(state, snapshot);
    (reason.is_none().then_some(Message::ServiceStatusRequested), reason)
}

pub fn logs_press(state: &ServicesState) -> (Option<Message>, Option<String>) {
    match state.selected.as_deref() {
        None => (None, Some(i18n::text("gui.services.select_first"))),
        Some(name) if UnitName::new(name).is_some() => (Some(Message::ServiceLogsRequested), None),
        Some(_) => (None, Some(i18n::text("gui.services.unsafe_name"))),
    }
}

fn logs_button<'a>(state: &ServicesState, palette: &Palette) -> Element<'a, Message> {
    let (press, reason) = logs_press(state);
    with_reason(text_button(&tr("View logs"), press, Look::Normal, palette), reason, palette)
}

fn verb_button<'a>(
    state: &ServicesState, snapshot: &ServicesSnapshot, verb: ServiceVerb, palette: &Palette,
) -> Element<'a, Message> {
    let (press, reason) = verb_press(state, snapshot, verb);
    with_reason(text_button(&verb_title(verb), press, Look::Normal, palette), reason, palette)
}

fn status_button<'a>(state: &ServicesState, snapshot: &ServicesSnapshot, palette: &Palette) -> Element<'a, Message> {
    let (press, reason) = status_press(state, snapshot);
    with_reason(text_button(&tr("Status"), press, Look::Normal, palette), reason, palette)
}

fn with_reason<'a>(button: Element<'a, Message>, reason: Option<String>, palette: &Palette) -> Element<'a, Message> {
    match reason {
        Some(reason) => hinted(button, reason, iced::widget::tooltip::Position::Top, palette),
        None => button,
    }
}

fn status_panel<'a>(state: &ServicesState, palette: &Palette) -> Option<Element<'a, Message>> {
    let (unit, content) = state.status_text.as_ref()?;
    (state.selected.as_ref() == Some(unit)).then(|| {
        let shown = text(content.clone()).size(13.0).font(Font::MONOSPACE).color(palette.fg).width(Length::Fill);
        scrollable(container(shown).padding([SM, MD]).width(Length::Fill))
            .style(super::scroll::style(palette))
            .width(Length::Fill)
            .height(STATUS_PANEL_HEIGHT)
            .into()
    })
}

fn detail<'a>(state: &ServicesState, snapshot: &ServicesSnapshot, palette: &Palette) -> Option<Element<'a, Message>> {
    let selected = state.selected.as_deref()?;
    let unit = snapshot.units.iter().find(|unit| unit.name == selected)?;
    let mut buttons = row![status_button(state, snapshot, palette), logs_button(state, palette)].spacing(SM);
    for verb in ALL_VERBS {
        buttons = buttons.push(verb_button(state, snapshot, verb, palette));
    }
    let line = row![
        container(bold_label(fit(&unit.name, NAME_CELLS), palette.fg_bright)).clip(true),
        container(iced::widget::space()).width(Length::Fill),
        buttons,
    ]
    .spacing(MD)
    .align_y(Alignment::Center);
    let mut page = column![hairline_across(palette), container(line).padding([SM, MD]).width(Length::Fill)];
    if let Some(panel) = status_panel(state, palette) {
        page = page.push(hairline_across(palette)).push(panel);
    }
    Some(page.width(Length::Fill).into())
}

fn failing_bar<'a>(message: String, palette: &Palette) -> Element<'a, Message> {
    let notice = hinted(
        label(i18n::text("gui.services.updating_failed"), palette.err).into(),
        message,
        iced::widget::tooltip::Position::Bottom,
        palette,
    );
    let line = row![notice, text_button(&tr("Retry"), Some(Message::ServicesRetry), Look::Primary, palette)]
        .spacing(SM)
        .align_y(Alignment::Center);
    container(line).padding([SM, MD]).width(Length::Fill).into()
}

fn snapshot_page<'a>(
    state: &ServicesState, snapshot: &ServicesSnapshot, failure: Option<String>, palette: &Palette,
) -> Element<'a, Message> {
    let mut page = column![];
    if let Some(message) = failure {
        page = page.push(failing_bar(message, palette));
    }
    page = page.push(table(state, snapshot, state.failing(), palette));
    if let Some(detail) = detail(state, snapshot, palette) {
        page = page.push(detail);
    }
    page.width(Length::Fill).height(Length::Fill).into()
}

fn body<'a>(app: &'a App, palette: &Palette) -> Element<'a, Message> {
    let state = app.services();
    match &state.state {
        None => centered(label(i18n::text("gui.services.loading"), palette.fg_dim)),
        Some(ServicesEvent::Unavailable(message)) => centered(label(message.to_string(), palette.fg_dim)),
        Some(ServicesEvent::Snapshot(snapshot)) => snapshot_page(state, snapshot, None, palette),
        Some(ServicesEvent::Failing(message)) => match state.snapshot() {
            Some(snapshot) => snapshot_page(state, snapshot, Some(message.to_string()), palette),
            None => centered(
                column![
                    label(message.to_string(), palette.err),
                    text_button(&tr("Retry"), Some(Message::ServicesRetry), Look::Primary, palette),
                ]
                .spacing(MD)
                .align_x(Alignment::Center),
            ),
        },
    }
}

fn status_choice<'a>(
    title: String, status: StatusFilter, current: StatusFilter, palette: &Palette,
) -> Element<'a, Message> {
    let look = if status == current { Look::Chosen } else { Look::Normal };
    text_button(&title, Some(Message::ServicesStatusFilterChanged(status)), look, palette)
}

pub(super) fn picker<'a>(app: &App, palette: &Palette) -> Element<'a, Message> {
    session_picker(app, app.health_target(), Message::HealthSessionChosen, palette)
}

pub(super) fn session_picker<'a>(
    app: &App, shown: Option<SessionId>, chosen: fn(String) -> Message, palette: &Palette,
) -> Element<'a, Message> {
    let current = shown.and_then(|id| app.sessions().by_id(id)).map(|session| session.name.clone());
    pick_list(app.health_session_names(), current, chosen)
        .text_size(13.0)
        .padding([6.0, 8.0])
        .style(super::dropdown::style(palette))
        .menu_style(super::dropdown::menu_style(palette))
        .width(Length::Fixed(PICKER_WIDTH))
        .into()
}

fn header<'a>(app: &App, palette: &Palette) -> Element<'a, Message> {
    let state = app.services();
    let mut line = row![].spacing(SM).align_y(Alignment::Center);
    if shows_session_picker(app) {
        line = line.push(picker(app, palette));
    }
    line = line
        .push(status_choice(tr("All"), StatusFilter::All, state.status, palette))
        .push(status_choice(tr("Running"), StatusFilter::Running, state.status, palette))
        .push(status_choice(tr("Failed"), StatusFilter::Failed, state.status, palette))
        .push(
            text_input(&tr("Filter services"), &state.filter)
                .style(super::input::style(palette))
                .on_input(Message::ServicesFilterChanged)
                .size(13.0)
                .font(Font::DEFAULT)
                .width(Length::Fill),
        );
    let refresh = text_button(&tr("Refresh"), state.started.map(|_| Message::ServicesRefresh), Look::Normal, palette);
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
