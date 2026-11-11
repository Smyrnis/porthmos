use super::*;
use crate::Paths;

fn parsed(text: &str) -> (ColorScheme, Vec<String>) {
    parse(ColorScheme::dark(), text)
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
fn the_values_are_applied_on_top_of_the_given_base() {
    let base = ColorScheme::dark().with_accent(Some(Rgb(1, 2, 3)));

    let (scheme, _) = parse(base, "[colors]\nbg = \"#101010\"\n");

    assert_eq!(scheme.accent, Rgb(1, 2, 3));
    assert_eq!(scheme.bg, Rgb(0x10, 0x10, 0x10));
}

#[test]
fn a_missing_file_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();

    let (scheme, warnings) = load_onto(&Paths::in_dir(dir.path()), ColorScheme::dark());

    assert_eq!(scheme, ColorScheme::dark());
    assert!(warnings.is_empty());
}

#[test]
fn the_file_is_read_from_the_config_folder() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.config_dir.join("theme.toml"), "[colors]\nfg = \"#abcdef\"\n").unwrap();

    let (scheme, warnings) = load_onto(&paths, ColorScheme::dark());

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(scheme.fg, Rgb(0xab, 0xcd, 0xef));
}

#[test]
fn a_file_that_cannot_be_read_warns_and_keeps_the_base() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(paths.config_dir.join("theme.toml")).unwrap();

    let (scheme, warnings) = load_onto(&paths, ColorScheme::dark());

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
