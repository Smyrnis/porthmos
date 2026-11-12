use iced::Background;
use iced::widget::scrollable::Status;

use super::face;
use crate::theme::{Palette, ThemeSettings};

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

fn resting() -> Status {
    Status::Active { is_horizontal_scrollbar_disabled: false, is_vertical_scrollbar_disabled: false }
}

fn hovered() -> Status {
    Status::Hovered {
        is_horizontal_scrollbar_hovered: false,
        is_vertical_scrollbar_hovered: true,
        is_horizontal_scrollbar_disabled: false,
        is_vertical_scrollbar_disabled: false,
    }
}

fn dragged() -> Status {
    Status::Dragged {
        is_horizontal_scrollbar_dragged: false,
        is_vertical_scrollbar_dragged: true,
        is_horizontal_scrollbar_disabled: false,
        is_vertical_scrollbar_disabled: false,
    }
}

#[test]
fn the_rail_has_no_background_so_the_window_shows_through() {
    let style = face(&palette(), resting());

    assert_eq!(style.vertical_rail.background, None);
    assert_eq!(style.horizontal_rail.background, None);
}

#[test]
fn the_thumb_is_the_strong_line_colour_at_rest() {
    let palette = palette();

    let style = face(&palette, resting());

    assert_eq!(style.vertical_rail.scroller.background, Background::Color(palette.line_strong));
}

#[test]
fn the_thumb_brightens_when_hovered_and_takes_the_accent_when_dragged() {
    let palette = palette();

    assert_eq!(face(&palette, hovered()).vertical_rail.scroller.background, Background::Color(palette.fg_muted));
    assert_eq!(face(&palette, dragged()).vertical_rail.scroller.background, Background::Color(palette.accent));
}

#[test]
fn the_thumb_is_rounded() {
    let style = face(&palette(), resting());

    assert!(style.vertical_rail.scroller.border.radius.top_left > 0.0);
}

#[test]
fn the_scrollable_itself_draws_no_background_or_border() {
    let style = face(&palette(), resting());

    assert_eq!(style.container.background, None);
    assert_eq!(style.container.border.width, 0.0);
}
