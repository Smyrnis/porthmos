use std::time::Duration;

use porthmos_core::{Command, Entry, Event, Location, PortsState as PortsEvent, RefreshSetting, RefreshTab, VfsPath};

use crate::app::Message;
use crate::drawer::DrawerView;
use crate::ports_view::PortsPane;
use crate::ports_view::tests::snapshot;
use crate::settings_view::GuiSettings;
use crate::shortcuts::Action;
use crate::testing::Harness;

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
        .filter_map(|command| if let Command::StartPorts { session } = command { Some(*session) } else { None })
        .collect()
}

fn stops(sent: &[Command]) -> Vec<u64> {
    sent.iter()
        .filter_map(|command| if let Command::StopPorts { session } = command { Some(*session) } else { None })
        .collect()
}

fn refreshes(sent: &[Command]) -> Vec<u64> {
    sent.iter()
        .filter_map(|command| if let Command::RefreshPorts { session } = command { Some(*session) } else { None })
        .collect()
}

fn with_auto(seconds: u64) -> Harness {
    let mut settings = GuiSettings::default();
    settings.refresh.ports = RefreshSetting { auto: true, interval_seconds: seconds };
    Harness::with_gui_settings(settings)
}

#[test]
fn the_ports_entry_follows_services_in_the_strip() {
    let harness = Harness::new();

    let labels = crate::view::drawer::strip_labels(&harness.app);

    assert_eq!(&labels[..4], ["Tunnels", "Health", "Services", "Ports"]);
}

#[test]
fn showing_ports_without_a_connection_shows_the_empty_state_and_sends_nothing() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::PortsViewSelected);

    assert_eq!(harness.app.drawer().view, DrawerView::Ports);
    assert!(harness.app.drawer().open);
    assert!(harness.sent().is_empty());
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[test]
fn becoming_visible_sends_one_start_and_nothing_more() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.sent();

    harness.press(Message::PortsViewSelected);
    harness.press(Message::PortsViewSelected);
    harness.press(Message::PortsFilterChanged("x".into()));
    harness.press(Message::PortsViewChosen(PortsPane::Interfaces));

    let sent = harness.sent();
    assert_eq!(starts(&sent), [3], "{sent:?}");
    assert!(stops(&sent).is_empty() && refreshes(&sent).is_empty());
}

#[test]
fn hiding_the_tab_closing_the_drawer_or_disconnecting_stops_the_fetching() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.sent();
    harness.press(Message::TunnelsViewSelected);
    assert_eq!(stops(&harness.sent()), [3]);

    harness.press(Message::PortsViewSelected);
    harness.sent();
    harness.press(Message::ActionRequested(Action::TerminalToggle));
    assert!(!harness.app.drawer().open);
    assert_eq!(stops(&harness.sent()), [3]);

    harness.press(Message::PortsViewSelected);
    harness.sent();
    harness.feed(Event::Disconnected { session: 3, name: "alpha".into() });
    assert_eq!(stops(&harness.sent()), [3]);
    harness.press(Message::PortsViewSelected);
    assert!(starts(&harness.sent()).is_empty());
}

#[test]
fn switching_to_health_stops_ports_and_back_starts_it_once() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.sent();

    harness.press(Message::HealthViewSelected);
    let sent = harness.sent();
    assert_eq!(stops(&sent), [3], "{sent:?}");

    harness.press(Message::PortsViewSelected);
    assert_eq!(starts(&harness.sent()), [3]);
}

#[test]
fn choosing_another_connection_stops_the_old_and_starts_the_new() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::PortsViewSelected);
    harness.sent();

    harness.press(Message::HealthSessionChosen("beta".into()));
    harness.press(Message::HealthSessionChosen("beta".into()));

    let sent = harness.sent();
    assert_eq!((stops(&sent), starts(&sent)), (vec![3], vec![4]), "{sent:?}");
    let position = |wanted: &Command| sent.iter().position(|command| format!("{command:?}") == format!("{wanted:?}"));
    assert!(position(&Command::StopPorts { session: 3 }) < position(&Command::StartPorts { session: 4 }));
    assert_eq!(harness.app.health_target(), Some(4));
}

#[test]
fn the_connection_choice_is_shared_with_health() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");

    harness.press(Message::HealthSessionChosen("alpha".into()));
    assert_eq!(harness.app.health_target(), Some(3));
    harness.press(Message::HealthSessionChosen("beta".into()));

    harness.press(Message::PortsViewSelected);
    assert_eq!(harness.app.ports().started, Some(4));
}

#[test]
fn a_lost_connection_stops_ports_and_a_restored_one_starts_it_again() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.sent();

    harness.feed(Event::SessionHealth { session: 3, healthy: false });
    assert_eq!(stops(&harness.sent()), [3]);
    harness.feed(Event::SessionHealth { session: 3, healthy: true });

    assert_eq!(starts(&harness.sent()), [3]);
}

#[test]
fn the_pick_list_appears_only_with_two_or_more_connections() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    assert!(!crate::view::ports::shows_session_picker(&harness.app));

    connect(&mut harness, 4, "beta");

    assert!(crate::view::ports::shows_session_picker(&harness.app));
    harness.feed(Event::SessionHealth { session: 4, healthy: false });
    assert!(!crate::view::ports::shows_session_picker(&harness.app));
}

#[test]
fn events_update_the_state_for_the_shown_session_only() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::PortsViewSelected);

    harness.feed(Event::Ports { session: 4, state: PortsEvent::Snapshot(Box::new(snapshot())) });
    assert!(harness.app.ports().state.is_none());
    harness.feed(Event::Ports { session: 3, state: PortsEvent::Snapshot(Box::new(snapshot())) });
    assert!(harness.app.ports().snapshot().is_some());
    harness.feed(Event::Ports { session: 3, state: PortsEvent::Failing("late".into()) });
    assert!(harness.app.ports().failing() && harness.app.ports().snapshot().is_some());
    harness.feed(Event::Ports { session: 3, state: PortsEvent::Unavailable("nope".into()) });
    assert!(matches!(harness.app.ports().state, Some(PortsEvent::Unavailable(_))));
}

#[test]
fn a_late_event_after_switching_connection_is_not_shown_on_the_new_one() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::PortsViewSelected);
    harness.press(Message::HealthSessionChosen("beta".into()));

    harness.feed(Event::Ports { session: 3, state: PortsEvent::Snapshot(Box::new(snapshot())) });

    assert!(harness.app.ports().state.is_none());
}

#[test]
fn an_event_after_the_tab_was_hidden_is_not_kept() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.press(Message::TunnelsViewSelected);

    harness.feed(Event::Ports { session: 3, state: PortsEvent::Snapshot(Box::new(snapshot())) });

    assert!(harness.app.ports().state.is_none());
}

#[test]
fn refresh_sends_exactly_one_refresh_command() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.sent();

    harness.press(Message::PortsRefresh);

    let sent = harness.sent();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(refreshes(&sent), [3]);
}

#[test]
fn retry_sends_one_refresh_and_keeps_the_dimmed_snapshot() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.feed(Event::Ports { session: 3, state: PortsEvent::Snapshot(Box::new(snapshot())) });
    harness.feed(Event::Ports { session: 3, state: PortsEvent::Failing("down".into()) });
    harness.sent();

    harness.press(Message::PortsRetry);

    let sent = harness.sent();
    assert_eq!(refreshes(&sent), [3], "{sent:?}");
    assert_eq!(sent.len(), 1);
    assert!(harness.app.ports().snapshot().is_some());
}

#[test]
fn refresh_and_retry_do_nothing_while_no_connection_is_shown() {
    let mut harness = Harness::new();
    harness.press(Message::PortsViewSelected);
    harness.sent();

    harness.press(Message::PortsRefresh);
    harness.press(Message::PortsRetry);

    assert!(harness.sent().is_empty());
}

#[test]
fn the_pane_and_filter_are_kept_across_events_and_tab_switches() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.press(Message::PortsViewChosen(PortsPane::Connections));
    harness.press(Message::PortsFilterChanged("ssh".into()));

    harness.feed(Event::Ports { session: 3, state: PortsEvent::Snapshot(Box::new(snapshot())) });
    harness.press(Message::TunnelsViewSelected);
    harness.press(Message::PortsViewSelected);

    assert_eq!(harness.app.ports().pane, PortsPane::Connections);
    assert_eq!(harness.app.ports().filter, "ssh");
}

#[test]
fn the_show_ports_action_opens_the_drawer_on_ports() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::ShowPorts));

    assert!(harness.app.drawer().open);
    assert_eq!(harness.app.drawer().view, DrawerView::Ports);
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[test]
fn show_ports_is_named_titled_and_has_no_default_chord() {
    assert_eq!(Action::ShowPorts.name(), "show_ports");
    assert_eq!(Action::ShowPorts.title(), "Show ports");
    let shortcuts = crate::shortcuts::Shortcuts::default();
    assert!(shortcuts.label_for(Action::ShowPorts).is_none(), "{:?}", shortcuts.label_for(Action::ShowPorts));
}

#[test]
fn a_tick_asks_for_a_refresh_only_while_the_tab_is_visible() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.sent();

    harness.press(Message::RefreshTick(RefreshTab::Ports));
    assert!(harness.sent().is_empty(), "hidden tab");

    harness.press(Message::PortsViewSelected);
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Ports));
    assert_eq!(refreshes(&harness.sent()), [3]);

    harness.press(Message::TunnelsViewSelected);
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Ports));
    assert!(harness.sent().is_empty(), "hidden again");
}

#[test]
fn a_tick_for_another_tab_does_not_refresh_ports() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.sent();

    harness.press(Message::RefreshTick(RefreshTab::Health));
    harness.press(Message::RefreshTick(RefreshTab::Services));

    assert!(harness.sent().is_empty());
}

#[test]
fn a_tick_without_a_connection_sends_nothing() {
    let mut harness = with_auto(10);
    harness.press(Message::PortsViewSelected);
    harness.sent();

    harness.press(Message::RefreshTick(RefreshTab::Ports));

    assert!(harness.sent().is_empty());
}

#[test]
fn the_timer_runs_only_while_the_tab_is_visible_started_and_auto_is_on() {
    let mut off = Harness::new();
    connect(&mut off, 3, "alpha");
    off.press(Message::PortsViewSelected);
    assert_eq!(off.app.ports_tick_interval(), None);

    let mut on = with_auto(15);
    assert_eq!(on.app.ports_tick_interval(), None, "hidden");
    on.press(Message::PortsViewSelected);
    assert_eq!(on.app.ports_tick_interval(), None, "visible but no connection, so nothing started");
    connect(&mut on, 3, "alpha");
    assert_eq!(on.app.ports_tick_interval(), Some(Duration::from_secs(15)));
    on.press(Message::TunnelsViewSelected);
    assert_eq!(on.app.ports_tick_interval(), None, "hidden again");
}

#[test]
fn the_timer_slows_down_while_failing() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);

    harness.feed(Event::Ports { session: 3, state: PortsEvent::Failing("down".into()) });
    assert_eq!(harness.app.ports_tick_interval(), Some(Duration::from_secs(30)));
    harness.feed(Event::Ports { session: 3, state: PortsEvent::Snapshot(Box::new(snapshot())) });
    assert_eq!(harness.app.ports_tick_interval(), Some(Duration::from_secs(10)));
}

#[test]
fn the_subscription_builds_while_the_timer_runs() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);

    assert_eq!(harness.app.ports_tick_interval(), Some(Duration::from_secs(10)));
    let _ = harness.app.subscription();
}
