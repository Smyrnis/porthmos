use porthmos_core::VfsPath;

use crossterm::event::{KeyCode, KeyEventState, KeyModifiers};

use super::{
    testing::{entry, test_app},
    *,
};

#[test]
fn at_with_applies_panel_settings_to_the_local_panel() {
    let (core, _commands) = CoreHandle::detached();
    let settings = PanelSettings { show_hidden: true, sort: Default::default() };
    let mut app = App::new(core, VfsPath::from("/d"), &settings, input::KeyBindings::defaults());

    app.apply_core_event(Event::Listed {
        location: Location::Local,
        path: VfsPath::from("/d"),
        entries: vec![entry(&VfsPath::new("/d"), ".hidden", false)],
    });

    assert_eq!(app.local.rows().len(), 2);
}

#[test]
fn starting_asks_the_core_for_the_local_listing() {
    let (core, mut commands) = CoreHandle::detached();

    App::new(core, VfsPath::from("/start"), &PanelSettings::default(), input::KeyBindings::defaults());

    assert_eq!(
        commands.try_recv().unwrap(),
        Command::List { location: Location::Local, path: Some(VfsPath::from("/start")) }
    );
}

#[test]
fn key_bindings_from_config_are_used_for_key_mapping() {
    let mut test = test_app(&VfsPath::new("/d"));
    let mut overrides = std::collections::HashMap::new();
    overrides.insert("quit".to_string(), "ctrl+q".to_string());
    let (bindings, _) = input::KeyBindings::from_overrides(&overrides);
    test.app.key_bindings = bindings;

    let event = KeyEvent {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };

    assert_eq!(test.app.key_bindings.map_key(test.app.screen.scope(), event), Action::Quit);
}

#[test]
fn set_status_pushes_an_error_notification_on_failure() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_core_event(Event::Notice { severity: Severity::Error, message: "boom".into() });

    let current = test.app.notifications.current().unwrap();
    assert_eq!(current.message, "boom");
    assert_eq!(current.severity, Severity::Error);
}

#[test]
fn set_status_does_nothing_on_success() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_core_event(Event::LocationChanged { location: Location::Local });

    assert!(test.app.notifications.current().is_none());
}

#[test]
fn a_location_change_relists_what_the_panel_shows() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(3, "srv");
    test.list_remote(session, "/srv/www", Vec::new());

    test.app.apply_core_event(Event::LocationChanged { location: Location::Session(session) });
    test.app.apply_core_event(Event::LocationChanged { location: Location::Local });

    assert_eq!(
        test.sent(),
        vec![
            Command::List { location: Location::Session(session), path: Some(VfsPath::from("/srv/www")) },
            Command::List { location: Location::Local, path: Some(VfsPath::from("/d")) },
        ]
    );
}

#[test]
fn a_startup_warning_is_in_the_log_as_well_as_on_screen() {
    let mut test = test_app(&VfsPath::new("/d"));

    let logs = crate::logging::testing::capture_logs(|| test.app.warn("config.toml: unknown key"));

    assert!(logs.contains("WARN") && logs.contains("config.toml: unknown key"), "{logs}");
}

#[test]
fn an_error_is_in_the_log_as_well_as_on_screen() {
    let mut test = test_app(&VfsPath::new("/d"));

    let logs = crate::logging::testing::capture_logs(|| test.app.error("Failed to launch ssh: not found"));

    assert!(logs.contains("ERROR") && logs.contains("Failed to launch ssh: not found"), "{logs}");
}

#[test]
fn file_letters_do_nothing_on_the_screens_that_do_not_bind_them() {
    let cases = [
        (Screen::Connections, "cdrt12"),
        (Screen::Transfers, "dent12"),
        (Screen::History, "drentx12"),
        (Screen::Sync, "cdretx12"),
    ];
    for (screen, letters) in cases {
        let mut test = test_app(&VfsPath::new("/d"));
        test.app.screen = screen;
        test.sent();

        test.type_text(letters);

        assert!(test.app.dialog.is_none(), "{screen:?}");
        test.assert_inert(screen);
    }
}

#[test]
fn q_quits_from_every_screen_when_nothing_owns_the_keyboard() {
    for screen in [Screen::Files, Screen::Connections, Screen::Transfers, Screen::History, Screen::Sync] {
        let mut test = test_app(&VfsPath::new("/d"));
        test.app.screen = screen;

        test.type_text("q");

        assert!(test.app.should_quit, "{screen:?}");
    }
}

const THREE_GB: u64 = 3 * 1024 * 1024 * 1024;

#[test]
fn free_space_of_the_local_folder_reaches_the_local_panel() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "a", false)]);

    test.app.apply_core_event(Event::FreeSpace {
        location: Location::Local,
        path: VfsPath::from("/d"),
        bytes: Some(THREE_GB),
    });

    assert!(testing::find_text(&testing::render_app(&test.app, 120, 30), "3.0 GB free").is_some());
}

#[test]
fn free_space_of_a_session_folder_reaches_that_sessions_panel() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(1, "srv");
    test.list_remote(session, "/srv", vec![entry(&VfsPath::new("/srv"), "a", false)]);

    test.app.apply_core_event(Event::FreeSpace {
        location: Location::Session(session),
        path: VfsPath::from("/srv"),
        bytes: Some(THREE_GB),
    });

    assert!(testing::find_text(&testing::render_app(&test.app, 120, 30), "3.0 GB free").is_some());
}

#[test]
fn free_space_for_a_session_that_is_gone_is_ignored() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_core_event(Event::FreeSpace {
        location: Location::Session(99),
        path: VfsPath::from("/srv"),
        bytes: Some(THREE_GB),
    });

    assert!(testing::find_text(&testing::render_app(&test.app, 120, 30), "free").is_none());
}

#[test]
fn free_space_for_a_folder_the_panel_has_left_is_ignored() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "a", false)]);

    test.app.apply_core_event(Event::FreeSpace {
        location: Location::Local,
        path: VfsPath::from("/old"),
        bytes: Some(THREE_GB),
    });

    assert!(testing::find_text(&testing::render_app(&test.app, 120, 30), "free").is_none());
}

fn dot_colour(test: &testing::TestApp, name: &str) -> ratatui::style::Color {
    let terminal = testing::render_app(&test.app, 120, 30);
    let (x, y) = testing::find_text(&terminal, name).expect("the tab name is drawn");
    let row = testing::screen_rows(&terminal)[usize::from(y)].clone();
    let after = row[row.find(name).unwrap()..].find(['\u{2022}', '\u{d7}']).expect("a marker follows the tab name");
    let dot_x = x + unicode_width::UnicodeWidthStr::width(&row[row.find(name).unwrap()..][..after]) as u16;
    testing::cell_fg(&terminal, dot_x, y)
}

#[test]
fn a_session_starts_with_a_green_dot() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.connect(1, "srv");

    assert_eq!(dot_colour(&test, "srv"), test.app.theme.ok);
}

#[test]
fn a_lost_session_turns_its_tab_dot_red_and_restoring_it_turns_it_green_again() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(1, "srv");

    test.app.apply_core_event(Event::SessionHealth { session, healthy: false });
    let lost = dot_colour(&test, "srv");
    test.app.apply_core_event(Event::SessionHealth { session, healthy: true });
    let restored = dot_colour(&test, "srv");

    assert_eq!(lost, test.app.theme.err);
    assert_eq!(restored, test.app.theme.ok);
}

#[test]
fn health_news_for_a_session_that_is_gone_changes_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.connect(1, "srv");

    test.app.apply_core_event(Event::SessionHealth { session: 99, healthy: false });

    assert_eq!(dot_colour(&test, "srv"), test.app.theme.ok);
}

fn found_event(index: usize) -> Event {
    Event::SearchFound(entry(&VfsPath::new("/d"), &format!("hit{index}.log"), false))
}

fn shell_ready() -> Event {
    Event::ShellReady {
        session: 1,
        invocation: porthmos_core::ShellInvocation { program: "ssh".into(), args: Vec::new(), env: Vec::new() },
    }
}

fn local_shell_ready() -> Event {
    Event::LocalShellReady {
        invocation: porthmos_core::ShellInvocation { program: "sh".into(), args: Vec::new(), env: Vec::new() },
    }
}

fn search_results(test: &testing::TestApp) -> usize {
    test.app.search.as_ref().unwrap().view.results.len()
}

fn waiting_events(count: usize) -> (tokio::sync::mpsc::UnboundedSender<Event>, UnboundedReceiver<Event>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    for index in 0..count {
        tx.send(found_event(index)).unwrap();
    }
    (tx, rx)
}

#[test]
fn every_core_event_that_is_already_waiting_is_applied_before_the_next_redraw() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    let (_tx, mut rx) = waiting_events(5);

    let special = test.app.apply_ready_events(&mut rx);

    assert!(special.is_none());
    assert_eq!(search_results(&test), 5);
    assert!(rx.try_recv().is_err());
}

#[test]
fn a_shell_hand_over_is_returned_to_the_caller_and_later_events_wait() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    let (tx, mut rx) = waiting_events(2);
    tx.send(shell_ready()).unwrap();
    tx.send(found_event(9)).unwrap();

    let special = test.app.apply_ready_events(&mut rx);

    assert!(matches!(special, Some(Event::ShellReady { .. })));
    assert_eq!(search_results(&test), 2);
    assert!(matches!(rx.try_recv(), Ok(Event::SearchFound(_))));
    assert!(rx.try_recv().is_err());
}

#[test]
fn a_local_shell_hand_over_is_returned_to_the_caller_and_later_events_wait() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    let (tx, mut rx) = waiting_events(2);
    tx.send(local_shell_ready()).unwrap();
    tx.send(found_event(9)).unwrap();

    let special = test.app.apply_ready_events(&mut rx);

    assert!(matches!(special, Some(Event::LocalShellReady { .. })));
    assert_eq!(search_results(&test), 2);
    assert!(matches!(rx.try_recv(), Ok(Event::SearchFound(_))));
}

#[test]
fn a_flood_of_events_is_applied_in_batches_so_keys_are_not_starved() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenSearch);
    let (_tx, mut rx) = waiting_events(READY_EVENT_BATCH + 10);

    let special = test.app.apply_ready_events(&mut rx);

    assert!(special.is_none());
    assert_eq!(search_results(&test), READY_EVENT_BATCH);
    assert_eq!(std::iter::from_fn(|| rx.try_recv().ok()).count(), 10);
}
