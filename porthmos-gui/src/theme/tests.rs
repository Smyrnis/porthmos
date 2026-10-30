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
fn every_token_has_the_spec_value() {
    let palette = Palette::new(&ThemeSettings::default());

    assert_eq!(palette.bg, hex("#000000"));
    assert_eq!(palette.fg_bright, hex("#ededed"));
    assert_eq!(palette.fg_focus, hex("#ffffff"));
    assert_eq!(palette.fg, hex("#b3b3b3"));
    assert_eq!(palette.fg_muted, hex("#8a8a8a"));
    assert_eq!(palette.fg_dim, hex("#6e6e6e"));
    assert_eq!(palette.line, hex("#1c1c1c"));
    assert_eq!(palette.line_strong, hex("#262626"));
    assert_eq!(palette.row_focus, hex("#161616"));
    assert_eq!(palette.row_focus_inactive, hex("#0b0b0b"));
    assert_eq!(palette.surface, hex("#0a0a0a"));
    assert_eq!(palette.accent, hex("#9fb3c8"));
    assert_eq!(palette.ok, hex("#8fb38a"));
    assert_eq!(palette.ok_dim, hex("#4f6b4c"));
    assert_eq!(palette.err, hex("#c98b84"));
    assert_eq!(palette.idle, hex("#333333"));
    assert_eq!(palette.paused, hex("#5a5a5a"));
    assert_eq!(palette.inactive_file, hex("#7a7a7a"));
    assert_eq!(palette.inactive_dir, hex("#a0a0a0"));
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
fn parse_accepts_six_hex_digits_only() {
    assert_eq!(Rgb::parse("#9fb3c8"), Some(Rgb(0x9f, 0xb3, 0xc8)));
    assert_eq!(Rgb::parse("9fb3c8"), None);
    assert_eq!(Rgb::parse("#9fb3c"), None);
    assert_eq!(Rgb::parse("#9fb3c8ff"), None);
    assert_eq!(Rgb::parse("#9fb3cg"), None);
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
