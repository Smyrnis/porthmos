use super::*;

fn process(pid: u32, cpu: Option<f32>, memory: f32, name: &str, user: Option<&str>) -> Process {
    Process {
        pid,
        uid: 1000,
        user: user.map(str::to_string),
        name: name.to_string(),
        command: format!("/usr/bin/{name} --flag"),
        cpu_percent: cpu,
        memory_percent: memory,
        nice: 0,
    }
}

fn order(arranged: &[&Process]) -> Vec<u32> {
    arranged.iter().map(|process| process.pid).collect()
}

#[test]
fn sorting_by_cpu_puts_unknown_last_and_is_stable_by_pid() {
    let all = [
        process(5, None, 1.0, "a", None),
        process(3, Some(10.0), 1.0, "b", None),
        process(4, Some(10.0), 1.0, "c", None),
        process(9, Some(50.0), 1.0, "d", None),
        process(2, None, 1.0, "e", None),
    ];

    assert_eq!(order(&arranged(&all, Sort::Cpu, "", None)), [9, 3, 4, 2, 5]);
}

#[test]
fn sorting_by_memory_is_descending_with_ties_by_pid() {
    let all = [process(7, None, 1.0, "a", None), process(3, None, 9.0, "b", None), process(4, None, 1.0, "c", None)];

    assert_eq!(order(&arranged(&all, Sort::Memory, "", None)), [3, 4, 7]);
}

#[test]
fn nan_values_sort_last_without_panicking() {
    let all = [process(1, Some(f32::NAN), f32::NAN, "a", None), process(2, Some(5.0), 5.0, "b", None)];

    assert_eq!(order(&arranged(&all, Sort::Cpu, "", None)), [2, 1]);
    assert_eq!(order(&arranged(&all, Sort::Memory, "", None)), [2, 1]);
}

#[test]
fn the_filter_is_case_insensitive_over_command_name_and_user() {
    let all = [
        process(2, None, 1.0, "nginx", Some("www-data")),
        process(3, None, 1.0, "postgres", Some("postgres")),
        process(4, None, 1.0, "cron", Some("root")),
    ];

    assert_eq!(order(&arranged(&all, Sort::Cpu, "NGINX", None)), [2]);
    assert_eq!(order(&arranged(&all, Sort::Cpu, "ROOT", None)), [4]);
    assert_eq!(order(&arranged(&all, Sort::Cpu, "--flag", None)).len(), 3);
    assert_eq!(order(&arranged(&all, Sort::Cpu, "  nginx  ", None)), [2]);
    assert!(arranged(&all, Sort::Cpu, "nothing like this", None).is_empty());
}

#[test]
fn the_list_is_capped_and_the_selected_process_is_kept_even_below_the_cut() {
    let all: Vec<Process> =
        (2..1002u32).map(|pid| process(pid, Some(1000.0 - pid as f32 / 2.0), 1.0, "w", None)).collect();

    let plain = arranged(&all, Sort::Cpu, "", None);
    assert_eq!(plain.len(), crate::MAX_PROCESSES);
    assert!(!order(&plain).contains(&1001));

    let kept = arranged(&all, Sort::Cpu, "", Some(1001));
    assert_eq!(kept.len(), crate::MAX_PROCESSES + 1);
    assert_eq!(*order(&kept).last().unwrap(), 1001);
}

#[test]
fn a_selected_process_hidden_by_the_filter_stays_hidden() {
    let all = [process(2, None, 1.0, "nginx", None), process(3, None, 1.0, "cron", None)];

    assert_eq!(order(&arranged(&all, Sort::Cpu, "nginx", Some(3))), [2]);
}
