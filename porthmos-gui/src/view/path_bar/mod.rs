use iced::widget::{button, container, mouse_area, row, space, text_input};
use iced::{Background, Border, Element, Length, alignment};

use super::button::{Look, flat, tool_button};
use super::{bold_label, filled, label};
use crate::app::{App, Message, PanelSide};
use crate::breadcrumbs::{Crumb, crumbs};
use crate::geometry::{HAIRLINE, PATH_BAR_HEIGHT, ROW_HEIGHT, TAB_HEIGHT, TOOLBAR_HEIGHT, TRANSFER_COLUMN_WIDTH};
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
            .style(flat(&palette, palette.fg, true));
        strip = strip.push(item);
        if !current {
            strip = strip.push(label("›", palette.fg_dim));
        }
    }
    let rest = mouse_area(space().width(Length::Fill).height(Length::Fill)).on_press(Message::PathEditStarted(side));
    let line = row![strip, rest].align_y(alignment::Vertical::Center).height(Length::Fill);
    field_frame(line.into(), &palette, false)
}

fn frame_border(editing: bool, palette: &crate::theme::Palette) -> Border {
    if editing {
        Border { color: palette.accent, width: 1.5, radius: 6.0.into() }
    } else {
        Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() }
    }
}

fn field_frame<'a>(
    content: Element<'a, Message>, palette: &crate::theme::Palette, editing: bool,
) -> Element<'a, Message> {
    let border = frame_border(editing, palette);
    let fill = palette.surface;
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
                .size(13.0)
                .style(super::input::bare_style(palette));
            field_frame(input.into(), palette, true)
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

pub fn more_text(more: usize) -> Option<String> {
    (more > 0).then(|| format!("and {more} more"))
}

const CANDIDATE_INSET: f32 = 118.0;
const CANDIDATE_MIN_WIDTH: f32 = 180.0;
const CANDIDATE_CHAR_WIDTH: f32 = 8.0;
const CANDIDATE_PADDING: f32 = 16.0;

fn panel_left(app: &App, side: PanelSide) -> f32 {
    match side {
        PanelSide::Local => 0.0,
        PanelSide::Remote => {
            let panel_width = (app.window_size().width - TRANSFER_COLUMN_WIDTH - 2.0 * HAIRLINE) / 2.0;
            panel_width + TRANSFER_COLUMN_WIDTH + 2.0 * HAIRLINE
        }
    }
}

pub fn candidates(app: &App) -> Option<Element<'_, Message>> {
    let edit = app.path_edit()?;
    let listing = edit.listing.as_ref()?;
    let palette = app.palette();
    let (shown, more) = listing.shown();
    let widest = shown.iter().map(|name| name.chars().count()).max().unwrap_or(0);
    let width = (widest as f32 * CANDIDATE_CHAR_WIDTH + CANDIDATE_PADDING).max(CANDIDATE_MIN_WIDTH);
    let mut rows = iced::widget::column![];
    for name in shown {
        let entry = button(label(name.clone(), palette.fg))
            .on_press(Message::PathCandidateChosen(name.clone()))
            .padding([0.0, 8.0])
            .height(Length::Fixed(ROW_HEIGHT))
            .width(Length::Fill)
            .style(flat(palette, palette.fg, true));
        rows = rows.push(entry);
    }
    if let Some(text) = more_text(more) {
        rows = rows.push(container(label(text, palette.fg_dim)).padding([0.0, 8.0]).height(Length::Fixed(ROW_HEIGHT)));
    }
    let border = Border { color: palette.line_strong, width: 1.0, radius: 4.0.into() };
    let surface = palette.surface;
    let boxed = container(rows).width(Length::Fixed(width)).padding(2).style(move |_| container::Style {
        background: Some(Background::Color(surface)),
        border,
        ..container::Style::default()
    });
    let top = TAB_HEIGHT + HAIRLINE + TOOLBAR_HEIGHT + HAIRLINE + PATH_BAR_HEIGHT + HAIRLINE;
    let left = panel_left(app, edit.side) + CANDIDATE_INSET;
    Some(
        container(boxed)
            .padding(iced::Padding { top, left, ..iced::Padding::ZERO })
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    )
}

#[cfg(test)]
mod tests;
