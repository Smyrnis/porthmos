use iced::widget::container;
use iced::widget::scrollable::{self, AutoScroll, Rail, Scroller, Status};
use iced::{Background, Border, Color, Theme};

use crate::theme::Palette;

const THUMB_RADIUS: f32 = 4.0;

fn thumb(palette: &Palette, status: Status) -> Color {
    match status {
        Status::Active { .. } => palette.line_strong,
        Status::Hovered { is_vertical_scrollbar_hovered, is_horizontal_scrollbar_hovered, .. } => {
            if is_vertical_scrollbar_hovered || is_horizontal_scrollbar_hovered {
                palette.fg_muted
            } else {
                palette.line_strong
            }
        }
        Status::Dragged { .. } => palette.accent,
    }
}

pub fn face(palette: &Palette, status: Status) -> scrollable::Style {
    let rail = Rail {
        background: None,
        border: Border::default(),
        scroller: Scroller {
            background: Background::Color(thumb(palette, status)),
            border: Border { radius: THUMB_RADIUS.into(), ..Border::default() },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: AutoScroll {
            background: Background::Color(palette.surface),
            border: Border { color: palette.line_strong, width: 1.0, radius: THUMB_RADIUS.into() },
            shadow: iced::Shadow::default(),
            icon: palette.fg,
        },
    }
}

pub fn style(palette: &Palette) -> impl Fn(&Theme, Status) -> scrollable::Style + use<> {
    let palette = *palette;
    move |_, status| face(&palette, status)
}

#[cfg(test)]
mod tests;
