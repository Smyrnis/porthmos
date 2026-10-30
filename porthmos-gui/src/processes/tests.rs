use crate::editing::CommandSpec;
use std::ffi::OsString;

use super::ProcessJob;

fn job(token: u64, program: &str, args: &[&str]) -> ProcessJob {
    ProcessJob {
        token,
        spec: CommandSpec { program: OsString::from(program), args: args.iter().map(OsString::from).collect() },
        env: Vec::new(),
        cleanup: Vec::new(),
    }
}

#[test]
fn process_job_debug_redacts_env_values() {
    let mut secret = job(1, "ssh", &["host"]);
    secret.env = vec![(OsString::from("SSHPASS"), OsString::from("s3cr3t-value"))];

    let text = format!("{secret:?}");

    assert!(text.contains("SSHPASS"));
    assert!(text.contains("<redacted>"));
    assert!(!text.contains("s3cr3t-value"));
}

#[cfg(unix)]
mod unix {
    use std::time::{Duration, Instant};

    use tokio::sync::mpsc::UnboundedReceiver;

    use porthmos_core::edit::EditorExit;

    use super::super::{ProcessEvent, Spawner, SystemSpawner, resolve_in_path};
    use super::*;

    fn next(receiver: &mut UnboundedReceiver<ProcessEvent>) -> ProcessEvent {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(event) = receiver.try_recv() {
                return event;
            }
            assert!(Instant::now() < deadline, "no process event arrived");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_system_spawner_runs_a_program_and_reports_its_exit() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        SystemSpawner.spawn(job(7, "true", &[]), sender.clone());
        assert_eq!(next(&mut receiver), ProcessEvent::Exited { token: 7, exit: EditorExit::Success });
        SystemSpawner.spawn(job(8, "sh", &["-c", "exit 3"]), sender);

        assert_eq!(next(&mut receiver), ProcessEvent::Exited { token: 8, exit: EditorExit::Status(3) });
    }

    #[test]
    fn the_job_environment_reaches_the_program() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut with_env = job(1, "sh", &["-c", "test \"$PORTHMOS_TEST_VALUE\" = expected"]);
        with_env.env = vec![(OsString::from("PORTHMOS_TEST_VALUE"), OsString::from("expected"))];

        SystemSpawner.spawn(with_env, sender);

        assert_eq!(next(&mut receiver), ProcessEvent::Exited { token: 1, exit: EditorExit::Success });
    }

    #[test]
    fn cleanup_files_survive_a_clean_exit_because_the_wrapper_owns_them() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("secret");
        std::fs::write(&file, b"x").unwrap();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut cleaning = job(1, "true", &[]);
        cleaning.cleanup = vec![file.clone()];

        SystemSpawner.spawn(cleaning, sender);
        next(&mut receiver);

        assert!(file.exists());
    }

    #[test]
    fn cleanup_files_are_deleted_after_a_failed_exit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("secret");
        std::fs::write(&file, b"x").unwrap();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut cleaning = job(1, "sh", &["-c", "exit 3"]);
        cleaning.cleanup = vec![file.clone()];

        SystemSpawner.spawn(cleaning, sender);
        next(&mut receiver);

        assert!(!file.exists());
    }

    #[test]
    fn cleanup_files_are_deleted_even_when_the_program_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("secret");
        std::fs::write(&file, b"x").unwrap();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut cleaning = job(1, "porthmos-no-such-program", &[]);
        cleaning.cleanup = vec![file.clone()];

        SystemSpawner.spawn(cleaning, sender);
        next(&mut receiver);

        assert!(!file.exists());
    }

    #[test]
    fn a_missing_program_reports_launch_failed() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        SystemSpawner.spawn(job(2, "porthmos-no-such-program", &[]), sender);

        match next(&mut receiver) {
            ProcessEvent::Exited { token: 2, exit: EditorExit::LaunchFailed(message) } => assert!(!message.is_empty()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn resolve_in_path_follows_symlinks_and_skips_non_executables() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("fake-term");
        std::fs::write(&real, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&real, dir.path().join("alias")).unwrap();
        let plain = dir.path().join("plain");
        std::fs::write(&plain, b"x").unwrap();
        let search = std::env::join_paths([dir.path()]).unwrap();

        assert_eq!(resolve_in_path("alias", &search), Some("fake-term".to_string()));
        assert_eq!(resolve_in_path("fake-term", &search), Some("fake-term".to_string()));
        assert_eq!(resolve_in_path("plain", &search), None);
        assert_eq!(resolve_in_path("missing", &search), None);
    }
}
