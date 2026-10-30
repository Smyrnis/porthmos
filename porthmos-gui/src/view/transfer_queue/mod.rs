use iced::widget::{column, container, responsive, row, scrollable, space, text_input};
use iced::{Alignment, Element, Font, Length, alignment};
use porthmos_core::transfer::Direction;
use porthmos_core::transfer::rows::{QueueRow, RowState, percent_of};

use super::button::compact_button;
use super::file_list::{PADDING, line_cells};
use super::text_fit::fit;
use super::{bold_label, filled, hairline_across, label};
use crate::app::{App, Message};
use crate::theme::Palette;
use crate::transfers::{
    QUEUE_HEADER_HEIGHT, QUEUE_HEIGHT, QUEUE_ROW_HEIGHT, failure_detail, failure_text, format_speed_limit,
    queue_window, row_buttons, state_word,
};

const BAR_WIDTH: f32 = 110.0;
const BAR_HEIGHT: f32 = 6.0;
const GLYPH_WIDTH: f32 = 22.0;
const COUNT_WIDTH: f32 = 70.0;
const PERCENT_WIDTH: f32 = 52.0;
const STATE_WIDTH: f32 = 180.0;
const BUTTONS_WIDTH: f32 = 260.0;
const SPEED_FIELD_WIDTH: f32 = 84.0;
const FIXED_WIDTH: f32 = GLYPH_WIDTH + COUNT_WIDTH + BAR_WIDTH + PERCENT_WIDTH + STATE_WIDTH + BUTTONS_WIDTH;

pub fn fill_width(total: f32, percent: u8) -> f32 {
    total * f32::from(percent.min(100)) / 100.0
}

pub fn direction_glyph(direction: Direction) -> &'static str {
    match direction {
        Direction::Upload => "↑",
        Direction::Download => "↓",
    }
}

pub fn row_text(row: &QueueRow) -> String {
    let same_file = row.failure.as_ref().is_some_and(|failure| failure.file == row.label);
    let failure = if same_file { failure_detail(row) } else { failure_text(row) };
    match failure {
        Some(failure) => format!("{}  ·  {failure}", row.label),
        None => row.label.clone(),
    }
}

pub fn percent_text(row: &QueueRow) -> String {
    match row.state {
        RowState::Scanning => String::new(),
        _ => format!("{}%", percent_of(row.bytes_done, row.bytes_total)),
    }
}

fn state_color(row: &QueueRow, palette: &Palette) -> iced::Color {
    match row.state {
        RowState::Done => palette.ok,
        RowState::Failed | RowState::PartlyFailed(_) => palette.err,
        _ => palette.fg_muted,
    }
}

fn button<'a>(title: &str, message: Message, palette: &Palette) -> Element<'a, Message> {
    compact_button(title, Some(message), palette)
}

fn bar<'a>(row: &QueueRow, palette: &Palette) -> Element<'a, Message> {
    let percent = percent_of(row.bytes_done, row.bytes_total);
    let fill = container(space())
        .width(Length::Fixed(fill_width(BAR_WIDTH, percent)))
        .height(Length::Fixed(BAR_HEIGHT))
        .style(filled(palette.accent));
    container(fill)
        .width(Length::Fixed(BAR_WIDTH))
        .height(Length::Fixed(BAR_HEIGHT))
        .style(filled(palette.line_strong))
        .into()
}

fn row_view<'a>(row: &QueueRow, cells: usize, palette: &Palette) -> Element<'a, Message> {
    let text_color = match row.state {
        RowState::Failed | RowState::PartlyFailed(_) => palette.err,
        RowState::Done | RowState::Cancelled => palette.fg_muted,
        _ => palette.fg,
    };
    let mut buttons = row![].spacing(2);
    for action in row_buttons(row) {
        buttons = buttons.push(button(action.title(), Message::QueueRowPressed(row.kind, action), palette));
    }
    let line = row![
        label(direction_glyph(row.direction), palette.fg_dim).width(Length::Fixed(GLYPH_WIDTH)),
        label(fit(&row_text(row), cells), text_color).width(Length::Fill),
        label(format!("{}/{}", row.files_done, row.files_total), palette.fg_muted)
            .width(Length::Fixed(COUNT_WIDTH))
            .align_x(alignment::Horizontal::Right),
        container(bar(row, palette)).padding([0.0, 8.0]),
        label(percent_text(row), palette.fg_muted)
            .width(Length::Fixed(PERCENT_WIDTH))
            .align_x(alignment::Horizontal::Right),
        label(format!("  {}", state_word(row)), state_color(row, palette)).width(Length::Fixed(STATE_WIDTH)),
        container(buttons).width(Length::Fixed(BUTTONS_WIDTH)),
    ]
    .align_y(Alignment::Center);
    container(line)
        .padding([0.0, PADDING])
        .width(Length::Fill)
        .height(Length::Fixed(QUEUE_ROW_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .style(filled(palette.bg))
        .into()
}

fn header<'a>(app: &'a App, palette: &Palette) -> Element<'a, Message> {
    let queue = app.queue();
    let shown = match queue.speed_text() {
        Some(text) => text.to_string(),
        None => format_speed_limit(app.transfer_speed_limit()),
    };
    let field = text_input("off", &shown)
        .on_input(Message::SpeedLimitTyped)
        .on_submit(Message::SpeedLimitSubmitted)
        .size(12.0)
        .font(Font::DEFAULT)
        .width(Length::Fixed(SPEED_FIELD_WIDTH));
    let mut line = row![
        bold_label("Transfers", palette.fg_bright),
        label(format!("  {}", app.transfer_summary().unwrap_or_default()), palette.accent),
        space().width(Length::Fill),
        label("Speed limit ", palette.fg_dim),
        field,
    ]
    .align_y(Alignment::Center);
    if let Some(error) = queue.speed_error() {
        line = line.push(label(format!("  {}", fit(error, 60)), palette.err));
    }
    line = line
        .push(space().width(Length::Fixed(16.0)))
        .push(button("Clear finished", Message::QueueClearFinished, palette))
        .push(button("Cancel all", Message::QueueCancelAll, palette))
        .push(button("Close", Message::ActionRequested(crate::shortcuts::Action::ToggleQueue), palette));
    container(line)
        .padding([0.0, PADDING])
        .width(Length::Fill)
        .height(Length::Fixed(QUEUE_HEADER_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .style(filled(palette.bg))
        .into()
}

fn list<'a>(app: &'a App, cells: usize) -> Element<'a, Message> {
    let palette = app.palette();
    let rows = app.transfers().rows();
    if rows.is_empty() {
        return container(label("No transfers", palette.fg_dim))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .into();
    }
    let range = queue_window(rows.len(), app.queue().scroll());
    let mut content = column![space().height(Length::Fixed(range.start as f32 * QUEUE_ROW_HEIGHT))];
    for row in &rows[range.clone()] {
        content = content.push(row_view(row, cells, palette));
    }
    content = content.push(space().height(Length::Fixed((rows.len() - range.end) as f32 * QUEUE_ROW_HEIGHT)));
    scrollable(content)
        .on_scroll(|viewport| Message::QueueScrolled(viewport.absolute_offset().y))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn pane(app: &App) -> Element<'_, Message> {
    responsive(move |size| {
        let palette = app.palette();
        let cells = line_cells((size.width - FIXED_WIDTH).max(0.0));
        column![header(app, palette), hairline_across(palette), list(app, cells)].into()
    })
    .height(Length::Fixed(QUEUE_HEIGHT))
    .into()
}

#[cfg(test)]
mod tests;
