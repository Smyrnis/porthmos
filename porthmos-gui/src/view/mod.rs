pub mod button;
pub mod checkbox;
mod connection_form;
pub mod connections;
mod context_menu;
mod drag_overlay;
mod dropdown;
mod file_list;
pub mod format;
mod header;
mod input;
mod modal;
pub mod palette;
mod path_bar;
pub mod preview;
mod scroll;
pub mod search;
pub mod settings;
mod spacing;
mod status;
pub mod sync;
mod tabs;
pub mod text_fit;
pub mod toolbar;
pub mod tools;
mod transfer_column;
mod transfer_queue;

use iced::widget::text::Wrapping;
use iced::widget::{column, container, mouse_area, responsive, row, space, stack, text};
use iced::{Background, Color, Element, Font, Length, font};

use crate::app::{App, Message, PanelSide, offer_label};
use crate::panel::Panel;
use crate::theme::Palette;

pub const BOLD: Font = Font { weight: font::Weight::Bold, ..Font::DEFAULT };

pub fn label<'a>(content: impl Into<String>, color: Color) -> iced::widget::Text<'a> {
    text(content.into()).size(file_list::TEXT_SIZE).font(Font::DEFAULT).color(color).wrapping(Wrapping::None)
}

pub fn wrapped_label<'a>(content: impl Into<String>, color: Color) -> iced::widget::Text<'a> {
    text(content.into()).size(file_list::TEXT_SIZE).font(Font::DEFAULT).color(color).width(Length::Fill)
}

pub fn bold_label<'a>(content: impl Into<String>, color: Color) -> iced::widget::Text<'a> {
    label(content, color).font(BOLD)
}

pub fn filled<'a>(color: Color) -> impl Fn(&iced::Theme) -> container::Style + 'a {
    move |_| container::Style { background: Some(Background::Color(color)), ..container::Style::default() }
}

pub const SCRIM_ALPHA: f32 = 0.6;

pub fn scrim_color(palette: &Palette) -> Color {
    palette.bg.scale_alpha(SCRIM_ALPHA)
}

pub fn scrim<'a>(palette: &Palette) -> container::Container<'a, Message> {
    container(space()).width(Length::Fill).height(Length::Fill).style(filled(scrim_color(palette)))
}

pub fn hairline_across<'a>(palette: &Palette) -> Element<'a, Message> {
    container(space()).width(Length::Fill).height(Length::Fixed(1.0)).style(filled(palette.line)).into()
}

pub fn hairline_down<'a>(palette: &Palette) -> Element<'a, Message> {
    container(space()).width(Length::Fixed(1.0)).height(Length::Fill).style(filled(palette.line)).into()
}

pub fn blank(app: &App) -> Element<'_, Message> {
    container(space()).width(Length::Fill).height(Length::Fill).style(filled(app.palette().bg)).into()
}

pub fn root(app: &App) -> Element<'_, Message> {
    let palette = app.palette();
    let panels = row![
        container(panel(app, PanelSide::Local)).width(Length::FillPortion(1)).height(Length::Fill),
        hairline_down(palette),
        transfer_column::bar(app),
        hairline_down(palette),
        container(panel(app, PanelSide::Remote)).width(Length::FillPortion(1)).height(Length::Fill),
    ]
    .height(Length::Fill);
    let mut page = column![
        tabs::strip(app),
        hairline_across(palette),
        toolbar::bar(app),
        hairline_across(palette),
        panels,
        hairline_across(palette)
    ];
    if app.queue().is_open() {
        page = page.push(transfer_queue::pane(app)).push(hairline_across(palette));
    }
    let page = page.push(status::bar(app));
    let page = container(page).width(Length::Fill).height(Length::Fill).style(filled(palette.bg));
    let mut layers = stack![page];
    if let Some(tool) = tools::overlay(app) {
        layers = layers.push(tool);
    }
    if app.more_open() {
        layers = layers.push(toolbar::more_menu(app));
    }
    if let Some(menu) = context_menu::overlay(app) {
        layers = layers.push(menu);
    }
    if let Some(dialog) = modal::overlay(app) {
        layers = layers.push(dialog);
    }
    if let Some(hint) = drag_overlay::hint(app) {
        layers = layers.push(hint);
    }
    mouse_area(layers).on_move(Message::PointerMoved).into()
}

fn panel(app: &App, side: PanelSide) -> Element<'_, Message> {
    let palette = app.palette();
    let hovered = app.drop_target().is_some_and(|hit| hit.side == side && hit.row.is_none());
    let bar = if hovered { palette.accent } else { palette.bg };
    let body = responsive(move |size| match panel_of(app, side) {
        Some(panel) => populated(app, side, panel, size),
        None => no_connection(app, side),
    });
    row![container(space()).width(Length::Fixed(2.0)).height(Length::Fill).style(filled(bar)), body].into()
}

fn panel_of(app: &App, side: PanelSide) -> Option<&Panel> {
    match side {
        PanelSide::Local => Some(&app.local().panel),
        PanelSide::Remote => app.right_panel(),
    }
}

fn populated<'a>(app: &'a App, side: PanelSide, panel: &'a Panel, size: iced::Size) -> Element<'a, Message> {
    let palette = app.palette();
    let shown = file_list::columns(size.width);
    column![
        path_bar::bar(app, side, panel, size.width),
        hairline_across(palette),
        header::columns(side, panel, shown, palette),
        header::rule(app.active_side() == side, palette),
        file_list::list(app, side, panel, shown, size),
        hairline_across(palette),
        status::panel_line(panel, palette, file_list::line_cells(size.width), app.filter_editing_side() == Some(side)),
    ]
    .into()
}

fn no_connection<'a>(app: &App, side: PanelSide) -> Element<'a, Message> {
    let palette = app.palette();
    let mut content = column![label("No connection", palette.fg_dim)].spacing(12).align_x(iced::Alignment::Center);
    if side == PanelSide::Remote && !app.reopen_offer().is_empty() {
        let offer = offer_label(app.reopen_offer());
        content =
            content.push(button::text_button(&offer, Some(Message::ReopenPressed), button::Look::Primary, palette));
    }
    content = content.push(saved_connections(app));
    container(content).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into()
}

fn saved_connections<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let names = app.quick_connections();
    if names.is_empty() {
        return column![
            label("No saved connections yet", palette.fg_dim),
            button::text_button("Add a connection", Some(Message::ManagerToggled), button::Look::Primary, palette),
        ]
        .spacing(8)
        .align_x(iced::Alignment::Center)
        .into();
    }
    let mut list = column![label("Connect to", palette.fg_dim)].spacing(6).align_x(iced::Alignment::Center);
    for name in names {
        let press = Message::QuickConnectPressed(name.clone());
        list = list.push(button::text_button(&name, Some(press), button::Look::Normal, palette));
    }
    let hidden = app.quick_connections_hidden();
    if hidden > 0 {
        let more = format!("{hidden} more in Connections");
        list = list.push(button::text_button(&more, Some(Message::ManagerToggled), button::Look::Normal, palette));
    }
    list.into()
}

#[cfg(test)]
mod tests;
