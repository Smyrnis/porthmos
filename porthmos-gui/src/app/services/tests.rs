use std::time::Duration;

use porthmos_core::{
    Command, Entry, Event, Location, RefreshSetting, RefreshTab, ServiceVerb, ServicesState as ServicesEvent,
    StatusFilter, VfsPath,
};

use crate::app::Message;
use crate::drawer::DrawerView;
use crate::modal::Modal;
use crate::services_view::tests::snapshot;
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
        .filter_map(|command| if let Command::StartServices { session } = command { Some(*session) } else { None })
        .collect()
}

fn stops(sent: &[Command]) -> Vec<u64> {
    sent.iter()
        .filter_map(|command| if let Command::StopServices { session } = command { Some(*session) } else { None })
        .collect()
}

fn refreshes(sent: &[Command]) -> Vec<u64> {
    sent.iter()
        .filter_map(|command| if let Command::RefreshServices { session } = command { Some(*session) } else { None })
        .collect()
}

fn with_auto(seconds: u64) -> Harness {
    let mut settings = GuiSettings::default();
    settings.refresh.services = RefreshSetting { auto: true, interval_seconds: seconds };
    Harness::with_gui_settings(settings)
}

fn watching(harness: &mut Harness, uid: u32) {
    connect(harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    let mut snapshot = snapshot();
    snapshot.uid = uid;
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Snapshot(Box::new(snapshot)) });
    harness.sent();
}

#[test]
fn the_services_entry_sits_between_health_and_ports_in_the_strip() {
    let harness = Harness::new();

    let labels = crate::view::drawer::strip_labels(&harness.app);

    assert_eq!(&labels[..4], ["Tunnels", "Health", "Services", "Ports"]);
}

#[test]
fn showing_services_without_a_connection_shows_the_empty_state_and_sends_nothing() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::ServicesViewSelected);

    assert_eq!(harness.app.drawer().view, DrawerView::Services);
    assert!(harness.app.drawer().open);
    assert!(harness.sent().is_empty());
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[test]
fn becoming_visible_sends_one_start_and_nothing_more() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.sent();

    harness.press(Message::ServicesViewSelected);
    harness.press(Message::ServicesViewSelected);
    harness.press(Message::ServicesFilterChanged("x".into()));
    harness.press(Message::ServicesStatusFilterChanged(StatusFilter::Failed));

    let sent = harness.sent();
    assert_eq!(starts(&sent), [3], "{sent:?}");
    assert!(stops(&sent).is_empty() && refreshes(&sent).is_empty());
}

#[test]
fn hiding_the_tab_closing_the_drawer_or_disconnecting_stops_the_fetching() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    harness.sent();
    harness.press(Message::TunnelsViewSelected);
    assert_eq!(stops(&harness.sent()), [3]);

    harness.press(Message::ServicesViewSelected);
    harness.sent();
    harness.press(Message::ActionRequested(Action::TerminalToggle));
    assert!(!harness.app.drawer().open);
    assert_eq!(stops(&harness.sent()), [3]);

    harness.press(Message::ServicesViewSelected);
    harness.sent();
    harness.feed(Event::Disconnected { session: 3, name: "alpha".into() });
    assert_eq!(stops(&harness.sent()), [3]);
    harness.press(Message::ServicesViewSelected);
    assert!(starts(&harness.sent()).is_empty());
}

#[test]
fn switching_between_services_and_ports_stops_one_and_starts_the_other_once() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    harness.sent();

    harness.press(Message::PortsViewSelected);
    let sent = harness.sent();
    assert_eq!(stops(&sent), [3], "{sent:?}");

    harness.press(Message::ServicesViewSelected);
    assert_eq!(starts(&harness.sent()), [3]);
}

#[test]
fn choosing_another_connection_stops_the_old_and_starts_the_new() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::ServicesViewSelected);
    harness.sent();

    harness.press(Message::HealthSessionChosen("beta".into()));
    harness.press(Message::HealthSessionChosen("beta".into()));

    let sent = harness.sent();
    assert_eq!((stops(&sent), starts(&sent)), (vec![3], vec![4]), "{sent:?}");
    let position = |wanted: &Command| sent.iter().position(|command| format!("{command:?}") == format!("{wanted:?}"));
    assert!(position(&Command::StopServices { session: 3 }) < position(&Command::StartServices { session: 4 }));
    assert_eq!(harness.app.health_target(), Some(4));
}

#[test]
fn a_lost_connection_stops_services_and_a_restored_one_starts_it_again() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
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
    assert!(!crate::view::services::shows_session_picker(&harness.app));

    connect(&mut harness, 4, "beta");

    assert!(crate::view::services::shows_session_picker(&harness.app));
}

#[test]
fn events_update_the_state_for_the_shown_session_only() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::ServicesViewSelected);

    harness.feed(Event::Services { session: 4, state: ServicesEvent::Snapshot(Box::new(snapshot())) });
    assert!(harness.app.services().state.is_none());
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Snapshot(Box::new(snapshot())) });
    assert!(harness.app.services().snapshot().is_some());
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Failing("late".into()) });
    assert!(harness.app.services().failing() && harness.app.services().snapshot().is_some());
}

#[test]
fn a_late_event_after_switching_connection_is_not_shown_on_the_new_one() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::ServicesViewSelected);
    harness.press(Message::HealthSessionChosen("beta".into()));

    harness.feed(Event::Services { session: 3, state: ServicesEvent::Snapshot(Box::new(snapshot())) });

    assert!(harness.app.services().state.is_none());
}

#[test]
fn an_event_after_the_tab_was_hidden_is_not_kept() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    harness.press(Message::TunnelsViewSelected);

    harness.feed(Event::Services { session: 3, state: ServicesEvent::Snapshot(Box::new(snapshot())) });

    assert!(harness.app.services().state.is_none());
}

#[test]
fn refresh_sends_exactly_one_refresh_command() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    harness.sent();

    harness.press(Message::ServicesRefresh);

    let sent = harness.sent();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(refreshes(&sent), [3]);
}

#[test]
fn retry_sends_one_refresh_and_keeps_the_dimmed_list() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Failing("down".into()) });
    harness.sent();

    harness.press(Message::ServicesRetry);

    let sent = harness.sent();
    assert_eq!(refreshes(&sent), [3], "{sent:?}");
    assert_eq!(sent.len(), 1);
    assert!(harness.app.services().snapshot().is_some());
}

#[test]
fn refresh_and_retry_do_nothing_while_no_connection_is_shown() {
    let mut harness = Harness::new();
    harness.press(Message::ServicesViewSelected);
    harness.sent();

    harness.press(Message::ServicesRefresh);
    harness.press(Message::ServicesRetry);

    assert!(harness.sent().is_empty());
}

#[test]
fn the_filters_are_kept_across_events_and_tab_switches() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    harness.press(Message::ServicesFilterChanged("ssh".into()));
    harness.press(Message::ServicesStatusFilterChanged(StatusFilter::Running));

    harness.feed(Event::Services { session: 3, state: ServicesEvent::Snapshot(Box::new(snapshot())) });
    harness.press(Message::TunnelsViewSelected);
    harness.press(Message::ServicesViewSelected);

    assert_eq!(harness.app.services().filter, "ssh");
    assert_eq!(harness.app.services().status, StatusFilter::Running);
}

#[test]
fn the_show_services_action_opens_the_drawer_on_services() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::ShowServices));

    assert!(harness.app.drawer().open);
    assert_eq!(harness.app.drawer().view, DrawerView::Services);
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[test]
fn show_services_is_named_titled_and_has_no_default_chord() {
    assert_eq!(Action::ShowServices.name(), "show_services");
    assert_eq!(Action::ShowServices.title(), "Show services");
    let shortcuts = crate::shortcuts::Shortcuts::default();
    assert!(shortcuts.label_for(Action::ShowServices).is_none());
}

#[test]
fn selecting_a_row_selects_and_selecting_it_again_clears() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);

    harness.press(Message::ServiceSelected("nginx.service".into()));
    assert_eq!(harness.app.services().selected.as_deref(), Some("nginx.service"));
    harness.press(Message::ServiceSelected("nginx.service".into()));
    assert!(harness.app.services().selected.is_none());
}

#[test]
fn a_verb_asks_first_and_only_the_confirmation_sends_the_command() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));

    harness.press(Message::ServiceActionRequested(ServiceVerb::Restart));
    assert!(harness.sent().is_empty());
    assert!(matches!(
        harness.app.modal(),
        Some(Modal::ConfirmServiceAction { session: 3, verb: ServiceVerb::Restart, .. })
    ));
    harness.press(Message::ModalAccepted);

    let sent = harness.sent();
    assert!(
        matches!(
            sent.as_slice(),
            [Command::ServiceAction { session: 3, unit, verb: ServiceVerb::Restart }] if unit == "nginx.service"
        ),
        "{sent:?}"
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn cancelling_the_confirmation_sends_nothing() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.press(Message::ServiceActionRequested(ServiceVerb::Stop));

    harness.press(Message::ModalDeclined);

    assert!(harness.sent().is_empty());
    assert!(harness.app.modal().is_none());
}

#[test]
fn enter_declines_the_confirmation() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.press(Message::ServiceActionRequested(ServiceVerb::Stop));

    assert!(harness.app.modal().unwrap().is_risky_to_accept());
}

#[test]
fn the_confirmation_carries_sudo_exactly_when_the_user_is_not_root() {
    use porthmos_core::Privilege;
    for (uid, expected) in [(0, Privilege::Direct), (1000, Privilege::Sudo), (u32::MAX, Privilege::Sudo)] {
        let mut harness = Harness::new();
        watching(&mut harness, uid);
        harness.press(Message::ServiceSelected("nginx.service".into()));

        harness.press(Message::ServiceActionRequested(ServiceVerb::Start));

        match harness.app.modal() {
            Some(Modal::ConfirmServiceAction { privilege, .. }) => assert_eq!(*privilege, expected, "uid {uid}"),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_verb_that_is_not_available_opens_nothing() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);

    harness.press(Message::ServiceActionRequested(ServiceVerb::Start));
    assert!(harness.app.modal().is_none(), "no selection");
    for (name, verb) in [
        ("masked.service", ServiceVerb::Start),
        ("missing.service", ServiceVerb::Restart),
        ("static.service", ServiceVerb::Enable),
        ("odd\\x2dname.service", ServiceVerb::Stop),
    ] {
        harness.press(Message::ServiceSelected(name.into()));
        harness.press(Message::ServiceActionRequested(verb));
        assert!(harness.app.modal().is_none(), "{name} {verb:?}");
    }
    assert!(harness.sent().is_empty());
}

#[test]
fn the_selection_of_a_unit_with_an_unsafe_name_sends_no_status_request_either() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("odd\\x2dname.service".into()));

    harness.press(Message::ServiceStatusRequested);

    assert!(harness.sent().is_empty());
}

#[test]
fn a_verb_during_a_failure_or_without_a_session_opens_nothing() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Failing("down".into()) });

    harness.press(Message::ServiceActionRequested(ServiceVerb::Start));

    assert!(harness.app.modal().is_none());
}

#[test]
fn the_status_button_sends_one_request_and_shows_the_returned_text() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));

    harness.press(Message::ServiceStatusRequested);
    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [Command::ServiceStatus { session: 3, unit }] if unit == "nginx.service"),
        "{sent:?}"
    );
    harness.feed(Event::ServiceStatus { session: 3, unit: "nginx.service".into(), text: "Active: active".into() });

    assert_eq!(harness.app.services().status_text, Some(("nginx.service".to_string(), "Active: active".to_string())));
    let _ = crate::view::services::view(&harness.app);
}

#[test]
fn a_status_for_another_unit_or_session_is_ignored() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.press(Message::ServiceStatusRequested);
    harness.press(Message::ServiceSelected("cron.service".into()));

    harness.feed(Event::ServiceStatus { session: 3, unit: "nginx.service".into(), text: "late".into() });
    harness.feed(Event::ServiceStatus { session: 4, unit: "cron.service".into(), text: "elsewhere".into() });

    assert!(harness.app.services().status_text.is_none());
}

#[test]
fn a_tick_asks_for_a_refresh_only_while_the_tab_is_visible_and_started() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.sent();

    harness.press(Message::RefreshTick(RefreshTab::Services));
    assert!(harness.sent().is_empty(), "hidden tab");

    harness.press(Message::ServicesViewSelected);
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Services));
    assert_eq!(refreshes(&harness.sent()), [3]);

    harness.press(Message::TunnelsViewSelected);
    harness.sent();
    harness.press(Message::RefreshTick(RefreshTab::Services));
    assert!(harness.sent().is_empty(), "hidden again");
}

#[test]
fn a_tick_for_another_tab_does_not_refresh_services() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    harness.sent();

    harness.press(Message::RefreshTick(RefreshTab::Health));
    harness.press(Message::RefreshTick(RefreshTab::Ports));

    assert!(harness.sent().is_empty());
}

#[test]
fn a_tick_without_a_connection_sends_nothing() {
    let mut harness = with_auto(10);
    harness.press(Message::ServicesViewSelected);
    harness.sent();

    harness.press(Message::RefreshTick(RefreshTab::Services));

    assert!(harness.sent().is_empty());
}

#[test]
fn the_timer_runs_only_while_the_tab_is_visible_started_and_auto_is_on() {
    let mut off = Harness::new();
    connect(&mut off, 3, "alpha");
    off.press(Message::ServicesViewSelected);
    assert_eq!(off.app.services_tick_interval(), None);

    let mut on = with_auto(15);
    assert_eq!(on.app.services_tick_interval(), None, "hidden");
    on.press(Message::ServicesViewSelected);
    assert_eq!(on.app.services_tick_interval(), None, "visible but no connection, so nothing started");
    connect(&mut on, 3, "alpha");
    assert_eq!(on.app.services_tick_interval(), Some(Duration::from_secs(15)));
    on.press(Message::TunnelsViewSelected);
    assert_eq!(on.app.services_tick_interval(), None, "hidden again");
}

#[test]
fn the_timer_slows_down_while_failing() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);

    harness.feed(Event::Services { session: 3, state: ServicesEvent::Failing("down".into()) });
    assert_eq!(harness.app.services_tick_interval(), Some(Duration::from_secs(30)));
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Snapshot(Box::new(snapshot())) });
    assert_eq!(harness.app.services_tick_interval(), Some(Duration::from_secs(10)));
}

#[test]
fn the_subscription_builds_while_the_timer_runs() {
    let mut harness = with_auto(10);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);

    assert_eq!(harness.app.services_tick_interval(), Some(Duration::from_secs(10)));
    let _ = harness.app.subscription();
}

#[test]
fn the_reading_note_does_not_stick_when_the_status_never_arrives() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.press(Message::ServiceStatusRequested);
    assert!(harness.app.services().status_text.is_some());

    harness.feed(Event::Services { session: 3, state: ServicesEvent::Failing("down".into()) });
    assert!(harness.app.services().status_text.is_none());

    harness.feed(Event::Services { session: 3, state: ServicesEvent::Snapshot(Box::new(snapshot())) });
    harness.press(Message::ServiceStatusRequested);
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Unavailable("no systemd".into()) });
    assert!(harness.app.services().status_text.is_none());
}

#[test]
fn the_reading_note_is_dropped_when_the_target_connection_changes() {
    let mut harness = Harness::new();
    connect(&mut harness, 4, "beta");
    watching(&mut harness, 1000);
    harness.press(Message::HealthSessionChosen("alpha".into()));
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.press(Message::ServiceStatusRequested);
    assert!(harness.app.services().status_text.is_some());

    harness.press(Message::HealthSessionChosen("beta".into()));

    assert!(harness.app.services().status_text.is_none());
}

#[test]
fn the_reading_note_is_dropped_when_the_tab_is_hidden() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.press(Message::ServiceStatusRequested);

    harness.press(Message::TunnelsViewSelected);
    harness.press(Message::ServicesViewSelected);

    assert!(harness.app.services().status_text.is_none());
}

#[test]
fn a_verb_or_status_during_a_failure_still_opens_nothing_and_sends_nothing() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));
    harness.feed(Event::Services { session: 3, state: ServicesEvent::Failing("down".into()) });
    harness.sent();

    harness.press(Message::ServiceStatusRequested);
    harness.press(Message::ServiceActionRequested(ServiceVerb::Stop));

    assert!(harness.sent().is_empty() && harness.app.modal().is_none());
}

fn log_starts(sent: &[Command]) -> Vec<(u64, porthmos_core::LogSource)> {
    sent.iter()
        .filter_map(|command| match command {
            Command::StartLog { session, source, .. } => Some((*session, source.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn view_logs_opens_the_logs_tab_on_the_selected_unit_and_starts_it() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("nginx.service".into()));

    harness.press(Message::ServiceLogsRequested);

    assert_eq!(harness.app.drawer().view, DrawerView::Logs);
    assert!(harness.app.drawer().open);
    let sent = harness.sent();
    assert_eq!(
        log_starts(&sent),
        [(3, porthmos_core::LogSource::Journal(porthmos_core::UnitName::new("nginx.service")))]
    );
    assert_eq!(harness.app.logs().kind, crate::logs_view::SourceKind::Journal);
    assert_eq!(harness.app.logs().unit, "nginx.service");
}

#[test]
fn view_logs_does_nothing_for_a_unit_the_journal_would_not_accept() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);
    harness.press(Message::ServiceSelected("odd\\x2dname.service".into()));

    harness.press(Message::ServiceLogsRequested);

    assert_eq!(harness.app.drawer().view, DrawerView::Services);
    assert!(log_starts(&harness.sent()).is_empty());
}

#[test]
fn view_logs_without_a_selection_does_nothing() {
    let mut harness = Harness::new();
    watching(&mut harness, 1000);

    harness.press(Message::ServiceLogsRequested);

    assert_eq!(harness.app.drawer().view, DrawerView::Services);
    assert!(log_starts(&harness.sent()).is_empty());
}
