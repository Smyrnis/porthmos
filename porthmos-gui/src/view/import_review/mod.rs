use iced::widget::{checkbox, column, container, row, scrollable, space, tooltip};
use iced::{Alignment, Color, Element, Length};

use crate::app::Message;
use crate::geometry::ROW_HEIGHT;
use crate::import_view::{ImportView, ImportViewRow};
use crate::preview_view::visible_window;
use crate::theme::Palette;
use crate::view::button::{Look, hinted, text_button};
use crate::view::text_fit::fit;
use crate::view::{hairline_across, label};

pub const WIDTH: f32 = 860.0;
pub const HEIGHT: f32 = 470.0;
const TABLE_HEIGHT: f32 = 264.0;
const TICK_WIDTH: f32 = 28.0;
const NAME_WIDTH: f32 = 170.0;
const SUMMARY_WIDTH: f32 = 250.0;
const GROUP_WIDTH: f32 = 110.0;
const GAP: f32 = 10.0;
const NAME_CELLS: usize = 24;
const SUMMARY_CELLS: usize = 36;
const GROUP_CELLS: usize = 14;
const DETAIL_CELLS: usize = 60;

fn button<'a>(title: &str, message: Message, palette: &Palette) -> Element<'a, Message> {
    text_button(title, Some(message), Look::Normal, palette)
}

pub struct RowTexts {
    pub name: String,
    pub summary: String,
    pub group: String,
    pub detail: String,
}

pub fn run_label(view: &ImportView) -> String {
    format!("Import {}", view.ticked_rows().len())
}

pub fn run_enabled(view: &ImportView) -> bool {
    !view.ticked_rows().is_empty()
}

pub fn rendered_range(scroll: f32, total: usize) -> (usize, usize) {
    visible_window(scroll, TABLE_HEIGHT, total, ROW_HEIGHT)
}

pub fn row_texts(row: &ImportViewRow) -> RowTexts {
    let mut details: Vec<&str> = Vec::new();
    if !row.verdict_text.is_empty() {
        details.push(&row.verdict_text);
    }
    details.extend(row.notes.iter().map(String::as_str));
    RowTexts {
        name: fit(&row.name, NAME_CELLS),
        summary: fit(&row.summary, SUMMARY_CELLS),
        group: fit(&row.group, GROUP_CELLS),
        detail: fit(&details.join("; "), DETAIL_CELLS),
    }
}

pub fn name_hint(row: &ImportViewRow) -> Option<String> {
    (fit(&row.name, NAME_CELLS) != row.name).then(|| row.name.clone())
}

fn name_cell<'a>(row: &ImportViewRow, color: Color, palette: &Palette) -> Element<'a, Message> {
    let shown = cell(row_texts(row).name, Length::Fixed(NAME_WIDTH), color);
    match name_hint(row) {
        Some(full) => hinted(shown, full, tooltip::Position::Bottom, palette),
        None => shown,
    }
}

fn cell<'a>(content: String, width: Length, color: Color) -> Element<'a, Message> {
    container(label(content, color)).width(width).clip(true).into()
}

fn tick<'a>(index: usize, row: &ImportViewRow, palette: &Palette) -> Element<'a, Message> {
    let content: Element<'a, Message> = if row.tickable {
        checkbox(row.ticked)
            .on_toggle(move |_| Message::ImportRowToggled(index))
            .style(crate::view::checkbox::style(palette))
            .into()
    } else {
        space().into()
    };
    container(content).width(Length::Fixed(TICK_WIDTH)).into()
}

fn table_row<'a>(index: usize, row: &ImportViewRow, palette: &Palette) -> Element<'a, Message> {
    let texts = row_texts(row);
    let name_color = if row.tickable { palette.fg } else { palette.fg_dim };
    let detail_color = if row.invalid { palette.err } else { palette.fg_dim };
    container(
        iced::widget::row![
            tick(index, row, palette),
            name_cell(row, name_color, palette),
            cell(texts.summary, Length::Fixed(SUMMARY_WIDTH), palette.fg_muted),
            cell(texts.group, Length::Fixed(GROUP_WIDTH), palette.fg_dim),
            cell(texts.detail, Length::Fill, detail_color),
        ]
        .spacing(GAP)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ROW_HEIGHT))
    .clip(true)
    .into()
}

fn header<'a>(palette: &Palette) -> Element<'a, Message> {
    row![
        space().width(Length::Fixed(TICK_WIDTH)),
        cell("Name".to_string(), Length::Fixed(NAME_WIDTH), palette.fg_dim),
        cell("Connection".to_string(), Length::Fixed(SUMMARY_WIDTH), palette.fg_dim),
        cell("Group".to_string(), Length::Fixed(GROUP_WIDTH), palette.fg_dim),
        cell("Note".to_string(), Length::Fill, palette.fg_dim),
    ]
    .spacing(GAP)
    .into()
}

fn rows_view<'a>(view: &ImportView, palette: &Palette) -> Element<'a, Message> {
    let total = view.rows().len();
    let (first, count) = rendered_range(view.scroll(), total);
    let mut lines = column![space().height(Length::Fixed(first as f32 * ROW_HEIGHT))];
    for index in first..first + count {
        if let Some(row) = view.rows().get(index) {
            lines = lines.push(table_row(index, row, palette));
        }
    }
    let below = total.saturating_sub(first + count);
    lines = lines.push(space().height(Length::Fixed(below as f32 * ROW_HEIGHT)));
    scrollable(lines)
        .style(crate::view::scroll::style(palette))
        .on_scroll(|viewport| Message::ImportScrolled(viewport.absolute_offset().y))
        .width(Length::Fill)
        .height(Length::Fixed(TABLE_HEIGHT))
        .into()
}

pub fn body<'a>(view: &ImportView, palette: &Palette) -> Element<'a, Message> {
    column![
        label(view.summary(), palette.fg_muted),
        header(palette),
        hairline_across(palette),
        rows_view(view, palette),
    ]
    .spacing(6)
    .into()
}

pub fn buttons<'a>(view: &ImportView, palette: &Palette) -> Element<'a, Message> {
    let run = text_button(
        &run_label(view),
        run_enabled(view).then_some(Message::ImportRunPressed),
        if run_enabled(view) { Look::Primary } else { Look::Disabled },
        palette,
    );
    row![
        button("Tick all new", Message::ImportTickNew, palette),
        button("Untick all", Message::ImportUntickAll, palette),
        space().width(Length::Fill),
        button("Cancel", Message::ImportCancelled, palette),
        run,
    ]
    .spacing(8)
    .into()
}

#[cfg(test)]
mod tests;
