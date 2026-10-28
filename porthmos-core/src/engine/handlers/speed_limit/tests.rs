use std::time::Duration;

use porthmos_lfs::from_os_path;
use porthmos_vfs::VfsPath;

use crate::{
    Severity,
    engine::{
        Command, Event, Location,
        testing::{TestEngine, test_engine},
    },
    state,
};

fn limits(events: &[Event]) -> Vec<Option<u64>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::TransfersChanged(snapshot) => Some(snapshot.speed_limit),
            _ => None,
        })
        .collect()
}

fn set(t: &mut TestEngine, limit: Option<u64>) {
    t.engine.handle_command(Command::SetSpeedLimit { bytes_per_second: limit });
}

fn saved(t: &TestEngine) -> state::UiState {
    state::load(&t.engine.paths).state
}

#[test]
fn setting_a_limit_applies_it_publishes_it_and_remembers_it() {
    let mut t = test_engine();

    set(&mut t, Some(2_097_152));

    assert_eq!(t.engine.speed_limit.get(), Some(2_097_152));
    assert_eq!(limits(&t.drain()), [Some(2_097_152)]);
    assert_eq!(saved(&t).speed_limit, Some(2_097_152));
}

#[test]
fn turning_it_off_applies_publishes_and_forgets_it() {
    let mut t = test_engine();
    set(&mut t, Some(1000));
    t.drain();

    set(&mut t, None);

    assert_eq!(t.engine.speed_limit.get(), None);
    assert_eq!(limits(&t.drain()), [None]);
    assert_eq!(saved(&t).speed_limit, None);
    assert!(!std::fs::read_to_string(t.engine.paths.state_file()).unwrap().contains("speed_limit"));
}

#[test]
fn a_limit_of_zero_means_off() {
    let mut t = test_engine();
    set(&mut t, Some(1000));
    t.drain();

    set(&mut t, Some(0));

    assert_eq!(t.engine.speed_limit.get(), None);
    assert_eq!(saved(&t).speed_limit, None);
}

#[test]
fn setting_the_same_limit_again_publishes_nothing_new() {
    let mut t = test_engine();
    set(&mut t, Some(1000));
    t.drain();

    set(&mut t, Some(1000));

    assert!(limits(&t.drain()).is_empty());
}

#[test]
fn a_remembered_limit_is_applied_and_published_at_start() {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.state_dir).unwrap();
    std::fs::write(t.engine.paths.state_file(), "speed_limit = 524288\n").unwrap();

    t.engine.load_ui_state();

    assert_eq!(t.engine.speed_limit.get(), Some(524_288));
    assert_eq!(limits(&t.drain()), [Some(524_288)]);
}

#[test]
fn a_remembered_limit_of_zero_is_no_limit() {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.state_dir).unwrap();
    std::fs::write(t.engine.paths.state_file(), "speed_limit = 0\n").unwrap();

    t.engine.load_ui_state();

    assert_eq!(t.engine.speed_limit.get(), None);
    assert!(limits(&t.drain()).is_empty());
}

#[test]
fn no_remembered_limit_publishes_no_snapshot_at_start() {
    let mut t = test_engine();

    t.engine.load_ui_state();

    assert!(limits(&t.drain()).is_empty());
}

#[test]
fn remembering_the_keyring_choice_keeps_the_limit() {
    let mut t = test_engine();
    set(&mut t, Some(4096));

    t.engine.handle_command(Command::RememberSaveChoice { save: true });

    let state = saved(&t);
    assert!(state.save_passwords_in_keyring);
    assert_eq!(state.speed_limit, Some(4096));
}

#[test]
fn setting_the_limit_keeps_the_keyring_choice() {
    let mut t = test_engine();
    t.engine.handle_command(Command::RememberSaveChoice { save: true });

    set(&mut t, Some(4096));

    let state = saved(&t);
    assert!(state.save_passwords_in_keyring);
    assert_eq!(state.speed_limit, Some(4096));
}

#[test]
fn a_protected_settings_file_still_gets_the_limit_for_this_run_but_is_not_written() {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.state_dir).unwrap();
    std::fs::write(t.engine.paths.state_file(), "not toml [").unwrap();
    t.engine.state_protected = true;
    t.drain();

    set(&mut t, Some(1000));

    assert_eq!(t.engine.speed_limit.get(), Some(1000));
    assert_eq!(std::fs::read_to_string(t.engine.paths.state_file()).unwrap(), "not toml [");
    let notices = t.notices();
    assert!(
        notices.iter().any(|(severity, message)| *severity == Severity::Warning
            && message.contains("speed limit")
            && message.contains("protected")),
        "{notices:?}"
    );
}

#[test]
fn a_settings_folder_that_cannot_be_written_warns_but_keeps_the_limit_for_this_run() {
    let mut t = test_engine();
    std::fs::write(&t.engine.paths.state_dir, b"a file where the folder should be").unwrap();
    t.drain();

    set(&mut t, Some(1000));

    assert_eq!(t.engine.speed_limit.get(), Some(1000));
    let notices = t.notices();
    assert!(
        notices.iter().any(|(severity, message)| *severity == Severity::Warning && message.contains("speed limit")),
        "{notices:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_transfer_obeys_the_limit() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::write(t.dir.path().join("a.bin"), vec![1u8; 3000]).unwrap();
    set(&mut t, Some(10_000));
    let entry = crate::Entry {
        modified: None,
        name: "a.bin".to_string(),
        path: from_os_path(&t.dir.path().join("a.bin")),
        is_dir: false,
        size: 3000,
        permissions: None,
        ..Default::default()
    };
    let started = std::time::Instant::now();

    t.engine.copy(Location::Local, vec![entry], Location::Session(session), VfsPath::from("/up"));
    tokio::time::timeout(Duration::from_secs(10), async {
        while t.engine.transfers.jobs().count() == 0
            || t.engine.transfers.jobs().any(|job| job.status != crate::transfer::JobStatus::Completed)
        {
            t.run_internal().await;
        }
    })
    .await
    .expect("the transfer finished");

    assert_eq!(remote.contents("/up/a.bin").unwrap().len(), 3000);
    assert!(started.elapsed() >= Duration::from_millis(150), "{:?}", started.elapsed());
}

#[test]
fn a_broken_settings_file_at_start_means_no_limit_and_a_warning_and_is_not_overwritten_by_a_new_limit() {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.state_dir).unwrap();
    std::fs::write(t.engine.paths.state_file(), "speed_limit = \"fast\"\n").unwrap();

    t.engine.load_ui_state();

    assert_eq!(t.engine.speed_limit.get(), None);
    let notices = t.notices();
    assert!(
        notices
            .iter()
            .any(|(severity, message)| *severity == Severity::Warning && message.contains("state.toml.broken")),
        "{notices:?}"
    );
    set(&mut t, Some(1000));
    assert_eq!(saved(&t).speed_limit, Some(1000));
}
