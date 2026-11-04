use semver::Version;

use super::*;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

#[test]
fn ids_programs_and_letters_are_fixed() {
    assert_eq!((Product::Tui.id(), Product::Tui.program(), Product::Tui.tag_letter()), ("tui", "porthmos", 'T'));
    assert_eq!((Product::Gui.id(), Product::Gui.program(), Product::Gui.tag_letter()), ("gui", "porthmos-gui", 'G'));
}

#[test]
fn from_id_round_trips_and_rejects_unknown() {
    assert_eq!(Product::from_id("tui"), Some(Product::Tui));
    assert_eq!(Product::from_id("gui"), Some(Product::Gui));
    for unknown in ["", "TUI", "cli", "tui ", "porthmos"] {
        assert_eq!(Product::from_id(unknown), None, "{unknown:?}");
    }
}

#[test]
fn program_file_has_exe_only_on_windows() {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    assert_eq!(Product::Tui.program_file(), format!("porthmos{suffix}"));
    assert_eq!(Product::Gui.program_file(), format!("porthmos-gui{suffix}"));
}

#[test]
fn channel_of_is_preview_only_for_dev_and_dev_number() {
    for preview in ["2026.5.0-dev", "2026.5.0-dev.1", "2026.5.0-dev.12"] {
        assert_eq!(Channel::of(&version(preview)), Channel::Preview, "{preview}");
    }
    for stable in [
        "2026.5.0",
        "2026.5.0-rc.1",
        "2026.5.0-developer",
        "2026.5.0-dev-1",
        "2026.5.0-alpha.dev",
        "2026.5.0-dev.1.2",
        "2026.5.0-dev.x",
    ] {
        assert_eq!(Channel::of(&version(stable)), Channel::Stable, "{stable}");
    }
}

#[test]
fn channel_ids_are_stable_and_preview() {
    assert_eq!((Channel::Stable.id(), Channel::Preview.id()), ("stable", "preview"));
}

#[test]
fn each_product_and_channel_has_its_own_feed_url() {
    let urls = [
        (Product::Tui, "2026.5.0", "https://github.com/Smyrnis/porthmos/releases/download/tui-stable/latest.json"),
        (
            Product::Tui,
            "2026.5.0-dev.1",
            "https://github.com/Smyrnis/porthmos/releases/download/tui-preview/latest.json",
        ),
        (Product::Gui, "2026.5.0", "https://github.com/Smyrnis/porthmos/releases/download/gui-stable/latest.json"),
        (
            Product::Gui,
            "2026.5.0-dev.1",
            "https://github.com/Smyrnis/porthmos/releases/download/gui-preview/latest.json",
        ),
    ];
    for (product, running, expected) in urls {
        assert_eq!(manifest_url_for(product, &version(running)), expected, "{running}");
    }
    let unique: std::collections::HashSet<_> = urls.iter().map(|(_, _, url)| *url).collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn feed_release_joins_product_and_channel() {
    assert_eq!(feed_release(Product::Gui, Channel::Preview), "gui-preview");
    assert_eq!(feed_release(Product::Tui, Channel::Stable), "tui-stable");
}
