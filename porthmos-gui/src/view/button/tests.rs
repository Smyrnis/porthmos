use iced::Color;
use iced::widget::button::Status;

use super::{Look, face, flat};
use crate::theme::{Palette, ThemeSettings};
use porthmos_core::theme::{ColorScheme, Rgb};

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

fn background(style: &iced::widget::button::Style) -> Color {
    match style.background {
        Some(iced::Background::Color(color)) => color,
        other => panic!("expected a plain color, got {other:?}"),
    }
}

#[test]
fn disabled_faces_use_the_dim_text_and_ignore_hover() {
    let palette = palette();

    let hovered = face(&palette, Look::Disabled, Status::Hovered);
    let active = face(&palette, Look::Disabled, Status::Active);

    assert_eq!(hovered.text_color, palette.fg_dim);
    assert_eq!(background(&hovered), background(&active));
}

#[test]
fn hovered_normal_is_lighter_than_active() {
    let palette = palette();

    let active = background(&face(&palette, Look::Normal, Status::Active));
    let hovered = background(&face(&palette, Look::Normal, Status::Hovered));
    let pressed = background(&face(&palette, Look::Normal, Status::Pressed));

    assert!(hovered.r > active.r);
    assert!(pressed.r > hovered.r);
}

#[test]
fn primary_has_an_accent_border_and_accent_text() {
    let palette = palette();

    let style = face(&palette, Look::Primary, Status::Active);

    assert_eq!(style.border.color, palette.accent);
    assert_eq!(style.text_color, palette.accent);
}

#[test]
fn every_other_button_has_a_line_strong_border() {
    let palette = palette();

    for look in [Look::Normal, Look::Disabled] {
        for status in [Status::Active, Status::Hovered, Status::Pressed] {
            assert_eq!(face(&palette, look, status).border.color, palette.line_strong);
        }
    }
}

#[test]
fn flat_buttons_are_transparent_until_hovered() {
    let style = flat(&palette(), Color::WHITE, true);

    assert_eq!(background(&style(&iced::Theme::Dark, Status::Active)), Color::TRANSPARENT);
    assert_ne!(background(&style(&iced::Theme::Dark, Status::Hovered)), Color::TRANSPARENT);
}

#[test]
fn a_flat_button_that_cannot_act_never_highlights() {
    let style = flat(&palette(), Color::WHITE, false);

    assert_eq!(background(&style(&iced::Theme::Dark, Status::Hovered)), Color::TRANSPARENT);
}

fn palette_with_surface(surface: Rgb) -> Palette {
    let scheme = ColorScheme { surface, ..ColorScheme::dark() };
    Palette::new(&ThemeSettings::default().with_scheme(scheme))
}

#[test]
fn a_resting_button_is_filled_with_the_surface_colour() {
    let palette = palette_with_surface(Rgb(0x30, 0x10, 0x50));

    for look in [Look::Normal, Look::Primary, Look::Disabled] {
        assert_eq!(background(&face(&palette, look, Status::Active)), palette.surface);
    }
}

#[test]
fn a_disabled_button_stays_on_the_surface_colour_when_hovered() {
    let palette = palette();

    assert_eq!(background(&face(&palette, Look::Disabled, Status::Hovered)), palette.surface);
}

#[test]
fn hover_and_press_move_the_surface_towards_bright_text() {
    let palette = palette_with_surface(Rgb(0x30, 0x10, 0x50));

    let hovered = background(&face(&palette, Look::Normal, Status::Hovered));
    let pressed = background(&face(&palette, Look::Normal, Status::Pressed));

    assert!(hovered.b > palette.surface.b && hovered.g > palette.surface.g);
    assert!(pressed.g > hovered.g);
    assert!(pressed.g < palette.fg_bright.g);
}

#[test]
fn a_chosen_button_is_filled_with_a_tint_of_the_accent() {
    let palette = palette();

    let chosen = background(&face(&palette, Look::Chosen, Status::Active));

    assert_ne!(chosen, palette.surface);
    assert!(chosen.b > palette.surface.b);
    assert!(chosen.b < palette.accent.b);
}

#[test]
fn a_chosen_button_has_an_accent_border_and_readable_accent_text() {
    let palette = palette();

    let style = face(&palette, Look::Chosen, Status::Active);

    assert_eq!(style.border.color, palette.accent);
    assert_eq!(style.text_color, palette.fg_focus);
}

#[test]
fn a_chosen_button_is_distinct_from_a_primary_one() {
    let palette = palette();

    let chosen = background(&face(&palette, Look::Chosen, Status::Active));
    let primary = background(&face(&palette, Look::Primary, Status::Active));

    assert_ne!(chosen, primary);
}

#[test]
fn hover_and_press_deepen_a_chosen_button() {
    let palette = palette();

    let active = background(&face(&palette, Look::Chosen, Status::Active));
    let hovered = background(&face(&palette, Look::Chosen, Status::Hovered));
    let pressed = background(&face(&palette, Look::Chosen, Status::Pressed));

    assert!(hovered.b > active.b);
    assert!(pressed.b > hovered.b);
}

#[test]
fn the_flat_hover_fill_comes_from_the_palette() {
    let palette = palette_with_surface(Rgb(0x30, 0x10, 0x50));
    let style = flat(&palette, Color::WHITE, true);

    let hovered = background(&style(&iced::Theme::Dark, Status::Hovered));

    assert!(hovered.b > palette.surface.b);
}

#[test]
fn chosen_text_is_readable_on_every_chosen_fill_in_the_default_scheme() {
    let palette = palette();
    let to_rgb = |color: Color| {
        Rgb((color.r * 255.0).round() as u8, (color.g * 255.0).round() as u8, (color.b * 255.0).round() as u8)
    };

    for status in [Status::Active, Status::Hovered, Status::Pressed] {
        let style = face(&palette, Look::Chosen, status);
        assert!(to_rgb(style.text_color).contrast(to_rgb(background(&style))) >= 4.5, "{status:?}");
    }
}
