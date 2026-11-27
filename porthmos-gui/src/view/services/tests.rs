use porthmos_core::{Event, Location, ServicesState as ServicesEvent};

use super::*;
use crate::app::Message;
use crate::services_view::tests::{snapshot, unit};
use crate::testing::Harness;
use porthmos_core::{ActiveState, BootState};

fn connect(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: porthmos_core::VfsPath::new("/"),
        entries: Vec::new(),
    });
}

fn shown_in(harness: &mut Harness, state: Option<ServicesEvent>) {
    if let Some(state) = state {
        harness.feed(Event::Services { session: 3, state });
    }
    let _ = view(&harness.app);
}

#[test]
fn the_tab_builds_in_every_state() {
    let mut harness = Harness::new();
    harness.press(Message::ServicesViewSelected);
    shown_in(&mut harness, None);

    connect(&mut harness, 3, "alpha");
    shown_in(&mut harness, None);
    for status in [StatusFilter::All, StatusFilter::Running, StatusFilter::Failed] {
        harness.press(Message::ServicesStatusFilterChanged(status));
        shown_in(&mut harness, Some(ServicesEvent::Snapshot(Box::new(snapshot()))));
        shown_in(&mut harness, Some(ServicesEvent::Failing("no answer".into())));
        shown_in(&mut harness, Some(ServicesEvent::Unavailable("Services need systemd".into())));
        shown_in(&mut harness, Some(ServicesEvent::Failing("no answer".into())));
        shown_in(&mut harness, None);
    }
    harness.press(Message::ServicesFilterChanged("nothing matches this".into()));
    shown_in(&mut harness, Some(ServicesEvent::Snapshot(Box::new(snapshot()))));
    shown_in(
        &mut harness,
        Some(ServicesEvent::Snapshot(Box::new(porthmos_core::ServicesSnapshot { units: vec![], uid: 0 }))),
    );
    connect(&mut harness, 4, "beta");
    shown_in(&mut harness, None);
}

#[test]
fn the_detail_strip_and_the_status_panel_build_for_every_kind_of_selection() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    shown_in(&mut harness, Some(ServicesEvent::Snapshot(Box::new(snapshot()))));

    for name in ["nginx.service", "backup.service", "masked.service", "odd\\x2dname.service", "missing.service"] {
        harness.press(Message::ServiceSelected(name.into()));
        harness.feed(Event::ServiceStatus { session: 3, unit: name.into(), text: "line\n".repeat(500) });
        let _ = view(&harness.app);
        harness.press(Message::ServiceSelected(name.into()));
    }
    harness.press(Message::ServiceSelected("nginx.service".into()));
    shown_in(&mut harness, Some(ServicesEvent::Failing("down".into())));
}

#[test]
fn a_snapshot_with_thousands_of_units_and_hostile_text_builds() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    let mut big = snapshot();
    let mut template = unit("a.service", ActiveState::Active, "running", Some(BootState::Enabled));
    template.description = format!("x\u{202e}\u{7}{}", "d".repeat(10_000));
    big.units = vec![template; 2000];

    shown_in(&mut harness, Some(ServicesEvent::Snapshot(Box::new(big))));
}

#[test]
fn the_picker_appears_with_two_connections() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    assert!(!shows_session_picker(&harness.app));
    connect(&mut harness, 4, "beta");
    assert!(shows_session_picker(&harness.app));
}

fn selected_nginx(harness: &mut Harness) {
    connect(harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    shown_in(harness, Some(ServicesEvent::Snapshot(Box::new(snapshot()))));
    harness.press(Message::ServiceSelected("nginx.service".into()));
}

fn pressed(press: &Option<Message>) -> Option<String> {
    press.as_ref().map(|message| format!("{message:?}"))
}

#[test]
fn while_updating_has_failed_no_button_has_a_press_message_and_each_carries_the_retry_first_tooltip() {
    use crate::services_view::ALL_VERBS;
    let mut harness = Harness::new();
    selected_nginx(&mut harness);
    shown_in(&mut harness, Some(ServicesEvent::Failing("down".into())));
    let state = harness.app.services();
    let kept = state.snapshot().expect("the dimmed list");
    let reason = Some(porthmos_core::i18n::text("gui.services.retry_first"));

    assert_eq!(state.selected.as_deref(), Some("nginx.service"));
    for verb in ALL_VERBS {
        let (press, tooltip) = verb_press(state, kept, verb);
        assert!(press.is_none(), "{verb:?} {press:?}");
        assert_eq!(tooltip, reason, "{verb:?}");
    }
    let (press, tooltip) = status_press(state, kept);
    assert!(press.is_none(), "{press:?}");
    assert_eq!(tooltip, reason);
}

#[test]
fn once_a_snapshot_arrives_again_every_button_sends_its_message_and_has_no_retry_tooltip() {
    use crate::services_view::ALL_VERBS;
    let mut harness = Harness::new();
    selected_nginx(&mut harness);
    shown_in(&mut harness, Some(ServicesEvent::Failing("down".into())));
    shown_in(&mut harness, Some(ServicesEvent::Snapshot(Box::new(snapshot()))));
    let state = harness.app.services();
    let live = state.snapshot().expect("the list");

    for verb in ALL_VERBS {
        let (press, tooltip) = verb_press(state, live, verb);
        assert_eq!(pressed(&press), Some(format!("{:?}", Message::ServiceActionRequested(verb))), "{verb:?}");
        assert_eq!(tooltip, None, "{verb:?}");
    }
    let (press, tooltip) = status_press(state, live);
    assert_eq!(pressed(&press), Some(format!("{:?}", Message::ServiceStatusRequested)));
    assert_eq!(tooltip, None);
}

#[test]
fn a_button_that_the_unit_does_not_allow_has_no_press_message_even_without_a_failure() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    shown_in(&mut harness, Some(ServicesEvent::Snapshot(Box::new(snapshot()))));
    harness.press(Message::ServiceSelected("static.service".into()));
    let state = harness.app.services();
    let live = state.snapshot().expect("the list");

    let (press, tooltip) = verb_press(state, live, ServiceVerb::Enable);
    assert!(press.is_none() && tooltip.is_some());
    let (press, tooltip) = verb_press(state, live, ServiceVerb::Start);
    assert!(press.is_some() && tooltip.is_none());
}

#[test]
fn a_cut_service_name_offers_the_whole_name_as_a_tooltip_and_a_short_one_offers_nothing() {
    let long = format!("{}.service", "x".repeat(NAME_CELLS + 10));

    assert_eq!(name_tooltip(&long), Some(long.clone()));
    assert_eq!(name_tooltip("nginx.service"), None);
}

#[test]
fn view_logs_is_enabled_only_for_a_unit_name_the_journal_accepts() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::ServicesViewSelected);
    shown_in(&mut harness, Some(ServicesEvent::Snapshot(Box::new(snapshot()))));

    harness.press(Message::ServiceSelected("nginx.service".into()));
    let (press, hint) = logs_press(harness.app.services());
    assert_eq!(pressed(&press), Some(format!("{:?}", Message::ServiceLogsRequested)));
    assert_eq!(hint, None);

    harness.press(Message::ServiceSelected("odd\\x2dname.service".into()));
    let (press, hint) = logs_press(harness.app.services());
    assert!(press.is_none());
    assert_eq!(hint.as_deref(), Some("Its name can't be used safely in a command"));
}

#[test]
fn view_logs_needs_a_selected_unit() {
    let state = ServicesState::default();

    let (press, hint) = logs_press(&state);

    assert!(press.is_none());
    assert_eq!(hint.as_deref(), Some("Select a service first"));
}
