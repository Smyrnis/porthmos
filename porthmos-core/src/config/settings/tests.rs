use super::*;
use crate::listing::{SortKey, SortOrder};

#[test]
fn settings_from_file_uses_defaults_when_panel_is_absent() {
    let (settings, warnings) = settings_from_file(SettingsFile::default());

    assert!(warnings.is_empty());
    assert!(!settings.panel.show_hidden);
    assert_eq!(settings.panel.sort.key, SortKey::Name);
    assert_eq!(settings.panel.sort.order, SortOrder::Ascending);
}

#[test]
fn settings_from_file_accepts_valid_panel_values() {
    let file = SettingsFile {
        panel: PanelSettingsFile {
            show_hidden: Some(true),
            sort_key: Some("size".to_string()),
            sort_order: Some("descending".to_string()),
        },
        frontend: toml::Table::new(),
        transfers: TransferSettingsFile::default(),
        edit: EditSettingsFile::default(),
        update: UpdateSettingsFile::default(),
    };

    let (settings, warnings) = settings_from_file(file);

    assert!(warnings.is_empty());
    assert!(settings.panel.show_hidden);
    assert_eq!(settings.panel.sort.key, SortKey::Size);
    assert_eq!(settings.panel.sort.order, SortOrder::Descending);
}

#[test]
fn settings_from_file_falls_back_and_warns_on_an_unknown_sort_key() {
    let file = SettingsFile {
        panel: PanelSettingsFile { show_hidden: None, sort_key: Some("date".to_string()), sort_order: None },
        frontend: toml::Table::new(),
        transfers: TransferSettingsFile::default(),
        edit: EditSettingsFile::default(),
        update: UpdateSettingsFile::default(),
    };

    let (settings, warnings) = settings_from_file(file);

    assert_eq!(settings.panel.sort.key, SortKey::Name);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("date"));
}

#[test]
fn settings_from_file_passes_keys_through_unvalidated() {
    let file: SettingsFile = toml::from_str("[keys]\nquit = \"ctrl+q\"\n[panel]\nshow_hidden = true\n").unwrap();

    let (settings, warnings) = settings_from_file(file);

    assert!(warnings.is_empty());
    assert!(settings.panel.show_hidden);
    assert_eq!(settings.frontend["keys"]["quit"].as_str(), Some("ctrl+q"));
}

#[test]
fn max_parallel_defaults_to_four() {
    let (settings, warnings) = settings_from_file(SettingsFile::default());

    assert!(warnings.is_empty());
    assert_eq!(settings.transfers.max_parallel, 4);
}

#[test]
fn max_parallel_uses_an_explicit_value() {
    let file = SettingsFile {
        transfers: TransferSettingsFile { max_parallel: Some(8), on_conflict: None },
        ..SettingsFile::default()
    };

    let (settings, warnings) = settings_from_file(file);

    assert!(warnings.is_empty());
    assert_eq!(settings.transfers.max_parallel, 8);
}

#[test]
fn max_parallel_below_one_falls_back_to_one_with_a_warning() {
    for value in [0, -3] {
        let file = SettingsFile {
            transfers: TransferSettingsFile { max_parallel: Some(value), on_conflict: None },
            ..SettingsFile::default()
        };

        let (settings, warnings) = settings_from_file(file);

        assert_eq!(settings.transfers.max_parallel, 1);
        assert_eq!(warnings, vec!["transfers.max_parallel must be at least 1, using 1".to_string()]);
    }
}

#[test]
fn negative_max_parallel_still_parses_the_rest_of_the_config() {
    let file: SettingsFile = toml::from_str("[panel]\nshow_hidden = true\n\n[transfers]\nmax_parallel = -2\n").unwrap();

    let (settings, _) = settings_from_file(file);

    assert!(settings.panel.show_hidden);
    assert_eq!(settings.transfers.max_parallel, 1);
}

#[test]
fn max_parallel_above_the_cap_is_limited_with_a_warning() {
    let file = SettingsFile {
        transfers: TransferSettingsFile { max_parallel: Some(500), on_conflict: None },
        ..SettingsFile::default()
    };

    let (settings, warnings) = settings_from_file(file);

    assert_eq!(settings.transfers.max_parallel, 16);
    assert_eq!(warnings, vec!["transfers.max_parallel is capped at 16, using 16".to_string()]);
}

#[test]
fn max_parallel_at_the_cap_is_accepted() {
    let file = SettingsFile {
        transfers: TransferSettingsFile { max_parallel: Some(16), on_conflict: None },
        ..SettingsFile::default()
    };

    let (settings, warnings) = settings_from_file(file);

    assert!(warnings.is_empty());
    assert_eq!(settings.transfers.max_parallel, 16);
}

#[test]
fn on_conflict_defaults_to_ask() {
    let (settings, warnings) = settings_from_file(SettingsFile::default());

    assert!(warnings.is_empty());
    assert_eq!(settings.transfers.on_conflict, ConflictPolicy::Ask);
}

#[test]
fn on_conflict_accepts_every_policy() {
    for (value, policy) in [
        ("ask", ConflictPolicy::Ask),
        ("overwrite", ConflictPolicy::Overwrite),
        ("skip", ConflictPolicy::Skip),
        ("rename", ConflictPolicy::Rename),
    ] {
        let file = SettingsFile {
            transfers: TransferSettingsFile { max_parallel: None, on_conflict: Some(value.to_string()) },
            ..SettingsFile::default()
        };

        let (settings, warnings) = settings_from_file(file);

        assert!(warnings.is_empty());
        assert_eq!(settings.transfers.on_conflict, policy);
    }
}

#[test]
fn an_unknown_on_conflict_falls_back_to_ask_with_a_warning() {
    let file = SettingsFile {
        transfers: TransferSettingsFile { max_parallel: None, on_conflict: Some("merge".to_string()) },
        ..SettingsFile::default()
    };

    let (settings, warnings) = settings_from_file(file);

    assert_eq!(settings.transfers.on_conflict, ConflictPolicy::Ask);
    assert_eq!(warnings, vec!["unknown transfers.on_conflict \"merge\", using \"ask\"".to_string()]);
}

#[test]
fn the_example_config_parses_without_warnings() {
    let file: SettingsFile = toml::from_str(include_str!("../../../../config/config.example.toml")).unwrap();

    let (settings, warnings) = settings_from_file(file);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(settings, Settings { frontend: settings.frontend.clone(), ..Settings::default() });
}

#[test]
fn edit_settings_default_to_auto_upload_and_no_editor() {
    let (settings, warnings) = settings_from_file(SettingsFile::default());

    assert!(warnings.is_empty());
    assert_eq!(settings.edit, EditSettings { auto_upload: true, editor: None });
}

#[test]
fn edit_settings_are_read_from_the_file() {
    let file: SettingsFile = toml::from_str("[edit]\nauto_upload = false\neditor = \"code --wait\"\n").unwrap();

    let (settings, warnings) = settings_from_file(file);

    assert!(warnings.is_empty());
    assert_eq!(settings.edit, EditSettings { auto_upload: false, editor: Some("code --wait".to_string()) });
}

#[test]
fn a_blank_editor_setting_means_unset() {
    let file: SettingsFile = toml::from_str("[edit]\neditor = \"   \"\n").unwrap();

    let (settings, _) = settings_from_file(file);

    assert_eq!(settings.edit.editor, None);
    assert!(settings.edit.auto_upload);
}

#[test]
fn an_edit_setting_of_the_wrong_type_does_not_parse() {
    assert!(toml::from_str::<SettingsFile>("[edit]\nauto_upload = \"yes\"\n").is_err());
    assert!(toml::from_str::<SettingsFile>("[edit]\neditor = 3\n").is_err());
}

fn parsed(text: &str) -> (Settings, Vec<String>) {
    settings_from_file(toml::from_str(text).unwrap())
}

#[test]
fn a_file_without_an_update_table_gets_the_defaults_and_no_warning() {
    let (settings, warnings) =
        parsed("[panel]\nshow_hidden = true\n[transfers]\nmax_parallel = 2\n[edit]\nauto_upload = false\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(settings.update, UpdateSettings { mode: UpdateMode::Auto, interval_hours: 24 });
}

#[test]
fn each_mode_is_read() {
    for (text, expected) in [("auto", UpdateMode::Auto), ("notify", UpdateMode::Notify), ("off", UpdateMode::Off)] {
        let (settings, warnings) = parsed(&format!("[update]\nmode = \"{text}\"\n"));

        assert!(warnings.is_empty(), "{text}: {warnings:?}");
        assert_eq!(settings.update.mode, expected, "{text}");
    }
}

#[test]
fn an_unknown_mode_warns_and_uses_auto() {
    let (settings, warnings) = parsed("[update]\nmode = \"sometimes\"\n");

    assert_eq!(settings.update.mode, UpdateMode::Auto);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("update.mode") && warnings[0].contains("sometimes"), "{warnings:?}");
}

#[test]
fn the_interval_is_read_within_one_to_720() {
    for hours in [1, 24, 168, 720] {
        let (settings, warnings) = parsed(&format!("[update]\ninterval_hours = {hours}\n"));

        assert!(warnings.is_empty(), "{hours}: {warnings:?}");
        assert_eq!(settings.update.interval_hours, hours as u64);
    }
}

#[test]
fn an_interval_below_one_or_above_720_warns_and_uses_24() {
    for hours in [-5, 0, 721, 100_000] {
        let (settings, warnings) = parsed(&format!("[update]\ninterval_hours = {hours}\n"));

        assert_eq!(settings.update.interval_hours, 24, "{hours}");
        assert_eq!(warnings.len(), 1, "{hours}: {warnings:?}");
        assert!(warnings[0].contains("update.interval_hours"), "{warnings:?}");
    }
}

#[test]
fn the_update_table_does_not_leak_into_the_frontend_settings() {
    let (settings, _) = parsed("[update]\nmode = \"off\"\n[keys]\nquit = \"ctrl+q\"\n");

    assert!(settings.frontend.contains_key("keys"));
    assert!(!settings.frontend.contains_key("update"));
}
