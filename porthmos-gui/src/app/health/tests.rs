use std::time::Duration;

use iced::keyboard::{Key, Modifiers};
use porthmos_core::{Command, Entry, Event, Handover, Location, MonitorState, RefreshSetting, RefreshTab, VfsPath};
use tokio::io::AsyncReadExt;

use crate::app::Message;
use crate::drawer::{DrawerView, TerminalSource};
use crate::health_view::tests::snapshot;
use crate::settings_view::GuiSettings;
use crate::shortcuts::{Action, KeyPress};
use crate::testing::{Harness, ShellEnd, shell_pipe};

fn connect(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: VfsPath::new("/srv"),
        entries: Vec::<Entry>::new(),
    });
}

fn starts(sent: &[Command]) -> Vec<u64> {
    sent.iter()
        .filter_map(|command| if let Command::StartMonitor { session } = command { Some(*session) } else { None })
        .collect()
}

fn stops(sent: &[Command]) -> Vec<u64> {
    sent.iter()
        .filter_map(|command| if let Command::StopMonitor { session } = command { Some(*session) } else { None })
        .collect()
}

fn type_key(harness: &mut Harness, character: &str) {
    let key = Key::Character(character.into());
    harness.press(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::empty(),
        text: Some(character.into()),
    }));
}

async fn expect_silence(end: &mut ShellEnd) {
    let mut buffer = [0u8; 1];
    let waited = tokio::time::timeout(Duration::from_millis(150), end.from_client.read(&mut buffer)).await;
    assert!(waited.is_err(), "nothing should have been written, got {buffer:?}");
}

#[test]
fn the_health_entry_follows_tunnels_in_the_strip() {
    let harness = Harness::new();

    let labels = crate::view::drawer::strip_labels(&harness.app);

    assert_eq!(&labels[..2], ["Tunnels", "Health"]);
}

#[test]
fn showing_health_without_a_connection_shows_the_empty_state_and_sends_nothing() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::HealthViewSelected);

    assert_eq!(harness.app.drawer().view, DrawerView::Health);
    assert!(harness.app.drawer().open);
    assert_eq!(harness.app.health_target(), None);
    assert!(harness.sent().is_empty());
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[test]
fn showing_health_with_a_connection_starts_the_monitor_once() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.sent();

    harness.press(Message::HealthViewSelected);
    harness.press(Message::HealthViewSelected);
    harness.press(Message::HealthFilterChanged("x".into()));

    let sent = harness.sent();
    assert_eq!(starts(&sent), [3], "{sent:?}");
    assert!(stops(&sent).is_empty());
}

#[test]
fn leaving_the_tab_closing_the_drawer_or_disconnecting_stops_the_monitor() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);
    harness.sent();
    harness.press(Message::TunnelsViewSelected);
    assert_eq!(stops(&harness.sent()), [3]);

    harness.press(Message::HealthViewSelected);
    harness.sent();
    harness.press(Message::ActionRequested(Action::TerminalToggle));
    assert!(!harness.app.drawer().open);
    assert_eq!(stops(&harness.sent()), [3]);

    harness.press(Message::HealthViewSelected);
    harness.sent();
    harness.feed(Event::Disconnected { session: 3, name: "alpha".into() });
    assert_eq!(stops(&harness.sent()), [3]);
    harness.press(Message::HealthViewSelected);
    assert!(starts(&harness.sent()).is_empty());
}

#[test]
fn a_lost_connection_stops_the_monitor_and_a_restored_one_starts_it_again() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);
    harness.sent();

    harness.feed(Event::SessionHealth { session: 3, healthy: false });
    assert_eq!(stops(&harness.sent()), [3]);
    harness.feed(Event::SessionHealth { session: 3, healthy: true });

    assert_eq!(starts(&harness.sent()), [3]);
}

#[test]
fn choosing_another_session_stops_the_old_and_starts_the_new() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthViewSelected);
    let first = starts(&harness.sent());
    assert_eq!(first.len(), 1);
    let (shown, other) = if first[0] == 3 { ("alpha", "beta") } else { ("beta", "alpha") };
    assert_eq!(harness.app.health_session_names().len(), 2);

    harness.press(Message::HealthSessionChosen(other.to_string()));
    harness.press(Message::HealthSessionChosen(other.to_string()));

    let sent = harness.sent();
    assert_eq!(stops(&sent), first, "{sent:?}");
    assert_eq!(starts(&sent).len(), 1);
    assert_ne!(starts(&sent), first);
    assert_eq!(harness.app.sessions().by_name(other).map(|session| session.id), harness.app.health_target());
    let _ = shown;
}

#[test]
fn a_pick_list_appears_only_with_two_or_more_connections() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    assert!(!crate::view::health::shows_session_picker(&harness.app));

    connect(&mut harness, 4, "beta");

    assert!(crate::view::health::shows_session_picker(&harness.app));
    harness.feed(Event::SessionHealth { session: 4, healthy: false });
    assert!(!crate::view::health::shows_session_picker(&harness.app));
}

#[test]
fn snapshot_and_failing_and_unavailable_events_update_the_state_for_that_session_only() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::HealthViewSelected);
    let shown = harness.app.health_target().unwrap();
    let other = if shown == 3 { 4 } else { 3 };

    harness.feed(Event::Monitor { session: other, state: MonitorState::Snapshot(Box::new(snapshot(&[1]))) });
    assert!(harness.app.health().state.is_none());
    harness.feed(Event::Monitor { session: shown, state: MonitorState::Snapshot(Box::new(snapshot(&[1, 2]))) });
    assert!(
        matches!(&harness.app.health().state, Some(MonitorState::Snapshot(snapshot)) if snapshot.processes.len() == 2)
    );
    harness.feed(Event::Monitor { session: shown, state: MonitorState::Failing("late".into()) });
    assert!(matches!(harness.app.health().state, Some(MonitorState::Failing(_))));
    harness.feed(Event::Monitor { session: shown, state: MonitorState::Unavailable("nope".into()) });
    assert!(matches!(harness.app.health().state, Some(MonitorState::Unavailable(_))));
}

fn refreshes(sent: &[Command]) -> Vec<(u64, bool)> {
    sent.iter()
        .filter_map(|command| match command {
            Command::RefreshMonitor { session, pair } => Some((*session, *pair)),
            _ => None,
        })
        .collect()
}

fn with_auto(seconds: u64) -> Harness {
    let mut settings = GuiSettings::default();
    settings.refresh.health = RefreshSetting { auto: true, interval_seconds: seconds };
    Harness::with_gui_settings(settings)
}

#[test]
fn refresh_sends_exactly_one_paired_refresh_and_keeps_the_snapshot() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Snapshot(Box::new(snapshot(&[1]))) });
    harness.sent();

    harness.press(Message::HealthRefresh);

    let sent = harness.sent();
    assert_eq!(refreshes(&sent), [(3, true)], "{sent:?}");
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(matches!(harness.app.health().state, Some(MonitorState::Snapshot(_))));
}

#[test]
fn refresh_without_a_started_monitor_sends_nothing() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.sent();

    harness.press(Message::HealthRefresh);
    harness.press(Message::HealthRetry);

    assert!(harness.sent().is_empty());
}

#[test]
fn retry_is_the_same_as_refresh_and_never_stops_or_starts_the_monitor() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Failing("down".into()) });
    harness.sent();

    harness.press(Message::HealthRetry);

    let sent = harness.sent();
    assert_eq!(refreshes(&sent), [(3, true)], "{sent:?}");
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(harness.app.health().started, Some(3));
}

#[test]
fn a_tick_refreshes_without_a_pair_only_while_the_tab_is_visible_and_started() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.sent();

    harness.press(Message::RefreshTick(RefreshTab::Health));
    assert!(harness.sent().is_empty(), "hidden tab");

    harness.press(Message::HealthViewSelected);
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Health));
    assert_eq!(refreshes(&harness.sent()), [(3, false)]);

    harness.press(Message::TunnelsViewSelected);
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Health));
    assert!(harness.sent().is_empty(), "hidden again");
}

#[test]
fn a_tick_for_another_tab_or_without_a_connection_sends_nothing() {
    let mut harness = with_auto(10);
    harness.press(Message::HealthViewSelected);
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Health));
    assert!(harness.sent().is_empty(), "no connection");

    connect(&mut harness, 3, "alpha");
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Services));
    harness.press(Message::RefreshTick(RefreshTab::Ports));
    assert!(refreshes(&harness.sent()).is_empty());
}

#[test]
fn the_timer_runs_only_while_the_tab_is_visible_started_and_auto_is_on() {
    let mut off = Harness::new();
    connect(&mut off, 3, "alpha");
    off.press(Message::HealthViewSelected);
    assert_eq!(off.app.health_tick_interval(), None);

    let mut on = with_auto(15);
    assert_eq!(on.app.health_tick_interval(), None, "hidden");
    on.press(Message::HealthViewSelected);
    assert_eq!(on.app.health_tick_interval(), None, "visible but no connection, so nothing started");
    connect(&mut on, 3, "alpha");
    assert_eq!(on.app.health_tick_interval(), Some(Duration::from_secs(15)));
    on.press(Message::TunnelsViewSelected);
    assert_eq!(on.app.health_tick_interval(), None, "hidden again");
}

#[test]
fn the_timer_slows_down_while_failing_and_speeds_up_on_recovery() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);

    harness.feed(Event::Monitor { session: 3, state: MonitorState::Failing("down".into()) });
    assert_eq!(harness.app.health_tick_interval(), Some(Duration::from_secs(30)));
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Snapshot(Box::new(snapshot(&[1]))) });
    assert_eq!(harness.app.health_tick_interval(), Some(Duration::from_secs(10)));
}

#[test]
fn the_subscription_builds_while_the_timer_runs() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);

    assert_eq!(harness.app.health_tick_interval(), Some(Duration::from_secs(10)));
    let _ = harness.app.subscription();
}

#[test]
fn a_failure_after_a_snapshot_keeps_the_snapshot_and_the_selection() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));

    harness.feed(Event::Monitor { session: 3, state: MonitorState::Failing("down".into()) });

    assert_eq!(harness.app.health().snapshot().map(|snapshot| snapshot.processes.len()), Some(1));
    assert_eq!(harness.app.health().selected, Some(10));
}

#[test]
fn process_actions_open_nothing_while_failing_and_work_again_after_a_snapshot() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Failing("down".into()) });

    harness.press(Message::HealthTerminateRequested);
    harness.press(Message::HealthKillRequested);
    harness.press(Message::HealthReniceRequested);
    assert!(harness.app.modal().is_none());

    let recovered = process_owned_by(1000, 10);
    let mut snapshot = snapshot(&[]);
    snapshot.processes = vec![recovered];
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Snapshot(Box::new(snapshot)) });
    harness.press(Message::HealthTerminateRequested);
    assert!(harness.app.modal().is_some());
}

#[test]
fn selecting_a_process_toggles_and_filter_and_sort_are_kept() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Snapshot(Box::new(snapshot(&[10, 20]))) });

    harness.press(Message::HealthProcessSelected(20));
    assert_eq!(harness.app.health().selected, Some(20));
    harness.press(Message::HealthProcessSelected(20));
    assert_eq!(harness.app.health().selected, None);
    harness.press(Message::HealthSortChosen(porthmos_core::ProcessSort::Memory));
    harness.press(Message::HealthFilterChanged("nginx".into()));
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Snapshot(Box::new(snapshot(&[10]))) });

    assert_eq!(harness.app.health().sort, porthmos_core::ProcessSort::Memory);
    assert_eq!(harness.app.health().filter, "nginx");
}

#[tokio::test]
async fn keys_no_longer_reach_a_shell_while_health_is_shown() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.app.open_embedded_terminal(TerminalSource::Remote(3));
    let id = harness
        .sent()
        .into_iter()
        .find_map(|command| match command {
            Command::OpenTerminal { id, .. } => Some(id),
            _ => None,
        })
        .expect("an OpenTerminal command");
    let (stream, mut end) = shell_pipe();
    harness.feed(Event::TerminalOpened { id, session: 3, stream: Handover::new(stream) });
    let cd = format!("cd -- {}\r", porthmos_ssh::shell_quote("/srv"));
    let mut bytes = vec![0u8; cd.len()];
    tokio::time::timeout(Duration::from_secs(2), end.from_client.read_exact(&mut bytes)).await.unwrap().unwrap();

    harness.press(Message::HealthViewSelected);
    type_key(&mut harness, "x");

    assert_eq!(harness.app.drawer().view, DrawerView::Health);
    assert!(!harness.app.terminal_has_keys());
    expect_silence(&mut end).await;
}

#[test]
fn the_show_health_action_opens_the_drawer_on_health() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::ShowHealth));

    assert!(harness.app.drawer().open);
    assert_eq!(harness.app.drawer().view, DrawerView::Health);
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

fn process_owned_by(uid: u32, pid: u32) -> porthmos_core::HealthProcess {
    porthmos_core::HealthProcess { uid, ..crate::health_view::tests::process(pid) }
}

fn watching(harness: &mut Harness, connection_uid: u32, processes: Vec<porthmos_core::HealthProcess>) {
    connect(harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);
    let mut snapshot = snapshot(&[]);
    snapshot.uid = connection_uid;
    snapshot.processes = processes;
    harness.feed(Event::Monitor { session: 3, state: MonitorState::Snapshot(Box::new(snapshot)) });
    harness.sent();
}

fn snapshot_of(harness: &Harness) -> &porthmos_core::HealthSnapshot {
    match &harness.app.health().state {
        Some(MonitorState::Snapshot(snapshot)) => snapshot,
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

#[test]
fn the_buttons_are_disabled_for_another_users_process_and_enabled_for_your_own() {
    use crate::health_view::{ActionState, action_state};
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10), process_owned_by(33, 20)]);
    let snapshot = snapshot_of(&harness);

    assert_eq!(action_state(snapshot, None), ActionState::NoSelection);
    assert_eq!(action_state(snapshot, Some(999)), ActionState::NoSelection);
    assert_eq!(action_state(snapshot, Some(10)), ActionState::Allowed);
    assert_eq!(action_state(snapshot, Some(20)), ActionState::NeedsRoot);
    harness.press(Message::HealthProcessSelected(20));
    harness.press(Message::HealthTerminateRequested);
    harness.press(Message::HealthKillRequested);
    harness.press(Message::HealthReniceRequested);
    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn root_enables_the_buttons_for_every_process() {
    use crate::health_view::{ActionState, action_state};
    let mut harness = Harness::new();
    watching(&mut harness, 0, vec![process_owned_by(33, 20), process_owned_by(u32::MAX, 21)]);
    let snapshot = snapshot_of(&harness);

    assert_eq!(action_state(snapshot, Some(20)), ActionState::Allowed);
    assert_eq!(action_state(snapshot, Some(21)), ActionState::Allowed);
}

#[test]
fn terminate_asks_first_and_only_the_confirmation_sends_the_command() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));

    harness.press(Message::HealthTerminateRequested);
    assert!(harness.sent().is_empty());
    assert!(matches!(harness.app.modal(), Some(crate::modal::Modal::ConfirmSignal { pid: 10, .. })));
    harness.press(Message::ModalAccepted);

    let sent = harness.sent();
    assert!(
        matches!(
            sent.as_slice(),
            [Command::SignalProcess { session: 3, pid: 10, name, signal: porthmos_core::ProcessSignal::Terminate }] if name == "proc10"
        ),
        "{sent:?}"
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn force_kill_confirms_and_sends_kill() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));

    harness.press(Message::HealthKillRequested);
    harness.press(Message::ModalAccepted);

    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [Command::SignalProcess { pid: 10, signal: porthmos_core::ProcessSignal::Kill, .. }]),
        "{sent:?}"
    );
}

#[test]
fn enter_in_the_confirmation_does_not_confirm_a_destructive_action() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));
    harness.press(Message::HealthKillRequested);

    assert!(harness.app.modal().unwrap().is_risky_to_accept());
    let key = Key::Named(iced::keyboard::key::Named::Enter);
    harness.press(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::empty(),
        text: None,
    }));

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn cancel_sends_nothing() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));
    harness.press(Message::HealthTerminateRequested);

    harness.press(Message::ModalDeclined);
    harness.press(Message::HealthReniceRequested);
    harness.press(Message::ModalDeclined);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn renice_validates_the_number_inline_and_keeps_the_dialog_open() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));
    harness.press(Message::HealthReniceRequested);

    for bad in ["", "abc", "20", "-21", "1.5", "99999999999999999999"] {
        harness.press(Message::HealthReniceChanged(bad.to_string()));
        harness.press(Message::ModalAccepted);
        match harness.app.modal() {
            Some(crate::modal::Modal::Renice { error: Some(_), .. }) => {}
            other => panic!("{bad:?} should leave the dialog open with an error, got {other:?}"),
        }
        assert!(harness.sent().is_empty(), "{bad:?}");
    }
    harness.press(Message::HealthReniceChanged("5".to_string()));
    assert!(matches!(harness.app.modal(), Some(crate::modal::Modal::Renice { error: None, .. })));
}

#[test]
fn renice_sends_the_exact_command_for_a_valid_number() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));
    harness.press(Message::HealthReniceRequested);

    harness.press(Message::HealthReniceChanged(" -5 ".to_string()));
    harness.press(Message::ModalAccepted);

    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [Command::ReniceProcess { session: 3, pid: 10, name, nice }] if nice.get() == -5 && name == "proc10"),
        "{sent:?}"
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_process_that_vanishes_while_the_dialog_is_open_still_sends_and_the_engine_reports_it() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000, vec![process_owned_by(1000, 10)]);
    harness.press(Message::HealthProcessSelected(10));
    harness.press(Message::HealthTerminateRequested);

    harness.feed(Event::Monitor { session: 3, state: MonitorState::Snapshot(Box::new(snapshot(&[]))) });
    harness.press(Message::ModalAccepted);

    let sent = harness.sent();
    assert!(matches!(sent.as_slice(), [Command::SignalProcess { pid: 10, .. }]), "{sent:?}");
    assert_eq!(harness.app.health().selected, None);
}

#[test]
fn the_renice_field_starts_at_the_nice_value_the_process_has_now() {
    let mut harness = Harness::new();
    let reniced = porthmos_core::HealthProcess { nice: 7, ..process_owned_by(1000, 10) };
    watching(&mut harness, 1000, vec![reniced]);
    harness.press(Message::HealthProcessSelected(10));

    harness.press(Message::HealthReniceRequested);

    assert!(matches!(harness.app.modal(), Some(crate::modal::Modal::Renice { text, .. }) if text == "7"));
    harness.press(Message::ModalAccepted);
    let sent = harness.sent();
    assert!(matches!(sent.as_slice(), [Command::ReniceProcess { nice, .. }] if nice.get() == 7), "{sent:?}");
}
