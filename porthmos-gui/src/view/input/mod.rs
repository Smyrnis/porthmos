use iced::widget::text_input::{self, Status};
use iced::{Background, Border, Color, Theme};

use crate::theme::Palette;

const EDGE_WIDTH: f32 = 1.0;
const FOCUS_EDGE_WIDTH: f32 = 1.5;
const EDGE_RADIUS: f32 = 6.0;
const SELECTION_ALPHA: f32 = 0.35;

pub fn face(palette: &Palette, status: Status) -> text_input::Style {
    let (edge, width) = match status {
        Status::Active => (palette.line_strong, EDGE_WIDTH),
        Status::Hovered => (palette.fg_muted, EDGE_WIDTH),
        Status::Focused { .. } => (palette.accent, FOCUS_EDGE_WIDTH),
        Status::Disabled => (palette.line, EDGE_WIDTH),
    };
    text_input::Style {
        background: Background::Color(palette.surface),
        border: Border { color: edge, width, radius: EDGE_RADIUS.into() },
        icon: palette.fg_muted,
        placeholder: palette.fg_dim,
        value: if status == Status::Disabled { palette.fg_dim } else { palette.fg_bright },
        selection: palette.accent.scale_alpha(SELECTION_ALPHA),
    }
}

pub fn bare(palette: &Palette, status: Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border { width: 0.0, ..face(palette, status).border },
        ..face(palette, status)
    }
}

pub fn style(palette: &Palette) -> impl Fn(&Theme, Status) -> text_input::Style + use<> {
    let palette = *palette;
    move |_, status| face(&palette, status)
}

pub fn bare_style(palette: &Palette) -> impl Fn(&Theme, Status) -> text_input::Style + use<> {
    let palette = *palette;
    move |_, status| bare(&palette, status)
}

#[cfg(test)]
mod tests;
