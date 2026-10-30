use porthmos_core::UpdateMode;

use super::{Edit, apply, save};

const FIXTURE: &str = "# my settings\n\n[panel]\nshow_hidden = true # keep\n\n[keys.global]\nquit = \"ctrl+q\"\n";

fn set_key(action: &str, chords: &[&str]) -> Edit {
    Edit::SetKey { action: action.to_string(), chords: chords.iter().map(|chord| chord.to_string()).collect() }
}

fn reset_key(action: &str) -> Edit {
    Edit::ResetKey { action: action.to_string() }
}

#[test]
fn set_key_creates_gui_keys_in_an_empty_file() {
    let written = apply("", &set_key("palette", &["ctrl+o"])).unwrap();

    assert_eq!(written, "[gui.keys]\npalette = \"ctrl+o\"\n");
}

#[test]
fn set_key_with_two_chords_writes_an_array() {
    let written = apply("", &set_key("move_down", &["down", "j"])).unwrap();

    assert_eq!(written, "[gui.keys]\nmove_down = [\"down\", \"j\"]\n");
}

#[test]
fn set_key_without_a_chord_is_refused() {
    let refused = apply(FIXTURE, &set_key("palette", &[])).unwrap_err();

    assert!(refused.contains("at least one"), "{refused}");
}

#[test]
fn set_key_keeps_comments_blank_lines_and_other_tables_byte_for_byte() {
    let written = apply(FIXTURE, &set_key("palette", &["ctrl+o"])).unwrap();

    assert!(written.starts_with(FIXTURE), "{written}");
    assert_eq!(written[FIXTURE.len()..].trim(), "[gui.keys]\npalette = \"ctrl+o\"");
}

#[test]
fn set_key_replaces_an_existing_value_in_place() {
    let before = "[gui]\n# keep this\nother = 1\n\n[gui.keys]\n# open it\npalette = \"ctrl+p\" # was the default\nquit = \"ctrl+q\"\n";

    let written = apply(before, &set_key("palette", &["ctrl+o"])).unwrap();

    assert_eq!(
        written,
        "[gui]\n# keep this\nother = 1\n\n[gui.keys]\n# open it\npalette = \"ctrl+o\" # was the default\nquit = \"ctrl+q\"\n"
    );
}

#[test]
fn set_key_adds_beside_the_keys_that_are_already_there() {
    let before = "[gui.keys]\nquit = \"ctrl+q\"\n";

    let written = apply(before, &set_key("palette", &["ctrl+o"])).unwrap();

    assert_eq!(written, "[gui.keys]\nquit = \"ctrl+q\"\npalette = \"ctrl+o\"\n");
}

#[test]
fn set_key_writes_into_an_inline_gui_table() {
    let written = apply("gui = { keys = { quit = \"ctrl+q\" } }\n", &set_key("palette", &["ctrl+o"])).unwrap();

    let parsed: toml::Table = written.parse().unwrap();
    let keys = parsed["gui"]["keys"].as_table().unwrap();
    assert_eq!(keys["palette"].as_str(), Some("ctrl+o"));
    assert_eq!(keys["quit"].as_str(), Some("ctrl+q"));
}

#[test]
fn reset_key_removes_it_and_prunes_empty_tables() {
    let written = apply("[gui.keys]\npalette = \"ctrl+o\"\n", &reset_key("palette")).unwrap();

    assert_eq!(written, "");
}

#[test]
fn reset_key_keeps_the_gui_table_when_it_holds_other_settings() {
    let before = "[gui]\ntheme = \"x\"\n\n[gui.keys]\npalette = \"ctrl+o\"\n";

    let written = apply(before, &reset_key("palette")).unwrap();

    assert_eq!(written.parse::<toml::Table>().unwrap(), "[gui]\ntheme = \"x\"\n".parse::<toml::Table>().unwrap());
    assert!(!written.contains("keys"), "{written}");
}

#[test]
fn reset_key_keeps_other_keys() {
    let before = "[gui.keys]\npalette = \"ctrl+o\"\nquit = \"ctrl+q\"\n";

    let written = apply(before, &reset_key("palette")).unwrap();

    assert_eq!(written, "[gui.keys]\nquit = \"ctrl+q\"\n");
}

#[test]
fn reset_key_on_a_missing_key_changes_nothing() {
    for before in ["", FIXTURE, "[gui]\n", "[gui.keys]\n", "[gui.keys]\nquit = \"ctrl+q\"\n"] {
        assert_eq!(apply(before, &reset_key("palette")).unwrap(), before);
    }
}

#[test]
fn set_editor_writes_and_none_removes_it() {
    let written = apply("", &Edit::SetEditor(Some("code --wait".to_string()))).unwrap();
    assert_eq!(written, "[edit]\neditor = \"code --wait\"\n");

    assert_eq!(apply(&written, &Edit::SetEditor(None)).unwrap(), "");

    let kept = apply("[edit]\nauto_upload = false\neditor = \"vi\"\n", &Edit::SetEditor(None)).unwrap();
    assert_eq!(kept, "[edit]\nauto_upload = false\n");
}

#[test]
fn a_blank_editor_removes_it() {
    let written = apply("[edit]\neditor = \"vi\"\n", &Edit::SetEditor(Some("   ".to_string()))).unwrap();

    assert_eq!(written, "");
}

#[test]
fn set_auto_upload_writes_a_bool() {
    assert_eq!(apply("", &Edit::SetAutoUpload(false)).unwrap(), "[edit]\nauto_upload = false\n");
    assert_eq!(
        apply("[edit]\nauto_upload = false\n", &Edit::SetAutoUpload(true)).unwrap(),
        "[edit]\nauto_upload = true\n"
    );
}

#[test]
fn set_update_mode_writes_each_mode() {
    for (mode, text) in [(UpdateMode::Auto, "auto"), (UpdateMode::Notify, "notify"), (UpdateMode::Off, "off")] {
        let written = apply("", &Edit::SetUpdateMode(mode)).unwrap();

        assert_eq!(written, format!("[update]\nmode = \"{text}\"\n"));
    }
}

#[test]
fn a_syntax_error_is_refused_with_its_position() {
    let refused = apply("[panel]\nshow_hidden = \n", &set_key("palette", &["ctrl+o"])).unwrap_err();

    assert!(refused.starts_with("config.toml has a syntax error (line 2, column "), "{refused}");
}

#[test]
fn a_table_that_is_not_a_table_is_refused() {
    let cases = [
        ("gui = 1\n", Edit::SetKey { action: "palette".to_string(), chords: vec!["ctrl+o".to_string()] }, "[gui]"),
        ("[gui]\nkeys = \"x\"\n", reset_key("palette"), "[gui.keys]"),
        ("[gui]\nkeys = \"x\"\n", set_key("palette", &["ctrl+o"]), "[gui.keys]"),
        ("edit = true\n", Edit::SetAutoUpload(false), "[edit]"),
        ("edit = true\n", Edit::SetEditor(None), "[edit]"),
        ("update = \"auto\"\n", Edit::SetUpdateMode(UpdateMode::Off), "[update]"),
    ];
    for (before, edit, table) in cases {
        let refused = apply(before, &edit).unwrap_err();

        assert_eq!(refused, format!("{table} is not a table"), "{before:?}");
    }
}

#[test]
fn save_writes_through_a_missing_file() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.toml");

    save(&path, &set_key("palette", &["ctrl+o"])).unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[gui.keys]\npalette = \"ctrl+o\"\n");
}

#[test]
fn save_creates_the_config_folder_when_it_is_missing() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("porthmos").join("config.toml");

    save(&path, &Edit::SetAutoUpload(false)).unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[edit]\nauto_upload = false\n");
}

#[test]
fn save_edits_an_existing_file_and_keeps_the_rest() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.toml");
    std::fs::write(&path, FIXTURE).unwrap();

    save(&path, &Edit::SetUpdateMode(UpdateMode::Notify)).unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.starts_with(FIXTURE), "{written}");
    assert!(written.contains("[update]\nmode = \"notify\""), "{written}");
}

#[test]
fn save_of_a_no_op_does_not_create_the_file() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.toml");

    save(&path, &reset_key("palette")).unwrap();

    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn save_keeps_the_permission_bits_of_an_existing_file() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.toml");
    std::fs::write(&path, FIXTURE).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    save(&path, &Edit::SetAutoUpload(false)).unwrap();

    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn save_gives_a_new_file_the_usual_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.toml");

    save(&path, &Edit::SetAutoUpload(false)).unwrap();

    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o644);
}

#[cfg(unix)]
#[test]
fn save_to_an_unwritable_folder_returns_a_message_and_writes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let locked = folder.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
    let path = locked.join("config.toml");
    let writable_anyway = std::fs::write(locked.join("probe"), b"").is_ok();

    let outcome = save(&path, &Edit::SetAutoUpload(false));

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    if writable_anyway {
        return;
    }
    let message = outcome.unwrap_err();
    assert!(message.contains("config.toml"), "{message}");
    assert!(!path.exists());
}

#[test]
fn save_refuses_a_broken_file_and_leaves_it_untouched() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.toml");
    let broken = "[panel\nshow_hidden = true\n";
    std::fs::write(&path, broken).unwrap();

    let message = save(&path, &Edit::SetAutoUpload(false)).unwrap_err();

    assert!(message.contains("syntax error"), "{message}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
}

#[test]
fn save_reports_an_unreadable_path_by_name() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.toml");
    std::fs::create_dir(&path).unwrap();

    let message = save(&path, &Edit::SetAutoUpload(false)).unwrap_err();

    assert!(message.contains("config.toml"), "{message}");
}
