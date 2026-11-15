use iced::widget::overlay::menu;
use iced::widget::pick_list::{self, Status};
use iced::{Background, Border, Theme};

use crate::theme::Palette;

const EDGE_WIDTH: f32 = 1.0;
const OPEN_EDGE_WIDTH: f32 = 1.5;
const EDGE_RADIUS: f32 = 6.0;

pub fn face(palette: &Palette, status: Status) -> pick_list::Style {
    let (edge, width) = match status {
        Status::Active => (palette.line_strong, EDGE_WIDTH),
        Status::Hovered => (palette.fg_muted, EDGE_WIDTH),
        Status::Opened { .. } => (palette.accent, OPEN_EDGE_WIDTH),
    };
    pick_list::Style {
        text_color: palette.fg_bright,
        placeholder_color: palette.fg_dim,
        handle_color: palette.fg_muted,
        background: Background::Color(palette.surface),
        border: Border { color: edge, width, radius: EDGE_RADIUS.into() },
    }
}

pub fn menu_face(palette: &Palette) -> menu::Style {
    menu::Style {
        background: Background::Color(palette.surface),
        border: Border { color: palette.line_strong, width: EDGE_WIDTH, radius: EDGE_RADIUS.into() },
        text_color: palette.fg,
        selected_text_color: palette.fg_focus,
        selected_background: Background::Color(palette.row_focus),
        shadow: iced::Shadow::default(),
    }
}

pub fn style(palette: &Palette) -> impl Fn(&Theme, Status) -> pick_list::Style + use<> {
    let palette = *palette;
    move |_, status| face(&palette, status)
}

pub fn menu_style(palette: &Palette) -> impl Fn(&Theme) -> menu::Style + use<> {
    let palette = *palette;
    move |_| menu_face(&palette)
}

#[cfg(test)]
mod tests;
