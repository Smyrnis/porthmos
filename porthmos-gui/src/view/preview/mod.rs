use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::text::Wrapping;
use iced::widget::{column, container, row, scrollable, space, text};
use iced::{Border, Element, Font, Length};

use super::button::{Look, text_button};
use super::tools::preview_list_height;
use super::{bold_label, hairline_across, label};
use crate::app::{App, Message};
use crate::preview_view::{PreviewState, PreviewStatus, visible_window};
use crate::theme::Palette;

pub const ROW_HEIGHT: f32 = 18.0;
const FONT_SIZE: f32 = 13.0;
const CHARACTER_WIDTH: f32 = 8.0;
const SIDE_PADDING: f32 = 8.0;

fn code_row<'a>(line: String, palette: &Palette) -> Element<'a, Message> {
    let line = text(line).size(FONT_SIZE).font(Font::MONOSPACE).color(palette.fg).wrapping(Wrapping::None);
    container(line).height(Length::Fixed(ROW_HEIGHT)).padding([0.0, SIDE_PADDING]).into()
}

fn rows_view<'a>(app: &'a App, state: &'a PreviewState, palette: &Palette) -> Element<'a, Message> {
    let viewport = preview_list_height(app.window_size());
    let total = state.row_count();
    let (first, count) = visible_window(app.tool_scroll(), viewport, total, ROW_HEIGHT);
    let mut lines = column![space().height(Length::Fixed(first as f32 * ROW_HEIGHT))];
    for line in state.rows(first, count) {
        lines = lines.push(code_row(line, palette));
    }
    let below = total.saturating_sub(first + count);
    lines = lines.push(space().height(Length::Fixed(below as f32 * ROW_HEIGHT)));
    let width = state.columns() as f32 * CHARACTER_WIDTH + 2.0 * SIDE_PADDING;
    let content = container(lines).width(Length::Fixed(width));
    scrollable(content)
        .id(App::tool_list_id())
        .direction(Direction::Both { vertical: Scrollbar::default(), horizontal: Scrollbar::default() })
        .on_scroll(|viewport| Message::PreviewScrolled(viewport.absolute_offset().y))
        .width(Length::Fill)
        .height(Length::Fixed(viewport))
        .into()
}

fn notice<'a>(text: String, color: iced::Color, height: f32) -> Element<'a, Message> {
    container(label(text, color)).padding(12).width(Length::Fill).height(Length::Fixed(height)).into()
}

pub fn body<'a>(app: &'a App, state: &'a PreviewState) -> Element<'a, Message> {
    let palette = app.palette();
    let viewport = preview_list_height(app.window_size());
    let content: Element<'a, Message> = match state.status() {
        PreviewStatus::Loading => notice("Loading…".to_string(), palette.fg_dim, viewport),
        PreviewStatus::Failed(message) => notice(message.clone(), palette.err, viewport),
        PreviewStatus::Ready(_) => rows_view(app, state, palette),
    };
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    let boxed = container(content).style(move |_| container::Style { border, ..container::Style::default() });
    column![
        row![
            bold_label(state.title(), palette.fg_bright),
            space().width(Length::Fill),
            label(state.info_text(), palette.fg_dim)
        ],
        boxed,
        hairline_across(palette),
        row![space().width(Length::Fill), text_button("Close", Some(Message::ToolClosed), Look::Primary, palette)],
    ]
    .spacing(10)
    .into()
}

#[cfg(test)]
mod tests;
