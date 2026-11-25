use std::time::Duration;

use iced::keyboard::{Key, Modifiers, key::Named};
use porthmos_core::{Command, Entry, Event, Handover, Location, Severity, ShellSize, VfsPath};
use porthmos_term::TerminalEvent;
use tokio::io::AsyncReadExt;

use crate::app::Message;
use crate::drawer::{TerminalSource, Wanted};
use crate::shortcuts::KeyPress;
use crate::testing::{Harness, ShellEnd, shell_pipe};
use crate::tools::Tool;

fn connect(harness: &mut Harness, session: u64, name: &str, folder: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    let path = VfsPath::new(folder);
    let entries: Vec<Entry> = Vec::new();
    harness.feed(Event::Listed { location: Location::Session(session), path, entries });
}

fn requested_id(harness: &mut Harness) -> u64 {
    harness
        .sent()
        .into_iter()
        .find_map(|command| match command {
            Command::OpenTerminal { id, .. } => Some(id),
            _ => None,
        })
        .expect("an OpenTerminal command")
}

fn open_remote(harness: &mut Harness, session: u64) -> (u64, ShellEnd) {
    harness.app.open_embedded_terminal(TerminalSource::Remote(session));
    let id = requested_id(harness);
    let (stream, end) = shell_pipe();
    harness.feed(Event::TerminalOpened { id, session, stream: Handover::new(stream) });
    (id, end)
}

async fn read_text(end: &mut ShellEnd, length: usize) -> String {
    let mut bytes = vec![0u8; length];
    tokio::time::timeout(Duration::from_secs(2), end.from_client.read_exact(&mut bytes))
        .await
        .expect("bytes within two seconds")
        .unwrap();
    String::from_utf8(bytes).unwrap()
}

#[tokio::test]
async fn requesting_a_remote_terminal_sends_open_terminal() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");

    harness.app.open_embedded_terminal(TerminalSource::Remote(3));

    let sent: Vec<Command> =
        harness.sent().into_iter().filter(|command| matches!(command, Command::OpenTerminal { .. })).collect();
    assert!(
        matches!(
            sent.as_slice(),
            [Command::OpenTerminal { session: 3, size: ShellSize { columns: 80, rows: 24 }, .. }]
        ),
        "{sent:?}"
    );
    assert!(harness.app.drawer().open && harness.app.drawer().focused);
}

#[tokio::test]
async fn a_remote_terminal_starts_in_the_panel_directory_with_a_quoted_cd() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/my data/it's");

    let (_id, mut end) = open_remote(&mut harness, 3);

    let expected = format!("cd -- {}\r", porthmos_ssh::shell_quote("/srv/my data/it's"));
    assert_eq!(read_text(&mut end, expected.len()).await, expected);
    assert!(harness.app.drawer().tab.is_some());
    assert!(harness.app.drawer().wanted.is_none());
}

#[tokio::test]
async fn a_stale_opened_event_is_dropped_and_closes_the_stream() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    harness.app.open_embedded_terminal(TerminalSource::Remote(3));
    let wanted = requested_id(&mut harness);
    let (stream, mut end) = shell_pipe();

    harness.feed(Event::TerminalOpened { id: wanted + 40, session: 3, stream: Handover::new(stream) });

    assert!(harness.app.drawer().tab.is_none());
    let mut buffer = [0u8; 1];
    let count = tokio::time::timeout(Duration::from_secs(2), end.from_client.read(&mut buffer)).await.unwrap().unwrap();
    assert_eq!(count, 0);
    assert!(harness.app.drawer().wanted.is_some());
}

#[tokio::test]
async fn a_second_request_replaces_the_first_terminal() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    let (first, _end) = open_remote(&mut harness, 3);

    harness.app.open_embedded_terminal(TerminalSource::Remote(3));

    let second = requested_id(&mut harness);
    assert!(second > first);
    assert!(harness.app.drawer().tab.is_none());
    assert_eq!(harness.app.drawer().wanted.as_ref().map(|wanted| wanted.id), Some(second));
}

#[tokio::test]
async fn the_exit_event_marks_the_tab_exited_and_keeps_it() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    let (id, _end) = open_remote(&mut harness, 3);

    harness.press(Message::TerminalEventReceived(id, TerminalEvent::Exited(Some(2))));

    let tab = harness.app.drawer().tab.as_ref().expect("the tab stays visible");
    assert_eq!(tab.exited, Some(Some(2)));
}

#[tokio::test]
async fn a_title_event_renames_the_tab() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    let (id, _end) = open_remote(&mut harness, 3);

    harness.press(Message::TerminalEventReceived(id, TerminalEvent::Title("vim".into())));

    assert_eq!(harness.app.drawer().tab.as_ref().unwrap().title, "vim");
}

#[tokio::test]
async fn events_for_another_terminal_are_ignored() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    let (id, _end) = open_remote(&mut harness, 3);

    harness.press(Message::TerminalEventReceived(id + 1, TerminalEvent::Exited(Some(1))));

    assert_eq!(harness.app.drawer().tab.as_ref().unwrap().exited, None);
}

#[tokio::test]
async fn disconnecting_the_session_marks_its_terminal_exited() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    let (_id, _end) = open_remote(&mut harness, 3);

    harness.feed(Event::Disconnected { session: 3, name: "alpha".to_string() });

    assert_eq!(harness.app.drawer().tab.as_ref().unwrap().exited, Some(None));
}

#[tokio::test]
async fn a_failed_open_pushes_an_error_notice_and_clears_wanted() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    harness.app.open_embedded_terminal(TerminalSource::Remote(3));
    let id = requested_id(&mut harness);

    harness.feed(Event::TerminalOpenFailed { id, message: "refused".into() });

    assert!(harness.app.drawer().wanted.is_none());
    let notice = harness.app.notices().last().expect("a notice");
    assert_eq!((notice.severity, notice.message.to_string()), (Severity::Error, "refused".to_string()));
}

#[tokio::test]
async fn closing_the_terminal_drops_the_tab_and_the_channel() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    let (_id, mut end) = open_remote(&mut harness, 3);

    harness.press(Message::TerminalClosed);

    assert!(harness.app.drawer().tab.is_none() && !harness.app.drawer().open && !harness.app.drawer().focused);
    let skipped = read_text(&mut end, 0).await;
    assert!(skipped.is_empty());
    let mut rest = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), end.from_client.read_to_end(&mut rest)).await.unwrap().unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn a_local_terminal_runs_in_the_local_panel_directory() {
    let mut harness = Harness::new();
    let scratch = tempfile::tempdir().unwrap();
    let folder = porthmos_core::from_os_path(&scratch.path().canonicalize().unwrap());
    let wanted = Wanted { id: 1, source: TerminalSource::Local, directory: folder.clone() };
    harness.app.start_local_terminal(wanted);
    let tab = harness.app.drawer().tab.as_ref().expect("a local terminal starts at once");
    tab.session.as_ref().unwrap().write(b"pwd\n".to_vec());

    let expected = folder.to_string_lossy().into_owned();
    let mut seen = Vec::new();
    for _ in 0..100 {
        let snapshot = tab.snapshot().unwrap();
        seen = (0..snapshot.size.rows).map(|row| snapshot.row_text(row)).collect();
        if seen.iter().any(|line| line.trim_end() == expected) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the working directory {expected:?} never appeared; the screen shows {seen:?}");
}

fn press_key(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    let text = match &key {
        Key::Character(text) => Some(text.to_string()),
        Key::Named(Named::Space) => Some(" ".to_string()),
        _ => None,
    };
    harness.press(Message::KeyPressed(KeyPress { key: key.clone(), modified_key: key, modifiers, text }));
}

fn chord(harness: &mut Harness) {
    press_key(harness, Key::Named(Named::Space), Modifiers::CTRL | Modifiers::SHIFT);
}

fn palette_is_open(harness: &Harness) -> bool {
    matches!(harness.app.tool(), Some(Tool::Palette(_)))
}

async fn expect_silence(end: &mut ShellEnd) {
    let mut buffer = [0u8; 1];
    let waited = tokio::time::timeout(Duration::from_millis(150), end.from_client.read(&mut buffer)).await;
    assert!(waited.is_err(), "nothing should have been written, got {buffer:?}");
}

async fn opened_and_ready(harness: &mut Harness) -> (u64, ShellEnd) {
    connect(harness, 3, "alpha", "/srv/data");
    let (id, mut end) = open_remote(harness, 3);
    let cd = format!("cd -- {}\r", porthmos_ssh::shell_quote("/srv/data"));
    read_text(&mut end, cd.len()).await;
    (id, end)
}

#[tokio::test]
async fn a_focused_terminal_receives_ctrl_p_instead_of_opening_the_palette() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;

    press_key(&mut harness, Key::Character("p".into()), Modifiers::CTRL);

    assert_eq!(read_text(&mut end, 1).await.as_bytes(), [0x10]);
    assert!(!palette_is_open(&harness));
}

#[tokio::test]
async fn the_chord_returns_focus_and_the_next_key_runs_a_shortcut() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;

    chord(&mut harness);
    press_key(&mut harness, Key::Character("p".into()), Modifiers::CTRL);

    assert!(!harness.app.drawer().focused);
    assert!(harness.app.drawer().open);
    assert!(palette_is_open(&harness));
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn the_chord_focuses_an_open_but_unfocused_drawer() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    chord(&mut harness);

    chord(&mut harness);

    assert!(harness.app.drawer().focused);
}

#[tokio::test]
async fn an_unfocused_drawer_leaves_every_key_to_the_file_panes() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;
    chord(&mut harness);

    press_key(&mut harness, Key::Character("x".into()), Modifiers::empty());

    expect_silence(&mut end).await;
}

#[tokio::test]
async fn keys_after_the_shell_exited_go_to_the_file_panes_and_write_nothing() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    harness.press(Message::TerminalEventReceived(id, TerminalEvent::Exited(Some(0))));

    press_key(&mut harness, Key::Character("p".into()), Modifiers::CTRL);

    assert!(palette_is_open(&harness));
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn toggling_the_drawer_hides_and_shows_it_without_ending_the_terminal() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;

    harness.app.toggle_terminal();
    assert!(!harness.app.drawer().open && !harness.app.drawer().focused && harness.app.drawer().tab.is_some());

    harness.app.toggle_terminal();
    assert!(harness.app.drawer().open && harness.app.drawer().focused);
}

#[tokio::test]
async fn a_resize_report_resizes_the_session_once_per_distinct_size() {
    let mut harness = Harness::new();
    let (id, end) = opened_and_ready(&mut harness).await;
    let size = porthmos_term::TermSize::clamped(100, 30);

    harness.press(Message::TerminalResized(id, size));
    harness.press(Message::TerminalResized(id, size));
    harness.press(Message::TerminalResized(id + 1, porthmos_term::TermSize::clamped(10, 10)));

    assert_eq!(*end.sizes.lock().unwrap(), vec![ShellSize { columns: 100, rows: 30 }]);
    assert_eq!(harness.app.drawer().tab.as_ref().unwrap().size, size);
}

#[tokio::test]
async fn an_open_drawer_shrinks_the_file_list() {
    let mut harness = Harness::new();
    let closed = harness.app.list_height();

    harness.app.toggle_terminal();

    assert!(harness.app.list_height() < closed);
    harness.app.toggle_terminal();
    assert_eq!(harness.app.list_height(), closed);
}

#[tokio::test]
async fn a_folder_name_with_control_bytes_is_never_typed_into_the_remote_shell() {
    for name in ["/srv/x\u{3}id>~/pwned\r", "/srv/a\nb", "/srv/\u{15}x", "/srv/\u{1b}[2J", "/srv/bad\u{fffd}name"] {
        let mut harness = Harness::new();
        connect(&mut harness, 3, "alpha", name);

        let (_id, mut end) = open_remote(&mut harness, 3);

        expect_silence(&mut end).await;
        assert!(harness.app.drawer().tab.is_some(), "{name:?}");
        let notice = harness.app.notices().last().expect("a notice about the folder");
        assert_eq!(notice.severity, Severity::Warning, "{name:?}");
    }
}

#[tokio::test]
async fn disconnecting_the_session_closes_the_remote_channel_and_keeps_the_last_screen() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;

    harness.feed(Event::Disconnected { session: 3, name: "alpha".to_string() });

    let mut buffer = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(2), end.from_client.read(&mut buffer))
        .await
        .expect("the server end must see the channel close");
    assert_eq!(read.unwrap(), 0);
    let tab = harness.app.drawer().tab.as_ref().unwrap();
    assert!(tab.session.is_none());
    assert!(tab.snapshot().is_some());
}

#[tokio::test]
async fn a_terminal_opened_after_its_session_disconnected_is_dropped() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    harness.app.open_embedded_terminal(TerminalSource::Remote(3));
    let id = requested_id(&mut harness);

    harness.feed(Event::Disconnected { session: 3, name: "alpha".to_string() });
    let (stream, mut end) = shell_pipe();
    harness.feed(Event::TerminalOpened { id, session: 3, stream: Handover::new(stream) });

    assert!(harness.app.drawer().tab.is_none());
    assert!(harness.app.drawer().wanted.is_none());
    let mut buffer = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(2), end.from_client.read(&mut buffer)).await;
    assert_eq!(read.expect("the stream must be closed").unwrap(), 0);
}

#[tokio::test]
async fn keys_typed_while_the_remote_terminal_is_opening_are_not_sent_to_the_file_panes() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    harness.app.open_embedded_terminal(TerminalSource::Remote(3));
    harness.sent();

    press_key(&mut harness, Key::Character("p".into()), Modifiers::CTRL);
    press_key(&mut harness, Key::Named(Named::Enter), Modifiers::empty());
    press_key(&mut harness, Key::Named(Named::F8), Modifiers::empty());

    assert!(!palette_is_open(&harness));
    assert_eq!(harness.sent(), vec![]);
    assert!(harness.app.modals.is_empty());
}

#[tokio::test]
async fn the_chord_unfocuses_a_drawer_whose_terminal_is_still_opening() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv/data");
    harness.app.open_embedded_terminal(TerminalSource::Remote(3));

    chord(&mut harness);

    assert!(!harness.app.drawer().focused);
}

#[tokio::test]
async fn overlays_opened_with_the_mouse_keep_their_keys_from_the_terminal() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    assert!(harness.app.terminal_has_keys());

    harness.app.more_open = true;
    assert!(!harness.app.terminal_has_keys());
    harness.app.more_open = false;

    harness.app.filtering = Some(crate::app::PanelSide::Local);
    assert!(!harness.app.terminal_has_keys());
}

#[test]
fn the_open_failure_text_is_translated_and_carries_the_error() {
    let text = porthmos_core::i18n::text_with("gui.terminal.open_failed", &[("error", "no pty")]);

    assert!(text.contains("no pty") && !text.contains("gui.terminal"), "{text}");
}

#[tokio::test]
async fn opening_a_terminal_closes_the_path_editor_so_it_gets_the_keys() {
    let mut harness = Harness::new();
    harness.press(Message::PathEditStarted(crate::app::PanelSide::Local));
    assert!(harness.app.path_edit().is_some());

    harness.app.open_embedded_terminal(TerminalSource::Local);

    assert!(harness.app.path_edit().is_none());
    assert!(harness.app.terminal_has_keys());
}

#[tokio::test]
async fn focusing_the_terminal_dismisses_menus_and_the_filter() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    chord(&mut harness);
    harness.app.more_open = true;
    harness.app.filtering = Some(crate::app::PanelSide::Local);
    harness.app.filter_text = "abc".to_string();

    harness.app.focus_terminal();

    assert!(!harness.app.more_open && harness.app.filtering.is_none() && harness.app.filter_text.is_empty());
    assert!(harness.app.terminal_has_keys());
}

#[tokio::test]
async fn clicking_the_terminal_focuses_it_and_leaves_path_editing() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;
    chord(&mut harness);
    harness.press(Message::PathEditStarted(crate::app::PanelSide::Local));

    harness.press(Message::TerminalClicked);
    press_key(&mut harness, Key::Character("x".into()), Modifiers::empty());

    assert!(harness.app.drawer().focused);
    assert!(harness.app.path_edit().is_none());
    assert_eq!(read_text(&mut end, 1).await, "x");
}
