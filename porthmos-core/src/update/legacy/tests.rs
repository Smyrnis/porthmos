use std::fs;

use porthmos_update::Product;

use super::*;

fn old_state(paths: &Paths) {
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.state_dir.join("update.toml"), "last_checked = \"2026-10-01T00:00:00Z\"\n").unwrap();
    fs::write(paths.state_dir.join("update.toml.broken"), "not toml").unwrap();
    let root = paths.update_dir();
    fs::create_dir_all(root.join("0.4.0")).unwrap();
    fs::write(root.join("0.4.0").join("porthmos"), b"old binary").unwrap();
    fs::write(root.join("staged.toml"), b"version = \"0.4.0\"").unwrap();
}

#[test]
fn the_old_update_state_and_staged_files_are_removed() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    old_state(&paths);

    remove(&paths);

    assert!(!paths.state_dir.join("update.toml").exists());
    assert!(!paths.state_dir.join("update.toml.broken").exists());
    assert!(!paths.update_dir().join("staged.toml").exists());
    assert!(!paths.update_dir().join("0.4.0").exists());
}

#[test]
fn the_per_app_update_state_and_staged_files_are_kept() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    old_state(&paths);
    for product in [Product::Tui, Product::Gui] {
        fs::write(paths.update_state_file(product.id()), "last_failure = \"kept\"\n").unwrap();
        let folder = paths.update_dir().join(product.id());
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("staged.toml"), b"kept").unwrap();
    }
    fs::write(paths.update_state_file("tui").with_extension("toml.broken"), "kept").unwrap();

    remove(&paths);

    for product in [Product::Tui, Product::Gui] {
        assert!(paths.update_state_file(product.id()).exists(), "{product:?}");
        assert!(paths.update_dir().join(product.id()).join("staged.toml").exists(), "{product:?}");
    }
    assert!(paths.update_state_file("tui").with_extension("toml.broken").exists());
}

#[test]
fn removing_the_old_files_when_there_are_none_is_harmless() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());

    remove(&paths);

    assert!(!paths.update_dir().exists());
    assert!(!paths.state_dir.join("update.toml").exists());
}
