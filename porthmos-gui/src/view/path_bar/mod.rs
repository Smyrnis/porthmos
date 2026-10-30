use iced::widget::{button, container, mouse_area, row, space, text_input};
use iced::{Background, Border, Color, Element, Length, alignment};

use super::button::{Look, flat, tool_button};
use super::{bold_label, filled, label};
use crate::app::{App, Message, PanelSide};
use crate::breadcrumbs::{Crumb, crumbs};
use crate::geometry::PATH_BAR_HEIGHT;
use crate::icons::Icon;
use crate::panel::Panel;

const CRUMB_PADDING: usize = 3;
const CHAR_WIDTH: f32 = 7.5;
const CONTROLS_WIDTH: f32 = 150.0;

pub fn visible_crumbs(all: &[Crumb], max_chars: usize) -> (bool, &[Crumb]) {
    let mut used = 0;
    let mut first = all.len();
    for (index, crumb) in all.iter().enumerate().rev() {
        used += crumb.label.chars().count() + CRUMB_PADDING;
        if used > max_chars && index + 1 != all.len() {
            break;
        }
        first = index;
    }
    (first > 0, &all[first..])
}

fn crumb_strip<'a>(app: &'a App, side: PanelSide, panel: &Panel, width: f32) -> Element<'a, Message> {
    let palette = *app.palette();
    let all = crumbs(panel.path());
    let room = ((width - CONTROLS_WIDTH).max(0.0) / CHAR_WIDTH) as usize;
    let (clipped, shown) = visible_crumbs(&all, room);
    let last = shown.len().saturating_sub(1);
    let mut strip = row![].spacing(2).align_y(alignment::Vertical::Center);
    if clipped {
        strip = strip.push(label("…", palette.fg_dim));
    }
    for (index, crumb) in shown.iter().enumerate() {
        let current = index == last;
        let title = if current {
            bold_label(crumb.label.clone(), palette.fg_bright)
        } else {
            label(crumb.label.clone(), palette.fg_muted)
        };
        let target = crumb.path.clone();
        let item = button(title)
            .padding([2.0, 4.0])
            .on_press(Message::CrumbPressed(side, target))
            .style(flat(palette.fg, true));
        strip = strip.push(item);
        if !current {
            strip = strip.push(label("›", palette.fg_dim));
        }
    }
    let rest = mouse_area(space().width(Length::Fill).height(Length::Fill)).on_press(Message::PathEditStarted(side));
    let line = row![strip, rest].align_y(alignment::Vertical::Center).height(Length::Fill);
    field_frame(line.into(), &palette)
}

fn field_frame<'a>(content: Element<'a, Message>, palette: &crate::theme::Palette) -> Element<'a, Message> {
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let fill = Color::from_rgb8(0x05, 0x05, 0x05);
    container(content)
        .padding([0.0, 6.0])
        .width(Length::Fill)
        .height(Length::Fixed(PATH_BAR_HEIGHT - 12.0))
        .align_y(alignment::Vertical::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border,
            ..container::Style::default()
        })
        .into()
}

pub fn bar<'a>(app: &'a App, side: PanelSide, panel: &'a Panel, width: f32) -> Element<'a, Message> {
    let palette = app.palette();
    let navigation = |icon: Icon, title: &str, enabled: bool, message: Message| {
        tool_button(Some(icon), title, false, title.to_string(), enabled.then_some(message), Look::Normal, palette)
    };
    let field: Element<'a, Message> = match app.path_edit().filter(|edit| edit.side == side) {
        Some(edit) => {
            let input = text_input("", &edit.text)
                .id(side.path_input_id())
                .on_input(Message::PathTyped)
                .on_submit(Message::PathSubmitted)
                .size(13.0);
            field_frame(input.into(), palette)
        }
        None => crumb_strip(app, side, panel, width),
    };
    container(
        row![
            navigation(Icon::Back, "Back", app.can_go_back(side), Message::NavigateBack(side)),
            navigation(Icon::Forward, "Forward", app.can_go_forward(side), Message::NavigateForward(side)),
            navigation(Icon::Up, "Up", app.can_go_up(side), Message::NavigateUp(side)),
            field,
        ]
        .spacing(4)
        .align_y(alignment::Vertical::Center),
    )
    .padding([0.0, 8.0])
    .width(Length::Fill)
    .height(Length::Fixed(PATH_BAR_HEIGHT))
    .align_y(alignment::Vertical::Center)
    .style(filled(palette.bg))
    .into()
}

#[cfg(test)]
mod tests;
