use iced::Color;
use iced::widget::button::Status;

use super::{Look, face, flat};
use crate::theme::{Palette, ThemeSettings};

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
    let style = flat(Color::WHITE, true);

    assert_eq!(background(&style(&iced::Theme::Dark, Status::Active)), Color::TRANSPARENT);
    assert_ne!(background(&style(&iced::Theme::Dark, Status::Hovered)), Color::TRANSPARENT);
}

#[test]
fn a_flat_button_that_cannot_act_never_highlights() {
    let style = flat(Color::WHITE, false);

    assert_eq!(background(&style(&iced::Theme::Dark, Status::Hovered)), Color::TRANSPARENT);
}
