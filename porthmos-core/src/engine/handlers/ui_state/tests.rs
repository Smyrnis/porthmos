use std::sync::Arc;

use porthmos_vfs::VfsPath;

use crate::{
    Location, Severity,
    config::settings::PanelSettings,
    engine::{
        Command, Event, Internal,
        testing::{test_engine, test_engine_with_secrets},
    },
    listing::{SortKey, SortOrder, SortSpec},
    panel_memory::PanelKind,
    secrets::TestBackend,
};

fn local_folder_shown(path: &str) -> Internal {
    Internal::FolderShown { location: Location::Local, path: VfsPath::new(path) }
}

fn saved_local_folder(t: &crate::engine::testing::TestEngine) -> Option<String> {
    crate::state::load(&t.engine.paths).state.local_folder
}

fn choices(events: &[Event]) -> Vec<bool> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::SaveChoice { save } => Some(*save),
            _ => None,
        })
        .collect()
}

#[test]
fn the_save_choice_is_published_and_remembered() {
    let backend = Arc::new(TestBackend::new());
    let mut t = test_engine_with_secrets(backend);

    t.engine.load_ui_state();
    t.engine.handle_command(Command::RememberSaveChoice { save: true });
    t.engine.load_ui_state();
    t.engine.handle_command(Command::RememberSaveChoice { save: false });
    t.engine.load_ui_state();

    assert_eq!(choices(&t.drain()), vec![false, true, false]);
}

#[test]
fn a_choice_that_cannot_be_written_is_a_warning() {
    let backend = Arc::new(TestBackend::new());
    let mut t = test_engine_with_secrets(backend);
    std::fs::remove_dir_all(&t.engine.paths.state_dir).ok();
    std::fs::create_dir_all(t.engine.paths.state_dir.parent().unwrap()).unwrap();
    std::fs::write(&t.engine.paths.state_dir, b"a file").unwrap();

    t.engine.handle_command(Command::RememberSaveChoice { save: true });

    assert!(matches!(t.drain().as_slice(), [Event::Notice { severity: Severity::Warning, .. }]));
}

#[test]
fn a_corrupt_state_file_is_announced_at_start_and_the_choice_defaults_to_off() {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.state_dir).unwrap();
    std::fs::write(t.engine.paths.state_file(), "not toml [").unwrap();

    t.engine.load_ui_state();

    let events = t.drain();
    assert!(events.iter().any(|event| matches!(event, Event::SaveChoice { save: false })));
    assert!(events.iter().any(|event| matches!(event, Event::Notice { severity: Severity::Warning, message } if message.to_string().contains("state.toml.broken"))));
}

#[test]
fn a_protected_state_file_is_never_overwritten_by_remembering_the_choice() {
    let mut t = test_engine();
    t.engine.state_protected = true;

    t.engine.remember_save_choice(true);

    assert!(!t.engine.paths.state_file().exists());
    let notice = t.first_notice().expect("a notice");
    assert_eq!(notice.0, Severity::Warning);
}

#[test]
fn the_local_folder_shown_is_remembered_in_the_state_file() {
    let mut t = test_engine();

    t.engine.handle_internal(local_folder_shown("/home/tester/docs"));

    assert_eq!(saved_local_folder(&t).as_deref(), Some("/home/tester/docs"));
    assert!(t.first_notice().is_none());
}

#[test]
fn remembering_the_local_folder_keeps_the_other_saved_choices() {
    let backend = Arc::new(TestBackend::new());
    let mut t = test_engine_with_secrets(backend);
    t.engine.handle_command(Command::RememberSaveChoice { save: true });

    t.engine.handle_internal(local_folder_shown("/x"));

    let state = crate::state::load(&t.engine.paths).state;
    assert!(state.save_passwords_in_keyring);
    assert_eq!(state.local_folder.as_deref(), Some("/x"));
}

#[test]
fn showing_the_same_local_folder_again_writes_nothing() {
    let mut t = test_engine();
    t.engine.handle_internal(local_folder_shown("/x"));
    std::fs::remove_file(t.engine.paths.state_file()).unwrap();

    t.engine.handle_internal(local_folder_shown("/x"));

    assert!(!t.engine.paths.state_file().exists());
}

#[test]
fn a_protected_state_file_is_left_alone_without_a_notice_when_the_local_folder_changes() {
    let mut t = test_engine();
    t.engine.state_protected = true;

    t.engine.handle_internal(local_folder_shown("/x"));

    assert!(!t.engine.paths.state_file().exists());
    assert!(t.first_notice().is_none());
}

#[test]
fn a_local_folder_that_cannot_be_written_warns_once() {
    let mut t = test_engine();
    std::fs::remove_dir_all(&t.engine.paths.state_dir).ok();
    std::fs::create_dir_all(t.engine.paths.state_dir.parent().unwrap()).unwrap();
    std::fs::write(&t.engine.paths.state_dir, b"a file").unwrap();

    t.engine.handle_internal(local_folder_shown("/a"));
    t.engine.handle_internal(local_folder_shown("/b"));

    let warnings: Vec<_> = t
        .drain()
        .into_iter()
        .filter(|event| matches!(event, Event::Notice { severity: Severity::Warning, .. }))
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn a_session_folder_does_not_touch_the_state_file() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");

    t.engine.handle_internal(Internal::FolderShown { location: Location::Session(session), path: VfsPath::new("/x") });

    assert!(!t.engine.paths.state_file().exists());
}

fn size_descending(show_hidden: bool) -> PanelSettings {
    PanelSettings { show_hidden, sort: SortSpec { key: SortKey::Size, order: SortOrder::Descending } }
}

#[test]
fn a_panels_choices_are_remembered_in_the_state_file() {
    let mut t = test_engine();

    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Local, settings: size_descending(true) });
    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Remote, settings: size_descending(false) });

    let state = crate::state::load(&t.engine.paths).state;
    let configured = PanelSettings::default();
    assert_eq!(state.local_panel.unwrap().applied_to(&configured), size_descending(true));
    assert_eq!(state.remote_panel.unwrap().applied_to(&configured), size_descending(false));
    assert!(t.first_notice().is_none());
}

#[test]
fn remembering_a_panel_keeps_the_folder_and_the_other_choices() {
    let mut t = test_engine();
    t.engine.handle_internal(local_folder_shown("/x"));

    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Local, settings: size_descending(true) });

    let state = crate::state::load(&t.engine.paths).state;
    assert_eq!(state.local_folder.as_deref(), Some("/x"));
    assert!(state.local_panel.is_some());
    assert!(state.remote_panel.is_none());
}

#[test]
fn a_panel_choice_that_cannot_be_saved_warns_once_together_with_the_folder() {
    let mut t = test_engine();
    std::fs::remove_dir_all(&t.engine.paths.state_dir).ok();
    std::fs::create_dir_all(t.engine.paths.state_dir.parent().unwrap()).unwrap();
    std::fs::write(&t.engine.paths.state_dir, b"a file").unwrap();

    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Local, settings: size_descending(true) });
    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Remote, settings: size_descending(true) });
    t.engine.handle_internal(local_folder_shown("/a"));

    let warnings = t
        .drain()
        .into_iter()
        .filter(|event| matches!(event, Event::Notice { severity: Severity::Warning, .. }))
        .count();
    assert_eq!(warnings, 1);
}

#[test]
fn a_protected_state_file_is_left_alone_when_a_panel_choice_changes() {
    let mut t = test_engine();
    t.engine.state_protected = true;

    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Local, settings: size_descending(true) });

    assert!(!t.engine.paths.state_file().exists());
    assert!(t.first_notice().is_none());
}

#[test]
fn the_same_panel_choice_again_writes_nothing() {
    let mut t = test_engine();
    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Local, settings: size_descending(true) });
    std::fs::remove_file(t.engine.paths.state_file()).unwrap();

    t.engine.handle_command(Command::RememberPanel { kind: PanelKind::Local, settings: size_descending(true) });

    assert!(!t.engine.paths.state_file().exists());
}

fn saved_open_groups(t: &crate::engine::testing::TestEngine) -> Vec<String> {
    crate::state::load(&t.engine.paths).state.open_groups
}

fn groups(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn the_open_groups_are_remembered_sorted_and_without_repeats() {
    let mut t = test_engine();

    t.engine.handle_command(Command::RememberOpenGroups { groups: groups(&["Work/Staging", "Home", "Work", "Home"]) });

    assert_eq!(saved_open_groups(&t), groups(&["Home", "Work", "Work/Staging"]));
}

#[test]
fn closing_every_group_forgets_them() {
    let mut t = test_engine();
    t.engine.handle_command(Command::RememberOpenGroups { groups: groups(&["Work"]) });

    t.engine.handle_command(Command::RememberOpenGroups { groups: Vec::new() });

    assert!(saved_open_groups(&t).is_empty());
}

#[test]
fn open_groups_sit_beside_the_other_remembered_state() {
    let mut t = test_engine();
    t.engine.handle_command(Command::RememberSaveChoice { save: true });

    t.engine.handle_command(Command::RememberOpenGroups { groups: groups(&["Work"]) });

    let state = crate::state::load(&t.engine.paths).state;
    assert!(state.save_passwords_in_keyring);
    assert_eq!(state.open_groups, groups(&["Work"]));
}

#[test]
fn a_protected_state_file_is_never_overwritten_by_remembering_open_groups() {
    let mut t = test_engine();
    t.engine.state_protected = true;

    t.engine.handle_command(Command::RememberOpenGroups { groups: groups(&["Work"]) });

    assert!(!t.engine.paths.state_file().exists());
}
