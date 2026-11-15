use ratatui::style::{Color, Modifier};

use super::*;

fn table(text: &str) -> toml::Table {
    text.parse().unwrap()
}

#[test]
fn colorterm_truecolor_and_24bit_select_truecolor_everything_else_indexed() {
    assert_eq!(ColorDepth::detect(Some("truecolor"), None), ColorDepth::TrueColor);
    assert_eq!(ColorDepth::detect(Some("24bit"), None), ColorDepth::TrueColor);
    assert_eq!(ColorDepth::detect(Some("TrueColor"), None), ColorDepth::TrueColor);
    assert_eq!(ColorDepth::detect(Some(""), None), ColorDepth::Indexed);
    assert_eq!(ColorDepth::detect(Some("yes"), None), ColorDepth::Indexed);
    assert_eq!(ColorDepth::detect(None, None), ColorDepth::Indexed);
}

#[test]
fn a_windows_terminal_session_is_true_colour_without_colorterm() {
    assert_eq!(ColorDepth::detect(None, Some("0d1e-guid")), ColorDepth::TrueColor);
}

#[test]
fn an_empty_windows_terminal_session_is_not_true_colour() {
    assert_eq!(ColorDepth::detect(None, Some("")), ColorDepth::Indexed);
}

#[test]
fn colorterm_wins_over_a_missing_windows_terminal_session() {
    assert_eq!(ColorDepth::detect(Some("24bit"), None), ColorDepth::TrueColor);
    assert_eq!(ColorDepth::detect(Some("yes"), None), ColorDepth::Indexed);
}

#[test]
fn truecolor_theme_takes_every_colour_from_the_shared_dark_scheme() {
    let theme = Theme::new(ColorDepth::TrueColor, &ThemeSettings::default());
    let scheme = ColorScheme::dark();
    let colour = |rgb: Rgb| Color::Rgb(rgb.0, rgb.1, rgb.2);

    assert_eq!(theme.bg, colour(scheme.bg));
    assert_eq!(theme.fg_bright, colour(scheme.fg_bright));
    assert_eq!(theme.fg_focus, colour(scheme.fg_focus));
    assert_eq!(theme.fg, colour(scheme.fg));
    assert_eq!(theme.fg_muted, colour(scheme.fg_muted));
    assert_eq!(theme.fg_dim, colour(scheme.fg_dim));
    assert_eq!(theme.line, colour(scheme.line));
    assert_eq!(theme.line_strong, colour(scheme.line_strong));
    assert_eq!(theme.row_focus, colour(scheme.row_focus));
    assert_eq!(theme.row_focus_inactive, colour(scheme.row_focus_inactive));
    assert_eq!(theme.surface, colour(scheme.surface));
    assert_eq!(theme.accent, colour(scheme.accent));
    assert_eq!(theme.ok, colour(scheme.ok));
    assert_eq!(theme.ok_dim, colour(scheme.ok_dim));
    assert_eq!(theme.err, colour(scheme.err));
    assert_eq!(theme.idle, colour(scheme.idle));
    assert_eq!(theme.paused, colour(scheme.paused));
    assert_eq!(theme.inactive_file, colour(scheme.inactive_file));
    assert_eq!(theme.inactive_dir, colour(scheme.inactive_dir));
}

#[test]
fn indexed_theme_uses_the_computed_fallback_column() {
    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default());

    assert_eq!(theme.bg, Color::Indexed(16));
    assert_eq!(theme.fg_bright, Color::Indexed(255));
    assert_eq!(theme.fg_focus, Color::Indexed(231));
    assert_eq!(theme.fg, Color::Indexed(252));
    assert_eq!(theme.fg_muted, Color::Indexed(145));
    assert_eq!(theme.fg_dim, Color::Indexed(247));
    assert_eq!(theme.line, Color::Indexed(236));
    assert_eq!(theme.line_strong, Color::Indexed(240));
    assert_eq!(theme.row_focus, Color::Indexed(236));
    assert_eq!(theme.row_focus_inactive, Color::Indexed(234));
    assert_eq!(theme.surface, Color::Indexed(233));
    assert_eq!(theme.accent, Color::Indexed(110));
    assert_eq!(theme.ok, Color::Indexed(108));
    assert_eq!(theme.ok_dim, Color::Indexed(108));
    assert_eq!(theme.err, Color::Indexed(174));
    assert_eq!(theme.idle, Color::Indexed(239));
    assert_eq!(theme.paused, Color::Indexed(246));
    assert_eq!(theme.inactive_file, Color::Indexed(248));
    assert_eq!(theme.inactive_dir, Color::Indexed(250));
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

    assert_eq!(
        settings,
        ThemeSettings { accent: Some(Rgb(0xc8, 0xb8, 0x9a)), terminal_background: true, ..ThemeSettings::default() }
    );
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

fn sources_outside_the_theme_containing(needles: &[&str]) -> Vec<String> {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&source, &mut files);
    files
        .iter()
        .filter(|path| !path.starts_with(source.join("theme")))
        .filter(|path| path.file_name().is_some_and(|name| name != "tests.rs"))
        .filter(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            needles.iter().any(|needle| text.contains(needle))
        })
        .map(|path| path.strip_prefix(&source).unwrap().display().to_string())
        .collect()
}

#[test]
fn only_the_theme_module_names_a_colour_or_the_dim_modifier() {
    let offenders = sources_outside_the_theme_containing(&["Color::", "Modifier::DIM"]);

    assert!(offenders.is_empty(), "colours must come from Theme: {offenders:?}");
}

#[test]
fn modifier_reversed_appears_nowhere_outside_the_theme() {
    let offenders = sources_outside_the_theme_containing(&["Modifier::REVERSED", "REVERSED"]);

    assert!(offenders.is_empty(), "reversed video is not part of the design: {offenders:?}");
}

#[test]
fn the_example_config_documents_the_default_theme() {
    let frontend: toml::Table = toml::from_str(include_str!("../../../config/config.example.toml")).unwrap();

    let (settings, warnings) = ThemeSettings::from_frontend(&frontend);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        settings,
        ThemeSettings { accent: Some(Rgb(0x9f, 0xb3, 0xc8)), terminal_background: false, ..ThemeSettings::default() }
    );
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

#[test]
fn backdrop_is_the_line_strong_foreground_only() {
    let theme = Theme::default();
    let style = theme.backdrop();

    assert_eq!(style.fg, Some(theme.line_strong));
    assert_eq!(style.bg, None);
    assert!(style.add_modifier.is_empty());
}

#[test]
fn a_scheme_from_the_theme_file_sets_the_colours() {
    let scheme = ColorScheme { bg: Rgb(1, 2, 3), fg: Rgb(0x10, 0x20, 0x30), ..ColorScheme::dark() };
    let settings = ThemeSettings::default().with_scheme(scheme);

    let theme = Theme::new(ColorDepth::TrueColor, &settings);

    assert_eq!(theme.bg, Color::Rgb(1, 2, 3));
    assert_eq!(theme.fg, Color::Rgb(0x10, 0x20, 0x30));
    assert_eq!(theme.accent, Color::Rgb(0x9f, 0xb3, 0xc8));
}

#[test]
fn a_scheme_colour_follows_the_depth_with_the_nearest_indexed_colour() {
    let scheme = ColorScheme { fg: Rgb(255, 0, 0), ..ColorScheme::dark() };

    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default().with_scheme(scheme));

    assert_eq!(theme.fg, Color::Indexed(196));
}

#[test]
fn the_terminal_background_setting_still_wins_over_a_scheme_background() {
    let scheme = ColorScheme { bg: Rgb(1, 2, 3), ..ColorScheme::dark() };
    let settings = ThemeSettings { terminal_background: true, ..ThemeSettings::default() }.with_scheme(scheme);

    assert_eq!(Theme::new(ColorDepth::TrueColor, &settings).bg, Color::Reset);
}

#[test]
fn a_scheme_accent_beats_the_config_accent_once_the_file_is_loaded() {
    let from_config = ThemeSettings { accent: Some(Rgb(9, 9, 9)), ..ThemeSettings::default() };
    let file_scheme = ColorScheme::dark().with_accent(from_config.accent);
    let from_file = ColorScheme { accent: Rgb(7, 7, 7), ..file_scheme };

    let theme = Theme::new(ColorDepth::TrueColor, &from_config.with_scheme(from_file));

    assert_eq!(theme.accent, Color::Rgb(7, 7, 7));
}
