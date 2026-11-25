use super::*;
use crate::{TermSize, Terminal, TerminalConfig};

fn terminal() -> Terminal {
    Terminal::new(TermSize::clamped(80, 24), TerminalConfig::default())
}

fn terminal_allowing_clipboard() -> Terminal {
    Terminal::new(
        TermSize::clamped(80, 24),
        TerminalConfig { allow_clipboard_store: true, ..TerminalConfig::default() },
    )
}

#[test]
fn a_cursor_position_query_is_answered() {
    let mut terminal = terminal();

    assert_eq!(terminal.feed(b"\x1b[6n").replies, b"\x1b[1;1R");
    assert_eq!(terminal.feed(b"\x1b[c").replies, b"\x1b[?6c");
}

#[test]
fn bells_and_titles_become_events() {
    let mut terminal = terminal();

    assert_eq!(terminal.feed(b"\x07").events, [TerminalEvent::Bell]);
    assert_eq!(terminal.feed(b"\x1b]0;build\x07").events, [TerminalEvent::Title("build".into())]);
}

#[test]
fn titles_lose_control_characters_and_are_capped() {
    assert_eq!(sanitize_title("wor\x01k\x7f\r\ning\u{202e}"), "working");
    assert_eq!(sanitize_title("  padded  "), "padded");
    assert_eq!(sanitize_title("ok\u{61c}x\u{2060}y\u{ad}z\u{180e}"), "okxyz");
    assert_eq!(sanitize_title(&"x".repeat(1000)).chars().count(), 256);

    let mut terminal = terminal();
    let long = format!("\x1b]0;{}\x07", "y".repeat(1000));
    assert_eq!(terminal.feed(long.as_bytes()).events, [TerminalEvent::Title("y".repeat(256))]);
}

#[test]
fn clipboard_writes_are_ignored_by_default() {
    let write = b"\x1b]52;c;aGVsbG8=\x07";

    assert!(terminal().feed(write).events.is_empty());
    assert_eq!(terminal_allowing_clipboard().feed(write).events, [TerminalEvent::ClipboardStore("hello".into())]);
}

#[test]
fn clipboard_reads_are_never_answered() {
    let read = b"\x1b]52;c;?\x07";

    assert!(terminal().feed(read).replies.is_empty());
    assert!(terminal_allowing_clipboard().feed(read).replies.is_empty());
}

#[test]
fn working_directory_comes_from_osc7() {
    let expected =
        TerminalEvent::WorkingDirectory(WorkingDirectory { host: "host".into(), path: "/home/u/some dir".into() });
    let mut terminal = terminal();

    assert_eq!(terminal.feed(b"\x1b]7;file://host/home/u/some%20dir\x07").events, std::slice::from_ref(&expected));
    assert_eq!(terminal.feed(b"\x1b]7;file://host/home/u/some%20dir\x1b\\").events, [expected]);
}

#[test]
fn an_osc7_split_across_feeds_is_still_seen() {
    let mut terminal = terminal();

    let first = terminal.feed(b"\x1b]7;file://host/ho");
    let second = terminal.feed(b"me/u\x07");

    assert!(first.events.is_empty());
    assert_eq!(
        second.events,
        [TerminalEvent::WorkingDirectory(WorkingDirectory { host: "host".into(), path: "/home/u".into() })]
    );
}

#[test]
fn working_directory_rejects_hostile_input() {
    let long = format!("file://h/{}", "a".repeat(5000));
    let hostile = [
        "http://h/x",
        "file:relative/path",
        "file://h",
        "file://h/a/../b",
        "file://h/a%00b",
        "file://h/%zz",
        "file://h/%4",
        "file://h/%ff",
        "file://h/a%0ab",
        "file://h/a\u{202e}gpj.exe",
        "file://h/a%E2%80%AEgpj.exe",
        "file://h\u{202e}x/a",
        "file://h/\u{200b}x",
        long.as_str(),
    ];

    for uri in hostile {
        assert_eq!(parse_working_directory(uri), None, "{uri}");
    }
    assert_eq!(
        parse_working_directory("file:///C:/Users/x"),
        Some(WorkingDirectory { host: String::new(), path: "/C:/Users/x".into() })
    );
}

#[test]
fn an_oversized_osc_is_aborted_and_does_not_become_a_title() {
    let mut terminal = terminal();
    let oversized = format!("\x1b]0;{}\x07", "x".repeat(100_000));

    let feed = terminal.feed(oversized.as_bytes());
    terminal.feed(b"ok");

    assert!(feed.events.is_empty());
    assert_eq!(terminal.snapshot().row_text(0), "ok");
}

#[test]
fn an_oversized_osc_split_across_many_feeds_is_aborted() {
    let mut terminal = terminal();
    let oversized = format!("\x1b]0;{}\x07", "x".repeat(100_000));

    let events: Vec<TerminalEvent> =
        oversized.as_bytes().chunks(1000).flat_map(|chunk| terminal.feed(chunk).events).collect();

    assert!(events.is_empty());
}

#[test]
fn text_after_an_aborted_osc_is_shown_once_a_cancel_arrives() {
    let mut terminal = terminal();
    terminal.feed(format!("\x1b]0;{}", "x".repeat(100_000)).as_bytes());

    terminal.feed(b"\x18ok");

    assert_eq!(terminal.snapshot().row_text(0), "ok");
}

#[test]
fn normal_titles_still_work_after_an_aborted_osc() {
    let mut terminal = terminal();
    terminal.feed(format!("\x1b]0;{}\x07", "x".repeat(100_000)).as_bytes());

    assert_eq!(terminal.feed(b"\x1b]0;fine\x07").events, [TerminalEvent::Title("fine".into())]);
}

fn report(host: &str, path: &str) -> WorkingDirectory {
    WorkingDirectory { host: host.to_string(), path: path.to_string() }
}

fn names() -> Vec<String> {
    vec!["localhost".to_string(), "laptop".to_string()]
}

#[test]
fn local_accepts_empty_localhost_and_own_host() {
    for host in ["", "localhost", "laptop", "LAPTOP"] {
        let mut tracker = DirectoryTracker::new(DirectoryOwner::Local);
        tracker.update(&report(host, "/home/me"), &names());
        assert_eq!(tracker.directory(), Some("/home/me"), "{host:?}");
    }
}

#[test]
fn local_ignores_and_clears_for_a_foreign_host() {
    let mut tracker = DirectoryTracker::new(DirectoryOwner::Local);
    tracker.update(&report("laptop", "/home/me"), &names());

    tracker.update(&report("server", "/srv"), &names());

    assert_eq!(tracker.directory(), None);
}

#[test]
fn remote_first_host_sticks_and_updates() {
    let mut tracker = DirectoryTracker::new(DirectoryOwner::Remote);

    tracker.update(&report("web01", "/srv"), &names());
    tracker.update(&report("web01", "/srv/app"), &names());

    assert_eq!(tracker.directory(), Some("/srv/app"));
}

#[test]
fn remote_nested_host_clears_and_original_host_restores() {
    let mut tracker = DirectoryTracker::new(DirectoryOwner::Remote);
    tracker.update(&report("web01", "/srv"), &names());

    tracker.update(&report("db01", "/var/lib"), &names());
    assert_eq!(tracker.directory(), None);
    tracker.update(&report("db01", "/var/log"), &names());
    assert_eq!(tracker.directory(), None);

    tracker.update(&report("web01", "/srv/app"), &names());
    assert_eq!(tracker.directory(), Some("/srv/app"));
}

#[test]
fn host_comparison_ignores_case() {
    let mut tracker = DirectoryTracker::new(DirectoryOwner::Remote);
    tracker.update(&report("Web01", "/srv"), &names());

    tracker.update(&report("WEB01", "/srv/app"), &names());

    assert_eq!(tracker.directory(), Some("/srv/app"));
}

#[test]
fn a_new_tracker_has_no_directory() {
    assert_eq!(DirectoryTracker::new(DirectoryOwner::Remote).directory(), None);
}

#[test]
fn local_host_names_contains_localhost_and_is_lower_case() {
    let names = local_host_names();

    assert!(names.contains(&"localhost".to_string()));
    assert!(names.iter().all(|name| *name == name.to_ascii_lowercase()));
}
