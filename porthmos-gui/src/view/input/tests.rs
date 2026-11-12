use iced::widget::text_input::Status;
use iced::{Background, Color};

use super::{bare, face};
use crate::theme::{Palette, ThemeSettings};

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

fn fill(style: &iced::widget::text_input::Style) -> Color {
    match style.background {
        Background::Color(color) => color,
        other => panic!("expected a plain color, got {other:?}"),
    }
}

#[test]
fn a_resting_input_has_a_visible_neutral_border() {
    let palette = palette();

    let style = face(&palette, Status::Active);

    assert_eq!(style.border.color, palette.line_strong);
    assert!(style.border.width >= 1.0);
}

#[test]
fn a_focused_input_has_an_accent_border_whether_or_not_it_is_hovered() {
    let palette = palette();

    for is_hovered in [false, true] {
        assert_eq!(face(&palette, Status::Focused { is_hovered }).border.color, palette.accent);
    }
}

#[test]
fn a_hovered_input_brightens_its_border_without_taking_the_accent() {
    let palette = palette();

    let border = face(&palette, Status::Hovered).border.color;

    assert_eq!(border, palette.fg_muted);
    assert_ne!(border, palette.accent);
}

#[test]
fn the_border_is_thicker_when_focused() {
    let palette = palette();

    let resting = face(&palette, Status::Active).border.width;
    let focused = face(&palette, Status::Focused { is_hovered: false }).border.width;

    assert!(focused > resting);
}

#[test]
fn typed_text_placeholder_and_selection_use_the_palette() {
    let palette = palette();

    let style = face(&palette, Status::Active);

    assert_eq!(style.value, palette.fg_bright);
    assert_eq!(style.placeholder, palette.fg_dim);
    assert_eq!(style.icon, palette.fg_muted);
    assert_eq!(style.selection, palette.accent.scale_alpha(0.35));
}

#[test]
fn an_input_sits_on_the_surface_colour() {
    let palette = palette();

    assert_eq!(fill(&face(&palette, Status::Active)), palette.surface);
}

#[test]
fn a_disabled_input_dims_its_text() {
    let palette = palette();

    assert_eq!(face(&palette, Status::Disabled).value, palette.fg_dim);
}

#[test]
fn a_bare_input_draws_no_border_and_no_fill_so_a_frame_can_hold_it() {
    let palette = palette();

    for status in [Status::Active, Status::Hovered, Status::Focused { is_hovered: false }] {
        let style = bare(&palette, status);
        assert_eq!(style.border.width, 0.0);
        assert_eq!(fill(&style), Color::TRANSPARENT);
        assert_eq!(style.value, palette.fg_bright);
    }
}
