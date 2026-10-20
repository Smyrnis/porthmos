use ratatui::style::{Color, Modifier};

use super::*;

fn table(text: &str) -> toml::Table {
    text.parse().unwrap()
}

#[test]
fn colorterm_truecolor_and_24bit_select_truecolor_everything_else_indexed() {
    assert_eq!(ColorDepth::detect(Some("truecolor")), ColorDepth::TrueColor);
    assert_eq!(ColorDepth::detect(Some("24bit")), ColorDepth::TrueColor);
    assert_eq!(ColorDepth::detect(Some("TrueColor")), ColorDepth::TrueColor);
    assert_eq!(ColorDepth::detect(Some("")), ColorDepth::Indexed);
    assert_eq!(ColorDepth::detect(Some("yes")), ColorDepth::Indexed);
    assert_eq!(ColorDepth::detect(None), ColorDepth::Indexed);
}

#[test]
fn rgb_parse_accepts_only_seven_character_hex() {
    assert_eq!(Rgb::parse("#9fb3c8"), Some(Rgb(0x9f, 0xb3, 0xc8)));
    assert_eq!(Rgb::parse("#9FB3C8"), Some(Rgb(0x9f, 0xb3, 0xc8)));
    for bad in ["9fb3c8", "#9fb3c", "#9fb3c8f", "#gggggg", "", "#", "red", "#9fb3c8 "] {
        assert_eq!(Rgb::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn nearest_indexed_maps_into_the_colour_cube_and_the_grey_ramp() {
    assert_eq!(Rgb(0, 0, 0).nearest_indexed(), 16);
    assert_eq!(Rgb(255, 255, 255).nearest_indexed(), 231);
    assert_eq!(Rgb(255, 0, 0).nearest_indexed(), 196);
    assert_eq!(Rgb(0x1c, 0x1c, 0x1c).nearest_indexed(), 234);
    assert_eq!(Rgb(0x6e, 0x6e, 0x6e).nearest_indexed(), 242);
}

#[test]
fn nearest_indexed_matches_the_xterm_palette_at_saturated_and_boundary_inputs() {
    let expected = [
        (Rgb(255, 0, 255), 201),
        (Rgb(0, 255, 0), 46),
        (Rgb(0, 0, 255), 21),
        (Rgb(255, 255, 0), 226),
        (Rgb(0, 255, 255), 51),
        (Rgb(95, 95, 95), 59),
        (Rgb(135, 135, 135), 102),
        (Rgb(3, 3, 3), 16),
        (Rgb(4, 4, 4), 16),
        (Rgb(5, 5, 5), 232),
        (Rgb(8, 8, 8), 232),
        (Rgb(18, 18, 18), 233),
        (Rgb(48, 48, 48), 236),
        (Rgb(128, 128, 128), 244),
        (Rgb(238, 238, 238), 255),
        (Rgb(47, 255, 0), 46),
        (Rgb(48, 255, 0), 82),
        (Rgb(194, 0, 0), 124),
        (Rgb(196, 0, 0), 160),
    ];

    for (rgb, index) in expected {
        assert_eq!(rgb.nearest_indexed(), index, "{rgb:?}");
    }
}

#[test]
fn nearest_indexed_never_returns_a_system_colour_or_panics() {
    let channel_values = (0..=255u8).step_by(15).chain([1, 94, 95, 96, 254, 255]);
    let channel_values: Vec<u8> = channel_values.collect();

    for red in &channel_values {
        for green in &channel_values {
            for blue in &channel_values {
                let index = Rgb(*red, *green, *blue).nearest_indexed();
                assert!(index >= 16, "{red} {green} {blue} -> {index}");
            }
        }
    }
}

#[test]
fn truecolor_theme_uses_the_spec_hex_values() {
    let theme = Theme::new(ColorDepth::TrueColor, &ThemeSettings::default());

    assert_eq!(theme.bg, Color::Rgb(0x00, 0x00, 0x00));
    assert_eq!(theme.fg_bright, Color::Rgb(0xed, 0xed, 0xed));
    assert_eq!(theme.fg_focus, Color::Rgb(0xff, 0xff, 0xff));
    assert_eq!(theme.fg, Color::Rgb(0xb3, 0xb3, 0xb3));
    assert_eq!(theme.fg_muted, Color::Rgb(0x8a, 0x8a, 0x8a));
    assert_eq!(theme.fg_dim, Color::Rgb(0x6e, 0x6e, 0x6e));
    assert_eq!(theme.line, Color::Rgb(0x1c, 0x1c, 0x1c));
    assert_eq!(theme.line_strong, Color::Rgb(0x26, 0x26, 0x26));
    assert_eq!(theme.row_focus, Color::Rgb(0x16, 0x16, 0x16));
    assert_eq!(theme.row_focus_inactive, Color::Rgb(0x0b, 0x0b, 0x0b));
    assert_eq!(theme.surface, Color::Rgb(0x0a, 0x0a, 0x0a));
    assert_eq!(theme.accent, Color::Rgb(0x9f, 0xb3, 0xc8));
    assert_eq!(theme.ok, Color::Rgb(0x8f, 0xb3, 0x8a));
    assert_eq!(theme.ok_dim, Color::Rgb(0x4f, 0x6b, 0x4c));
    assert_eq!(theme.err, Color::Rgb(0xc9, 0x8b, 0x84));
    assert_eq!(theme.idle, Color::Rgb(0x33, 0x33, 0x33));
    assert_eq!(theme.paused, Color::Rgb(0x5a, 0x5a, 0x5a));
    assert_eq!(theme.inactive_file, Color::Rgb(0x7a, 0x7a, 0x7a));
    assert_eq!(theme.inactive_dir, Color::Rgb(0xa0, 0xa0, 0xa0));
}

#[test]
fn indexed_theme_uses_the_spec_fallback_column() {
    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default());

    assert_eq!(theme.bg, Color::Indexed(16));
    assert_eq!(theme.fg_bright, Color::Indexed(255));
    assert_eq!(theme.fg_focus, Color::Indexed(231));
    assert_eq!(theme.fg, Color::Indexed(249));
    assert_eq!(theme.fg_muted, Color::Indexed(245));
    assert_eq!(theme.fg_dim, Color::Indexed(242));
    assert_eq!(theme.line, Color::Indexed(234));
    assert_eq!(theme.line_strong, Color::Indexed(235));
    assert_eq!(theme.row_focus, Color::Indexed(233));
    assert_eq!(theme.row_focus_inactive, Color::Indexed(232));
    assert_eq!(theme.surface, Color::Indexed(232));
    assert_eq!(theme.accent, Color::Indexed(110));
    assert_eq!(theme.ok, Color::Indexed(108));
    assert_eq!(theme.ok_dim, Color::Indexed(65));
    assert_eq!(theme.err, Color::Indexed(174));
    assert_eq!(theme.idle, Color::Indexed(236));
    assert_eq!(theme.paused, Color::Indexed(240));
    assert_eq!(theme.inactive_file, Color::Indexed(243));
    assert_eq!(theme.inactive_dir, Color::Indexed(247));
}

#[test]
fn every_indexed_token_is_distinct_where_the_spec_distinguishes_them() {
    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default());
    let text_ladder = [theme.fg_focus, theme.fg_bright, theme.fg, theme.fg_muted, theme.fg_dim, theme.line_strong];

    for (i, a) in text_ladder.iter().enumerate() {
        for b in &text_ladder[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_ne!(theme.row_focus, theme.bg);
    assert_ne!(theme.accent, theme.fg_bright);
}

#[test]
fn a_custom_accent_replaces_only_the_accent_and_follows_the_depth() {
    let settings = ThemeSettings { accent: Some(Rgb(0xc8, 0xb8, 0x9a)), ..ThemeSettings::default() };

    let truecolor = Theme::new(ColorDepth::TrueColor, &settings);
    let indexed = Theme::new(ColorDepth::Indexed, &settings);

    assert_eq!(truecolor.accent, Color::Rgb(0xc8, 0xb8, 0x9a));
    assert_eq!(indexed.accent, Color::Indexed(Rgb(0xc8, 0xb8, 0x9a).nearest_indexed()));
    assert_eq!(truecolor.ok, Theme::default().ok);
}

#[test]
fn the_default_accent_given_explicitly_keeps_its_hand_picked_fallback() {
    let settings = ThemeSettings { accent: Some(Rgb(0x9f, 0xb3, 0xc8)), ..ThemeSettings::default() };

    assert_eq!(Theme::new(ColorDepth::Indexed, &settings).accent, Color::Indexed(110));
    assert_eq!(Theme::new(ColorDepth::TrueColor, &settings).accent, Color::Rgb(0x9f, 0xb3, 0xc8));
}

#[test]
fn the_terminal_background_setting_uses_the_terminals_own_background() {
    let settings = ThemeSettings { terminal_background: true, ..ThemeSettings::default() };

    assert_eq!(Theme::new(ColorDepth::TrueColor, &settings).bg, Color::Reset);
    assert_eq!(Theme::new(ColorDepth::Indexed, &settings).bg, Color::Reset);
}

#[test]
fn settings_default_when_there_is_no_theme_table() {
    let (settings, warnings) = ThemeSettings::from_frontend(&table(""));

    assert_eq!(settings, ThemeSettings::default());
    assert!(warnings.is_empty());
}

#[test]
fn settings_read_accent_and_background() {
    let (settings, warnings) =
        ThemeSettings::from_frontend(&table("[theme]\naccent = \"#c8b89a\"\nbackground = \"terminal\"\n"));

    assert_eq!(settings, ThemeSettings { accent: Some(Rgb(0xc8, 0xb8, 0x9a)), terminal_background: true });
    assert!(warnings.is_empty());
}

#[test]
fn a_black_background_value_is_the_default() {
    let (settings, warnings) = ThemeSettings::from_frontend(&table("[theme]\nbackground = \"black\"\n"));

    assert!(!settings.terminal_background);
    assert!(warnings.is_empty());
}

#[test]
fn invalid_values_warn_and_fall_back_one_by_one() {
    let (settings, warnings) =
        ThemeSettings::from_frontend(&table("[theme]\naccent = \"blue\"\nbackground = \"white\"\n"));

    assert_eq!(settings, ThemeSettings::default());
    assert_eq!(warnings.len(), 2);
    assert!(warnings.iter().any(|w| w.contains("accent") && w.contains("blue")));
    assert!(warnings.iter().any(|w| w.contains("background") && w.contains("white")));
}

#[test]
fn a_theme_entry_of_the_wrong_type_warns_and_keeps_the_defaults() {
    let (settings, warnings) = ThemeSettings::from_frontend(&table("theme = 3\n"));
    assert_eq!(settings, ThemeSettings::default());
    assert_eq!(warnings.len(), 1);

    let (settings, warnings) = ThemeSettings::from_frontend(&table("[theme]\naccent = 3\nbackground = true\n"));
    assert_eq!(settings, ThemeSettings::default());
    assert_eq!(warnings.len(), 2);
}

#[test]
fn unknown_theme_keys_warn_so_typos_are_noticed() {
    let (_, warnings) = ThemeSettings::from_frontend(&table("[theme]\naccnt = \"#ffffff\"\n"));

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("accnt"));
}

#[test]
fn style_helpers_follow_the_spec_tables() {
    let theme = Theme::default();

    assert_eq!(theme.text().fg, Some(theme.fg));
    assert_eq!(theme.bright().fg, Some(theme.fg_bright));
    assert_eq!(theme.muted().fg, Some(theme.fg_muted));
    assert_eq!(theme.dim().fg, Some(theme.fg_dim));
    assert_eq!(theme.accent_text().fg, Some(theme.accent));
    assert_eq!(theme.ok_text().fg, Some(theme.ok));
    assert_eq!(theme.err_text().fg, Some(theme.err));
    assert_eq!(theme.hairline().fg, Some(theme.line));
    assert_eq!(theme.popup_border().fg, Some(theme.line_strong));

    let focused_name = theme.focused_name();
    assert_eq!(focused_name.fg, Some(theme.fg_focus));
    assert!(focused_name.add_modifier.contains(Modifier::BOLD));

    assert_eq!(theme.focus_row().bg, Some(theme.row_focus));
    assert_eq!(theme.focus_row_inactive().bg, Some(theme.row_focus_inactive));
    assert_eq!(theme.toast_surface().bg, Some(theme.surface));
}

#[test]
fn no_helper_uses_italic_underline_or_dim_modifiers() {
    let theme = Theme::default();
    let styles = [
        theme.text(),
        theme.bright(),
        theme.focused_name(),
        theme.muted(),
        theme.dim(),
        theme.accent_text(),
        theme.ok_text(),
        theme.err_text(),
        theme.warning_text(),
        theme.hairline(),
        theme.popup_border(),
        theme.focus_row(),
        theme.focus_row_inactive(),
        theme.toast_surface(),
    ];

    for style in styles {
        assert!(!style.add_modifier.intersects(Modifier::ITALIC | Modifier::UNDERLINED | Modifier::DIM));
    }
}

#[test]
fn only_the_theme_module_names_a_colour_or_the_dim_modifier() {
    fn rust_files(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }

    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&source, &mut files);

    let offenders: Vec<String> = files
        .iter()
        .filter(|path| !path.starts_with(source.join("theme")))
        .filter(|path| path.file_name().is_some_and(|name| name != "tests.rs"))
        .filter(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            text.contains("Color::") || text.contains("Modifier::DIM")
        })
        .map(|path| path.strip_prefix(&source).unwrap().display().to_string())
        .collect();

    assert!(offenders.is_empty(), "colours must come from Theme: {offenders:?}");
}

#[test]
fn the_example_config_documents_the_default_theme() {
    let frontend: toml::Table = toml::from_str(include_str!("../../../config/config.example.toml")).unwrap();

    let (settings, warnings) = ThemeSettings::from_frontend(&frontend);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(settings, ThemeSettings { accent: Some(Rgb(0x9f, 0xb3, 0xc8)), terminal_background: false });
    assert_eq!(Theme::new(ColorDepth::TrueColor, &settings), Theme::default());
    assert_eq!(Theme::new(ColorDepth::Indexed, &settings), Theme::new(ColorDepth::Indexed, &ThemeSettings::default()));
}

#[test]
fn name_styles_follow_the_row_state_table() {
    let theme = Theme::default();

    let directory = theme.directory_name();
    assert_eq!(directory.fg, Some(theme.fg_bright));
    assert!(directory.add_modifier.contains(Modifier::BOLD));

    assert_eq!(theme.inactive_file_name().fg, Some(theme.inactive_file));
    assert!(!theme.inactive_file_name().add_modifier.contains(Modifier::BOLD));

    let inactive_directory = theme.inactive_dir_name();
    assert_eq!(inactive_directory.fg, Some(theme.inactive_dir));
    assert!(inactive_directory.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn inactive_panel_colours_are_distinct_from_the_text_ladder_on_256_colours() {
    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default());

    for inactive in [theme.inactive_file, theme.inactive_dir] {
        for other in [theme.fg, theme.fg_muted, theme.fg_dim, theme.fg_bright] {
            assert_ne!(inactive, other);
        }
    }
    assert_ne!(theme.inactive_file, theme.inactive_dir);
}
