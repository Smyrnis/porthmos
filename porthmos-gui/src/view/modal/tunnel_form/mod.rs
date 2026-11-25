use iced::widget::{checkbox, column, pick_list, row, text_input};
use iced::{Element, Font, Length, alignment};
use porthmos_core::config::tunnels::spec_error_text;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;

use super::super::button::{Look, text_button};
use super::super::{label, wrapped_label};
use crate::app::{App, Message};
use crate::theme::Palette;
use crate::tunnel_form::{DraftChange, DraftError, DraftKind, Field, TunnelDraft};

const LABEL_WIDTH: f32 = 110.0;
const PORT_WIDTH: f32 = 90.0;
const GAP: f32 = 8.0;
const FIELD_GAP: f32 = 6.0;

pub fn title(draft: &TunnelDraft) -> String {
    if draft.original.is_some() { tr("Edit tunnel") } else { tr("New tunnel") }
}

pub fn accept_title(draft: &TunnelDraft) -> String {
    if draft.original.is_some() { tr("Save") } else { tr("Create") }
}

pub fn problem<'a>(errors: &[(Field, DraftError)], field: Field, palette: &Palette) -> Option<Element<'a, Message>> {
    let (_, error) = errors.iter().find(|(candidate, _)| *candidate == field)?;
    Some(label(spec_error_text(*error), palette.err).into())
}

fn input<'a>(
    placeholder: &str, value: &str, change: fn(String) -> DraftChange, palette: &Palette,
) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .style(crate::view::input::style(palette))
        .on_input(move |typed| Message::TunnelDraftChanged(change(typed)))
        .on_submit(Message::ModalAccepted)
        .size(13.0)
        .font(Font::DEFAULT)
}

fn line<'a>(name: String, body: Element<'a, Message>, palette: &Palette) -> Element<'a, Message> {
    row![label(name, palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)), body]
        .spacing(GAP)
        .align_y(alignment::Vertical::Center)
        .into()
}

fn with_problems<'a>(
    body: Element<'a, Message>, errors: &[(Field, DraftError)], fields: &[Field], palette: &Palette,
) -> Element<'a, Message> {
    let mut stack = column![body].spacing(FIELD_GAP);
    for field in fields {
        if let Some(problem) = problem(errors, *field, palette) {
            stack = stack.push(problem);
        }
    }
    stack.width(Length::Fill).into()
}

fn connection_line<'a>(
    draft: &TunnelDraft, connections: &[String], errors: &[(Field, DraftError)], palette: &Palette,
) -> Element<'a, Message> {
    let body: Element<Message> = if draft.original.is_some() {
        label(draft.connection.clone(), palette.fg_bright).into()
    } else {
        pick_list(connections.to_vec(), Some(draft.connection.clone()), |chosen| {
            Message::TunnelDraftChanged(DraftChange::Connection(chosen))
        })
        .text_size(13.0)
        .padding([6.0, 8.0])
        .style(crate::view::dropdown::style(palette))
        .menu_style(crate::view::dropdown::menu_style(palette))
        .width(Length::Fill)
        .into()
    };
    line(tr("Connection"), with_problems(body, errors, &[Field::Connection], palette), palette)
}

fn kind_line<'a>(draft: &TunnelDraft, palette: &Palette) -> Element<'a, Message> {
    let choice = |title: String, kind: DraftKind| {
        let look = if draft.kind == kind { Look::Chosen } else { Look::Normal };
        text_button(&title, Some(Message::TunnelDraftChanged(DraftChange::Kind(kind))), look, palette)
    };
    let choices = row![
        choice(tr("Local forward (-L)"), DraftKind::Local),
        choice(tr("Remote forward (-R)"), DraftKind::Remote),
        choice(tr("SOCKS proxy (-D)"), DraftKind::Dynamic),
    ]
    .spacing(FIELD_GAP);
    line(tr("Type"), choices.into(), palette)
}

pub fn listen_label(kind: DraftKind) -> String {
    if kind == DraftKind::Remote { tr("Server listens on") } else { tr("Listen on") }
}

pub fn target_label(kind: DraftKind) -> String {
    if kind == DraftKind::Remote { tr("Local target") } else { tr("Forward to") }
}

pub fn external_warning(kind: DraftKind) -> String {
    i18n::text(if kind == DraftKind::Remote {
        "gui.tunnels.remote_external_warning"
    } else {
        "gui.tunnels.external_warning"
    })
}

fn address_line<'a>(
    name: String, host: (&str, fn(String) -> DraftChange, &str), port: (&str, fn(String) -> DraftChange),
    errors: &[(Field, DraftError)], fields: [Field; 2], palette: &Palette,
) -> Element<'a, Message> {
    let inputs = row![
        input(host.2, host.0, host.1, palette).width(Length::Fill),
        input(&tr("Port"), port.0, port.1, palette).width(Length::Fixed(PORT_WIDTH)),
    ]
    .spacing(GAP);
    line(name, with_problems(inputs.into(), errors, &fields, palette), palette)
}

pub fn body<'a>(
    draft: &TunnelDraft, connections: &[String], errors: &[(Field, DraftError)], palette: &Palette,
) -> Element<'a, Message> {
    let name = input(&tr("Name"), &draft.name, DraftChange::Name, palette).id(App::dialog_input_id());
    let mut form = column![
        connection_line(draft, connections, errors, palette),
        line(tr("Name"), with_problems(name.width(Length::Fill).into(), errors, &[Field::Name], palette), palette),
        kind_line(draft, palette),
        address_line(
            listen_label(draft.kind),
            (&draft.listen_host, DraftChange::ListenHost, "127.0.0.1"),
            (&draft.listen_port, DraftChange::ListenPort),
            errors,
            [Field::ListenHost, Field::ListenPort],
            palette,
        ),
    ]
    .spacing(GAP);
    if draft.kind != DraftKind::Dynamic {
        form = form.push(address_line(
            target_label(draft.kind),
            (&draft.target_host, DraftChange::TargetHost, "db.internal"),
            (&draft.target_port, DraftChange::TargetPort),
            errors,
            [Field::TargetHost, Field::TargetPort],
            palette,
        ));
    }
    if draft.needs_external_warning() {
        form = form.push(
            checkbox(draft.allow_external)
                .label(tr("Allow other computers to connect"))
                .text_size(13.0)
                .on_toggle(|on| Message::TunnelDraftChanged(DraftChange::AllowExternal(on)))
                .style(crate::view::checkbox::style(palette)),
        );
        form = form.push(wrapped_label(external_warning(draft.kind), palette.paused));
    }
    form.push(
        checkbox(draft.auto_start)
            .label(tr("Start when the connection connects"))
            .text_size(13.0)
            .on_toggle(|on| Message::TunnelDraftChanged(DraftChange::AutoStart(on)))
            .style(crate::view::checkbox::style(palette)),
    )
    .into()
}

#[cfg(test)]
mod tests;
