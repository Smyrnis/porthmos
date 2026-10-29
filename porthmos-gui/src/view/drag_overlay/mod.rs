use iced::widget::container;
use iced::{Border, Element, Length, Padding};

use super::file_list::CELL_WIDTH;
use super::label;
use crate::app::{App, Message};

const OFFSET: f32 = 14.0;
const CHIP_PADDING: f32 = 24.0;
const CHIP_HEIGHT: f32 = 28.0;

pub fn hint(app: &App) -> Option<Element<'_, Message>> {
    let hint = app.drag_hint()?;
    let at = app.hint_anchor()?;
    let palette = app.palette();
    let color = if hint.refused { palette.err } else { palette.fg_bright };
    let border = Border { color: palette.line_strong, width: 1.0, ..Border::default() };
    let surface = palette.surface;
    let chip = container(label(format!("{} {}", hint.glyph, hint.text), color)).padding([3.0, 8.0]).style(move |_| {
        container::Style { background: Some(iced::Background::Color(surface)), border, ..container::Style::default() }
    });
    let chip_width = (hint.glyph.chars().count() + 1 + hint.text.chars().count()) as f32 * CELL_WIDTH + CHIP_PADDING;
    let left = (at.x + OFFSET).min(app.window_size().width - chip_width).max(0.0);
    let top = (at.y + OFFSET).min(app.window_size().height - CHIP_HEIGHT).max(0.0);
    Some(
        container(chip)
            .padding(Padding { top, left, right: 0.0, bottom: 0.0 })
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    )
}
