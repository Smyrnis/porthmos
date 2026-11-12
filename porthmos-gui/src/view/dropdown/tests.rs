use iced::widget::pick_list::Status;
use iced::{Background, Color};

use super::{face, menu_face};
use crate::theme::{Palette, ThemeSettings};

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

fn fill(style: &iced::widget::pick_list::Style) -> Color {
    match style.background {
        Background::Color(color) => color,
        other => panic!("expected a plain color, got {other:?}"),
    }
}

#[test]
fn a_dropdown_looks_like_a_text_input_at_rest() {
    let palette = palette();

    let dropdown = face(&palette, Status::Active);
    let input = crate::view::input::face(&palette, iced::widget::text_input::Status::Active);

    assert_eq!(dropdown.border.color, input.border.color);
    assert_eq!(dropdown.border.width, input.border.width);
    assert_eq!(dropdown.border.radius, input.border.radius);
    assert_eq!(Background::Color(fill(&dropdown)), input.background);
}

#[test]
fn an_open_dropdown_takes_the_accent_border() {
    let palette = palette();

    for is_hovered in [false, true] {
        assert_eq!(face(&palette, Status::Opened { is_hovered }).border.color, palette.accent);
    }
}

#[test]
fn a_hovered_dropdown_brightens_its_border_without_the_accent() {
    let palette = palette();

    assert_eq!(face(&palette, Status::Hovered).border.color, palette.fg_muted);
}

#[test]
fn text_placeholder_and_handle_come_from_the_palette() {
    let palette = palette();

    let style = face(&palette, Status::Active);

    assert_eq!(style.text_color, palette.fg_bright);
    assert_eq!(style.placeholder_color, palette.fg_dim);
    assert_eq!(style.handle_color, palette.fg_muted);
}

#[test]
fn the_menu_is_a_raised_surface_with_a_strong_border_and_an_accent_free_selection() {
    let palette = palette();

    let menu = menu_face(&palette);

    assert_eq!(menu.background, Background::Color(palette.surface));
    assert_eq!(menu.border.color, palette.line_strong);
    assert_eq!(menu.text_color, palette.fg);
    assert_eq!(menu.selected_text_color, palette.fg_focus);
    assert_eq!(menu.selected_background, Background::Color(palette.row_focus));
}
