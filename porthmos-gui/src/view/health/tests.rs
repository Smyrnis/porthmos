use std::time::{Duration, Instant};

use porthmos_core::{
    Cpu, Disk, Event, HealthSection as Section, HealthSnapshot, Host, Interface, Load, Location, Memory, MonitorState,
    PROCESS_ACTION_MAX_AGE, ProcessSort,
};

use super::*;
use crate::health_view::tests::{process, snapshot};
use crate::testing::Harness;

fn connect(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: porthmos_core::VfsPath::new("/"),
        entries: Vec::new(),
    });
}

fn full() -> HealthSnapshot {
    HealthSnapshot {
        host: Host {
            hostname: Some("web-01".into()),
            os: Some("Debian GNU/Linux 12".into()),
            kernel: Some("6.1.0".into()),
            uptime_seconds: Some(100_000),
        },
        cpu: Cpu { cores: Some(4), percent: Some(42.0) },
        load: Some(Load { one: 0.5, five: 0.6, fifteen: 0.7 }),
        memory: Some(Memory { total: 4096, available: 1024, swap_total: 2048, swap_free: 2048 }),
        disks: vec![Disk { mount: "/".into(), device: "/dev/vda1".into(), total: 1000, used: 990 }],
        interfaces: vec![
            Interface { name: "eth0".into(), rx_per_second: Some(1000), tx_per_second: None },
            Interface { name: "lo".into(), rx_per_second: None, tx_per_second: None },
        ],
        processes: vec![process(10), process(11)],
        uid: 1000,
        problems: Vec::new(),
    }
}

fn shown_in(harness: &mut Harness, state: Option<MonitorState>) {
    if let Some(state) = state {
        harness.feed(Event::Monitor { session: 3, state });
    }
    let _ = view(&harness.app);
}

#[test]
fn the_tab_builds_in_every_state() {
    let mut harness = Harness::new();
    harness.press(crate::app::Message::HealthViewSelected);
    shown_in(&mut harness, None);

    connect(&mut harness, 3, "alpha");
    shown_in(&mut harness, None);
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(full()))));
    let mut partial = full();
    partial.problems = vec![Section::Load, Section::Disks];
    partial.load = None;
    partial.memory = None;
    partial.disks.clear();
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(partial))));
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(snapshot(&[])))));
    shown_in(&mut harness, Some(MonitorState::Failing("the server didn't answer".into())));
    shown_in(&mut harness, Some(MonitorState::Unavailable("Health is only available for Linux".into())));
    let many: Vec<u32> = (2..202).collect();
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(snapshot(&many)))));
    connect(&mut harness, 4, "beta");
    shown_in(&mut harness, None);
}

#[test]
fn the_process_table_respects_sort_and_filter() {
    let mut harness = Harness::new();
    let mut snapshot = full();
    snapshot.processes = vec![process(10), process(30), process(20)];
    snapshot.processes[1].name = "nginx".into();
    snapshot.processes[1].command = "nginx: worker".into();
    snapshot.processes[2].memory_percent = 9.0;
    let mut state = HealthState::default();

    let pids = |state: &HealthState| shown(state, &snapshot).iter().map(|process| process.pid).collect::<Vec<_>>();
    assert_eq!(pids(&state), [30, 20, 10]);
    state.sort = ProcessSort::Memory;
    assert_eq!(pids(&state), [20, 10, 30]);
    state.filter = "NGINX".into();
    assert_eq!(pids(&state), [30]);
    state.filter = "no such process".into();
    assert!(pids(&state).is_empty());
    let _ = &mut harness;
}

fn pressed(press: &Option<crate::app::Message>) -> Option<String> {
    press.as_ref().map(|message| format!("{message:?}"))
}

fn watching(harness: &mut Harness) {
    connect(harness, 3, "alpha");
    harness.press(crate::app::Message::HealthViewSelected);
    shown_in(harness, Some(MonitorState::Snapshot(Box::new(full()))));
    harness.press(crate::app::Message::HealthProcessSelected(10));
}

#[test]
fn while_updating_has_failed_no_action_has_a_press_message_and_each_carries_the_retry_first_tooltip() {
    use crate::app::Message;
    let mut harness = Harness::new();
    watching(&mut harness);
    shown_in(&mut harness, Some(MonitorState::Failing("down".into())));
    let state = harness.app.health();
    let kept = state.snapshot().expect("the dimmed table");
    let reason = Some(porthmos_core::i18n::text("gui.health.retry_first"));

    for message in [Message::HealthTerminateRequested, Message::HealthKillRequested, Message::HealthReniceRequested] {
        let (press, tooltip) = action_press(state, kept, message.clone(), Instant::now());
        assert!(press.is_none(), "{message:?} {press:?}");
        assert_eq!(tooltip, reason, "{message:?}");
    }
}

#[test]
fn once_a_snapshot_arrives_again_every_action_sends_its_message_and_has_no_tooltip() {
    use crate::app::Message;
    let mut harness = Harness::new();
    watching(&mut harness);
    shown_in(&mut harness, Some(MonitorState::Failing("down".into())));
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(full()))));
    let state = harness.app.health();
    let live = state.snapshot().expect("the table");

    for message in [Message::HealthTerminateRequested, Message::HealthKillRequested, Message::HealthReniceRequested] {
        let (press, tooltip) = action_press(state, live, message.clone(), Instant::now());
        assert_eq!(pressed(&press), Some(format!("{message:?}")));
        assert_eq!(tooltip, None, "{message:?}");
    }
}

#[test]
fn another_users_process_has_no_press_message_and_the_needs_root_tooltip_and_no_selection_has_neither() {
    use crate::app::Message;
    let mut harness = Harness::new();
    let mut other = full();
    other.processes[0].uid = 33;
    connect(&mut harness, 3, "alpha");
    harness.press(Message::HealthViewSelected);
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(other))));
    let state = harness.app.health();
    let live = state.snapshot().expect("the table");
    let (press, tooltip) = action_press(state, live, Message::HealthKillRequested, Instant::now());
    assert!(press.is_none() && tooltip.is_none());

    harness.press(Message::HealthProcessSelected(10));
    let state = harness.app.health();
    let live = state.snapshot().expect("the table");
    let (press, tooltip) = action_press(state, live, Message::HealthKillRequested, Instant::now());
    assert!(press.is_none());
    assert_eq!(tooltip, Some(porthmos_core::i18n::label("Needs root to change this process")));
}

#[test]
fn a_dimmed_row_uses_the_dim_colour_over_the_selected_one() {
    let harness = Harness::new();
    let palette = harness.app.palette();
    assert_eq!(row_color(false, true, palette), palette.fg_dim);
    assert_eq!(row_color(true, true, palette), palette.fg_dim);
    assert_eq!(row_color(true, false, palette), palette.fg_focus);
    assert_eq!(row_color(false, false, palette), palette.fg);
}

#[test]
fn the_summary_is_drawn_in_the_dim_palette_only_while_updating_has_failed() {
    let harness = Harness::new();
    let palette = harness.app.palette();

    let live = summary_palette(palette, false);
    let faded = summary_palette(palette, true);

    assert_eq!(
        (live.fg, live.fg_muted, live.fg_bright, live.accent, live.err),
        (palette.fg, palette.fg_muted, palette.fg_bright, palette.accent, palette.err)
    );
    for color in [faded.fg, faded.fg_muted, faded.fg_bright, faded.accent, faded.err] {
        assert_eq!(color, palette.fg_dim);
    }
    assert_eq!((faded.bg, faded.line, faded.fg_dim), (palette.bg, palette.line, palette.fg_dim));
}

#[test]
fn the_first_event_of_a_pair_shows_unknown_cpu_and_rates_until_the_second_fills_them_in() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(crate::app::Message::HealthViewSelected);
    let mut first = full();
    first.cpu.percent = None;
    first.interfaces[0].rx_per_second = None;
    first.processes[0].cpu_percent = None;
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(first))));
    let early = harness.app.health().snapshot().expect("the first event of a pair is shown");
    let unknown = "\u{2026}";
    assert_eq!(cpu_text(&early.cpu), format!("{unknown} \u{b7} 4 cores"));
    assert_eq!(rates_text(&early.interfaces[0]), format!("\u{2193} {unknown} \u{b7} \u{2191} {unknown}"));
    assert_eq!(percent_text(early.processes[0].cpu_percent), unknown);

    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(full()))));
    let later = harness.app.health().snapshot().expect("the second event");
    let down = crate::tunnels_view::rate_text(1000);
    assert_eq!(cpu_text(&later.cpu), "42% \u{b7} 4 cores");
    assert_eq!(rates_text(&later.interfaces[0]), format!("\u{2193} {down} \u{b7} \u{2191} {unknown}"));
}

#[test]
fn a_failure_after_a_snapshot_and_a_failure_with_none_both_build() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(crate::app::Message::HealthViewSelected);
    shown_in(&mut harness, Some(MonitorState::Failing("early".into())));
    shown_in(&mut harness, Some(MonitorState::Snapshot(Box::new(full()))));
    shown_in(&mut harness, Some(MonitorState::Failing("late".into())));
    assert!(harness.app.health().snapshot().is_some());
}

fn fresh_state() -> HealthState {
    let mut state = HealthState::default();
    state.begin(3);
    state.apply(3, MonitorState::Snapshot(Box::new(full())));
    state.selected = Some(10);
    state
}

const PROCESS_ACTIONS: [crate::app::Message; 3] = [
    crate::app::Message::HealthTerminateRequested,
    crate::app::Message::HealthKillRequested,
    crate::app::Message::HealthReniceRequested,
];

#[test]
fn every_process_action_works_on_a_list_inside_the_age_limit() {
    let state = fresh_state();
    let live = state.snapshot().expect("the table");
    let inside = Instant::now() + PROCESS_ACTION_MAX_AGE - Duration::from_secs(5);

    for message in PROCESS_ACTIONS {
        let (press, tooltip) = action_press(&state, live, message.clone(), inside);
        assert_eq!(pressed(&press), Some(format!("{message:?}")));
        assert_eq!(tooltip, None, "{message:?}");
    }
}

#[test]
fn every_process_action_is_disabled_with_the_old_list_reason_once_the_list_is_older_than_the_limit() {
    let state = fresh_state();
    let live = state.snapshot().expect("the table");
    let later = Instant::now() + PROCESS_ACTION_MAX_AGE + Duration::from_secs(5);
    let reason = Some(porthmos_core::i18n::text("gui.health.list_is_old"));

    for message in PROCESS_ACTIONS {
        let (press, tooltip) = action_press(&state, live, message.clone(), later);
        assert!(press.is_none(), "{message:?} {press:?}");
        assert_eq!(tooltip, reason, "{message:?}");
    }
}

#[test]
fn a_new_snapshot_enables_the_process_actions_again_after_the_list_went_old() {
    let mut state = fresh_state();
    let later = Instant::now() + PROCESS_ACTION_MAX_AGE + Duration::from_secs(5);
    assert!(action_press(&state, state.snapshot().unwrap(), PROCESS_ACTIONS[0].clone(), later).0.is_none());

    state.apply(3, MonitorState::Snapshot(Box::new(full())));
    let just_after = Instant::now() + Duration::from_millis(1);

    let (press, tooltip) = action_press(&state, state.snapshot().unwrap(), PROCESS_ACTIONS[0].clone(), just_after);
    assert!(press.is_some() && tooltip.is_none());
}

#[test]
fn a_failing_update_keeps_the_retry_first_reason_even_when_the_kept_list_is_also_old() {
    let mut state = fresh_state();
    state.apply(3, MonitorState::Failing("down".into()));
    let live = state.snapshot().expect("the dimmed table");
    let later = Instant::now() + PROCESS_ACTION_MAX_AGE + Duration::from_secs(5);
    let reason = Some(porthmos_core::i18n::text("gui.health.retry_first"));

    for message in PROCESS_ACTIONS {
        let (press, tooltip) = action_press(&state, live, message.clone(), later);
        assert!(press.is_none());
        assert_eq!(tooltip, reason, "{message:?}");
    }
}
