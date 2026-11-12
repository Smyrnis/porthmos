use porthmos_core::UpdateMode;

use super::{GuiSettings, SettingsState, editor_from_text};

fn settings(editor: Option<&str>) -> GuiSettings {
    GuiSettings {
        editor: editor.map(str::to_string),
        terminal: None,
        auto_upload: true,
        update_mode: UpdateMode::Notify,
        search_preview: false,
        reopen: porthmos_core::ReopenMode::Off,
        reconnect: false,
        theme_base: porthmos_core::theme::ThemeBase::Dark,
    }
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
    assert!(!defaults.search_preview);
}

#[test]
fn the_state_starts_with_the_saved_terminal_command() {
    let settings = GuiSettings { terminal: Some("kitty".to_string()), ..GuiSettings::default() };

    assert_eq!(SettingsState::new(&settings).terminal_text, "kitty");
    assert_eq!(SettingsState::new(&GuiSettings::default()).terminal_text, "");
    assert_eq!(GuiSettings::default().terminal, None);
}

mod categories {
    use super::super::{SettingsCategory, SettingsSection, SettingsState};
    use crate::settings_view::GuiSettings;

    #[test]
    fn the_categories_come_in_a_fixed_order_with_their_titles() {
        let titles: Vec<_> = SettingsCategory::ALL.iter().map(|category| category.title()).collect();

        assert_eq!(titles, vec!["General", "Editor and terminal", "Appearance", "Shortcuts", "Updates"]);
    }

    #[test]
    fn the_settings_open_on_the_first_category() {
        assert_eq!(SettingsState::new(&GuiSettings::default()).category, SettingsCategory::General);
    }

    #[test]
    fn every_section_belongs_to_exactly_one_category() {
        let all = [
            SettingsSection::Shortcuts,
            SettingsSection::Appearance,
            SettingsSection::Editing,
            SettingsSection::Terminal,
            SettingsSection::Search,
            SettingsSection::Startup,
            SettingsSection::Connections,
            SettingsSection::Updates,
        ];

        for section in all {
            let homes = SettingsCategory::ALL.iter().filter(|category| category.sections().contains(&section)).count();
            assert_eq!(homes, 1, "{section:?}");
        }
        let total: usize = SettingsCategory::ALL.iter().map(|category| category.sections().len()).sum();
        assert_eq!(total, all.len());
    }

    #[test]
    fn no_category_is_empty() {
        for category in SettingsCategory::ALL {
            assert!(!category.sections().is_empty(), "{category:?}");
        }
    }

    #[test]
    fn general_holds_the_startup_connection_and_search_options() {
        assert_eq!(
            SettingsCategory::General.sections(),
            &[SettingsSection::Startup, SettingsSection::Connections, SettingsSection::Search]
        );
    }
}
