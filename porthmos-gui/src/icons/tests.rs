use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons")
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
    assert!(!found.is_empty());
    for path in found {
        let relative = path.strip_prefix(assets()).unwrap().to_string_lossy().replace('\\', "/");
        assert!(sources.contains(&format!("| {relative} |")), "{relative} is not listed in SOURCES.md");
        let head: String = fs::read_to_string(&path).unwrap().lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(head.contains("Apache"), "{relative} has no Apache header");
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
