use std::ffi::OsString;

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
#[cfg(unix)]
use porthmos_core::Severity;
use porthmos_core::edit::EditorExit;
use porthmos_core::{Command, Entry, Event, Location, ShellInvocation, VfsPath};

use crate::shortcuts::KeyPress;
use crate::testing::Harness;

use super::super::*;

fn named(harness: &mut Harness, key: Named) {
    let key = Key::Named(key);
    harness.press(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::empty(),
        text: None,
    }));
}

fn connect(harness: &mut Harness, session: u64, name: &str, shell_available: bool) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: VfsPath::new("/srv/data"),
        entries: vec![Entry { name: "r.log".to_string(), path: VfsPath::new("/srv/data/r.log"), ..Entry::default() }],
    });
    harness.sent();
}

fn invocation(env: &[(&str, &str)]) -> ShellInvocation {
    ShellInvocation {
        program: OsString::from("ssh"),
        args: ["-p", "22", "host"].map(OsString::from).to_vec(),
        env: env.iter().map(|(key, value)| (OsString::from(*key), OsString::from(*value))).collect(),
    }
}

fn strings(values: &[OsString]) -> Vec<String> {
    values.iter().map(|value| value.to_string_lossy().to_string()).collect()
}

#[test]
fn f4_on_the_remote_panel_requests_the_remote_shell() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.press(Message::PanelClicked(PanelSide::Remote, 0));

    named(&mut harness, Named::F4);

    assert_eq!(harness.sent(), vec![Command::PrepareShell { session: 3, directory: Some(VfsPath::new("/srv/data")) }]);
}

#[test]
fn f4_on_the_local_panel_requests_a_local_shell_even_with_a_connection_open() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.press(Message::PanelClicked(PanelSide::Local, 0));

    named(&mut harness, Named::F4);

    assert_eq!(harness.sent(), vec![Command::PrepareLocalShell { directory: VfsPath::new("/home/tester") }]);
}

#[test]
fn f4_without_a_connection_opens_a_local_shell() {
    let mut harness = Harness::new();
    harness.sent();

    named(&mut harness, Named::F4);

    assert_eq!(harness.sent(), vec![Command::PrepareLocalShell { directory: VfsPath::new("/home/tester") }]);
    assert!(harness.app.notices().is_empty());
}

#[test]
fn f4_on_the_remote_panel_of_a_connection_without_shell_support_still_warns() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", false);
    harness.press(Message::PanelClicked(PanelSide::Remote, 0));

    named(&mut harness, Named::F4);

    assert!(harness.sent().is_empty());
    assert_eq!(
        harness.app.notices().last().unwrap().message.to_string(),
        "This connection doesn't support a terminal session"
    );
}

#[test]
fn a_local_shell_is_spawned_like_a_remote_one() {
    let mut harness = Harness::new();
    harness.launched();

    harness.feed(Event::LocalShellReady {
        invocation: ShellInvocation {
            program: OsString::from("sh"),
            args: ["-c", "cd", "sh", "/x", "/bin/sh"].map(OsString::from).to_vec(),
            env: Vec::new(),
        },
    });

    let jobs = harness.launched();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].spec.program, OsString::from("xterm"));
    assert_eq!(strings(&jobs[0].spec.args), ["-e", "sh", "-c", "cd", "sh", "/x", "/bin/sh"]);
}

#[test]
fn the_terminal_is_possible_on_the_local_panel_without_any_connection() {
    let harness = Harness::new();

    assert!(harness.app.terminal_possible());
}

#[test]
fn the_terminal_on_the_remote_panel_needs_shell_support() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", false);
    harness.press(Message::PanelClicked(PanelSide::Remote, 0));

    assert!(!harness.app.terminal_possible());
}

#[test]
fn shell_available_is_remembered_per_session() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "with", true);
    connect(&mut harness, 2, "without", false);
    harness.press(Message::PanelClicked(PanelSide::Remote, 0));

    assert!(!harness.app.terminal_possible());
    harness.app.sessions.activate(1);
    assert!(harness.app.terminal_possible());
}

#[test]
fn shell_ready_spawns_the_terminal_without_waiting() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);

    harness.feed(Event::ShellReady { session: 3, invocation: invocation(&[]) });

    let jobs = harness.launched();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].spec.program, OsString::from("xterm"));
    assert_eq!(strings(&jobs[0].spec.args), ["-e", "ssh", "-p", "22", "host"]);
    assert!(jobs[0].cleanup.is_empty());
}

#[test]
#[cfg(unix)]
fn the_invocation_env_reaches_the_launcher_but_not_the_argv() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);

    harness.feed(Event::ShellReady { session: 3, invocation: invocation(&[("SSHPASS", "s3cr3t-value")]) });

    let jobs = harness.launched();
    assert_eq!(jobs.len(), 1);
    let argv = strings(&jobs[0].spec.args).join(" ");
    assert!(!argv.contains("s3cr3t-value"), "{argv}");
    assert!(!format!("{:?}", jobs[0]).contains("s3cr3t-value"));
    assert_eq!(jobs[0].cleanup.len(), 1);
    let handed_over = std::fs::read_to_string(&jobs[0].cleanup[0]).unwrap();
    assert!(handed_over.contains("export SSHPASS='s3cr3t-value'"));
}

#[test]
#[cfg(unix)]
fn a_missing_terminal_shows_an_error_notice_with_the_hint() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.install_terminals(&[]);

    harness.feed(Event::ShellReady { session: 3, invocation: invocation(&[("SSHPASS", "s3cr3t-value")]) });

    assert!(harness.launched().is_empty());
    let notice = harness.app.notices().last().unwrap();
    assert_eq!(notice.severity, Severity::Error);
    assert_eq!(
        notice.message,
        "Couldn't open a terminal: No terminal program found. Set $TERMINAL to your terminal's command."
    );
    assert!(!notice.message.to_string().contains("s3cr3t-value"));
    let leftovers = std::fs::read_dir(harness.scratch().join("handover")).map_or(0, |dir| dir.count());
    assert_eq!(leftovers, 0);
}

#[test]
#[cfg(unix)]
fn a_launch_failure_becomes_an_error_notice_without_the_secret() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.feed(Event::ShellReady { session: 3, invocation: invocation(&[("SSHPASS", "s3cr3t-value")]) });
    let token = harness.launched()[0].token;

    harness.finish_process(token, EditorExit::LaunchFailed("xterm: not found".to_string()));

    let notice = harness.app.notices().last().unwrap();
    assert_eq!(notice.severity, Severity::Error);
    assert_eq!(notice.message, "Couldn't open a terminal: xterm: not found");
    assert!(!notice.message.to_string().contains("s3cr3t-value"));
    assert!(harness.sent().is_empty());
}

#[test]
fn a_terminal_that_closes_normally_or_with_a_status_says_nothing() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    let notices = harness.app.notices().len();
    for exit in [EditorExit::Success, EditorExit::Status(255)] {
        harness.feed(Event::ShellReady { session: 3, invocation: invocation(&[]) });
        let token = harness.launched()[0].token;

        harness.finish_process(token, exit);
    }

    assert_eq!(harness.app.notices().len(), notices);
}

#[test]
fn the_local_terminal_button_opens_a_local_shell_while_the_remote_panel_is_active() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.press(Message::PanelClicked(PanelSide::Remote, 0));
    harness.sent();

    harness.press(Message::TerminalRequested(PanelSide::Local));

    assert_eq!(harness.sent(), vec![Command::PrepareLocalShell { directory: VfsPath::new("/home/tester") }]);
}

#[test]
fn the_remote_terminal_button_opens_the_remote_shell_while_the_local_panel_is_active() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.press(Message::PanelClicked(PanelSide::Local, 0));
    harness.sent();

    harness.press(Message::TerminalRequested(PanelSide::Remote));

    assert_eq!(harness.sent(), vec![Command::PrepareShell { session: 3, directory: Some(VfsPath::new("/srv/data")) }]);
}

#[test]
fn the_remote_terminal_is_not_possible_without_a_connection() {
    let harness = Harness::new();

    assert!(harness.app.terminal_possible_on(PanelSide::Local));
    assert!(!harness.app.terminal_possible_on(PanelSide::Remote));
}

#[test]
fn the_remote_terminal_is_not_possible_on_a_connection_without_shell_support() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", false);

    assert!(!harness.app.terminal_possible_on(PanelSide::Remote));
    assert!(harness.app.terminal_possible_on(PanelSide::Local));
}

#[test]
fn the_toolbar_enables_each_terminal_button_independently_of_the_active_panel() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    for side in [PanelSide::Local, PanelSide::Remote] {
        harness.press(Message::PanelClicked(side, 0));

        let state = harness.app.toolbar_state();

        assert!(state.local_terminal && state.remote_terminal, "{side:?}");
    }
}

#[test]
fn the_toolbar_mutes_the_remote_terminal_button_without_a_usable_shell() {
    let mut harness = Harness::new();
    assert!(harness.app.toolbar_state().local_terminal);
    assert!(!harness.app.toolbar_state().remote_terminal);

    connect(&mut harness, 3, "alpha", false);

    assert!(harness.app.toolbar_state().local_terminal);
    assert!(!harness.app.toolbar_state().remote_terminal);
}

#[test]
fn a_terminal_request_for_a_side_that_cannot_open_one_only_warns() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::TerminalRequested(PanelSide::Remote));

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.notices().last().unwrap().message.to_string(), "Connect to a server first");
}
