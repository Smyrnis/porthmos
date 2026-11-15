use iced::widget::{column, row, text_input};
use iced::{Alignment, Element, Font, Length};
use porthmos_core::i18n::label as tr;
use porthmos_core::import::ImportSource;

use crate::app::{App, Message};
use crate::theme::Palette;
use crate::view::button::{Look, text_button};
use crate::view::{label, wrapped_label};

fn button<'a>(title: &str, message: Message, palette: &Palette) -> Element<'a, Message> {
    text_button(title, Some(message), Look::Normal, palette)
}

pub const WIDTH: f32 = 560.0;
pub const HEIGHT: f32 = 250.0;

pub fn cancel_message() -> Message {
    Message::ScreenCancelled
}

pub fn source_title(source: ImportSource) -> &'static str {
    source.label()
}

pub fn read_enabled(waiting: Option<u64>) -> bool {
    waiting.is_none()
}

fn source_choice<'a>(candidate: ImportSource, chosen: ImportSource, palette: &Palette) -> Element<'a, Message> {
    let look = if candidate == chosen { Look::Chosen } else { Look::Normal };
    text_button(source_title(candidate), Some(Message::ImportSourceChosen(candidate)), look, palette)
}

pub fn browse_labels(source: ImportSource) -> Vec<(String, bool)> {
    match source {
        ImportSource::Putty => vec![(tr("File\u{2026}"), false), (tr("Folder\u{2026}"), true)],
        ImportSource::FileZilla | ImportSource::WinScp => vec![(tr("Browse\u{2026}"), false)],
    }
}

fn browse_buttons<'a>(source: ImportSource, waiting: Option<u64>, palette: &Palette) -> Element<'a, Message> {
    row(browse_labels(source).into_iter().map(|(title, folder)| {
        let look = if read_enabled(waiting) { Look::Normal } else { Look::Disabled };
        text_button(&title, read_enabled(waiting).then_some(Message::ImportBrowsePressed(folder)), look, palette)
    }))
    .spacing(6)
    .into()
}

pub fn body<'a>(
    source: ImportSource, path: &str, error: &Option<String>, waiting: Option<u64>, palette: &Palette,
) -> Element<'a, Message> {
    let sources =
        row(ImportSource::ALL.into_iter().map(|candidate| source_choice(candidate, source, palette))).spacing(6);
    let mut input = text_input(&tr("Path of the file or folder"), path)
        .style(crate::view::input::style(palette))
        .id(App::import_input_id())
        .size(13.0)
        .font(Font::DEFAULT);
    if read_enabled(waiting) {
        input = input.on_input(Message::ImportPathTyped).on_submit(Message::ImportReadPressed);
    }
    let browse = browse_buttons(source, waiting, palette);
    let path_line = row![input.width(Length::Fill), browse].spacing(8).align_y(Alignment::Center);
    let mut content = column![sources, path_line, wrapped_label(source.hint(), palette.fg_dim)].spacing(8);
    if waiting.is_some() {
        content = content.push(label(tr("Reading\u{2026}"), palette.fg_dim));
    }
    if let Some(error) = error {
        content = content.push(wrapped_label(error.clone(), palette.err));
    }
    content.into()
}

pub fn buttons<'a>(waiting: Option<u64>, palette: &Palette) -> Element<'a, Message> {
    let read = text_button(
        &tr("Read"),
        read_enabled(waiting).then_some(Message::ImportReadPressed),
        if read_enabled(waiting) { Look::Primary } else { Look::Disabled },
        palette,
    );
    row![read, button(&tr("Cancel"), cancel_message(), palette)].spacing(8).into()
}

#[cfg(test)]
mod tests;
