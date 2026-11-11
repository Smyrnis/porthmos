use std::time::{Duration, Instant};

use porthmos_core::{Command, CoreHandle, Environment, Event, Paths, UpdateMode};
use tokio::sync::mpsc::unbounded_channel;

use super::*;

#[tokio::test]
async fn started_carries_the_update_mode() {
    for (text, expected) in [("off", UpdateMode::Off), ("notify", UpdateMode::Notify), ("auto", UpdateMode::Auto)] {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(root.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_file(), format!("[update]\nmode = \"{text}\"\n")).unwrap();
        let env = Environment::from_process();

        let started = start(env, paths).unwrap();

        assert_eq!(started.settings.update_mode, expected, "{text}");
        started.core.shutdown().await;
    }
}

#[tokio::test]
async fn a_theme_file_in_the_config_folder_colours_the_started_theme() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.config_file(), "[gui.theme]\naccent = \"#111111\"\n").unwrap();
    std::fs::write(paths.theme_file(), "[colors]\nbg = \"#102030\"\nbogus = \"#000000\"\n").unwrap();

    let started = start(Environment::from_process(), paths).unwrap();

    let palette = crate::theme::Palette::new(&started.theme);
    assert_eq!(palette.bg, iced::Color::from_rgb8(0x10, 0x20, 0x30));
    assert_eq!(palette.accent, iced::Color::from_rgb8(0x11, 0x11, 0x11));
    assert!(started.warnings.iter().any(|warning| warning.contains("bogus")), "{:?}", started.warnings);
    started.core.shutdown().await;
}

#[tokio::test]
async fn the_theme_file_wins_over_the_config_accent() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.config_file(), "[gui.theme]\naccent = \"#111111\"\n").unwrap();
    std::fs::write(paths.theme_file(), "[colors]\naccent = \"#222222\"\n").unwrap();

    let started = start(Environment::from_process(), paths).unwrap();

    assert_eq!(crate::theme::Palette::new(&started.theme).accent, iced::Color::from_rgb8(0x22, 0x22, 0x22));
    started.core.shutdown().await;
}

#[test]
fn the_event_source_hands_out_its_receiver_exactly_once() {
    let (_sender, receiver) = unbounded_channel::<Event>();
    let source = EventSource::new(receiver);
    let clone = source.clone();

    assert!(source.take().is_some());
    assert!(clone.take().is_none());
    assert!(source.take().is_none());
}

#[tokio::test]
async fn a_future_that_finishes_in_time_reports_success() {
    assert!(bounded(async {}, Duration::from_millis(200)).await);
}

#[tokio::test]
async fn a_core_that_never_answers_does_not_hold_the_exit_longer_than_the_bound() {
    let started = Instant::now();

    let finished = bounded(std::future::pending::<()>(), Duration::from_millis(60)).await;

    assert!(!finished);
    assert!(started.elapsed() < Duration::from_millis(1000));
}

#[tokio::test]
async fn shutting_down_sends_the_shutdown_command() {
    let (handle, mut commands): (CoreHandle, _) = CoreHandle::detached();

    shut_down(handle, Duration::from_millis(100)).await;

    assert!(matches!(commands.recv().await, Some(Command::Shutdown)));
}

fn paths_with_mode(text: &str) -> (tempfile::TempDir, Paths) {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.config_file(), format!("[update]\nmode = \"{text}\"\n")).unwrap();
    (root, paths)
}

#[test]
fn the_mode_on_exit_is_the_one_saved_in_the_config_not_the_one_at_startup() {
    for (text, expected) in [("off", UpdateMode::Off), ("notify", UpdateMode::Notify), ("auto", UpdateMode::Auto)] {
        let (_root, paths) = paths_with_mode(text);
        for started_with in [UpdateMode::Off, UpdateMode::Notify, UpdateMode::Auto] {
            assert_eq!(
                update_mode_on_exit(&paths, started_with),
                expected,
                "{text} after starting with {started_with:?}"
            );
        }
    }
}

#[test]
fn the_mode_on_exit_falls_back_to_the_startup_mode_when_the_config_cannot_be_read() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.config_file(), "[update\nmode = ").unwrap();
    assert_eq!(update_mode_on_exit(&paths, UpdateMode::Notify), UpdateMode::Notify);
    assert_eq!(update_mode_on_exit(&paths, UpdateMode::Off), UpdateMode::Off);

    std::fs::remove_file(paths.config_file()).unwrap();
    std::fs::create_dir(paths.config_file()).unwrap();
    assert_eq!(update_mode_on_exit(&paths, UpdateMode::Off), UpdateMode::Off);
}

#[test]
fn the_window_starts_in_the_remembered_folder_when_there_is_one() {
    let start = local_start(Some(VfsPath::new("/work/notes")), Some("/home/me".into()), Some("/cwd".into()));

    assert_eq!(start, VfsPath::new("/work/notes"));
}

#[test]
fn without_a_remembered_folder_the_window_starts_at_home_then_the_current_folder_then_the_root() {
    assert_eq!(local_start(None, Some("/home/me".into()), Some("/cwd".into())), VfsPath::new("/home/me"));
    assert_eq!(local_start(None, None, Some("/cwd".into())), VfsPath::new("/cwd"));
    assert_eq!(local_start(None, None, None), VfsPath::root());
}

#[tokio::test]
async fn started_offers_the_folder_the_last_run_ended_in() {
    let root = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(
        paths.state_file(),
        format!("local_folder = {:?}\n", porthmos_core::from_os_path(folder.path()).to_string_lossy()),
    )
    .unwrap();

    let started = start(Environment::from_process(), paths).unwrap();

    assert_eq!(started.core.remembered_local_folder(), Some(porthmos_core::from_os_path(folder.path())));
    started.core.shutdown().await;
}

#[tokio::test]
async fn started_carries_the_search_preview_setting() {
    for (text, expected) in [("true", true), ("false", false)] {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(root.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_file(), format!("[search]\npreview = {text}\n")).unwrap();

        let started = start(Environment::from_process(), paths).unwrap();

        assert_eq!(started.settings.search_preview, expected, "{text}");
        started.core.shutdown().await;
    }

    let root = tempfile::tempdir().unwrap();
    let started = start(Environment::from_process(), Paths::in_dir(root.path())).unwrap();
    assert!(!started.settings.search_preview);
    started.core.shutdown().await;
}

#[tokio::test]
async fn started_carries_the_reopen_mode() {
    use porthmos_core::ReopenMode;
    for (text, expected) in [("off", ReopenMode::Off), ("ask", ReopenMode::Ask), ("auto", ReopenMode::Auto)] {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(root.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_file(), format!("[startup]\nreopen = \"{text}\"\n")).unwrap();

        let started = start(Environment::from_process(), paths).unwrap();

        assert_eq!(started.settings.reopen, expected, "{text}");
        started.core.shutdown().await;
    }

    let root = tempfile::tempdir().unwrap();
    let started = start(Environment::from_process(), Paths::in_dir(root.path())).unwrap();
    assert_eq!(started.settings.reopen, ReopenMode::Off);
    started.core.shutdown().await;
}
