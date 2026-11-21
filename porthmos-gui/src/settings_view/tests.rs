use porthmos_core::UpdateMode;

use super::{GuiSettings, SettingsState, editor_from_text, search_depth_from_text, search_results_from_text};

fn settings(editor: Option<&str>) -> GuiSettings {
    GuiSettings {
        editor: editor.map(str::to_string),
        terminal: None,
        auto_upload: true,
        compare_limit_mib: 1,
        update_mode: UpdateMode::Notify,
        search_preview: false,
        search_max_depth: 16,
        search_max_results: 1000,
        reopen: porthmos_core::ReopenMode::Off,
        reconnect: false,
        theme_base: porthmos_core::theme::ThemeBase::Dark,
        language: None,
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
            SettingsSection::Language,
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
    fn general_holds_the_language_startup_connection_and_search_options() {
        assert_eq!(
            SettingsCategory::General.sections(),
            &[
                SettingsSection::Language,
                SettingsSection::Startup,
                SettingsSection::Connections,
                SettingsSection::Search
            ]
        );
    }
}

#[test]
fn the_search_limit_fields_start_from_the_saved_values() {
    let state = SettingsState::new(&GuiSettings { search_max_depth: 7, search_max_results: 90, ..settings(None) });

    assert_eq!((state.search_depth_text.as_str(), state.search_results_text.as_str()), ("7", "90"));
}

#[test]
fn search_depth_text_accepts_one_to_sixty_four() {
    assert_eq!(search_depth_from_text(" 1 "), Some(1));
    assert_eq!(search_depth_from_text("64"), Some(64));
    for text in ["", "0", "65", "-1", "1.5", "deep"] {
        assert_eq!(search_depth_from_text(text), None, "{text:?}");
    }
}

#[test]
fn search_results_text_accepts_ten_to_ten_thousand() {
    assert_eq!(search_results_from_text(" 10 "), Some(10));
    assert_eq!(search_results_from_text("10000"), Some(10000));
    for text in ["", "9", "10001", "-10", "1e3", "many"] {
        assert_eq!(search_results_from_text(text), None, "{text:?}");
    }
}

#[test]
fn the_search_limit_defaults_match_the_core_defaults() {
    let defaults = GuiSettings::default();
    let core = porthmos_core::config::SearchSettings::default();

    assert_eq!((defaults.search_max_depth, defaults.search_max_results), (core.max_depth, core.max_results));
}
