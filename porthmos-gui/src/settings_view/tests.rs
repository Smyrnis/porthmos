use porthmos_core::UpdateMode;

use super::{GuiSettings, SettingsState, editor_from_text};

fn settings(editor: Option<&str>) -> GuiSettings {
    GuiSettings { editor: editor.map(str::to_string), auto_upload: true, update_mode: UpdateMode::Notify }
}

#[test]
fn state_starts_from_the_settings() {
    let state = SettingsState::new(&settings(Some("code --wait")));

    assert_eq!(state.editor_text, "code --wait");
    assert_eq!(state.capturing, None);
    assert_eq!(state.message, None);
    assert_eq!(SettingsState::new(&settings(None)).editor_text, "");
}

#[test]
fn a_blank_editor_text_means_none() {
    assert_eq!(editor_from_text(""), None);
    assert_eq!(editor_from_text("   \t"), None);
    assert_eq!(editor_from_text("  code --wait "), Some("code --wait".to_string()));
}

#[test]
fn the_defaults_match_the_core_defaults() {
    let defaults = GuiSettings::default();

    assert_eq!(defaults.editor, None);
    assert!(defaults.auto_upload);
    assert_eq!(defaults.update_mode, UpdateMode::Auto);
}
