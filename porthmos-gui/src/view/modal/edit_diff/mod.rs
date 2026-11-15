use iced::widget::{column, container, scrollable};
use iced::{Color, Element, Font, Length};
use porthmos_core::diff::{DiffKind, DiffOutcome};
use porthmos_core::edit::printable;

use crate::app::Message;
use crate::theme::{Palette, mix};
use crate::view::{filled, wrapped_label};

const ROW_TEXT_SIZE: f32 = 12.0;
const BODY_HEIGHT: f32 = 340.0;
const TINT: f32 = 0.18;
const ROW_PADDING: [u16; 2] = [1, 6];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub kind: Option<DiffKind>,
}

pub fn rows(outcome: &DiffOutcome) -> Vec<Row> {
    match outcome {
        DiffOutcome::Changes(lines) => {
            lines.iter().map(|line| Row { text: line.marked_text(), kind: Some(line.kind) }).collect()
        }
        other => other.note().map(|text| Row { text, kind: None }).into_iter().collect(),
    }
}

pub fn row_background(kind: Option<DiffKind>, palette: &Palette) -> Option<Color> {
    match kind? {
        DiffKind::Removed => Some(mix(palette.surface, palette.err, TINT)),
        DiffKind::Added => Some(mix(palette.surface, palette.ok, TINT)),
        DiffKind::Context | DiffKind::Skipped(_) => None,
    }
}

pub fn row_color(kind: Option<DiffKind>, palette: &Palette) -> Color {
    match kind {
        Some(DiffKind::Removed) => palette.err,
        Some(DiffKind::Added) => palette.ok,
        Some(DiffKind::Skipped(_)) => palette.fg_dim,
        Some(DiffKind::Context) | None => palette.fg,
    }
}

pub fn failed_text(message: &str) -> String {
    printable(&message.replace('\n', " "))
}

fn row_view<'a>(row: Row, palette: &Palette) -> Element<'a, Message> {
    let text = iced::widget::text(row.text)
        .size(ROW_TEXT_SIZE)
        .font(Font::MONOSPACE)
        .color(row_color(row.kind, palette))
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph);
    let background = row_background(row.kind, palette).unwrap_or(Color::TRANSPARENT);
    container(text).width(Length::Fill).padding(ROW_PADDING).style(filled(background)).into()
}

pub fn body<'a>(outcome: &DiffOutcome, palette: &Palette) -> Element<'a, Message> {
    if let DiffOutcome::Changes(_) = outcome {
        let list = column(rows(outcome).into_iter().map(|row| row_view(row, palette)));
        let scrolling = scrollable(list).style(crate::view::scroll::style(palette)).height(Length::Shrink);
        return container(scrolling).max_height(BODY_HEIGHT).into();
    }
    match rows(outcome).into_iter().next() {
        Some(row) => wrapped_label(row.text, palette.fg).into(),
        None => column![].into(),
    }
}

#[cfg(test)]
mod tests;
