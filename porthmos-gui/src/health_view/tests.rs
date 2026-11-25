use porthmos_core::{Cpu, Disk, HealthProcess, HealthSnapshot, Host, Memory, MonitorState};

use super::*;

pub(crate) fn process(pid: u32) -> HealthProcess {
    HealthProcess {
        pid,
        uid: 1000,
        user: Some("mine".to_string()),
        name: format!("proc{pid}"),
        command: format!("/usr/bin/proc{pid} --flag"),
        cpu_percent: Some(pid as f32 / 10.0),
        memory_percent: 1.5,
        nice: 0,
    }
}

pub(crate) fn snapshot(pids: &[u32]) -> HealthSnapshot {
    HealthSnapshot {
        host: Host::default(),
        cpu: Cpu::default(),
        load: None,
        memory: None,
        disks: Vec::new(),
        interfaces: Vec::new(),
        processes: pids.iter().copied().map(process).collect(),
        uid: 1000,
        problems: Vec::new(),
    }
}

#[test]
fn percent_and_uptime_text_cover_unknown_zero_and_huge_values() {
    assert_eq!(percent_text(None), "\u{2026}");
    assert_eq!(percent_text(Some(f32::NAN)), "\u{2026}");
    assert_eq!(percent_text(Some(-3.0)), "\u{2026}");
    assert_eq!(percent_text(Some(f32::INFINITY)), "\u{2026}");
    assert_eq!(percent_text(Some(0.0)), "0.0%");
    assert_eq!(percent_text(Some(3.46)), "3.5%");
    assert_eq!(percent_text(Some(12.34)), "12%");
    assert_eq!(percent_text(Some(100.0)), "100%");
    assert_eq!(percent_text(Some(1.0e9)), "1000000000%");

    assert_eq!(uptime_text(0), "0m");
    assert_eq!(uptime_text(59), "0m");
    assert_eq!(uptime_text(60 * 42), "42m");
    assert_eq!(uptime_text(3600 * 5 + 60 * 12), "5h 12m");
    assert_eq!(uptime_text(86_400 * 3 + 3600 * 4 + 60), "3d 4h");
    assert!(uptime_text(u64::MAX).ends_with('h'));
}

#[test]
fn disk_and_memory_text_never_divide_by_zero_or_overflow() {
    let disk = Disk { mount: "/".into(), device: "/dev/vda1".into(), total: 2048, used: 1024 };
    assert_eq!(disk_text(&disk), "1.0 KB / 2.0 KB (50%)");
    let empty = Disk { total: 0, used: 0, ..disk.clone() };
    assert!(disk_text(&empty).ends_with("(0%)"));
    let overfull = Disk { total: 100, used: 400, ..disk };
    assert!(disk_text(&overfull).ends_with("(100%)"));

    let memory = Memory { total: 1000, available: 400, swap_total: 0, swap_free: 0 };
    assert!(memory_text(&memory).ends_with("(60%)"), "{}", memory_text(&memory));
    assert_eq!(swap_text(&memory), None);
    let with_swap = Memory { swap_total: 1000, swap_free: 250, ..memory };
    assert!(swap_text(&with_swap).unwrap().ends_with("(75%)"));
    let odd = Memory { total: 10, available: 99, swap_total: 5, swap_free: 50 };
    assert!(memory_text(&odd).ends_with("(0%)"));
    assert!(swap_text(&odd).unwrap().ends_with("(0%)"));
}

#[test]
fn the_state_keeps_the_filter_and_sort_across_snapshots_and_clears_selection_when_the_process_disappears() {
    let mut state = HealthState { filter: "nginx".to_string(), sort: ProcessSort::Memory, ..HealthState::default() };
    state.begin(3);
    state.selected = Some(20);

    state.apply(3, MonitorState::Snapshot(Box::new(snapshot(&[10, 20]))));
    assert_eq!(state.selected, Some(20));
    state.apply(3, MonitorState::Snapshot(Box::new(snapshot(&[10, 30]))));

    assert_eq!(state.selected, None);
    assert_eq!((state.filter.as_str(), state.sort), ("nginx", ProcessSort::Memory));
    assert!(matches!(&state.state, Some(MonitorState::Snapshot(snapshot)) if snapshot.processes.len() == 2));
}

#[test]
fn events_for_another_session_are_ignored_and_clear_resets_everything_but_the_view_settings() {
    let mut state = HealthState { filter: "x".to_string(), ..HealthState::default() };
    state.begin(3);

    state.apply(4, MonitorState::Snapshot(Box::new(snapshot(&[1]))));
    assert!(state.state.is_none());
    state.apply(3, MonitorState::Snapshot(Box::new(snapshot(&[1]))));
    state.selected = Some(1);
    state.clear();

    assert_eq!((state.session, state.selected), (None, None));
    assert!(state.state.is_none());
    assert_eq!(state.filter, "x");
}
