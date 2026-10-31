use chrono::Local;
use iced::widget::{checkbox, column, row};
use iced::{Element, Length, alignment};
use porthmos_core::transfer::conflicts::Resolution;
use porthmos_core::transfer::plan::ExistingFile;

use super::{body_text, button};
use crate::app::Message;
use crate::conflicts::ConflictView;
use crate::theme::Palette;
use crate::view::{format, label};

const COLUMN_WIDTH: f32 = 190.0;
const LABEL_WIDTH: f32 = 80.0;
const NONE: &str = "—";

pub fn existing_text(file: Option<ExistingFile>) -> (String, String) {
    match file {
        Some(file) => {
            let modified = match format::modified(file.modified, &Local) {
                text if text.is_empty() => NONE.to_string(),
                text => text,
            };
            (format::size(file.size, false), modified)
        }
        None => (NONE.to_string(), NONE.to_string()),
    }
}

pub fn new_text(view: &ConflictView) -> (String, String) {
    let modified = match format::modified(view.new_modified, &Local) {
        text if text.is_empty() => NONE.to_string(),
        text => text,
    };
    let modified = match view.freshness() {
        Some(word) => format!("{modified} ({word})"),
        None => modified,
    };
    (format::size(view.new_size, false), modified)
}

pub fn partial_text(view: &ConflictView) -> Option<String> {
    view.partial.map(|partial| {
        let size = match partial.size {
            bytes if bytes < 1024 => format!("{bytes} bytes"),
            bytes => format::size(bytes, false),
        };
        format!("A partial copy of {size} is on the destination; Resume continues it.")
    })
}

pub fn title(view: &ConflictView) -> &'static str {
    if view.existing.is_some() { "File already exists" } else { "File was partly copied before" }
}

pub fn apply_text() -> &'static str {
    "Apply to the rest of this batch"
}

fn line<'a>(title: &str, existing: String, new: String, palette: &Palette) -> Element<'a, Message> {
    row![
        label(title.to_string(), palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)),
        label(existing, palette.fg).width(Length::Fixed(COLUMN_WIDTH)),
        label(new, palette.fg_bright).width(Length::Fixed(COLUMN_WIDTH)),
    ]
    .into()
}

pub fn body<'a>(view: &ConflictView, apply_to_rest: bool, palette: &Palette) -> Element<'a, Message> {
    let (existing_size, existing_modified) = existing_text(view.existing);
    let (new_size, new_modified) = new_text(view);
    let mut content = column![
        body_text(format!("{}  ({} of {})", view.file_name, view.index + 1, view.total), palette),
        row![
            label("", palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)),
            label("Existing", palette.fg_dim).width(Length::Fixed(COLUMN_WIDTH)),
            label("New", palette.fg_dim).width(Length::Fixed(COLUMN_WIDTH)),
        ],
        line("Size", existing_size, new_size, palette),
        line("Modified", existing_modified, new_modified, palette),
    ]
    .spacing(4);
    if let Some(partial) = partial_text(view) {
        content = content.push(label(partial, palette.fg_muted));
    }
    let toggle = checkbox(apply_to_rest)
        .label(apply_text())
        .on_toggle(|_| Message::ConflictApplyToRestToggled)
        .size(16)
        .text_size(13);
    content.push(toggle).spacing(8).into()
}

pub fn buttons<'a>(view: &ConflictView, palette: &Palette) -> Element<'a, Message> {
    let mut line = row![
        button("Overwrite", Message::ConflictAnswered(Some(Resolution::Overwrite)), palette),
        button("Skip", Message::ConflictAnswered(Some(Resolution::Skip)), palette),
        button("Rename", Message::ConflictAnswered(Some(Resolution::Rename)), palette),
    ]
    .spacing(8)
    .align_y(alignment::Vertical::Center);
    if view.can_resume() {
        line = line.push(button("Resume", Message::ConflictAnswered(Some(Resolution::Resume)), palette));
    }
    line.push(button("Cancel batch", Message::ConflictAnswered(None), palette)).into()
}

#[cfg(test)]
mod tests;
