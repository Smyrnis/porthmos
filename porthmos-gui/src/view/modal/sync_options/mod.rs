use iced::widget::{column, row};
use iced::{Element, Length};
use porthmos_core::sync::{SyncBy, SyncDirection};

use super::super::button::{Look, text_button};
use super::super::label;
use crate::app::{Message, SyncChoice};
use crate::theme::Palette;

const LABEL_WIDTH: f32 = 110.0;

fn choice<'a>(title: &str, chosen: bool, picked: SyncChoice, palette: &Palette) -> Element<'a, Message> {
    let look = if chosen { Look::Primary } else { Look::Normal };
    text_button(title, Some(Message::SyncOptionChosen(picked)), look, palette)
}

fn line<'a>(name: &str, choices: Element<'a, Message>, palette: &Palette) -> Element<'a, Message> {
    row![label(name.to_string(), palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)), choices]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Top)
        .into()
}

fn beside<'a>(choices: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    row(choices).spacing(6).into()
}

fn stacked<'a>(choices: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    column(choices).spacing(6).into()
}

fn direction_titles(offer_both: bool) -> Vec<(&'static str, SyncDirection)> {
    let mut titles = vec![
        ("Local \u{2192} Remote", SyncDirection::LocalToRemote),
        ("Remote \u{2192} Local", SyncDirection::RemoteToLocal),
    ];
    if offer_both {
        titles.push(("Both (the newer file wins)", SyncDirection::Both));
    }
    titles
}

pub fn body<'a>(
    direction: SyncDirection, by: SyncBy, subfolders: bool, offer_both: bool, error: &Option<String>, palette: &Palette,
) -> Element<'a, Message> {
    let directions: Vec<_> = direction_titles(offer_both)
        .into_iter()
        .map(|(title, value)| choice(title, direction == value, SyncChoice::Direction(value), palette))
        .collect();
    let comparisons = vec![
        choice("Modification time", by == SyncBy::Time, SyncChoice::By(SyncBy::Time), palette),
        choice("Size", by == SyncBy::Size, SyncChoice::By(SyncBy::Size), palette),
    ];
    let folders = vec![
        choice("Included", subfolders, SyncChoice::Subfolders(true), palette),
        choice("Not included", !subfolders, SyncChoice::Subfolders(false), palette),
    ];
    let mut content = column![
        line("Direction", stacked(directions), palette),
        line("Compare by", beside(comparisons), palette),
        line("Subfolders", beside(folders), palette),
    ]
    .spacing(10);
    if !offer_both {
        content =
            content.push(label("Both isn't offered: this connection can't keep modification times", palette.fg_dim));
    }
    if let Some(error) = error {
        content = content.push(label(error.clone(), palette.err));
    }
    content.into()
}

#[cfg(test)]
mod tests;
