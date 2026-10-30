use iced::widget::{column, container, mouse_area, row, scrollable, space, text_input};
use iced::{Alignment, Border, Element, Font, Length, alignment};

use super::tools::palette_list_height;
use super::{filled, label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::palette::{Entry, PaletteState};
use crate::theme::Palette;

const HINT_MARGIN: f32 = 20.0;
const NO_MATCHES: &str = "No matches.";
pub fn more_hint(shown: usize, total: usize) -> Option<String> {
    (total > shown).then(|| format!("Showing the first {shown} of {total} matches. Keep typing to narrow them."))
}

const PLACEHOLDER: &str = "Type a command, @host or #file";

fn entry_row<'a>(palette: &Palette, index: usize, entry: &Entry, selected: bool) -> Element<'a, Message> {
    let name_color = if selected { palette.fg_focus } else { palette.fg };
    let accent = if selected { palette.accent } else { iced::Color::TRANSPARENT };
    let hint = entry.hint.clone().unwrap_or_default();
    let line = row![
        container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(accent)),
        space().width(Length::Fixed(8.0)),
        container(label(entry.label.clone(), name_color)).width(Length::FillPortion(3)).clip(true),
        container(label(hint, palette.fg_dim))
            .width(Length::FillPortion(1))
            .clip(true)
            .align_x(alignment::Horizontal::Right),
        space().width(Length::Fixed(HINT_MARGIN)),
    ]
    .align_y(Alignment::Center);
    let background = if selected { palette.row_focus } else { iced::Color::TRANSPARENT };
    let line =
        container(line).width(Length::Fill).height(Length::Fixed(ROW_HEIGHT)).clip(true).style(filled(background));
    mouse_area(line)
        .on_press(Message::PaletteRowPressed(index))
        .on_double_click(Message::PaletteRowActivated(index))
        .into()
}

pub fn body<'a>(app: &'a App, state: &'a PaletteState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input(PLACEHOLDER, state.query())
        .id(App::palette_input_id())
        .on_input(Message::PaletteTyped)
        .size(13.0)
        .font(Font::DEFAULT);
    let mut rows = column![];
    for (index, entry) in state.hits().into_iter().enumerate() {
        rows = rows.push(entry_row(palette, index, entry, state.selected() == index));
    }
    if state.hit_count() == 0 {
        rows = rows.push(container(label(NO_MATCHES, palette.fg_dim)).padding(12));
    }
    if let Some(hint) = more_hint(state.hit_count(), state.total_matches()) {
        rows = rows.push(container(label(hint, palette.fg_dim)).padding(12));
    }
    let list = scrollable(rows)
        .id(App::tool_list_id())
        .width(Length::Fill)
        .height(Length::Fixed(palette_list_height(app.window_size())));
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    let boxed = container(list).style(move |_| container::Style { border, ..container::Style::default() });
    column![input, boxed].spacing(10).into()
}

#[cfg(test)]
mod tests;
