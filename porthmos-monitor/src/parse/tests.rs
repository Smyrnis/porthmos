use std::time::Duration;

use super::*;
use crate::{Load, Memory, Section, Token};

const DEBIAN: &str = include_str!("../../tests/fixtures/debian.txt");
const ALPINE: &str = include_str!("../../tests/fixtures/alpine.txt");
const AMAZON: &str = include_str!("../../tests/fixtures/amazon.txt");
const CONTAINER: &str = include_str!("../../tests/fixtures/container.txt");
const MACOS: &str = include_str!("../../tests/fixtures/macos.txt");

const INTERVAL: Duration = Duration::from_secs(3);

fn token() -> Token {
    Token::from_hex("00112233445566778899aabbccddeeff").unwrap()
}

fn render(fixture: &str) -> Vec<u8> {
    fixture.replace("@@T:", &format!("@@{}:", token().as_str())).into_bytes()
}

fn linux(output: &[u8], previous: Option<&Sample>) -> (Box<Snapshot>, Sample) {
    match parse(output, &token(), previous, INTERVAL) {
        Parsed::Linux { snapshot, sample } => (snapshot, sample),
        other => panic!("expected a Linux snapshot, got {other:?}"),
    }
}

fn without_section(fixture: &str, name: &str) -> String {
    let mut kept = Vec::new();
    let mut skipping = false;
    for line in fixture.lines() {
        if let Some(found) = line.strip_prefix("@@T:") {
            skipping = found == name;
        }
        if !skipping {
            kept.push(line);
        }
    }
    kept.join("\n") + "\n"
}

#[test]
fn a_debian_fixture_parses_header_load_memory_and_cores() {
    let (snapshot, _) = linux(&render(DEBIAN), None);

    assert_eq!(snapshot.host.hostname.as_deref(), Some("web-01"));
    assert_eq!(snapshot.host.os.as_deref(), Some("Debian GNU/Linux 12 (bookworm)"));
    assert_eq!(snapshot.host.kernel.as_deref(), Some("6.1.0-18-amd64"));
    assert_eq!(snapshot.host.uptime_seconds, Some(351735));
    assert_eq!(snapshot.load, Some(Load { one: 0.52, five: 0.58, fifteen: 0.59 }));
    assert_eq!(
        snapshot.memory,
        Some(Memory {
            total: 4028124 * 1024,
            available: 2500000 * 1024,
            swap_total: 1048572 * 1024,
            swap_free: 1000572 * 1024
        })
    );
    assert_eq!(snapshot.cpu.cores, Some(2));
    assert_eq!(snapshot.uid, 1000);
    assert!(snapshot.problems.is_empty(), "{:?}", snapshot.problems);
}

#[test]
fn alpine_amazon_and_a_minimal_container_parse_the_same_fields() {
    let (alpine, _) = linux(&render(ALPINE), None);
    assert_eq!(alpine.host.os.as_deref(), Some("Alpine Linux v3.19"));
    assert_eq!((alpine.cpu.cores, alpine.uid), (Some(1), 0));
    assert_eq!(alpine.memory.map(|memory| memory.swap_total), Some(0));

    let (amazon, _) = linux(&render(AMAZON), None);
    assert_eq!(amazon.host.os.as_deref(), Some("Amazon Linux 2023"));
    assert_eq!(amazon.cpu.cores, Some(4));

    let (container, _) = linux(&render(CONTAINER), None);
    assert_eq!(container.host.os, None);
    assert_eq!(container.host.hostname.as_deref(), Some("3f2a9c0d1b7e"));
    assert_eq!(container.memory.map(|memory| memory.swap_total), Some(0));
    assert_eq!(container.cpu.cores, Some(8));
    assert!(container.problems.is_empty(), "{:?}", container.problems);
}

#[test]
fn memory_falls_back_to_free_plus_buffers_and_cache_without_memavailable() {
    let text = DEBIAN.replace("MemAvailable:    2500000 kB\n", "");

    let (snapshot, _) = linux(&render(&text), None);

    assert_eq!(snapshot.memory.map(|memory| memory.available), Some((312000 + 120000 + 1900000) * 1024));
}

#[test]
fn cpu_percent_is_none_on_the_first_sample_and_computed_on_the_second() {
    let (first, sample) = linux(&render(DEBIAN), None);
    assert_eq!(first.cpu.percent, None);
    let later = DEBIAN
        .replace("cpu  2255 34 2290 22625563 6290 127 456 0 0 0", "cpu  2355 34 2390 22625663 6290 127 456 0 0 0");

    let (second, _) = linux(&render(&later), Some(&sample));

    let percent = second.cpu.percent.expect("a percentage on the second sample");
    assert!((percent - 66.666).abs() < 0.1, "{percent}");
}

#[test]
fn cpu_percent_is_never_nan_or_out_of_range_when_counters_reset_or_stand_still() {
    let (_, sample) = linux(&render(DEBIAN), None);
    let reset = DEBIAN.replace("cpu  2255 34 2290 22625563 6290 127 456 0 0 0", "cpu  10 1 10 100 0 0 0 0 0 0");
    let (after_reset, _) = linux(&render(&reset), Some(&sample));
    assert_eq!(after_reset.cpu.percent, None);

    let (same, _) = linux(&render(DEBIAN), Some(&sample));
    assert_eq!(same.cpu.percent, None);

    let idle_went_backwards =
        DEBIAN.replace("cpu  2255 34 2290 22625563 6290 127 456 0 0 0", "cpu  9999 34 9999 22000000 100 127 456 0 0 0");
    let (odd, _) = linux(&render(&idle_went_backwards), Some(&sample));
    assert!(odd.cpu.percent.is_none_or(|percent| percent.is_finite() && (0.0..=100.0).contains(&percent)));
}

#[test]
fn a_non_linux_uname_is_unsupported_with_the_os_name() {
    match parse(&render(MACOS), &token(), None, INTERVAL) {
        Parsed::Unsupported { os } => assert_eq!(os, "Darwin"),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn empty_garbage_and_non_utf8_output_are_garbled_or_lossy_never_a_panic() {
    assert!(matches!(parse(b"", &token(), None, INTERVAL), Parsed::Garbled));
    assert!(matches!(parse(b"\xff\xfe\x00 not a probe", &token(), None, INTERVAL), Parsed::Garbled));
    assert!(matches!(parse(b"\n\n\n", &token(), None, INTERVAL), Parsed::Garbled));
    let mut bytes = render(DEBIAN);
    let at = bytes.windows(6).position(|window| window == b"web-01").unwrap();
    bytes[at] = 0xff;

    let (snapshot, _) = linux(&bytes, None);

    assert!(snapshot.host.hostname.as_deref().is_some_and(|name| name.contains('\u{fffd}')));
}

#[test]
fn a_missing_section_is_listed_in_problems_and_the_rest_still_parse() {
    let text = without_section(DEBIAN, "loadavg");

    let (snapshot, _) = linux(&render(&text), None);

    assert_eq!(snapshot.load, None);
    assert!(snapshot.problems.contains(&Section::Load));
    assert!(snapshot.memory.is_some());
    assert_eq!(snapshot.problems.len(), 1, "{:?}", snapshot.problems);
}

#[test]
fn a_truncated_output_loses_only_the_section_it_was_cut_in() {
    let full = render(DEBIAN);
    let marker = format!("@@{}:meminfo\nMemTotal:", token().as_str());
    let cut = full.windows(marker.len()).position(|window| window == marker.as_bytes()).unwrap() + marker.len();

    let (snapshot, _) = linux(&full[..cut], None);

    assert_eq!(snapshot.memory, None);
    assert!(snapshot.problems.contains(&Section::Memory));
    assert!(snapshot.load.is_some());
    assert!(snapshot.host.hostname.is_some());
}

#[test]
fn the_host_is_a_problem_only_when_every_part_of_it_is_missing() {
    let mut text = DEBIAN.to_string();
    for name in ["hostname", "osrelease", "kernel", "uptime"] {
        text = without_section(&text, name);
    }

    let (snapshot, _) = linux(&render(&text), None);

    assert!(snapshot.problems.contains(&Section::Host));
    let (partial, _) = linux(&render(&without_section(DEBIAN, "kernel")), None);
    assert!(!partial.problems.contains(&Section::Host));
    assert_eq!(partial.host.kernel, None);
}

#[test]
fn a_line_longer_than_the_cap_is_cut_to_the_cap_without_blanking_its_neighbours() {
    let text = DEBIAN.replace("web-01", &"a".repeat(100_000));

    let (snapshot, _) = linux(&render(&text), None);

    assert_eq!(snapshot.host.hostname.as_deref().map(|name| name.chars().count()), Some(255));
    assert_eq!(snapshot.host.kernel.as_deref(), Some("6.1.0-18-amd64"));
}

#[test]
fn a_very_long_but_allowed_hostname_is_shortened_and_cleaned_of_control_characters() {
    let text = DEBIAN.replace("web-01", &format!("a\u{7}b{}", "c".repeat(3000)));

    let (snapshot, _) = linux(&render(&text), None);

    let hostname = snapshot.host.hostname.unwrap();
    assert!(hostname.chars().count() <= 255 && !hostname.chars().any(char::is_control), "{hostname:?}");
    assert!(hostname.starts_with("ab"));
}

#[test]
fn output_over_the_cap_is_truncated_not_a_panic_and_loses_the_cut_section() {
    let mut bytes = render(&DEBIAN.replace("@@T:procs", &format!("{}\n@@T:procs", "filler line\n".repeat(150_000))));
    assert!(bytes.len() > crate::MAX_OUTPUT_BYTES);
    bytes.extend_from_slice(b"\n");

    let (snapshot, _) = linux(&bytes, None);

    assert!(snapshot.host.hostname.is_some());
    assert!(snapshot.memory.is_some());
}

#[test]
fn markers_with_the_wrong_token_are_data_and_a_repeated_marker_never_replaces_a_section() {
    let wrong = "@@deadbeefdeadbeef:meminfo\nMemTotal: 1 kB\n@@deadbeefdeadbeef:end\n";
    let repeated = format!("@@{}:meminfo\nMemTotal: 2 kB\n", token().as_str());
    let text = DEBIAN
        .replace("@@T:uptime\n", &format!("{wrong}@@T:uptime\n"))
        .replace("@@T:stat\n", &format!("{repeated}@@T:stat\n"));

    let (snapshot, _) = linux(&render(&text), None);

    assert_eq!(snapshot.memory.map(|memory| memory.total), Some(4028124 * 1024));
    assert_eq!(snapshot.host.uptime_seconds, Some(351735));
    assert_eq!(snapshot.cpu.cores, Some(2));
}

#[test]
fn lines_after_a_repeated_marker_are_dropped_until_the_next_real_section() {
    let repeated = format!("@@{}:loadavg\n9.0 9.0 9.0 1/1 1\n", token().as_str());
    let text = DEBIAN.replace("@@T:meminfo\n", &format!("{repeated}@@T:meminfo\n"));

    let (snapshot, _) = linux(&render(&text), None);

    assert_eq!(snapshot.load.map(|load| load.one), Some(0.52));
    assert!(snapshot.memory.is_some());
}

#[test]
fn nan_and_negative_numbers_in_load_and_uptime_are_rejected() {
    let text = DEBIAN.replace("0.52 0.58 0.59", "nan inf -1").replace("351735.47", "-5");

    let (snapshot, _) = linux(&render(&text), None);

    assert_eq!(snapshot.load, None);
    assert_eq!(snapshot.host.uptime_seconds, None);
}

#[test]
fn memory_with_a_missing_total_or_absurd_numbers_never_overflows() {
    let (no_total, _) = linux(&render(&DEBIAN.replace("MemTotal:        4028124 kB\n", "")), None);
    assert_eq!(no_total.memory, None);

    let huge = DEBIAN.replace("4028124 kB", "18446744073709551615 kB");
    let (snapshot, _) = linux(&render(&huge), None);
    assert_eq!(snapshot.memory.map(|memory| memory.total), Some(u64::MAX));
}

use crate::testing::{ProcSpec, with_processes};

fn linux_after(output: &[u8], previous: Option<&Sample>, elapsed: Duration) -> (Box<Snapshot>, Sample) {
    match parse(output, &token(), previous, elapsed) {
        Parsed::Linux { snapshot, sample } => (snapshot, sample),
        other => panic!("expected a Linux snapshot, got {other:?}"),
    }
}

fn pids(snapshot: &Snapshot) -> Vec<u32> {
    snapshot.processes.iter().map(|process| process.pid).collect()
}

#[test]
fn the_debian_fixture_lists_its_disks_without_the_pseudo_file_systems() {
    let (snapshot, _) = linux(&render(DEBIAN), None);

    let mounts: Vec<&str> = snapshot.disks.iter().map(|disk| disk.mount.as_str()).collect();
    assert_eq!(mounts, ["/", "/boot/efi"]);
    assert_eq!(snapshot.disks[0].device, "/dev/vda1");
    assert_eq!(snapshot.disks[0].total, 41151808 * 1024);
    assert_eq!(snapshot.disks[0].used, 12345678 * 1024);
}

#[test]
fn df_output_with_spaces_in_mount_names_and_wrapped_device_lines_parses() {
    let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
              /dev/sdb1  1000 500 500 50% /mnt/my disk\n\
              /dev/mapper/a-very-long-volume-group-name\n\
              \x20            2000 100 1900 5% /data\n\
              nas:/export  3000 3000 0 100% /mnt/nas\n\
              none 0 0 0 - /mnt/empty\n";
    let text = DEBIAN
        .replace(&DEBIAN[DEBIAN.find("@@T:df").unwrap()..DEBIAN.find("@@T:passwd").unwrap()], &format!("@@T:df\n{df}"));

    let (snapshot, _) = linux(&render(&text), None);

    let found: Vec<(&str, &str, u64, u64)> =
        snapshot.disks.iter().map(|disk| (disk.device.as_str(), disk.mount.as_str(), disk.total, disk.used)).collect();
    assert_eq!(
        found,
        [
            ("/dev/sdb1", "/mnt/my disk", 1000 * 1024, 500 * 1024),
            ("/dev/mapper/a-very-long-volume-group-name", "/data", 2000 * 1024, 100 * 1024),
            ("nas:/export", "/mnt/nas", 3000 * 1024, 3000 * 1024),
            ("none", "/mnt/empty", 0, 0),
        ]
    );
}

#[test]
fn a_garbled_df_section_is_a_problem_for_disks_only() {
    let text = DEBIAN.replace(
        &DEBIAN[DEBIAN.find("@@T:df").unwrap()..DEBIAN.find("@@T:passwd").unwrap()],
        "@@T:df\nnot a table\nat all\n",
    );

    let (snapshot, _) = linux(&render(&text), None);

    assert!(snapshot.disks.is_empty());
    assert!(snapshot.problems.contains(&Section::Disks));
    assert!(snapshot.memory.is_some());
}

#[test]
fn network_rates_come_from_deltas_and_loopback_sorts_last() {
    let (first, sample) = linux(&render(DEBIAN), None);
    let names: Vec<&str> = first.interfaces.iter().map(|interface| interface.name.as_str()).collect();
    assert_eq!(names, ["eth0", "lo"]);
    assert!(
        first.interfaces.iter().all(|interface| interface.rx_per_second.is_none() && interface.tx_per_second.is_none())
    );
    let later = DEBIAN.replace("98765432", "98768432").replace("12345678    70000", "12351678    70000");

    let (second, _) = linux(&render(&later), Some(&sample));

    let eth0 = &second.interfaces[0];
    assert_eq!((eth0.rx_per_second, eth0.tx_per_second), (Some(1000), Some(2000)));
    let lo = &second.interfaces[1];
    assert_eq!((lo.rx_per_second, lo.tx_per_second), (Some(0), Some(0)));
}

#[test]
fn a_counter_that_goes_backwards_or_a_clock_that_stood_still_gives_no_rate() {
    let (_, sample) = linux(&render(DEBIAN), None);
    let backwards = DEBIAN.replace("98765432", "100");
    let (after, _) = linux(&render(&backwards), Some(&sample));
    assert_eq!(after.interfaces[0].rx_per_second, None);
    assert!(after.interfaces[0].tx_per_second.is_some());

    let (frozen, _) = linux_after(&render(DEBIAN), Some(&sample), Duration::ZERO);
    assert!(frozen.interfaces.iter().all(|interface| interface.rx_per_second.is_none()));
}

#[test]
fn a_netdev_section_with_hostile_lines_never_panics_and_keeps_good_interfaces() {
    let hostile = "Inter-|\n face |\n    lo: 1 2 3\n  eth0:99999999999999999999999 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15\nnonsense line\n  eth1:5 0 0 0 0 0 0 0 7 0 0 0 0 0 0 0\n";
    let text = DEBIAN.replace(
        &DEBIAN[DEBIAN.find("@@T:netdev").unwrap()..DEBIAN.find("@@T:df").unwrap()],
        &format!("@@T:netdev\n{hostile}"),
    );

    let (snapshot, _) = linux(&render(&text), None);

    let names: Vec<&str> = snapshot.interfaces.iter().map(|interface| interface.name.as_str()).collect();
    assert_eq!(names, ["eth1"]);
}

#[test]
fn the_debian_fixture_lists_its_processes_with_users_commands_and_memory() {
    let (snapshot, _) = linux(&render(DEBIAN), None);

    assert_eq!(pids(&snapshot).len(), 3);
    let nginx = snapshot.processes.iter().find(|process| process.pid == 512).unwrap();
    assert_eq!((nginx.uid, nginx.user.as_deref(), nginx.name.as_str()), (33, Some("www-data"), "nginx"));
    assert_eq!(nginx.command, "nginx: worker process");
    assert_eq!(nginx.nice, -5);
    assert_eq!(nginx.cpu_percent, None);
    let expected = 5000.0 * 4096.0 / (4028124.0 * 1024.0) * 100.0;
    assert!((f64::from(nginx.memory_percent) - expected).abs() < 0.01, "{}", nginx.memory_percent);
}

#[test]
fn processes_parse_names_with_spaces_and_parentheses_in_comm() {
    let mut odd = ProcSpec::new(5, "a (b) c");
    odd.cmdline = "x".into();

    let (snapshot, _) = linux(&render(&with_processes(DEBIAN, &[odd])), None);

    assert_eq!(snapshot.processes[0].name, "a (b) c");
}

#[test]
fn cmdline_with_newlines_and_tabs_becomes_one_capped_line_and_overlong_ones_fall_back_to_the_name() {
    let mut multi = ProcSpec::new(5, "multi");
    multi.cmdline = "line1\nline2\t\ttab".into();
    let mut long = ProcSpec::new(6, "long");
    long.cmdline = "x".repeat(3000);
    let mut huge = ProcSpec::new(7, "huge");
    huge.cmdline = "y".repeat(6000);
    let kernel = ProcSpec { cmdline: String::new(), ..ProcSpec::new(8, "kthreadd") };

    let (snapshot, _) = linux(&render(&with_processes(DEBIAN, &[multi, long, huge, kernel])), None);

    let command = |pid: u32| snapshot.processes.iter().find(|process| process.pid == pid).unwrap().command.clone();
    assert_eq!(command(5), "line1 line2 tab");
    assert_eq!(command(6).chars().count(), 512);
    assert_eq!(command(7).chars().count(), 512);
    assert_eq!(command(8), "[kthreadd]");
}

#[test]
fn a_process_without_a_status_line_is_kept_with_an_unknown_uid() {
    let mut orphan = ProcSpec::new(9, "orphan");
    orphan.uid = None;

    let (snapshot, _) = linux(&render(&with_processes(DEBIAN, &[orphan])), None);

    assert_eq!((snapshot.processes[0].uid, snapshot.processes[0].user.clone()), (u32::MAX, None));
}

#[test]
fn thousands_of_processes_are_reduced_to_the_busiest_by_cpu_and_by_memory() {
    let specs: Vec<ProcSpec> = (2..3002u32)
        .map(|pid| {
            let mut spec = ProcSpec::new(pid, "worker");
            spec.ticks = u64::from(pid) * 10;
            spec.rss_pages = u64::from(3002 - pid) * 10;
            spec
        })
        .collect();
    let first = with_processes(DEBIAN, &specs);
    let (_, sample) = linux(&render(&first), None);
    let more: Vec<ProcSpec> = specs
        .iter()
        .map(|spec| ProcSpec { ticks: spec.ticks + u64::from(spec.pid), ..ProcSpec::new(spec.pid, "worker") })
        .map(|mut spec| {
            spec.rss_pages = u64::from(3002 - spec.pid) * 10;
            spec
        })
        .collect();
    let later = DEBIAN
        .replace("cpu  2255 34 2290 22625563 6290 127 456 0 0 0", "cpu  3255 34 3290 22626563 6290 127 456 0 0 0");
    let second = with_processes(&later, &more);

    let (snapshot, _) = linux(&render(&second), Some(&sample));

    assert!(snapshot.processes.len() <= 2 * crate::MAX_PROCESSES, "{}", snapshot.processes.len());
    let kept = pids(&snapshot);
    assert!(kept.contains(&3001), "the busiest by cpu is kept");
    assert!(kept.contains(&2), "the biggest by memory is kept");
    assert!(!kept.contains(&1500), "a middling process is dropped");
}

#[test]
fn process_cpu_percent_is_a_percent_of_one_core_and_never_negative_or_nan() {
    let mut busy = ProcSpec::new(20, "busy");
    busy.ticks = 100;
    let (_, sample) = linux(&render(&with_processes(DEBIAN, &[busy])), None);
    let later_cpu = DEBIAN
        .replace("cpu  2255 34 2290 22625563 6290 127 456 0 0 0", "cpu  2355 34 2390 22625663 6290 127 456 0 0 0");
    let mut busier = ProcSpec::new(20, "busy");
    busier.ticks = 150;

    let (second, _) = linux(&render(&with_processes(&later_cpu, &[busier])), Some(&sample));

    let percent = second.processes[0].cpu_percent.expect("a rate on the second sample");
    assert!((percent - 33.33).abs() < 0.1, "{percent}");

    let mut reset = ProcSpec::new(20, "busy");
    reset.ticks = 10;
    let (after_reset, _) = linux(&render(&with_processes(&later_cpu, &[reset])), Some(&sample));
    assert_eq!(after_reset.processes[0].cpu_percent, None);

    let (standing, _) = linux(&render(&with_processes(DEBIAN, &[ProcSpec::new(20, "busy")])), Some(&sample));
    assert_eq!(standing.processes[0].cpu_percent, None);
}

#[test]
fn a_process_that_appeared_between_samples_has_no_cpu_percent_yet() {
    let (_, sample) = linux(&render(&with_processes(DEBIAN, &[ProcSpec::new(20, "old")])), None);
    let later_cpu = DEBIAN
        .replace("cpu  2255 34 2290 22625563 6290 127 456 0 0 0", "cpu  2355 34 2390 22625663 6290 127 456 0 0 0");

    let (second, _) = linux(
        &render(&with_processes(&later_cpu, &[ProcSpec::new(20, "old"), ProcSpec::new(21, "new")])),
        Some(&sample),
    );

    let rate = |pid: u32| second.processes.iter().find(|process| process.pid == pid).unwrap().cpu_percent;
    assert!(rate(20).is_some());
    assert_eq!(rate(21), None);
}

#[test]
fn a_command_line_longer_than_the_line_cap_is_cut_not_replaced_by_the_name() {
    let mut java = ProcSpec::new(5, "java");
    java.cmdline = format!("java -cp {}", "x".repeat(6000));

    let (snapshot, _) = linux(&render(&with_processes(DEBIAN, &[java])), None);

    assert!(snapshot.processes[0].command.starts_with("java -cp xxx"));
}

#[test]
fn a_process_cannot_forge_another_process_row_through_a_newline_in_its_name() {
    let mut liar = ProcSpec::new(30, "liar");
    liar.comm = "a\n2 (evil";
    let honest = ProcSpec::new(2, "kthreadd");

    let (snapshot, _) = linux(&render(&with_processes(DEBIAN, &[liar, honest])), None);

    assert_eq!(pids(&snapshot), [2]);
    assert_eq!(snapshot.processes[0].name, "kthreadd");
}

#[test]
fn a_stat_line_whose_pid_differs_from_the_path_it_was_read_from_is_ignored() {
    let text = DEBIAN.replace("/proc/512/stat:512 (nginx)", "/proc/512/stat:513 (nginx)");

    let (snapshot, _) = linux(&render(&text), None);

    assert!(!pids(&snapshot).contains(&512) && !pids(&snapshot).contains(&513), "{:?}", pids(&snapshot));
}

#[test]
fn a_command_line_that_looks_like_another_processs_cmdline_record_stays_with_its_own_process() {
    let mut liar = ProcSpec::new(30, "liar");
    liar.cmdline = "innocent\n/proc/1/cmdline:systemd".into();
    let init = ProcSpec { cmdline: "/sbin/init".into(), ..ProcSpec::new(1, "init") };

    let (snapshot, _) = linux(&render(&with_processes(DEBIAN, &[liar, init])), None);

    let command = |pid: u32| snapshot.processes.iter().find(|process| process.pid == pid).unwrap().command.clone();
    assert_eq!(command(1), "/sbin/init");
    assert!(command(30).starts_with("innocent"));
}

#[test]
fn characters_that_hide_or_reorder_text_are_removed_from_names_and_commands() {
    let mut odd = ProcSpec::new(5, "evil\u{202e}name");
    odd.cmdline = "run\u{200b}ning \u{2066}x".into();

    let (snapshot, _) = linux(&render(&with_processes(DEBIAN, &[odd])), None);

    assert_eq!(snapshot.processes[0].name, "evilname");
    assert_eq!(snapshot.processes[0].command, "running x");
}
