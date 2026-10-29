use iced::widget::{button, column, container, mouse_area, row, space, svg};
use iced::{Alignment, Color, Element, Length};

use super::button::flat;
use super::{bold_label, filled, label};
use crate::app::{App, Message};
use crate::geometry::TAB_HEIGHT;
use crate::icons::{self, Icon};

const ACCENT_EDGE: f32 = 2.0;
const CLOSE_SIZE: f32 = 14.0;

fn close_button<'a>(index: usize) -> Element<'a, Message> {
    button(svg(icons::handle(Icon::Close)).width(CLOSE_SIZE).height(CLOSE_SIZE))
        .padding(2)
        .on_press(Message::TabClosed(index))
        .style(flat(Color::WHITE, true))
        .into()
}

pub fn strip(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let local_active = app.sessions().is_empty() || app.active_side() == crate::app::PanelSide::Local;
    let local_color = if local_active { palette.fg_bright } else { palette.fg_muted };
    let mut tabs = row![container(label("Local", local_color)).padding([6.0, 14.0])].align_y(Alignment::Center);
    for (index, tab) in app.sessions().names().into_iter().enumerate() {
        let (title, color) = match (tab.healthy, tab.active) {
            (false, _) => (format!("! {}", tab.name), palette.err),
            (true, true) => (tab.name, palette.fg_bright),
            (true, false) => (tab.name, palette.fg_muted),
        };
        let text = if tab.active { bold_label(title, color) } else { label(title, color) };
        let edge = if tab.active { palette.accent } else { palette.bg };
        let body = row![
            container(text).padding(iced::Padding { top: 6.0, right: 4.0, bottom: 6.0, left: 12.0 }),
            close_button(index)
        ]
        .spacing(2)
        .align_y(Alignment::Center);
        let tab = column![
            container(space()).width(Length::Fill).height(Length::Fixed(ACCENT_EDGE)).style(filled(edge)),
            container(body).padding(iced::Padding { right: 6.0, ..iced::Padding::ZERO }),
        ];
        tabs = tabs.push(mouse_area(tab).on_press(Message::TabClicked(index)));
    }
    tabs = tabs.push(space().width(Length::Fill)).push(super::connections::button(palette));
    container(tabs)
        .width(Length::Fill)
        .height(Length::Fixed(TAB_HEIGHT))
        .align_y(iced::alignment::Vertical::Center)
        .style(filled(palette.surface))
        .into()
}
