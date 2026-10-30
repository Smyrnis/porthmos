use iced::widget::{column, container};
use iced::{Element, Length, alignment};

use super::button::{Look, tool_button};
use super::filled;
use crate::app::{App, Message, PanelSide};
use crate::geometry::TRANSFER_COLUMN_WIDTH;
use crate::icons::Icon;

pub fn arrow_states(app: &App) -> (bool, bool) {
    (app.transfer_possible(PanelSide::Local), app.transfer_possible(PanelSide::Remote))
}

pub fn bar<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    let (to_right, to_left) = arrow_states(app);
    let arrow = |icon: Icon, hint: &str, enabled: bool, from: PanelSide| {
        let press = enabled.then_some(Message::TransferArrowPressed(from));
        tool_button(Some(icon), hint, false, hint.to_string(), press, Look::Normal, palette)
    };
    container(
        column![
            arrow(Icon::Forward, "Copy to the right panel", to_right, PanelSide::Local),
            arrow(Icon::Back, "Copy to the left panel", to_left, PanelSide::Remote),
        ]
        .spacing(8)
        .align_x(alignment::Horizontal::Center),
    )
    .width(Length::Fixed(TRANSFER_COLUMN_WIDTH))
    .height(Length::Fill)
    .center_x(Length::Fixed(TRANSFER_COLUMN_WIDTH))
    .center_y(Length::Fill)
    .style(filled(palette.surface))
    .into()
}

#[cfg(test)]
mod tests;
