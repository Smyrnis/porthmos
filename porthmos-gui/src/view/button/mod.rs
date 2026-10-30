use std::time::Duration;

use iced::widget::button::{self, Status};
use iced::widget::{button as button_widget, container, row, svg, text, tooltip};
use iced::{Background, Border, Color, Element, Font, Length, Padding, alignment};

use crate::app::Message;
use crate::icons::{self, Icon};
use crate::theme::Palette;

const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
const ICON_SIZE: f32 = 16.0;
const DISABLED_OPACITY: f32 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Normal,
    Primary,
    Disabled,
}

fn grey(level: u8) -> Color {
    Color::from_rgb8(level, level, level)
}

pub fn face(palette: &Palette, look: Look, status: Status) -> button::Style {
    let fill = match (look, status) {
        (Look::Disabled, _) | (_, Status::Active | Status::Disabled) => grey(0x11),
        (_, Status::Hovered) => grey(0x1c),
        (_, Status::Pressed) => grey(0x26),
    };
    let (text_color, border_color) = match look {
        Look::Normal => (palette.fg_bright, palette.line_strong),
        Look::Primary => (palette.accent, palette.accent),
        Look::Disabled => (palette.fg_dim, palette.line_strong),
    };
    button::Style {
        background: Some(Background::Color(fill)),
        text_color,
        border: Border { color: border_color, width: 1.0, radius: 6.0.into() },
        ..button::Style::default()
    }
}

fn content<'a>(icon: Option<Icon>, title: &str, show_title: bool, look: Look) -> Element<'a, Message> {
    let mut line = row![].spacing(6).align_y(alignment::Vertical::Center);
    if let Some(icon) = icon {
        let opacity = if look == Look::Disabled { DISABLED_OPACITY } else { 1.0 };
        line = line.push(svg(icons::handle(icon)).width(ICON_SIZE).height(ICON_SIZE).opacity(opacity));
    }
    if show_title || icon.is_none() {
        line = line.push(text(title.to_string()).size(13.0).font(Font::DEFAULT));
    }
    line.into()
}

fn pressable<'a>(
    body: Element<'a, Message>, padding: Padding, press: Option<Message>, look: Look, palette: &Palette,
) -> Element<'a, Message> {
    let palette = *palette;
    button_widget(body)
        .padding(padding)
        .on_press_maybe(press)
        .style(move |_, status| face(&palette, look, status))
        .into()
}

pub fn flat(text_color: Color, hoverable: bool) -> impl Fn(&iced::Theme, Status) -> button::Style {
    move |_, status| {
        let hovered = hoverable && matches!(status, Status::Hovered | Status::Pressed);
        let fill = if hovered { grey(0x1c) } else { Color::TRANSPARENT };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color,
            border: Border { radius: 4.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    }
}

pub fn tool_button<'a>(
    icon: Option<Icon>, title: &str, show_title: bool, hint: String, press: Option<Message>, look: Look,
    palette: &Palette,
) -> Element<'a, Message> {
    let look = if press.is_none() { Look::Disabled } else { look };
    let padding = if show_title || icon.is_none() { Padding::from([5.0, 10.0]) } else { Padding::from([5.0, 8.0]) };
    let body = content(icon, title, show_title, look);
    let button = pressable(body, padding, press, look, palette);
    hinted(button, hint, tooltip::Position::Bottom, palette)
}

pub fn hinted<'a>(
    body: Element<'a, Message>, hint: String, position: tooltip::Position, palette: &Palette,
) -> Element<'a, Message> {
    let surface = palette.surface;
    let border = Border { color: palette.line_strong, width: 1.0, radius: 4.0.into() };
    let tip = container(text(hint).size(12.0).color(palette.fg_bright)).padding([4.0, 8.0]).style(move |_| {
        container::Style { background: Some(Background::Color(surface)), border, ..container::Style::default() }
    });
    tooltip(body, tip, position).delay(TOOLTIP_DELAY).into()
}

pub fn text_button<'a>(title: &str, press: Option<Message>, look: Look, palette: &Palette) -> Element<'a, Message> {
    let look = if press.is_none() { Look::Disabled } else { look };
    let body = container(content(None, title, true, look)).width(Length::Shrink);
    pressable(body.into(), Padding::from([5.0, 16.0]), press, look, palette)
}

pub fn compact_button<'a>(title: &str, press: Option<Message>, palette: &Palette) -> Element<'a, Message> {
    let look = if press.is_none() { Look::Disabled } else { Look::Normal };
    let body = content(None, title, true, look);
    pressable(body, Padding::from([1.0, 8.0]), press, look, palette)
}

#[cfg(test)]
mod tests;
