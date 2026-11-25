use std::{ffi::OsString, time::Duration};

use super::*;
use crate::{TermSize, TerminalConfig, TerminalEvent, TerminalSession};

#[cfg(unix)]
fn lookup_with(shell: Option<&str>) -> impl Fn(&str) -> Option<OsString> {
    let shell = shell.map(OsString::from);
    move |name| if name == "SHELL" { shell.clone() } else { None }
}

#[cfg(windows)]
fn lookup_with(shell: Option<&str>) -> impl Fn(&str) -> Option<OsString> {
    let shell = shell.map(OsString::from);
    move |name| if name == "ComSpec" { shell.clone() } else { None }
}

#[cfg(unix)]
#[test]
fn default_shell_prefers_the_environment_then_falls_back() {
    assert_eq!(default_shell(lookup_with(Some("/usr/bin/zsh"))), PathBuf::from("/usr/bin/zsh"));
    assert_eq!(default_shell(lookup_with(Some(""))), PathBuf::from("/bin/sh"));
    assert_eq!(default_shell(lookup_with(None)), PathBuf::from("/bin/sh"));
}

#[cfg(windows)]
#[test]
fn default_shell_prefers_the_environment_then_falls_back() {
    assert_eq!(default_shell(lookup_with(Some("C:\\Windows\\cmd.exe"))), PathBuf::from("C:\\Windows\\cmd.exe"));
    assert_eq!(default_shell(lookup_with(Some(""))), PathBuf::from("powershell.exe"));
    assert_eq!(default_shell(lookup_with(None)), PathBuf::from("powershell.exe"));
}

fn screen_lines(session: &TerminalSession) -> Vec<String> {
    let snapshot = session.snapshot();
    (0..snapshot.size.rows).map(|row| snapshot.row_text(row)).collect()
}

#[cfg(target_os = "linux")]
async fn wait_until(mut condition: impl FnMut() -> bool) {
    for _ in 0..100 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the condition did not hold within five seconds");
}

async fn wait_for_line(session: &TerminalSession, expected: &str) {
    for _ in 0..100 {
        if screen_lines(session).iter().any(|line| line.ends_with(expected)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the line {expected:?} never appeared; the screen shows {:?}", screen_lines(session));
}

fn start(shell: &LocalShell) -> (TerminalSession, tokio::sync::mpsc::Receiver<TerminalEvent>) {
    let transport = spawn_local(shell, TermSize::clamped(80, 24)).unwrap();
    TerminalSession::spawn(transport, TermSize::clamped(80, 24), TerminalConfig::default())
}

#[cfg(unix)]
fn sh() -> LocalShell {
    LocalShell { program: Some(PathBuf::from("/bin/sh")), ..LocalShell::default() }
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_local_shell_echoes_through_a_session() {
    let (session, _events) = start(&sh());

    session.write(b"echo porthmos-$((20+22))\n".to_vec());

    wait_for_line(&session, "porthmos-42").await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_exit_status_is_reported() {
    let (session, mut events) = start(&sh());

    session.write(b"exit 7\n".to_vec());

    let exited = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            if let TerminalEvent::Exited(status) = event {
                return Some(status);
            }
        }
        None
    })
    .await
    .unwrap();
    assert_eq!(exited, Some(Some(7)));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resizing_reaches_the_child() {
    let (session, _events) = start(&sh());

    session.resize(TermSize::clamped(100, 30));
    session.write(b"stty size\n".to_vec());

    wait_for_line(&session, "30 100").await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_terminal_type_is_set() {
    let (session, _events) = start(&sh());

    session.write(b"echo $TERM $COLORTERM\n".to_vec());

    wait_for_line(&session, "xterm-256color truecolor").await;
}

#[cfg(unix)]
#[test]
fn a_missing_program_is_an_error() {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let shell = LocalShell { program: Some(PathBuf::from("/nonexistent/porthmos-shell")), ..LocalShell::default() };

    let result = runtime.block_on(async { spawn_local(&shell, TermSize::clamped(80, 24)) });

    assert!(result.is_err());
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_the_session_ends_the_shell() {
    let (session, _events) = start(&sh());
    session.write(b"echo pid=$$\n".to_vec());
    let mut pid = None;
    wait_until(|| {
        pid = screen_lines(&session).iter().find_map(|line| {
            let digits: String = line.split_once("pid=")?.1.chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then_some(digits)
        });
        pid.is_some()
    })
    .await;
    let process = PathBuf::from(format!("/proc/{}", pid.unwrap()));
    assert!(process.exists());

    drop(session);

    wait_until(|| !process.exists()).await;
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_the_session_ends_background_jobs_too() {
    let bash = PathBuf::from("/bin/bash");
    if !bash.exists() {
        return;
    }
    let (session, _events) =
        start(&LocalShell { program: Some(bash), arguments: vec!["--norc".into()], ..LocalShell::default() });
    session.write(b"sleep 31337 &\necho job=$!\n".to_vec());
    let mut job = None;
    wait_until(|| {
        job = screen_lines(&session).iter().find_map(|line| {
            let digits: String = line.split_once("job=")?.1.chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then_some(digits)
        });
        job.is_some()
    })
    .await;
    let process = PathBuf::from(format!("/proc/{}", job.unwrap()));
    assert!(process.exists());

    drop(session);
    wait_until(|| !process.exists()).await;
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_local_shell_runs_a_command() {
    let shell = LocalShell {
        program: Some(PathBuf::from("cmd.exe")),
        arguments: vec!["/C".into(), "echo porthmos-ok".into()],
        ..LocalShell::default()
    };
    let (session, _events) = start(&shell);

    wait_for_line(&session, "porthmos-ok").await;
}
