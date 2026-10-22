use std::sync::Arc;

use crate::{
    Severity,
    engine::{
        Command, Event,
        testing::{test_engine, test_engine_with_secrets},
    },
    secrets::TestBackend,
};

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
    assert!(events.iter().any(|event| matches!(event, Event::Notice { severity: Severity::Warning, message } if message.contains("state.toml.broken"))));
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
