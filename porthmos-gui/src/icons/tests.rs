use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons")
}

fn main_assets() -> PathBuf {
    assets().join("main")
}

fn svgs(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            svgs(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "svg") {
            found.push(path);
        }
    }
}

#[test]
fn every_vendored_svg_is_listed_in_sources_and_carries_the_apache_header() {
    let sources = fs::read_to_string(assets().join("SOURCES.md")).unwrap();
    let mut found = Vec::new();
    svgs(&assets(), &mut found);
    found.retain(|path| !path.starts_with(main_assets()));
    assert!(!found.is_empty());
    for path in found {
        let relative = path.strip_prefix(assets()).unwrap().to_string_lossy().replace('\\', "/");
        assert!(sources.contains(&format!("| {relative} |")), "{relative} is not listed in SOURCES.md");
        let head: String = fs::read_to_string(&path).unwrap().lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(head.contains("Apache"), "{relative} has no Apache header");
    }
}

#[test]
fn every_svg_drawn_for_porthmos_is_listed_in_sources_and_carries_the_project_header() {
    let sources = fs::read_to_string(assets().join("SOURCES.md")).unwrap();
    let mut found = Vec::new();
    svgs(&main_assets(), &mut found);
    assert!(!found.is_empty());
    for path in found {
        let relative = path.strip_prefix(assets()).unwrap().to_string_lossy().replace('\\', "/");
        assert!(sources.contains(&format!("| {relative} |")), "{relative} is not listed in SOURCES.md");
        let head = fs::read_to_string(&path).unwrap().lines().next().unwrap_or_default().to_string();
        assert!(head.contains("Drawn for Porthmos"), "{relative} has no Porthmos header");
    }
}

#[test]
fn every_vendored_svg_is_used_by_an_icon() {
    let mut found = Vec::new();
    svgs(&assets(), &mut found);
    assert_eq!(found.len(), Icon::ALL.len());
}

#[test]
fn extensions_map_to_their_icons() {
    let cases = [
        ("a.rs", Icon::Text),
        ("x.md", Icon::Markdown),
        ("x.json", Icon::Json),
        ("x.yml", Icon::Yaml),
        ("x.yaml", Icon::Yaml),
        ("x.toml", Icon::Toml),
        ("x.html", Icon::Html),
        ("x.sh", Icon::Shell),
        ("x.png", Icon::Image),
        ("x.zip", Icon::Archive),
        ("x.cpp", Icon::Text),
        ("x.h", Icon::Text),
        ("x.exe", Icon::Binary),
        ("x.csv", Icon::Csv),
    ];
    for (name, icon) in cases {
        assert_eq!(icon_for_entry(name, false), icon, "{name}");
    }
}

#[test]
fn matching_is_case_insensitive_and_uses_the_last_extension() {
    assert_eq!(icon_for_entry("PHOTO.JPG", false), Icon::Image);
    assert_eq!(icon_for_entry("a.tar.gz", false), Icon::Archive);
}

#[test]
fn exact_file_names_win_over_extensions() {
    assert_eq!(icon_for_entry("Dockerfile", false), Icon::Docker);
    assert_eq!(icon_for_entry("docker-compose.yml", false), Icon::Docker);
    assert_eq!(icon_for_entry("Cargo.toml", false), Icon::Toml);
    assert_eq!(icon_for_entry("build.gradle", false), Icon::Gradle);
    assert_eq!(icon_for_entry(".gitignore", false), Icon::GitIgnore);
    assert_eq!(icon_for_entry(".editorconfig", false), Icon::EditorConfig);
}

#[test]
fn names_without_a_known_extension_are_a_plain_file() {
    for name in ["README", "weird.zzz", "trailingdot.", ".hidden", ""] {
        assert_eq!(icon_for_entry(name, false), Icon::File, "{name:?}");
    }
}

#[test]
fn directories_ignore_extensions() {
    assert_eq!(icon_for_entry("src", true), Icon::Folder);
    assert_eq!(icon_for_entry("photos.jpg", true), Icon::Folder);
}

#[test]
fn every_icon_has_a_nonempty_svg() {
    for icon in Icon::ALL {
        let text = std::str::from_utf8(bytes(*icon)).unwrap().trim();
        assert!(text.starts_with('<') && text.contains("<svg"), "{icon:?}");
    }
}

#[test]
fn all_lists_the_icons_in_declaration_order() {
    for (index, icon) in Icon::ALL.iter().enumerate() {
        assert_eq!(*icon as usize, index, "{icon:?}");
    }
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn icons_for_a_dark_background_are_the_vendored_files_untouched() {
    for icon in Icon::ALL {
        assert_eq!(recolored(*icon, false).as_ref(), bytes(*icon), "{icon:?}");
    }
}

#[test]
fn icons_for_a_light_background_lose_the_pale_greys_made_for_a_dark_one() {
    for icon in Icon::ALL {
        let text = text_of(&recolored(*icon, true));
        for pale in ["#CED0D6", "#B4B8BF"] {
            assert!(!text.contains(pale), "{icon:?} still has {pale}");
        }
    }
}

#[test]
fn light_icons_get_dark_greys_that_stand_out_on_a_light_background() {
    let ground = porthmos_core::theme::ColorScheme::light().bg;

    for (_, replacement) in LIGHT_SWAPS.iter().filter(|(from, _)| ["#CED0D6", "#B4B8BF"].contains(from)) {
        let colour = porthmos_core::theme::Rgb::parse(replacement).unwrap();
        assert!(colour.contrast(ground) >= 4.5, "{replacement}");
    }
}

#[test]
fn light_icons_swap_the_dark_panel_fills_for_pale_ones() {
    for (from, to) in LIGHT_SWAPS {
        let (before, after) =
            (porthmos_core::theme::Rgb::parse(from).unwrap(), porthmos_core::theme::Rgb::parse(to).unwrap());
        let white = porthmos_core::theme::Rgb(255, 255, 255);
        let panel = ["#43454A", "#25324D", "#1E1F22", "#402929", "#2F2936", "#253627", "#45322B"].contains(from);
        if panel {
            assert!(after.contrast(white) < before.contrast(white), "{from}");
            assert!(after.contrast(white) < 1.6, "{from} -> {to}");
        }
    }
}

#[test]
fn every_swap_replaces_a_whole_hex_colour() {
    for (from, to) in LIGHT_SWAPS {
        assert_eq!((from.len(), to.len()), (7, 7));
        assert!(from.starts_with('#') && to.starts_with('#'));
        assert_eq!(*from, from.to_ascii_uppercase());
    }
}

#[test]
fn recolouring_keeps_the_svg_otherwise_intact() {
    for icon in Icon::ALL {
        let original = text_of(bytes(*icon));
        let light = text_of(&recolored(*icon, true));
        assert_eq!(original.len(), light.len(), "{icon:?}");
        assert!(light.starts_with(original.split('<').next().unwrap()));
        assert_eq!(light.matches("<path").count(), original.matches("<path").count());
    }
}

#[test]
fn handles_exist_for_both_backgrounds() {
    for icon in Icon::ALL {
        let _ = handle(*icon, false);
        let _ = handle(*icon, true);
    }
}
