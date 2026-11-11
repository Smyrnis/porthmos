use iced::widget::checkbox::Status;
use iced::{Background, Color};

use super::face;
use crate::theme::{Palette, Rgb, ThemeSettings};

const MIN_EDGE_CONTRAST: f32 = 3.0;

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

fn linear(channel: f32) -> f32 {
    if channel <= 0.03928 { channel / 12.92 } else { ((channel + 0.055) / 1.055).powf(2.4) }
}

fn luminance(color: Color) -> f32 {
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

fn contrast(a: Color, b: Color) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn fill(style: &iced::widget::checkbox::Style) -> Color {
    match style.background {
        Background::Color(color) => color,
        other => panic!("expected a plain color, got {other:?}"),
    }
}

#[test]
fn an_unchecked_box_has_an_edge_that_stands_out_from_the_window_and_from_dialogs() {
    let palette = palette();

    for status in [Status::Active { is_checked: false }, Status::Hovered { is_checked: false }] {
        let style = face(&palette, status);

        assert!(style.border.width >= 1.0, "{status:?}");
        assert!(contrast(style.border.color, palette.bg) >= MIN_EDGE_CONTRAST, "{status:?} on the window");
        assert!(contrast(style.border.color, palette.surface) >= MIN_EDGE_CONTRAST, "{status:?} on a dialog");
    }
}

#[test]
fn a_checked_box_is_filled_with_the_accent_and_keeps_a_visible_edge() {
    let palette = palette();

    let style = face(&palette, Status::Active { is_checked: true });

    assert_eq!(fill(&style), palette.accent);
    assert!(contrast(style.border.color, palette.bg) >= MIN_EDGE_CONTRAST);
    assert!(contrast(style.icon_color, fill(&style)) >= MIN_EDGE_CONTRAST);
}

#[test]
fn a_dark_user_accent_still_leaves_the_checked_box_visible() {
    let palette = Palette::new(&ThemeSettings { accent: Some(Rgb(0x10, 0x10, 0x10)), ..ThemeSettings::default() });

    let style = face(&palette, Status::Active { is_checked: true });

    assert!(contrast(style.border.color, palette.bg) >= MIN_EDGE_CONTRAST);
}

#[test]
fn hovering_makes_the_edge_brighter_than_at_rest() {
    let palette = palette();

    let at_rest = face(&palette, Status::Active { is_checked: false });
    let hovered = face(&palette, Status::Hovered { is_checked: false });

    assert!(luminance(hovered.border.color) > luminance(at_rest.border.color));
}

#[test]
fn a_disabled_box_is_dimmer_than_an_enabled_one_but_still_has_an_edge() {
    let palette = palette();

    let enabled = face(&palette, Status::Active { is_checked: false });
    let disabled = face(&palette, Status::Disabled { is_checked: false });

    assert!(disabled.border.color.a < enabled.border.color.a);
    assert!(disabled.border.width >= 1.0);
}

#[test]
fn the_label_uses_the_normal_text_colour() {
    let palette = palette();

    let style = face(&palette, Status::Active { is_checked: false });

    assert_eq!(style.text_color, Some(palette.fg));
}
