use std::ffi::OsString;

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::edit::EditorExit;
use porthmos_core::{Command, Entry, Event, Location, Severity, ShellInvocation, VfsPath};

use crate::menus::{MoreItem, MoreState, more_items};
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
fn f4_requests_a_shell_in_the_active_folder() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);

    named(&mut harness, Named::F4);

    assert_eq!(harness.sent(), vec![Command::PrepareShell { session: 3, directory: Some(VfsPath::new("/srv/data")) }]);
}

#[test]
fn f4_from_the_local_panel_still_targets_the_connection() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.press(Message::PanelClicked(PanelSide::Local, 0));

    named(&mut harness, Named::F4);

    assert_eq!(harness.sent(), vec![Command::PrepareShell { session: 3, directory: Some(VfsPath::new("/srv/data")) }]);
}

#[test]
fn f4_without_a_connection_warns() {
    let mut harness = Harness::new();
    harness.sent();

    named(&mut harness, Named::F4);

    assert!(harness.sent().is_empty());
    let notice = harness.app.notices().last().unwrap();
    assert_eq!((notice.severity, notice.message.as_str()), (Severity::Warning, "Connect to a server first"));
}

#[test]
fn f4_on_a_connection_without_shell_support_warns() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", false);

    named(&mut harness, Named::F4);

    assert!(harness.sent().is_empty());
    let notice = harness.app.notices().last().unwrap();
    assert_eq!(
        (notice.severity, notice.message.as_str()),
        (Severity::Warning, "This connection doesn't support a terminal session")
    );
}

#[test]
fn shell_available_is_remembered_per_session() {
    let mut harness = Harness::new();
    connect(&mut harness, 1, "with", true);
    connect(&mut harness, 2, "without", false);

    assert!(!harness.app.terminal_possible());
    harness.app.sessions.activate(1);
    assert!(harness.app.terminal_possible());
}

#[test]
fn no_connection_means_no_terminal() {
    assert!(!Harness::new().app.terminal_possible());
}

#[test]
fn the_more_item_is_disabled_without_shell_support() {
    let enabled_for = |terminal: bool| {
        more_items(MoreState { hidden_shown: false, file: true, terminal, sync: true })
            .into_iter()
            .find(|(item, _, _)| *item == MoreItem::Terminal)
            .map(|(_, title, enabled)| (title, enabled))
            .unwrap()
    };

    assert_eq!(enabled_for(true), ("Open terminal here".to_string(), true));
    assert_eq!(enabled_for(false), ("Open terminal here".to_string(), false));
}

#[test]
fn the_more_item_requests_the_shell() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);

    harness.press(Message::MoreItemChosen(MoreItem::Terminal));

    assert_eq!(harness.sent().len(), 1);
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
    assert!(!notice.message.contains("s3cr3t-value"));
    let leftovers = std::fs::read_dir(harness.scratch().join("handover")).map_or(0, |dir| dir.count());
    assert_eq!(leftovers, 0);
}

#[test]
fn a_launch_failure_becomes_an_error_notice_without_the_secret() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true);
    harness.feed(Event::ShellReady { session: 3, invocation: invocation(&[("SSHPASS", "s3cr3t-value")]) });
    let token = harness.launched()[0].token;

    harness.finish_process(token, EditorExit::LaunchFailed("xterm: not found".to_string()));

    let notice = harness.app.notices().last().unwrap();
    assert_eq!(notice.severity, Severity::Error);
    assert_eq!(notice.message, "Couldn't open a terminal: xterm: not found");
    assert!(!notice.message.contains("s3cr3t-value"));
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
