use iced::widget::{column, container, row, scrollable, text, text_input};
use iced::{Element, Font, Length, alignment};
use porthmos_core::edit::printable;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::properties::text::{contents_line, progress_line, size_line, skipped_line};
use porthmos_core::size_text;

use super::{body_text, button, primary_button};
use crate::app::Message;
use crate::modal::properties::{FileHash, HashState, MeasurePane, PropertiesPane};
use crate::theme::Palette;
use crate::view::button::{Look, text_button};
use crate::view::text_fit::fit;
use crate::view::{label, wrapped_label};

pub const WIDTH: f32 = 640.0;
const LABEL_WIDTH: f32 = 84.0;
const NAME_CELLS: usize = 60;
const FILES_HEIGHT: f32 = 220.0;
const DIGEST_SIZE: f32 = 12.0;
fn error_label() -> String {
    tr("Error")
}

fn one_line(message: &str) -> String {
    printable(&message.replace(":\n", ": ").replace('\n', " "))
}

pub fn measure_rows(pane: &PropertiesPane) -> Vec<(String, String)> {
    match &pane.measure {
        MeasurePane::Running(measured) => {
            vec![(tr("Size"), size_line(measured.bytes)), (tr("Progress"), progress_line(measured))]
        }
        MeasurePane::Done(measured) => {
            let mut rows = vec![(tr("Size"), size_line(measured.bytes)), (tr("Contents"), contents_line(measured))];
            rows.extend(skipped_line(measured).map(|skipped| (tr("Skipped"), skipped)));
            rows
        }
        MeasurePane::Failed(message) => vec![(error_label(), one_line(message))],
        MeasurePane::Cancelled => vec![(tr("Size"), i18n::text("props.cancelled"))],
    }
}

pub fn file_name(file: &FileHash) -> String {
    fit(&printable(&file.name), NAME_CELLS)
}

pub fn file_status(file: &FileHash) -> String {
    match &file.state {
        HashState::Idle => i18n::text("props.not_computed"),
        HashState::Running { done, total: 0 } => i18n::text_with("props.reading", &[("done", &size_text(*done))]),
        HashState::Running { done, total } => {
            i18n::text_with("props.progress", &[("done", &size_text(*done)), ("total", &size_text(*total))])
        }
        HashState::Failed(message) => one_line(message),
        HashState::Cancelled => i18n::text("props.cancelled"),
        HashState::Done(digest) => match file.listed_total {
            Some(total) if total != digest.bytes => i18n::text_with(
                "gui.props.bytes_read_listing",
                &[("count", &digest.bytes.to_string()), ("total", &total.to_string())],
            ),
            _ => String::new(),
        },
    }
}

pub fn verdict_line(pane: &PropertiesPane) -> Option<(String, bool)> {
    pane.verdict().map(|matched| (i18n::text(if matched { "props.matches" } else { "props.no_match" }), matched))
}

pub fn close_label(pane: &PropertiesPane) -> String {
    tr(if pane.running() { "Cancel" } else { "Close" })
}

fn measure_view<'a>(pane: &PropertiesPane, palette: &Palette) -> Element<'a, Message> {
    let mut rows = column![].spacing(4);
    for (name, value) in measure_rows(pane) {
        let color = if name == error_label() { palette.err } else { palette.fg };
        rows = rows.push(
            row![label(name, palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)), wrapped_label(value, color)].spacing(8),
        );
    }
    rows.into()
}

fn file_view<'a>(index: usize, file: &FileHash, pane: &PropertiesPane, palette: &Palette) -> Element<'a, Message> {
    let status = file_status(file);
    let status_color = if matches!(file.state, HashState::Failed(_)) { palette.err } else { palette.fg_dim };
    let mut head = row![label(file_name(file), palette.fg)].spacing(8);
    if !status.is_empty() {
        head = head.push(label(status, status_color));
    }
    let mut block = column![head].spacing(2);
    if let Some(hex) = pane.digest_text(index) {
        let digest = text(hex)
            .size(DIGEST_SIZE)
            .font(Font::MONOSPACE)
            .color(palette.fg_bright)
            .width(Length::Fill)
            .wrapping(text::Wrapping::WordOrGlyph);
        block = block.push(
            row![digest, button(&tr("Copy"), Message::PropertiesCopyDigest(index), palette)]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
        );
    }
    block.into()
}

fn expected_view<'a>(pane: &PropertiesPane, palette: &Palette) -> Element<'a, Message> {
    let input = text_input(&tr("Paste the expected SHA-256 to compare"), &pane.expected)
        .style(crate::view::input::style(palette))
        .on_input(Message::PropertiesExpectedTyped)
        .size(13.0)
        .font(Font::MONOSPACE);
    let mut block = column![input].spacing(4);
    if let Some((word, matched)) = verdict_line(pane) {
        block = block.push(label(word, if matched { palette.ok } else { palette.err }));
    }
    block.into()
}

pub fn body<'a>(pane: &PropertiesPane, palette: &Palette) -> Element<'a, Message> {
    let mut content = column![measure_view(pane, palette)].spacing(12);
    if pane.files.is_empty() {
        return content.into();
    }
    content = content.push(label(tr("SHA-256"), palette.fg_dim));
    let files = column(pane.files.iter().enumerate().map(|(index, file)| file_view(index, file, pane, palette)))
        .spacing(8)
        .width(Length::Fill);
    let scrolling = scrollable(files).style(crate::view::scroll::style(palette)).height(Length::Shrink);
    content = content.push(container(scrolling).max_height(FILES_HEIGHT));
    if pane.has_expected_field() {
        content = content.push(expected_view(pane, palette));
    }
    if matches!(pane.measure, MeasurePane::Failed(_)) {
        content = content.push(body_text(String::new(), palette));
    }
    content.into()
}

pub fn buttons<'a>(pane: &PropertiesPane, palette: &Palette) -> Element<'a, Message> {
    let close = primary_button(&close_label(pane), Message::ModalDeclined, palette);
    if pane.files.is_empty() {
        return close;
    }
    let press = pane.can_compute().then_some(Message::PropertiesCompute);
    row![text_button(&tr("Calculate SHA-256"), press, Look::Normal, palette), close].spacing(8).into()
}

#[cfg(test)]
mod tests;
