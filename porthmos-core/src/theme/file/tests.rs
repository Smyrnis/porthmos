use super::*;
use crate::Paths;

fn parsed(text: &str) -> (ColorScheme, Vec<String>) {
    parse(text, None)
}

#[test]
fn an_empty_file_keeps_the_base_scheme() {
    let (scheme, warnings) = parsed("");

    assert_eq!(scheme, ColorScheme::dark());
    assert!(warnings.is_empty());
}

#[test]
fn hex_colours_replace_the_named_roles_and_nothing_else() {
    let (scheme, warnings) = parsed("[colors]\nbg = \"#101820\"\naccent = \"#FFAA00\"\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(scheme.bg, Rgb(0x10, 0x18, 0x20));
    assert_eq!(scheme.accent, Rgb(0xff, 0xaa, 0x00));
    assert_eq!(
        ColorScheme { bg: ColorScheme::dark().bg, accent: ColorScheme::dark().accent, ..scheme },
        ColorScheme::dark()
    );
}

#[test]
fn every_role_can_be_set() {
    let mut text = String::from("[colors]\n");
    for (index, name) in ColorScheme::ROLES.iter().enumerate() {
        text.push_str(&format!("{name} = \"#{index:02x}0000\"\n"));
    }

    let (scheme, warnings) = parsed(&text);

    assert!(warnings.is_empty(), "{warnings:?}");
    let mut scheme = scheme;
    for (index, name) in ColorScheme::ROLES.iter().enumerate() {
        assert_eq!(*scheme.role_mut(name).unwrap(), Rgb(index as u8, 0, 0), "{name}");
    }
}

#[test]
fn the_roles_are_the_fields_of_the_scheme() {
    let mut scheme = ColorScheme::dark();

    for name in ColorScheme::ROLES {
        assert!(scheme.role_mut(name).is_some(), "{name}");
    }
    assert!(scheme.role_mut("nonsense").is_none());
    assert_eq!(ColorScheme::ROLES.len(), 19);
}

#[test]
fn a_bad_colour_warns_and_keeps_that_role_only() {
    let (scheme, warnings) = parsed("[colors]\nbg = \"red\"\nfg = \"#112233\"\naccent = \"#12345\"\n");

    assert_eq!(scheme.bg, ColorScheme::dark().bg);
    assert_eq!(scheme.accent, ColorScheme::dark().accent);
    assert_eq!(scheme.fg, Rgb(0x11, 0x22, 0x33));
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|text| text.contains("bg") && text.contains("red")), "{warnings:?}");
    assert!(warnings.iter().any(|text| text.contains("accent") && text.contains("#12345")), "{warnings:?}");
}

#[test]
fn a_colour_that_is_not_a_string_warns() {
    let (scheme, warnings) = parsed("[colors]\nbg = 5\n");

    assert_eq!(scheme, ColorScheme::dark());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("bg") && warnings[0].contains("string"), "{warnings:?}");
}

#[test]
fn an_unknown_colour_name_warns_so_typos_are_noticed() {
    let (scheme, warnings) = parsed("[colors]\nbackgound = \"#000000\"\n");

    assert_eq!(scheme, ColorScheme::dark());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("backgound"), "{warnings:?}");
}

#[test]
fn an_unknown_top_level_setting_warns() {
    let (_, warnings) = parsed("mode = \"light\"\n");

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("mode"), "{warnings:?}");
}

#[test]
fn a_colors_entry_that_is_not_a_table_warns() {
    let (scheme, warnings) = parsed("colors = \"#000000\"\n");

    assert_eq!(scheme, ColorScheme::dark());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("[colors]"), "{warnings:?}");
}

#[test]
fn a_file_that_does_not_parse_warns_with_the_line_and_keeps_the_base() {
    let (scheme, warnings) = parsed("[colors\nbg = \"#000000\"\n");

    assert_eq!(scheme, ColorScheme::dark());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with("failed to parse theme.toml: line 1"), "{warnings:?}");
}

#[test]
fn a_colour_in_a_broken_file_never_leaks_into_the_warning() {
    let (_, warnings) = parsed("[colors]\nbg = \"#abcdef\" oops\n");

    assert!(!warnings.concat().contains("#abcdef"), "{warnings:?}");
}

#[test]
fn the_values_are_applied_on_top_of_the_accent_from_the_config() {
    let (scheme, _) = parse("[colors]\nbg = \"#101010\"\n", Some(Rgb(1, 2, 3)));

    assert_eq!(scheme.accent, Rgb(1, 2, 3));
    assert_eq!(scheme.bg, Rgb(0x10, 0x10, 0x10));
}

#[test]
fn a_missing_file_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();

    let (scheme, warnings) = load(&Paths::in_dir(dir.path()), None);

    assert_eq!(scheme, ColorScheme::dark());
    assert!(warnings.is_empty());
}

#[test]
fn the_file_is_read_from_the_config_folder() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.config_dir.join("theme.toml"), "[colors]\nfg = \"#abcdef\"\n").unwrap();

    let (scheme, warnings) = load(&paths, None);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(scheme.fg, Rgb(0xab, 0xcd, 0xef));
}

#[test]
fn a_file_that_cannot_be_read_warns_and_keeps_the_base() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(paths.config_dir.join("theme.toml")).unwrap();

    let (scheme, warnings) = load(&paths, None);

    assert_eq!(scheme, ColorScheme::dark());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with("could not read theme.toml"), "{warnings:?}");
}

#[test]
fn the_example_config_documents_every_role_with_its_default() {
    let example = include_str!("../../../../config/config.example.toml");
    let documented: String = example
        .lines()
        .skip_while(|line| !line.starts_with("#   [colors]"))
        .take_while(|line| line.starts_with("#  "))
        .map(|line| {
            let uncommented = line.trim_start_matches('#').trim_start();
            let end = uncommented.rfind('"').map_or(uncommented.len(), |at| at + 1);
            format!("{}\n", &uncommented[..end])
        })
        .collect();

    let (scheme, warnings) = parsed(&documented);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(scheme, ColorScheme::dark());
    for role in ColorScheme::ROLES {
        assert!(documented.contains(&format!("{role} = ")), "{role} is not documented");
    }
}

#[test]
fn the_base_names_the_scheme_the_colours_start_from() {
    for (name, base) in
        [("dark", ThemeBase::Dark), ("high-contrast", ThemeBase::HighContrast), ("light", ThemeBase::Light)]
    {
        let (scheme, warnings) = parse(&format!("base = \"{name}\"\n"), None);

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(scheme, base.scheme(), "{name}");
    }
}

#[test]
fn colours_are_applied_on_top_of_the_chosen_base() {
    let (scheme, warnings) = parse("base = \"light\"\n[colors]\nfg = \"#123456\"\n", None);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(scheme.fg, Rgb(0x12, 0x34, 0x56));
    assert_eq!(ColorScheme { fg: ColorScheme::light().fg, ..scheme }, ColorScheme::light());
}

#[test]
fn the_order_of_base_and_colours_in_the_file_does_not_matter() {
    let (first, _) = parse("colors = { fg = \"#123456\" }\nbase = \"light\"\n", None);
    let (second, _) = parse("base = \"light\"\n[colors]\nfg = \"#123456\"\n", None);

    assert_eq!(first, second);
}

#[test]
fn a_base_written_below_the_colours_table_is_reported_as_a_misplaced_colour() {
    let (scheme, warnings) = parse("[colors]\nfg = \"#123456\"\nbase = \"light\"\n", None);

    assert_eq!(scheme.bg, ColorScheme::dark().bg);
    assert_eq!(warnings, vec!["unknown colour \"base\" in theme.toml".to_string()]);
}

#[test]
fn the_accent_from_the_config_is_applied_to_the_chosen_base() {
    let (scheme, _) = parse("base = \"light\"\n", Some(Rgb(1, 2, 3)));

    assert_eq!(scheme, ColorScheme { accent: Rgb(1, 2, 3), ..ColorScheme::light() });
}

#[test]
fn an_accent_in_the_file_wins_over_the_one_from_the_config() {
    let (scheme, _) = parse("[colors]\naccent = \"#0a0b0c\"\n", Some(Rgb(1, 2, 3)));

    assert_eq!(scheme.accent, Rgb(0x0a, 0x0b, 0x0c));
}

#[test]
fn an_unknown_base_warns_with_the_valid_names_and_keeps_dark() {
    let (scheme, warnings) = parse("base = \"solarized\"\n", None);

    assert_eq!(scheme, ColorScheme::dark());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("solarized") && warnings[0].contains("high-contrast"), "{warnings:?}");
}

#[test]
fn a_base_that_is_not_a_string_warns() {
    let (scheme, warnings) = parse("base = 3\n", None);

    assert_eq!(scheme, ColorScheme::dark());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("base"), "{warnings:?}");
}

#[test]
fn the_example_config_names_every_base() {
    let example = include_str!("../../../../config/config.example.toml");

    for name in ThemeBase::NAMES {
        assert!(example.contains(name), "{name} is not documented");
    }
}

#[test]
fn the_base_can_be_read_without_applying_anything_else() {
    assert_eq!(base_in("base = \"light\"\n[colors]\nfg = \"#123456\"\n"), ThemeBase::Light);
    assert_eq!(base_in("base = \"high-contrast\"\n"), ThemeBase::HighContrast);
}

#[test]
fn a_missing_unknown_or_unreadable_base_reads_as_dark() {
    for text in ["", "[colors]\nfg = \"#123456\"\n", "base = \"nope\"\n", "base = 3\n", "base = = =\n"] {
        assert_eq!(base_in(text), ThemeBase::Dark, "{text:?}");
    }
}

#[test]
fn the_base_is_read_from_a_file_and_a_missing_file_is_dark() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("theme.toml");

    assert_eq!(read_base(&path), ThemeBase::Dark);
    std::fs::write(&path, "base = \"light\"\n").unwrap();
    assert_eq!(read_base(&path), ThemeBase::Light);
}

#[test]
fn loading_from_a_path_gives_the_same_result_as_loading_from_the_paths() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.theme_file(), "base = \"light\"\n[colors]\nfg = \"#abcdef\"\n").unwrap();

    assert_eq!(load_from(&paths.theme_file(), Some(Rgb(1, 2, 3))), load(&paths, Some(Rgb(1, 2, 3))));
}
