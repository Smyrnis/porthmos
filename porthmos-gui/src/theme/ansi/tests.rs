use porthmos_core::theme::{ColorScheme, Rgb};
use porthmos_term::Color as TermColor;

use super::*;
use crate::theme::ThemeSettings;

fn palette_of(scheme: ColorScheme) -> Palette {
    Palette::new(&ThemeSettings::default().with_scheme(scheme))
}

fn all_schemes() -> [(&'static str, Palette); 3] {
    [
        ("dark", palette_of(ColorScheme::dark())),
        ("high contrast", palette_of(ColorScheme::high_contrast())),
        ("light", palette_of(ColorScheme::light())),
    ]
}

fn rgb(color: Color) -> Rgb {
    let [red, green, blue, _] = color.into_rgba8();
    Rgb(red, green, blue)
}

#[test]
fn default_colours_follow_the_palette_in_all_three_schemes() {
    for (name, palette) in all_schemes() {
        assert_eq!(resolve(TermColor::Default, true, &palette), palette.fg, "{name}");
        assert_eq!(resolve(TermColor::Default, false, &palette), palette.bg, "{name}");
    }
}

#[test]
fn the_sixteen_ansi_colours_are_distinct_in_every_scheme() {
    for (name, palette) in all_schemes() {
        let mut seen: Vec<Rgb> = (0..16).map(|index| rgb(resolve(TermColor::Indexed(index), true, &palette))).collect();
        seen.sort_by_key(|Rgb(red, green, blue)| (*red, *green, *blue));
        seen.dedup();
        assert_eq!(seen.len(), 16, "{name}");
    }
}

#[test]
fn indexed_cube_and_grayscale_match_xterm_values() {
    let palette = palette_of(ColorScheme::dark());
    let at = |index: u8| rgb(resolve(TermColor::Indexed(index), true, &palette));

    assert_eq!(at(16), Rgb(0, 0, 0));
    assert_eq!(at(21), Rgb(0, 0, 255));
    assert_eq!(at(196), Rgb(255, 0, 0));
    assert_eq!(at(231), Rgb(255, 255, 255));
    assert_eq!(at(232), Rgb(8, 8, 8));
    assert_eq!(at(255), Rgb(238, 238, 238));
}

#[test]
fn true_colour_is_passed_through() {
    let palette = palette_of(ColorScheme::dark());

    assert_eq!(rgb(resolve(TermColor::Rgb(1, 2, 3), true, &palette)), Rgb(1, 2, 3));
}

#[test]
fn coloured_text_is_readable_on_the_default_background_in_every_scheme() {
    for (name, palette) in all_schemes() {
        let background = rgb(palette.bg);
        for index in (1..=6).chain(9..=14) {
            let text = rgb(resolve(TermColor::Indexed(index), true, &palette));
            let contrast = text.contrast(background);
            assert!(contrast >= 4.5, "{name}: colour {index} {text:?} has contrast {contrast:.2}");
        }
    }
}
