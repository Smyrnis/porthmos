use iced::widget::{button, column, container, mouse_area, row, space, stack};
use iced::{Background, Border, Color, Element, Length, Padding, Point, Size, alignment};

use super::button::flat;
use super::{filled, label};
use crate::app::{App, Message};
use crate::menus::context_items;

const MENU_WIDTH: f32 = 240.0;
const ITEM_HEIGHT: f32 = 28.0;
const MENU_PADDING: f32 = 4.0;

pub fn menu_size(items: usize) -> Size {
    Size::new(MENU_WIDTH, items as f32 * ITEM_HEIGHT + 2.0 * MENU_PADDING)
}

pub fn clamp_menu(at: Point, menu: Size, window: Size) -> Point {
    let limit_x = (window.width - menu.width).max(0.0);
    let limit_y = (window.height - menu.height).max(0.0);
    Point::new(at.x.clamp(0.0, limit_x), at.y.clamp(0.0, limit_y))
}

fn item<'a>(app: &App, entry: (crate::menus::MenuItem, bool)) -> Element<'a, Message> {
    let palette = *app.palette();
    let (menu_item, enabled) = entry;
    let color = if enabled { palette.fg_bright } else { palette.fg_dim };
    let keys = app.shortcuts().label_for(menu_item.action()).unwrap_or_default();
    let line = row![label(menu_item.title(), color).width(Length::Fill), label(keys, palette.fg_dim)]
        .align_y(alignment::Vertical::Center);
    button(line)
        .padding([0.0, 10.0])
        .width(Length::Fixed(MENU_WIDTH - 2.0 * MENU_PADDING))
        .height(Length::Fixed(ITEM_HEIGHT))
        .on_press_maybe(enabled.then_some(Message::MenuItemChosen(menu_item)))
        .style(flat(color, enabled))
        .into()
}

pub fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let menu = app.context_menu()?;
    let palette = *app.palette();
    let items = context_items(&menu.target, app.has_other_panel(menu.side));
    let size = menu_size(items.len());
    let at = clamp_menu(menu.at, size, app.window_size());
    let mut list = column![];
    for entry in items {
        list = list.push(item(app, entry));
    }
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let surface = palette.surface;
    let panel = container(list).padding(MENU_PADDING).width(Length::Fixed(size.width)).style(move |_| {
        container::Style { background: Some(Background::Color(surface)), border, ..container::Style::default() }
    });
    let backdrop =
        mouse_area(container(space()).width(Length::Fill).height(Length::Fill).style(filled(Color::TRANSPARENT)))
            .on_press(Message::ContextMenuDismissed)
            .on_right_press(Message::ContextMenuDismissed);
    let placed = container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding { left: at.x, top: at.y, ..Padding::ZERO })
        .align_x(alignment::Horizontal::Left)
        .align_y(alignment::Vertical::Top);
    Some(stack![backdrop, placed].into())
}

#[cfg(test)]
mod tests;
