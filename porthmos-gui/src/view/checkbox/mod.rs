use iced::widget::checkbox::{self, Status};
use iced::{Background, Border, Color, Theme};

use crate::theme::Palette;

const EDGE_WIDTH: f32 = 1.5;
const EDGE_RADIUS: f32 = 3.0;
const DISABLED_OPACITY: f32 = 0.4;

pub fn face(palette: &Palette, status: Status) -> checkbox::Style {
    let (is_checked, hovered, enabled) = match status {
        Status::Active { is_checked } => (is_checked, false, true),
        Status::Hovered { is_checked } => (is_checked, true, true),
        Status::Disabled { is_checked } => (is_checked, false, false),
    };
    let edge = if hovered { palette.fg_bright } else { palette.fg_muted };
    let dim = |color: Color| if enabled { color } else { color.scale_alpha(DISABLED_OPACITY) };
    let fill = if is_checked { palette.accent } else { palette.bg };
    checkbox::Style {
        background: Background::Color(dim(fill)),
        icon_color: dim(palette.bg),
        border: Border { color: dim(edge), width: EDGE_WIDTH, radius: EDGE_RADIUS.into() },
        text_color: Some(dim(palette.fg)),
    }
}

pub fn style(palette: &Palette) -> impl Fn(&Theme, Status) -> checkbox::Style + use<> {
    let palette = *palette;
    move |_, status| face(&palette, status)
}

#[cfg(test)]
mod tests;
