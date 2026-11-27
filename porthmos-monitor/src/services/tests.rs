use super::*;
use crate::{Token, UNKNOWN_UID};

const DEBIAN: &str = include_str!("../../tests/fixtures/services_debian.txt");
const ALPINE: &str = include_str!("../../tests/fixtures/services_alpine.txt");
const MACOS: &str = include_str!("../../tests/fixtures/services_macos.txt");

fn token() -> Token {
    Token::from_hex("00112233445566778899aabbccddeeff").unwrap()
}

fn render(fixture: &str) -> Vec<u8> {
    fixture.replace("@@T:", &format!("@@{}:", token().as_str())).into_bytes()
}

fn linux(output: &[u8]) -> Box<ServicesSnapshot> {
    match parse_services(output, &token()) {
        ParsedServices::Linux(snapshot) => snapshot,
        other => panic!("expected a Linux snapshot, got {other:?}"),
    }
}

fn synthetic(units: &str, files: &str) -> Vec<u8> {
    render(&format!("@@T:os\nLinux\n@@T:uid\n0\n@@T:systemd\nyes\n@@T:units\n{units}@@T:files\n{files}@@T:end\n"))
}

fn named(name: &str) -> UnitName {
    UnitName::new(name).unwrap()
}

fn find<'a>(snapshot: &'a ServicesSnapshot, name: &str) -> &'a Unit {
    snapshot.units.iter().find(|unit| unit.name == name).unwrap_or_else(|| panic!("no unit {name}"))
}

fn unit(name: &str, load: &str, active: ActiveState, sub: &str, boot: Option<BootState>) -> Unit {
    Unit {
        name: name.to_string(),
        description: format!("{name} description"),
        load: load.to_string(),
        active,
        sub: sub.to_string(),
        boot,
    }
}

fn names(units: &[&Unit]) -> Vec<String> {
    units.iter().map(|unit| unit.name.clone()).collect()
}

#[test]
fn unit_name_rejects_anything_that_could_reach_a_shell() {
    for bad in [
        "",
        "-x.service",
        "a b.service",
        "a;rm.service",
        "a$(x).service",
        "a`x`.service",
        "a\\x2db.service",
        "nginx",
        "nginx.timer",
        ".servicex",
        "a\n.service",
        "a'b.service",
        "a\"b.service",
        "a|b.service",
        "a&b.service",
        "a>b.service",
        "a<b.service",
        "a*b.service",
        "a?b.service",
        "a/b.service",
        "caf\u{e9}.service",
        "a\u{0}.service",
    ] {
        assert!(UnitName::new(bad).is_none(), "{bad:?} was accepted");
    }
    let too_long = format!("{}.service", "a".repeat(250));
    assert_eq!(too_long.chars().count(), 258);
    assert!(UnitName::new(&too_long).is_none());
    let longest = format!("{}.service", "a".repeat(248));
    assert_eq!(longest.chars().count(), 256);
    assert!(UnitName::new(&longest).is_some());
    let just_over = format!("{}.service", "a".repeat(249));
    assert!(UnitName::new(&just_over).is_none());
}

#[test]
fn unit_name_accepts_ordinary_service_names() {
    for good in ["nginx.service", "getty@tty1.service", "systemd-logind.service", "a:b_c.d@e-f.service"] {
        assert_eq!(UnitName::new(good).unwrap().as_str(), good);
    }
}

#[test]
fn service_command_is_exact_for_every_verb_and_privilege() {
    let unit = named("nginx.service");
    for (verb, word) in [
        (ServiceVerb::Start, "start"),
        (ServiceVerb::Stop, "stop"),
        (ServiceVerb::Restart, "restart"),
        (ServiceVerb::Enable, "enable"),
        (ServiceVerb::Disable, "disable"),
    ] {
        assert_eq!(
            service_command(verb, &unit, Privilege::Direct),
            format!("systemctl {word} nginx.service </dev/null")
        );
        assert_eq!(
            service_command(verb, &unit, Privilege::Sudo),
            format!("sudo -n systemctl {word} nginx.service </dev/null")
        );
    }
    assert_eq!(
        service_command(ServiceVerb::Restart, &unit, Privilege::Direct),
        "systemctl restart nginx.service </dev/null"
    );
    assert_eq!(
        service_command(ServiceVerb::Restart, &named("getty@tty1.service"), Privilege::Sudo),
        "sudo -n systemctl restart getty@tty1.service </dev/null"
    );
}

#[test]
fn status_command_is_exact_and_never_sudo() {
    assert_eq!(
        status_command(&named("nginx.service")),
        "systemctl status --no-pager --lines=20 nginx.service </dev/null"
    );
}

#[test]
fn only_root_runs_directly() {
    assert_eq!(Privilege::for_uid(0), Privilege::Direct);
    assert_eq!(Privilege::for_uid(1000), Privilege::Sudo);
    assert_eq!(Privilege::for_uid(UNKNOWN_UID), Privilege::Sudo);
}

#[test]
fn every_verb_needs_a_loaded_unit_and_enable_disable_need_an_enabled_or_disabled_boot_state() {
    let verbs =
        [ServiceVerb::Start, ServiceVerb::Stop, ServiceVerb::Restart, ServiceVerb::Enable, ServiceVerb::Disable];
    let allowed = |boot: Option<BootState>| -> Vec<bool> {
        let unit = unit("a.service", "loaded", ActiveState::Active, "running", boot);
        verbs.iter().map(|verb| verb_allowed(&unit, *verb)).collect()
    };
    assert_eq!(allowed(Some(BootState::Enabled)), [true; 5]);
    assert_eq!(allowed(Some(BootState::Disabled)), [true; 5]);
    for boot in [
        Some(BootState::Static),
        Some(BootState::Masked),
        Some(BootState::Alias),
        Some(BootState::Indirect),
        Some(BootState::Generated),
        Some(BootState::Other),
        None,
    ] {
        assert_eq!(allowed(boot), [true, true, true, false, false], "{boot:?}");
    }

    for load in ["masked", "not-found", "error", "bad-setting", ""] {
        let unit = unit("a.service", load, ActiveState::Inactive, "dead", Some(BootState::Enabled));
        for verb in verbs {
            assert!(!verb_allowed(&unit, verb), "{load:?} {verb:?}");
        }
    }
}

#[test]
fn the_debian_fixture_parses_markers_descriptions_instances_and_the_join() {
    let snapshot = linux(&render(DEBIAN));
    assert_eq!(snapshot.uid, 1000);

    let nginx = find(&snapshot, "nginx.service");
    assert_eq!(nginx.description, "A high performance web server and a reverse proxy server");
    assert_eq!(nginx.load, "loaded");
    assert_eq!(nginx.active, ActiveState::Active);
    assert_eq!(nginx.sub, "running");
    assert_eq!(nginx.boot, Some(BootState::Enabled));

    let old = find(&snapshot, "apache-old.service");
    assert_eq!(old.load, "not-found");
    assert_eq!(old.active, ActiveState::Inactive);
    assert_eq!(old.boot, None);

    let failing = find(&snapshot, "failing-job.service");
    assert_eq!(failing.active, ActiveState::Failed);
    assert_eq!(failing.boot, Some(BootState::Disabled));
    assert_eq!(failing.description, "A job that keeps failing");

    let getty = find(&snapshot, "getty@tty1.service");
    assert_eq!(getty.active, ActiveState::Active);
    assert_eq!(getty.boot, Some(BootState::Other));

    assert_eq!(find(&snapshot, "systemd-udevd.service").active, ActiveState::Activating);
    assert_eq!(find(&snapshot, "systemd-udevd.service").boot, Some(BootState::Static));
    assert_eq!(find(&snapshot, "weird-state.service").active, ActiveState::Other);
    assert_eq!(find(&snapshot, "masked-thing.service").boot, Some(BootState::Masked));
    assert_eq!(find(&snapshot, "runtime-only.service").boot, None);

    let file_only = find(&snapshot, "file-only.service");
    assert_eq!(file_only.boot, Some(BootState::Disabled));
    assert_eq!(file_only.active, ActiveState::Inactive);
    assert_eq!(file_only.load, "loaded");
    assert_eq!(find(&snapshot, "indirect-one.service").boot, Some(BootState::Indirect));
    assert_eq!(find(&snapshot, "generated-one.service").boot, Some(BootState::Generated));
    assert_eq!(find(&snapshot, "alias-one.service").boot, Some(BootState::Alias));
    assert_eq!(find(&snapshot, "mystery.service").boot, Some(BootState::Other));
    assert!(snapshot.units.iter().all(|unit| unit.name != "getty@.service"));

    let count = snapshot.units.iter().filter(|unit| unit.name == "nginx.service").count();
    assert_eq!(count, 1);
}

#[test]
fn a_masked_unit_file_nobody_loaded_has_no_actions() {
    let output = synthetic("", "ghost.service masked enabled\n");
    let snapshot = linux(&output);
    let ghost = find(&snapshot, "ghost.service");
    assert_eq!(ghost.load, "masked");
    assert!(!verb_allowed(ghost, ServiceVerb::Start));
}

#[test]
fn duplicate_names_keep_the_first_row() {
    let output = synthetic(
        "a.service loaded active running First\na.service loaded failed failed Second\n",
        "a.service enabled -\na.service disabled -\n",
    );
    let snapshot = linux(&output);
    assert_eq!(snapshot.units.len(), 1);
    assert_eq!(snapshot.units[0].description, "First");
    assert_eq!(snapshot.units[0].boot, Some(BootState::Enabled));
}

#[test]
fn rows_that_are_not_units_are_skipped() {
    let output = synthetic(
        "short.service loaded\n\n   \n● \nlegend text here that is not a unit row\nok.service loaded active running Fine\n",
        "garbage\nok.service enabled\n",
    );
    let snapshot = linux(&output);
    let found: Vec<&str> = snapshot.units.iter().map(|unit| unit.name.as_str()).collect();
    assert!(found.contains(&"ok.service"));
    assert_eq!(find(&snapshot, "ok.service").boot, Some(BootState::Enabled));
}

#[test]
fn control_and_bidi_characters_are_stripped_from_text() {
    let output = synthetic("a.service loaded active running Bad\u{1}\u{202e}Name\n", "");
    let snapshot = linux(&output);
    let description = &find(&snapshot, "a.service").description;
    assert!(description.chars().all(|character| !character.is_control() && character != '\u{202e}'), "{description:?}");
}

#[test]
fn no_systemd_is_reported_from_the_alpine_fixture() {
    assert!(matches!(parse_services(&render(ALPINE), &token()), ParsedServices::NoSystemd));
}

#[test]
fn macos_is_unsupported() {
    match parse_services(&render(MACOS), &token()) {
        ParsedServices::Unsupported { os } => assert_eq!(os, "Darwin"),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn empty_truncated_and_non_utf8_output_never_panics() {
    assert!(matches!(parse_services(b"", &token()), ParsedServices::Garbled));
    assert!(matches!(parse_services(b"no markers\n", &token()), ParsedServices::Garbled));
    assert!(matches!(parse_services(&[0xff, 0xfe, 0x00, 0x80], &token()), ParsedServices::Garbled));

    let full = render(DEBIAN);
    for length in 0..full.len() {
        match parse_services(&full[..length], &token()) {
            ParsedServices::Garbled | ParsedServices::Linux(_) | ParsedServices::NoSystemd => {}
            ParsedServices::Unsupported { os } => panic!("truncated at {length} became {os}"),
        }
    }

    let mut hostile = full.clone();
    for byte in hostile.iter_mut().filter(|byte| **byte == b'd') {
        *byte = 0xff;
    }
    assert!(matches!(
        parse_services(&hostile, &token()),
        ParsedServices::Linux(_) | ParsedServices::Garbled | ParsedServices::NoSystemd
    ));
}

#[test]
fn a_run_cut_off_before_the_unit_list_is_garbled_not_empty() {
    let cut = render("@@T:os\nLinux\n@@T:uid\n0\n@@T:systemd\nyes\n@@T:units\na.service loaded active running A\n");
    assert!(matches!(parse_services(&cut, &token()), ParsedServices::Garbled));
}

#[test]
fn hostile_input_is_capped() {
    let many: String =
        (0..10_000).map(|index| format!("unit{index}.service loaded active running Unit number {index}\n")).collect();
    let files: String = (0..10_000).map(|index| format!("other{index}.service enabled enabled\n")).collect();
    let snapshot = linux(&synthetic(&many, &files));
    assert_eq!(snapshot.units.len(), MAX_UNITS);

    let long_line = format!("a.service loaded active running {}\n", "x".repeat(900_000));
    let snapshot = linux(&synthetic(&format!("{long_line}b.service loaded active running Fine\n"), ""));
    assert!(snapshot.units.iter().any(|unit| unit.name == "b.service"));
    assert!(snapshot.units.iter().all(|unit| unit.description.chars().count() <= 256));

    let wall = format!("{}\n", "a".repeat(1_000_000));
    assert!(linux(&synthetic(&wall, &wall)).units.is_empty());

    let name = "n".repeat(5000);
    let snapshot = linux(&synthetic(&format!("{name}.service loaded active running X\n"), ""));
    assert!(snapshot.units.iter().all(|unit| unit.name.chars().count() <= 256));
}

#[test]
fn file_only_units_are_not_shown_as_inactive_when_the_unit_list_was_cut_off_by_the_cap() {
    let valid = |count: usize| -> String {
        (0..count).map(|index| format!("unit{index}.service loaded active running Unit {index}\n")).collect()
    };
    let junk = "junk\n".repeat(MAX_UNITS + 1 - 1990);
    let cut_off = format!("{}{junk}", valid(1990));
    let complete = format!("{}{}", valid(1990), "junk\n".repeat(MAX_UNITS - 1990));
    let files = "unit5.service enabled enabled\ncut.service enabled enabled\n";

    let truncated = linux(&synthetic(&cut_off, files));
    let whole = linux(&synthetic(&complete, files));

    assert_eq!(truncated.units.len(), 1990);
    assert!(truncated.units.iter().all(|unit| unit.name != "cut.service"));
    assert_eq!(find(&truncated, "unit5.service").boot, Some(BootState::Enabled));
    assert_eq!(find(&whole, "cut.service").active, ActiveState::Inactive);
}

#[test]
fn arranged_units_puts_failed_first_then_running_then_by_name() {
    let all = vec![
        unit("zeta.service", "loaded", ActiveState::Active, "running", None),
        unit("alpha.service", "loaded", ActiveState::Inactive, "dead", None),
        unit("mid.service", "loaded", ActiveState::Failed, "failed", None),
        unit("beta.service", "loaded", ActiveState::Active, "running", None),
        unit("exited.service", "loaded", ActiveState::Active, "exited", None),
        unit("also-failed.service", "loaded", ActiveState::Failed, "failed", None),
    ];
    let shown = arranged_units(&all, "", StatusFilter::All);
    assert_eq!(
        names(&shown),
        ["also-failed.service", "mid.service", "beta.service", "zeta.service", "alpha.service", "exited.service"]
    );
}

#[test]
fn arranged_units_status_filters() {
    let all = vec![
        unit("run.service", "loaded", ActiveState::Active, "running", None),
        unit("done.service", "loaded", ActiveState::Active, "exited", None),
        unit("bad.service", "loaded", ActiveState::Failed, "failed", None),
        unit("off.service", "loaded", ActiveState::Inactive, "dead", None),
    ];
    assert_eq!(names(&arranged_units(&all, "", StatusFilter::Running)), ["run.service"]);
    assert_eq!(names(&arranged_units(&all, "", StatusFilter::Failed)), ["bad.service"]);
    assert_eq!(arranged_units(&all, "", StatusFilter::All).len(), 4);
}

#[test]
fn arranged_units_text_filter_is_case_insensitive_over_name_and_description() {
    let mut all = vec![
        unit("nginx.service", "loaded", ActiveState::Active, "running", None),
        unit("cron.service", "loaded", ActiveState::Active, "running", None),
        unit("other.service", "loaded", ActiveState::Active, "running", None),
    ];
    all[1].description = "Regular Background program".to_string();
    all[2].description = "\u{3a3}\u{3b5}\u{3c1}\u{3b2}\u{3af}\u{3c2}".to_string();
    assert_eq!(names(&arranged_units(&all, "NGINX", StatusFilter::All)), ["nginx.service"]);
    assert_eq!(names(&arranged_units(&all, "  background ", StatusFilter::All)), ["cron.service"]);
    assert_eq!(names(&arranged_units(&all, "\u{3c3}\u{3b5}\u{3c1}", StatusFilter::All)), ["other.service"]);
    assert!(arranged_units(&all, "nothing matches", StatusFilter::All).is_empty());
    assert_eq!(names(&arranged_units(&all, "NGINX", StatusFilter::Failed)), Vec::<String>::new());
}

#[test]
fn sudo_problem_recognises_each_message_ignoring_case() {
    assert_eq!(sudo_problem("sudo: a password is required\n"), Some(SudoProblem::NeedsPassword));
    assert_eq!(sudo_problem("SUDO: A PASSWORD IS REQUIRED"), Some(SudoProblem::NeedsPassword));
    assert_eq!(sudo_problem("bob is not in the sudoers file."), Some(SudoProblem::NotAllowed));
    assert_eq!(sudo_problem("Sorry, user bob may not run sudo on host."), Some(SudoProblem::NotAllowed));
    assert_eq!(sudo_problem("sh: sudo: not found"), Some(SudoProblem::NotInstalled));
    assert_eq!(sudo_problem("bash: sudo: command not found"), Some(SudoProblem::NotInstalled));
    assert_eq!(sudo_problem("Failed to restart nginx.service: Unit nginx.service not found."), None);
    assert_eq!(sudo_problem(""), None);
}

#[test]
fn services_command_is_posix_sh_with_every_marker_once_in_order() {
    let script = services_command(&token());
    for forbidden in ["[[", "<(", ">(", "local ", "$'", "function ", "echo -e", "source ", "=(", "&>", "${!", "sudo"] {
        assert!(!script.contains(forbidden), "the script uses {forbidden:?}");
    }
    assert!(script.ends_with('\n'));
    assert!(script.len() < 4 * 1024, "{} bytes", script.len());
    assert!(!script.contains("@@T:"));
    assert_eq!(script.matches("00112233445566778899aabbccddeeff").count(), 1);
    let names: Vec<&str> = script
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("m "))
        .map(|rest| rest.split(';').next().unwrap().trim())
        .collect();
    assert_eq!(names, ["os", "end", "uid", "systemd", "units", "files", "end"]);
    assert!(script.contains("/run/systemd/system"));
    assert!(script.contains(
        "systemctl list-units --type=service --all --plain --no-legend --no-pager --full 2>/dev/null | head -n 2001"
    ));
    assert!(script.contains(
        "systemctl list-unit-files --type=service --plain --no-legend --no-pager 2>/dev/null | head -n 4001"
    ));
}

#[cfg(target_os = "linux")]
fn assert_a_real_run_parses(shell: &[&str]) {
    let Some((program, arguments)) = shell.split_first() else { return };
    let token = Token::new();
    let Ok(output) =
        std::process::Command::new(program).args(arguments).arg("-c").arg(services_command(&token)).output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    match parse_services(&output.stdout, &token) {
        ParsedServices::Linux(_) | ParsedServices::NoSystemd => {}
        other => panic!("a Linux host should parse under {shell:?}, got {other:?}"),
    }
}

#[cfg(target_os = "linux")]
#[test]
fn the_script_runs_under_the_local_sh_dash_and_busybox_and_its_output_parses() {
    assert_a_real_run_parses(&["sh"]);
    assert_a_real_run_parses(&["dash"]);
    assert_a_real_run_parses(&["busybox", "sh"]);
}

#[cfg(target_os = "linux")]
#[test]
fn a_systemd_directory_without_a_systemctl_binary_is_no_systemd() {
    let directory = std::env::temp_dir().join(format!("porthmos-services-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let token = Token::new();
    let script = services_command(&token)
        .replace("/run/systemd/system", &directory.display().to_string())
        .replace("systemctl", "porthmos-absent-systemctl");

    let output = std::process::Command::new("sh").arg("-c").arg(script).output();
    let _ = std::fs::remove_dir_all(&directory);
    let Ok(output) = output else { return };

    assert!(matches!(parse_services(&output.stdout, &token), ParsedServices::NoSystemd));
}
