pub mod button;
mod connection_form;
mod connections;
mod context_menu;
mod drag_overlay;
mod file_list;
pub mod format;
mod header;
mod modal;
pub mod palette;
mod path_bar;
pub mod preview;
pub mod settings;
mod status;
pub mod sync;
mod tabs;
pub mod text_fit;
pub mod toolbar;
pub mod tools;
mod transfer_column;
mod transfer_queue;

use iced::widget::text::Wrapping;
use iced::widget::{column, container, responsive, row, space, stack, text};
use iced::{Background, Color, Element, Font, Length, font};

use crate::app::{App, Message, PanelSide};
use crate::panel::Panel;
use crate::theme::Palette;

pub const BOLD: Font = Font { weight: font::Weight::Bold, ..Font::DEFAULT };

pub fn label<'a>(content: impl Into<String>, color: Color) -> iced::widget::Text<'a> {
    text(content.into()).size(file_list::TEXT_SIZE).font(Font::DEFAULT).color(color).wrapping(Wrapping::None)
}

pub fn bold_label<'a>(content: impl Into<String>, color: Color) -> iced::widget::Text<'a> {
    label(content, color).font(BOLD)
}

pub fn filled<'a>(color: Color) -> impl Fn(&iced::Theme) -> container::Style + 'a {
    move |_| container::Style { background: Some(Background::Color(color)), ..container::Style::default() }
}

pub fn hairline_across<'a>(palette: &Palette) -> Element<'a, Message> {
    container(space()).width(Length::Fill).height(Length::Fixed(1.0)).style(filled(palette.line)).into()
}

pub fn hairline_down<'a>(palette: &Palette) -> Element<'a, Message> {
    container(space()).width(Length::Fixed(1.0)).height(Length::Fill).style(filled(palette.line)).into()
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
    if let Some(manager) = connections::overlay(app) {
        layers = layers.push(manager);
    }
    if let Some(form) = connection_form::overlay(app) {
        layers = layers.push(form);
    }
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
    layers.into()
}

fn panel(app: &App, side: PanelSide) -> Element<'_, Message> {
    let palette = app.palette();
    let hovered = app.drop_target().is_some_and(|hit| hit.side == side && hit.row.is_none());
    let bar = if hovered { palette.accent } else { palette.bg };
    let body = responsive(move |size| match panel_of(app, side) {
        Some(panel) => populated(app, side, panel, size),
        None => no_connection(app.palette()),
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
        hairline_across(palette),
        file_list::list(app, side, panel, shown, size),
        hairline_across(palette),
        status::panel_line(panel, palette, file_list::line_cells(size.width), app.filter_editing_side() == Some(side)),
    ]
    .into()
}

fn no_connection<'a>(palette: &Palette) -> Element<'a, Message> {
    container(label("No connection", palette.fg_dim))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
