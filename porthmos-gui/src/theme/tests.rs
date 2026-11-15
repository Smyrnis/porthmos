use iced::Color;

use super::*;

fn table(text: &str) -> toml::Table {
    text.parse().unwrap()
}

fn hex(text: &str) -> Color {
    let Rgb(red, green, blue) = Rgb::parse(text).unwrap();
    Color::from_rgb8(red, green, blue)
}

#[test]
fn every_role_takes_its_colour_from_the_shared_dark_scheme() {
    let palette = Palette::new(&ThemeSettings::default());
    let scheme = ColorScheme::dark();

    assert_eq!(palette.bg, color_of(scheme.bg));
    assert_eq!(palette.fg_bright, color_of(scheme.fg_bright));
    assert_eq!(palette.fg_focus, color_of(scheme.fg_focus));
    assert_eq!(palette.fg, color_of(scheme.fg));
    assert_eq!(palette.fg_muted, color_of(scheme.fg_muted));
    assert_eq!(palette.fg_dim, color_of(scheme.fg_dim));
    assert_eq!(palette.line, color_of(scheme.line));
    assert_eq!(palette.line_strong, color_of(scheme.line_strong));
    assert_eq!(palette.row_focus, color_of(scheme.row_focus));
    assert_eq!(palette.row_focus_inactive, color_of(scheme.row_focus_inactive));
    assert_eq!(palette.surface, color_of(scheme.surface));
    assert_eq!(palette.accent, color_of(scheme.accent));
    assert_eq!(palette.ok, color_of(scheme.ok));
    assert_eq!(palette.ok_dim, color_of(scheme.ok_dim));
    assert_eq!(palette.err, color_of(scheme.err));
    assert_eq!(palette.idle, color_of(scheme.idle));
    assert_eq!(palette.paused, color_of(scheme.paused));
    assert_eq!(palette.inactive_file, color_of(scheme.inactive_file));
    assert_eq!(palette.inactive_dir, color_of(scheme.inactive_dir));
}

#[test]
fn an_accent_setting_replaces_only_the_accent() {
    let (settings, warnings) = ThemeSettings::from_frontend(&table("[gui.theme]\naccent = \"#112233\""));
    let palette = Palette::new(&settings);
    let plain = Palette::new(&ThemeSettings::default());

    assert!(warnings.is_empty());
    assert_eq!(palette.accent, hex("#112233"));
    assert_eq!(Palette { accent: plain.accent, ..palette }, plain);
}

#[test]
fn an_invalid_accent_warns_and_keeps_the_default() {
    let (settings, warnings) = ThemeSettings::from_frontend(&table("[gui.theme]\naccent = \"blue\""));

    assert_eq!(settings, ThemeSettings::default());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("blue"), "{warnings:?}");
}

#[test]
fn an_unknown_gui_theme_key_warns() {
    let (_, warnings) = ThemeSettings::from_frontend(&table("[gui.theme]\nshadow = \"#000000\""));

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("shadow"), "{warnings:?}");
}

#[test]
fn a_gui_table_that_is_not_a_table_warns() {
    let (settings, warnings) = ThemeSettings::from_frontend(&table("gui = \"yes\""));

    assert_eq!(settings, ThemeSettings::default());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn a_missing_gui_table_is_silent() {
    let (settings, warnings) = ThemeSettings::from_frontend(&table("[theme]\naccent = \"#9fb3c8\""));

    assert_eq!(settings, ThemeSettings::default());
    assert!(warnings.is_empty());
}

#[test]
fn the_iced_theme_uses_the_palette_for_background_text_and_accent() {
    let palette = Palette::new(&ThemeSettings::default());

    let iced_palette = palette.iced_theme().palette();

    assert_eq!(iced_palette.background, palette.bg);
    assert_eq!(iced_palette.text, palette.fg);
    assert_eq!(iced_palette.primary, palette.accent);
    assert_eq!(iced_palette.danger, palette.err);
}

#[test]
fn a_scheme_from_the_theme_file_sets_the_palette() {
    let scheme = ColorScheme { bg: Rgb(1, 2, 3), row_focus: Rgb(0x30, 0x40, 0x50), ..ColorScheme::dark() };

    let palette = Palette::new(&ThemeSettings::default().with_scheme(scheme));

    assert_eq!(palette.bg, hex("#010203"));
    assert_eq!(palette.row_focus, hex("#304050"));
    assert_eq!(palette.fg, color_of(ColorScheme::dark().fg));
}

#[test]
fn a_scheme_accent_beats_the_config_accent_once_the_file_is_loaded() {
    let from_config = ThemeSettings { accent: Some(Rgb(9, 9, 9)), ..ThemeSettings::default() };
    let from_file = ColorScheme { accent: Rgb(7, 7, 7), ..ColorScheme::dark() };

    let palette = Palette::new(&from_config.with_scheme(from_file));

    assert_eq!(palette.accent, hex("#070707"));
}

#[test]
fn mixing_nothing_keeps_the_first_colour_and_mixing_everything_gives_the_second() {
    let (from, to) = (Color::from_rgb8(10, 20, 30), Color::from_rgb8(210, 120, 90));

    assert_eq!(mix(from, to, 0.0), from);
    assert_eq!(mix(from, to, 1.0), to);
}

#[test]
fn mixing_halfway_lands_between_the_two_colours() {
    let mixed = mix(Color::from_rgb8(0, 0, 0), Color::from_rgb8(200, 100, 50), 0.5);

    assert_eq!(mixed, Color::from_rgb(100.0 / 255.0, 50.0 / 255.0, 25.0 / 255.0));
}

#[test]
fn only_a_palette_on_a_light_ground_asks_for_light_icons() {
    let on = |scheme: ColorScheme| Palette::new(&ThemeSettings::default().with_scheme(scheme)).light;

    assert!(on(ColorScheme::light()));
    assert!(!on(ColorScheme::dark()));
    assert!(!on(ColorScheme::high_contrast()));
}
