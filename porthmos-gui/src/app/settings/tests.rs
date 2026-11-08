use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::UpdateMode;

use crate::settings_view::{GuiSettings, SettingsMessage};
use crate::shortcuts::{Action, KeyPress};
use crate::testing::Harness;
use crate::windows::WindowKind;

use super::super::*;

const RESTART_NOTE: &str = "Saved. Takes effect the next time Porthmos starts.";
const APPLIED_NOTE: &str = "Saved.";

fn press(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    let text = match &key {
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    };
    let window = harness.settings_id().unwrap_or(harness.main_id());
    harness.press(Message::WindowKey(window, KeyPress { key: key.clone(), modified_key: key, modifiers, text }));
}

fn named(harness: &mut Harness, key: Named) {
    press(harness, Key::Named(key), Modifiers::empty());
}

fn ctrl(harness: &mut Harness, character: &str) {
    press(harness, Key::Character(character.into()), Modifiers::CTRL);
}

fn open(harness: &mut Harness) {
    harness.press(Message::ActionRequested(Action::Settings));
    harness.sent();
}

fn state(harness: &Harness) -> &crate::settings_view::SettingsState {
    harness.app.settings_state().expect("the settings are open")
}

fn config_path(harness: &Harness) -> std::path::PathBuf {
    harness.scratch().join("config.toml")
}

fn config_text(harness: &Harness) -> String {
    std::fs::read_to_string(config_path(harness)).unwrap_or_default()
}

fn error_of(harness: &Harness) -> String {
    match &state(harness).message {
        Some(SettingsMessage::Error(text)) => text.clone(),
        other => panic!("expected an error, got {other:?}"),
    }
}

fn lookup(harness: &Harness, key: Key, modifiers: Modifiers) -> Option<Action> {
    harness.app.shortcuts().action_for(&key, modifiers)
}

fn character(text: &str) -> Key {
    Key::Character(text.into())
}

#[test]
fn the_toolbar_button_and_ctrl_comma_open_the_settings() {
    let mut harness = Harness::new();

    ctrl(&mut harness, ",");
    assert!(harness.app.settings_open());
    ctrl(&mut harness, ",");
    assert!(harness.app.tool().is_none());

    harness.press(Message::ActionRequested(Action::Settings));
    assert!(harness.app.settings_open());
    assert_eq!(state(&harness).capturing, None);
}

#[test]
fn settings_is_ignored_behind_a_modal() {
    let mut harness = Harness::new();
    named(&mut harness, Named::F7);

    ctrl(&mut harness, ",");
    harness.press(Message::ActionRequested(Action::Settings));

    assert!(harness.app.tool().is_none());
}

#[test]
fn opening_shows_the_saved_editor() {
    let settings = GuiSettings {
        editor: Some("vi".to_string()),
        terminal: None,
        auto_upload: false,
        search_preview: false,
        update_mode: UpdateMode::Off,
        reopen: porthmos_core::ReopenMode::Off,
    };
    let mut harness = Harness::with_gui_settings(settings);

    open(&mut harness);

    assert_eq!(state(&harness).editor_text, "vi");
    assert!(!harness.app.gui_settings().auto_upload);
}

#[test]
fn change_then_a_key_assigns_it_and_writes_gui_keys() {
    let mut harness = Harness::new();
    open(&mut harness);

    harness.press(Message::SettingsChangePressed(Action::Refresh));
    assert_eq!(state(&harness).capturing, Some(Action::Refresh));
    ctrl(&mut harness, "o");

    assert_eq!(config_text(&harness), "[gui.keys]\nrefresh = \"ctrl+o\"\n");
    assert_eq!(lookup(&harness, character("o"), Modifiers::CTRL), Some(Action::Refresh));
    assert_eq!(lookup(&harness, character("r"), Modifiers::CTRL), None);
    assert_eq!(state(&harness).capturing, None);
    assert_eq!(state(&harness).message, None);
}

#[test]
fn change_refuses_a_reserved_chord_and_writes_nothing() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));

    press(&mut harness, Key::Named(Named::F4), Modifiers::ALT);

    assert!(error_of(&harness).contains("reserved"), "{}", error_of(&harness));
    assert!(!config_path(&harness).exists());
    assert_eq!(state(&harness).capturing, Some(Action::Refresh));
    assert_eq!(lookup(&harness, character("r"), Modifiers::CTRL), Some(Action::Refresh));
}

#[test]
fn change_refuses_a_used_chord_and_names_the_action() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));

    ctrl(&mut harness, "f");

    assert_eq!(error_of(&harness), "Already used by Search");
    assert!(!config_path(&harness).exists());
    assert_eq!(lookup(&harness, character("f"), Modifiers::CTRL), Some(Action::Search));
    assert_eq!(state(&harness).capturing, Some(Action::Refresh));
}

#[test]
fn a_refusal_clears_when_the_next_key_is_accepted() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));
    ctrl(&mut harness, "f");

    ctrl(&mut harness, "o");

    assert_eq!(state(&harness).message, None);
    assert_eq!(state(&harness).capturing, None);
}

#[test]
fn escape_cancels_capture_and_a_second_escape_closes_the_overlay() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));

    named(&mut harness, Named::Escape);
    assert!(harness.app.settings_open());
    assert_eq!(state(&harness).capturing, None);
    assert!(!config_path(&harness).exists());

    named(&mut harness, Named::Escape);
    assert!(harness.app.tool().is_none());
}

#[test]
fn a_modifier_only_press_keeps_capturing() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));

    press(&mut harness, Key::Named(Named::Control), Modifiers::CTRL);
    press(&mut harness, Key::Named(Named::Shift), Modifiers::SHIFT);

    assert_eq!(state(&harness).capturing, Some(Action::Refresh));
    assert_eq!(state(&harness).message, None);
    assert!(!config_path(&harness).exists());
}

#[test]
fn reset_restores_the_defaults_and_removes_the_key() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));
    ctrl(&mut harness, "o");
    assert!(config_text(&harness).contains("refresh"));

    harness.press(Message::SettingsResetPressed(Action::Refresh));

    assert_eq!(config_text(&harness), "");
    assert_eq!(lookup(&harness, character("r"), Modifiers::CTRL), Some(Action::Refresh));
    assert_eq!(lookup(&harness, character("o"), Modifiers::CTRL), None);
}

#[cfg(unix)]
#[test]
fn a_failed_write_rolls_the_shortcut_back_and_shows_the_error() {
    use std::os::unix::fs::PermissionsExt;
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));
    let folder = harness.scratch().to_path_buf();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).unwrap();
    let writable_anyway = std::fs::write(folder.join("probe"), b"").is_ok();

    ctrl(&mut harness, "o");

    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    if writable_anyway {
        return;
    }
    assert!(error_of(&harness).contains("config.toml"), "{}", error_of(&harness));
    assert_eq!(lookup(&harness, character("r"), Modifiers::CTRL), Some(Action::Refresh));
    assert_eq!(lookup(&harness, character("o"), Modifiers::CTRL), None);
    assert!(!config_path(&harness).exists());
}

#[test]
fn a_config_with_a_syntax_error_is_refused_and_untouched() {
    let mut harness = Harness::new();
    let broken = "[panel\nshow_hidden = true\n";
    std::fs::write(config_path(&harness), broken).unwrap();
    open(&mut harness);
    harness.press(Message::SettingsChangePressed(Action::Refresh));

    ctrl(&mut harness, "o");

    assert!(error_of(&harness).contains("syntax error"), "{}", error_of(&harness));
    assert_eq!(config_text(&harness), broken);
    assert_eq!(lookup(&harness, character("r"), Modifiers::CTRL), Some(Action::Refresh));
}

#[test]
fn the_editor_applies_on_enter_and_on_the_button_and_writes_edit_editor() {
    let mut harness = Harness::new();
    open(&mut harness);

    harness.press(Message::SettingsEditorTyped("code --wait".to_string()));
    assert_eq!(config_text(&harness), "");
    harness.press(Message::SettingsEditorApplied);

    assert_eq!(config_text(&harness), "[edit]\neditor = \"code --wait\"\n");
    assert_eq!(harness.app.gui_settings().editor.as_deref(), Some("code --wait"));
    assert_eq!(state(&harness).message, Some(SettingsMessage::Note(RESTART_NOTE.to_string())));
}

#[test]
fn the_terminal_command_applies_and_writes_terminal_command() {
    let mut harness = Harness::new();
    open(&mut harness);

    harness.press(Message::SettingsTerminalTyped("  kitty --single-instance ".to_string()));
    harness.press(Message::SettingsTerminalApplied);

    assert_eq!(config_text(&harness), "[terminal]\ncommand = \"kitty --single-instance\"\n");
    assert_eq!(harness.app.gui_settings().terminal.as_deref(), Some("kitty --single-instance"));
}

#[test]
fn a_blank_terminal_command_removes_the_key() {
    let mut harness =
        Harness::with_gui_settings(GuiSettings { terminal: Some("xterm".to_string()), ..GuiSettings::default() });
    std::fs::write(config_path(&harness), "[terminal]\ncommand = \"xterm\"\n").unwrap();
    open(&mut harness);

    harness.press(Message::SettingsTerminalTyped("  ".to_string()));
    harness.press(Message::SettingsTerminalApplied);

    assert_eq!(config_text(&harness), "");
    assert_eq!(harness.app.gui_settings().terminal, None);
}

#[test]
fn a_saved_terminal_command_is_used_by_the_next_terminal_without_restarting() {
    let mut harness = Harness::new();
    harness.install_terminals(&["kitty", "xterm"]);
    open(&mut harness);
    harness.press(Message::SettingsTerminalTyped("kitty".to_string()));
    harness.press(Message::SettingsTerminalApplied);
    harness.launched();

    harness.feed(porthmos_core::Event::ShellReady {
        session: 3,
        invocation: porthmos_core::ShellInvocation {
            program: "ssh".into(),
            args: vec!["host".into()],
            env: Vec::new(),
        },
    });

    let jobs = harness.launched();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].spec.program, std::ffi::OsString::from("kitty"));
}

#[test]
fn applying_the_terminal_command_says_it_took_effect_at_once() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsTerminalTyped("kitty".to_string()));

    harness.press(Message::SettingsTerminalApplied);

    assert_eq!(state(&harness).message, Some(SettingsMessage::Note(APPLIED_NOTE.to_string())));
}

#[test]
fn a_blank_editor_removes_the_key() {
    let mut harness =
        Harness::with_gui_settings(GuiSettings { editor: Some("vi".to_string()), ..GuiSettings::default() });
    std::fs::write(config_path(&harness), "[edit]\neditor = \"vi\"\n").unwrap();
    open(&mut harness);

    harness.press(Message::SettingsEditorTyped("   ".to_string()));
    harness.press(Message::SettingsEditorApplied);

    assert_eq!(config_text(&harness), "");
    assert_eq!(harness.app.gui_settings().editor, None);
}

#[test]
fn the_checkbox_writes_auto_upload() {
    let mut harness = Harness::new();
    open(&mut harness);

    harness.press(Message::SettingsAutoUploadToggled(false));

    assert_eq!(config_text(&harness), "[edit]\nauto_upload = false\n");
    assert!(!harness.app.gui_settings().auto_upload);
}

#[test]
fn each_update_mode_choice_writes_update_mode() {
    for (mode, text) in [(UpdateMode::Notify, "notify"), (UpdateMode::Off, "off"), (UpdateMode::Auto, "auto")] {
        let mut harness = Harness::new();
        open(&mut harness);

        harness.press(Message::SettingsUpdateModeChosen(mode));

        assert_eq!(config_text(&harness), format!("[update]\nmode = \"{text}\"\n"));
        assert_eq!(harness.app.gui_settings().update_mode, mode);
    }
}

#[test]
fn a_failed_core_owned_save_keeps_the_old_value() {
    let mut harness = Harness::new();
    std::fs::write(config_path(&harness), "[panel\n").unwrap();
    open(&mut harness);

    harness.press(Message::SettingsAutoUploadToggled(false));
    harness.press(Message::SettingsUpdateModeChosen(UpdateMode::Off));

    assert!(harness.app.gui_settings().auto_upload);
    assert_eq!(harness.app.gui_settings().update_mode, UpdateMode::Auto);
    assert!(error_of(&harness).contains("syntax error"));
}

#[test]
fn core_owned_changes_show_the_restart_note_and_shortcut_changes_do_not() {
    let mut harness = Harness::new();
    open(&mut harness);

    harness.press(Message::SettingsUpdateModeChosen(UpdateMode::Notify));
    assert_eq!(state(&harness).message, Some(SettingsMessage::Note(RESTART_NOTE.to_string())));

    harness.press(Message::SettingsChangePressed(Action::Refresh));
    ctrl(&mut harness, "o");
    assert_eq!(state(&harness).message, None);
}

#[test]
fn typing_in_the_editor_field_does_not_fire_panel_shortcuts() {
    let mut harness = Harness::new();
    open(&mut harness);
    let cursor = harness.app.local().panel.cursor();

    for key in ["j", "k", "/", "e", "p", "a"] {
        press(&mut harness, Key::Character(key.into()), Modifiers::empty());
    }
    named(&mut harness, Named::Space);
    named(&mut harness, Named::Delete);
    named(&mut harness, Named::F7);
    named(&mut harness, Named::F5);
    named(&mut harness, Named::Enter);

    assert!(harness.app.settings_open());
    assert!(harness.app.modal().is_none());
    assert_eq!(harness.app.local().panel.cursor(), cursor);
    assert!(harness.sent().is_empty());
    assert!(!config_path(&harness).exists());
}

#[test]
fn the_overlay_closes_with_the_button_and_escape() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsClosed);
    assert!(!harness.app.settings_open());

    open(&mut harness);
    named(&mut harness, Named::Escape);
    assert!(!harness.app.settings_open());
}

#[test]
fn the_palette_lists_settings_and_runs_it() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Palette));
    harness.press(Message::PaletteTyped(">settings".to_string()));

    named(&mut harness, Named::Enter);

    assert!(harness.app.settings_open());
}

#[test]
fn settings_can_be_rebound_by_name() {
    let defaults = crate::shortcuts::Shortcuts::default();

    assert_eq!(defaults.label_for(Action::Settings).as_deref(), Some("Ctrl+,"));
    assert_eq!(Action::Settings.name(), "settings");
}

#[test]
fn the_search_preview_checkbox_writes_the_setting_and_applies_it_at_once() {
    let mut harness = Harness::new();
    open(&mut harness);
    assert!(!harness.app.gui_settings().search_preview);

    harness.press(Message::SettingsSearchPreviewToggled(true));

    assert_eq!(config_text(&harness), "[search]\npreview = true\n");
    assert!(harness.app.gui_settings().search_preview);
    assert_eq!(state(&harness).message, Some(SettingsMessage::Note(APPLIED_NOTE.to_string())));
}

#[test]
fn a_failed_save_leaves_the_search_preview_setting_unchanged() {
    let mut harness = Harness::new();
    std::fs::write(config_path(&harness), "[search\n").unwrap();
    open(&mut harness);

    harness.press(Message::SettingsSearchPreviewToggled(true));

    assert!(!harness.app.gui_settings().search_preview);
    assert!(matches!(state(&harness).message, Some(SettingsMessage::Error(_))));
}

fn settings_windows(harness: &Harness) -> Option<iced::window::Id> {
    harness.app.window_id_of(WindowKind::Settings)
}

#[test]
fn opening_settings_registers_one_settings_window_and_creates_its_state() {
    let mut harness = Harness::new();

    open(&mut harness);

    assert!(harness.app.settings_open());
    assert!(settings_windows(&harness).is_some());
    assert_eq!(harness.settings_id(), settings_windows(&harness));
}

#[test]
fn opening_settings_twice_keeps_one_window() {
    let mut harness = Harness::new();
    open(&mut harness);
    let first = settings_windows(&harness);

    open(&mut harness);

    assert_eq!(settings_windows(&harness), first);
    assert!(harness.app.settings_open());
}

#[test]
fn settings_open_beside_another_tool_without_replacing_it() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Search));
    assert!(harness.app.search_state().is_some());

    open(&mut harness);

    assert!(harness.app.settings_open());
    assert!(harness.app.search_state().is_some());
}

#[test]
fn esc_in_the_settings_window_closes_only_that_window() {
    let mut harness = Harness::new();
    harness.press(Message::PathEditStarted(PanelSide::Local));
    open(&mut harness);

    named(&mut harness, Named::Escape);

    assert!(!harness.app.settings_open());
    assert_eq!(settings_windows(&harness), None);
    assert!(harness.app.path_edit().is_some());
    assert!(!harness.app.is_closing());
}

#[test]
fn the_close_request_of_the_settings_window_clears_its_state_and_leaves_the_main_window() {
    let mut harness = Harness::new();
    open(&mut harness);
    let id = harness.settings_id().unwrap();

    harness.press(Message::WindowCloseRequested(id));

    assert!(!harness.app.settings_open());
    assert_eq!(settings_windows(&harness), None);
    assert!(!harness.app.is_closing());
    assert!(harness.app.is_main(harness.main_id()));
}

#[test]
fn reopening_settings_starts_from_the_saved_values_not_the_old_draft() {
    let mut harness = Harness::new();
    open(&mut harness);
    harness.press(Message::SettingsEditorTyped("half typed".to_string()));
    named(&mut harness, Named::Escape);

    open(&mut harness);

    assert_eq!(state(&harness).editor_text, "");
}

#[test]
fn a_key_in_the_settings_window_does_not_run_main_shortcuts() {
    let mut harness = Harness::new();
    open(&mut harness);
    let cursor = harness.app.local().panel.cursor();
    harness.sent();

    named(&mut harness, Named::Delete);
    named(&mut harness, Named::F7);
    named(&mut harness, Named::F5);
    named(&mut harness, Named::Enter);

    assert!(harness.app.settings_open());
    assert!(harness.app.modal().is_none());
    assert_eq!(harness.app.local().panel.cursor(), cursor);
    assert!(harness.sent().is_empty());
}

#[test]
fn a_key_in_the_main_window_does_not_reach_the_settings() {
    let mut harness = Harness::new();
    open(&mut harness);
    let main = harness.main_id();

    harness.press(Message::WindowKey(
        main,
        KeyPress {
            key: Key::Named(Named::Escape),
            modified_key: Key::Named(Named::Escape),
            modifiers: Modifiers::empty(),
            text: None,
        },
    ));

    assert!(harness.app.settings_open());
}

#[test]
fn resizing_the_settings_window_changes_only_its_own_size() {
    let mut harness = Harness::new();
    open(&mut harness);
    let id = harness.settings_id().unwrap();
    let main_before = harness.app.window_size();

    harness.press(Message::WindowSized(id, iced::Size::new(700.0, 480.0)));

    assert_eq!(harness.app.settings_window_size(), iced::Size::new(700.0, 480.0));
    assert_eq!(harness.app.window_size(), main_before);
}

#[test]
fn the_settings_window_builds_its_view() {
    let mut harness = Harness::new();
    open(&mut harness);
    let id = harness.settings_id().unwrap();

    drop(harness.app.view_of(id));
}

#[test]
fn opening_settings_a_second_time_raises_the_window_with_one_task() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));

    let units = harness.app.update(Message::ActionRequested(Action::Settings)).units();

    assert_eq!(units, 1);
}

#[test]
fn settings_open_behind_a_notice_is_still_raised() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));
    harness.app.modals.push(crate::modal::Modal::Info { title: "t".to_string(), body: "b".to_string() });

    let units = harness.app.open_settings().units();

    assert_eq!(units, 1);
}

#[test]
fn settings_are_not_opened_while_a_modal_is_up() {
    let mut harness = Harness::new();
    harness.app.modals.push(crate::modal::Modal::Info { title: "t".to_string(), body: "b".to_string() });

    let units = harness.app.open_settings().units();

    assert_eq!(units, 0);
    assert!(harness.settings_id().is_none());
}

#[test]
fn the_reopen_choice_writes_the_setting_tells_core_and_applies_at_once() {
    use porthmos_core::ReopenMode;
    for (mode, name) in [(ReopenMode::Ask, "ask"), (ReopenMode::Auto, "auto"), (ReopenMode::Off, "off")] {
        let mut harness = Harness::new();
        open(&mut harness);

        harness.press(Message::SettingsReopenChosen(mode));

        assert_eq!(config_text(&harness), format!("[startup]\nreopen = \"{name}\"\n"));
        assert_eq!(harness.app.gui_settings().reopen, mode);
        assert!(harness.sent().contains(&porthmos_core::Command::SetReopenMode { mode }));
        assert_eq!(state(&harness).message, Some(SettingsMessage::Note(APPLIED_NOTE.to_string())));
    }
}

#[test]
fn a_failed_save_leaves_the_reopen_choice_unchanged_and_tells_core_nothing() {
    use porthmos_core::ReopenMode;
    let mut harness = Harness::new();
    std::fs::write(config_path(&harness), "[startup\n").unwrap();
    open(&mut harness);

    harness.press(Message::SettingsReopenChosen(ReopenMode::Auto));

    assert_eq!(harness.app.gui_settings().reopen, ReopenMode::Off);
    assert!(!harness.sent().iter().any(|command| matches!(command, porthmos_core::Command::SetReopenMode { .. })));
    assert!(matches!(state(&harness).message, Some(SettingsMessage::Error(_))));
}

#[test]
fn turning_reopening_off_withdraws_a_pending_offer() {
    use porthmos_core::{Event, ReopenMode};
    let mut harness = Harness::new();
    open(&mut harness);
    harness.feed(Event::ReopenOffer { names: vec!["prod".to_string()] });

    harness.press(Message::SettingsReopenChosen(ReopenMode::Off));

    assert!(harness.app.reopen_offer().is_empty());
}

#[test]
fn reopening_is_off_unless_the_user_chose_otherwise() {
    assert_eq!(GuiSettings::default().reopen, porthmos_core::ReopenMode::Off);
}
