use porthmos_core::{
    Cpu, Disk, Event, HealthSection as Section, HealthSnapshot, Host, Interface, Load, Location, Memory, MonitorState,
    ProcessSort,
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
