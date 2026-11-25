use std::time::Duration;

use iced::keyboard::{Key, Modifiers, key::Named};
use porthmos_core::{Command, Entry, Event, Handover, Location, Severity, ShellSize, VfsPath};
use porthmos_term::{MouseButton, Side, TerminalEvent, ViewPoint};
use tokio::io::AsyncReadExt;

use crate::app::Message;
use crate::drawer::TerminalSource;
use crate::drawer::pointer::PointerKind;
use crate::view::terminal_grid::PointerEvent;

use super::{ClipboardRequest, PASTE_CHUNK, PRIMARY_SELECTION, paste_chunks};
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
    use crate::drawer::Wanted;
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
    let (id, mut end) = opened_and_ready(&mut harness).await;
    chord(&mut harness);
    harness.press(Message::PathEditStarted(crate::app::PanelSide::Local));

    harness.press(Message::TerminalPointer(id, press(0, 0, 1)));
    press_key(&mut harness, Key::Character("x".into()), Modifiers::empty());

    assert!(harness.app.drawer().focused);
    assert!(harness.app.path_edit().is_none());
    assert_eq!(read_text(&mut end, 1).await, "x");
}

fn pointer(kind: PointerKind, row: u16, column: u16, side: Side, clicks: u8, lines: i32) -> PointerEvent {
    PointerEvent { kind, point: ViewPoint { row, column }, side, clicks, lines }
}

fn press(row: u16, column: u16, clicks: u8) -> PointerEvent {
    pointer(PointerKind::Press(MouseButton::Left), row, column, Side::Left, clicks, 0)
}

fn drag(row: u16, column: u16) -> PointerEvent {
    pointer(PointerKind::Move { dragging: true }, row, column, Side::Right, 0, 0)
}

fn release(row: u16, column: u16) -> PointerEvent {
    pointer(PointerKind::Release(MouseButton::Left), row, column, Side::Right, 0, 0)
}

fn wheel(lines: i32) -> PointerEvent {
    pointer(PointerKind::Wheel, 0, 0, Side::Left, 0, lines)
}

fn print(harness: &Harness, text: &str) {
    let tab = harness.app.drawer().tab.as_ref().unwrap();
    tab.session.as_ref().unwrap().with_terminal(|terminal| terminal.feed(text.as_bytes()));
}

fn selected(harness: &Harness) -> Option<String> {
    let tab = harness.app.drawer().tab.as_ref().unwrap();
    tab.session.as_ref().unwrap().with_terminal(|terminal| terminal.selected_text())
}

fn offset(harness: &Harness) -> usize {
    harness.app.drawer().tab.as_ref().unwrap().snapshot().unwrap().display_offset
}

async fn read_bytes(end: &mut ShellEnd, length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    tokio::time::timeout(Duration::from_secs(5), end.from_client.read_exact(&mut bytes))
        .await
        .expect("bytes within five seconds")
        .unwrap();
    bytes
}

#[tokio::test]
async fn the_first_click_on_an_unfocused_drawer_only_focuses_it() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    chord(&mut harness);

    harness.app.route_pointer(id, press(0, 0, 1));

    assert!(harness.app.drawer().focused);
    assert_eq!(selected(&harness), None);
    expect_silence(&mut end).await;

    harness.app.route_pointer(id, press(0, 0, 1));
    harness.app.route_pointer(id, drag(0, 4));
    assert_eq!(selected(&harness), Some("hello".to_string()));
}

#[tokio::test]
async fn dragging_selects_and_the_release_offers_the_primary_selection() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");

    harness.app.route_pointer(id, press(0, 0, 1));
    harness.app.route_pointer(id, drag(0, 4));
    let outcome = harness.app.route_pointer(id, release(0, 4));

    assert_eq!(selected(&harness), Some("hello".to_string()));
    let expected =
        if PRIMARY_SELECTION { ClipboardRequest::WritePrimary("hello".to_string()) } else { ClipboardRequest::None };
    assert_eq!(outcome, expected);
}

#[tokio::test]
async fn a_double_click_selects_a_word_and_a_triple_click_a_line() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");

    harness.app.route_pointer(id, press(0, 7, 2));
    assert_eq!(selected(&harness), Some("world".to_string()));

    harness.app.route_pointer(id, press(0, 7, 3));
    assert_eq!(selected(&harness), Some("hello world\n".to_string()));
}

#[tokio::test]
async fn alt_drag_selects_a_block() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "abcd\r\nefgh\r\n");
    harness.app.modifiers = Modifiers::ALT;

    harness.app.route_pointer(id, press(0, 1, 1));
    harness.app.route_pointer(id, drag(1, 2));

    assert_eq!(selected(&harness), Some("bc\nfg".to_string()));
}

#[tokio::test]
async fn a_press_and_release_on_one_cell_clears_the_selection() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    harness.app.route_pointer(id, press(0, 0, 2));
    assert!(selected(&harness).is_some());

    harness.app.route_pointer(id, press(0, 3, 1));
    let outcome =
        harness.app.route_pointer(id, pointer(PointerKind::Release(MouseButton::Left), 0, 3, Side::Left, 0, 0));

    assert_eq!(selected(&harness), None);
    assert_eq!(outcome, ClipboardRequest::None);
}

#[tokio::test]
async fn a_program_that_asked_for_the_mouse_gets_press_and_release_bytes() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "\x1b[?1000h\x1b[?1006h");

    harness.app.route_pointer(id, press(0, 0, 1));
    harness.app.route_pointer(id, pointer(PointerKind::Release(MouseButton::Left), 0, 0, Side::Left, 0, 0));

    assert_eq!(read_bytes(&mut end, 9).await, b"\x1b[<0;1;1M");
    assert_eq!(read_bytes(&mut end, 9).await, b"\x1b[<0;1;1m");
    assert_eq!(selected(&harness), None);
}

#[tokio::test]
async fn shift_bypasses_reporting_and_selects_instead() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "\x1b[?1000h\x1b[?1006hhello world");
    harness.app.modifiers = Modifiers::SHIFT;

    harness.app.route_pointer(id, press(0, 0, 1));
    harness.app.route_pointer(id, drag(0, 4));

    assert_eq!(selected(&harness), Some("hello".to_string()));
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn the_wheel_scrolls_the_history_and_typing_returns_to_the_bottom() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    for number in 0..100 {
        print(&harness, &format!("row {number}\r\n"));
    }

    harness.app.route_pointer(id, wheel(5));
    assert_eq!(offset(&harness), 5);
    harness.app.route_pointer(id, wheel(-2));
    assert_eq!(offset(&harness), 3);

    press_key(&mut harness, Key::Character("a".into()), Modifiers::empty());
    assert_eq!(offset(&harness), 0);
    assert_eq!(read_text(&mut end, 1).await, "a");
}

#[tokio::test]
async fn typing_clears_the_selection() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    harness.app.route_pointer(id, press(0, 0, 2));
    assert!(selected(&harness).is_some());

    press_key(&mut harness, Key::Character("a".into()), Modifiers::empty());

    assert_eq!(selected(&harness), None);
}

#[tokio::test]
async fn the_wheel_on_the_alternate_screen_sends_arrow_keys() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "\x1b[?1049h");

    harness.app.route_pointer(id, wheel(3));
    harness.app.route_pointer(id, wheel(-1));

    assert_eq!(read_bytes(&mut end, 9).await, b"\x1b[A\x1b[A\x1b[A");
    assert_eq!(read_bytes(&mut end, 3).await, b"\x1b[B");
}

#[tokio::test]
async fn a_middle_click_asks_for_the_primary_selection() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;

    let outcome =
        harness.app.route_pointer(id, pointer(PointerKind::Press(MouseButton::Middle), 0, 0, Side::Left, 1, 0));

    assert_eq!(outcome, ClipboardRequest::ReadPrimary);
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn pasted_text_goes_through_the_bracketed_paste_encoding() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "\x1b[?2004h");

    harness.app.terminal_pasted(id, Some("ls\n".to_string()));

    assert_eq!(read_bytes(&mut end, 15).await, b"\x1b[200~ls\n\x1b[201~");
}

#[tokio::test]
async fn pasting_escape_sequences_never_writes_them_in_either_mode() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;

    harness.app.terminal_pasted(id, Some("a\x1b[31mb\u{85}c".to_string()));
    assert_eq!(read_bytes(&mut end, 7).await, b"a[31mbc");
    print(&harness, "\x1b[?2004h");
    harness.app.terminal_pasted(id, Some("x\x1b[201~y".to_string()));

    assert_eq!(read_bytes(&mut end, 19).await, b"\x1b[200~x[201~y\x1b[201~");
}

#[tokio::test]
async fn a_huge_paste_arrives_complete_and_in_order() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    let text: String = (0..1_048_576usize).map(|index| char::from(b'a' + (index % 26) as u8)).collect();

    harness.app.terminal_pasted(id, Some(text.clone()));

    assert_eq!(read_bytes(&mut end, text.len()).await, text.into_bytes());
}

#[test]
fn paste_chunks_never_exceed_the_chunk_size_and_keep_every_byte() {
    let bytes: Vec<u8> = (0..(PASTE_CHUNK * 2 + 5)).map(|index| index as u8).collect();

    let chunks = paste_chunks(&bytes);

    assert!(chunks.iter().all(|chunk| !chunk.is_empty() && chunk.len() <= PASTE_CHUNK));
    assert_eq!(chunks.concat(), bytes);
    assert!(paste_chunks(&[]).is_empty());
}

#[tokio::test]
async fn a_paste_for_a_closed_or_replaced_tab_is_dropped() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    harness.press(Message::TerminalClosed);

    harness.app.terminal_pasted(id, Some("late".to_string()));
    harness.app.terminal_pasted(id + 5, Some("other".to_string()));
    harness.app.terminal_pasted(id, None);
    harness.app.terminal_pasted(id, Some(String::new()));

    assert!(harness.app.drawer().tab.is_none());
}

#[tokio::test]
async fn selection_and_scroll_still_work_after_the_shell_exited_but_reporting_does_not() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "\x1b[?1000hhello world");
    harness.press(Message::TerminalEventReceived(id, TerminalEvent::Exited(Some(0))));

    harness.app.route_pointer(id, press(0, 0, 1));
    expect_silence(&mut end).await;

    harness.app.modifiers = Modifiers::SHIFT;
    harness.app.route_pointer(id, press(0, 0, 1));
    harness.app.route_pointer(id, drag(0, 4));
    assert_eq!(selected(&harness), Some("hello".to_string()));
}

#[tokio::test]
async fn pointer_events_for_another_terminal_are_ignored() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello");

    let outcome = harness.app.route_pointer(id + 1, press(0, 0, 2));

    assert_eq!(outcome, ClipboardRequest::None);
    assert_eq!(selected(&harness), None);
}

#[tokio::test]
async fn the_clipboard_event_from_a_program_changes_nothing() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello");

    harness.press(Message::TerminalEventReceived(id, TerminalEvent::ClipboardStore("secret".to_string())));

    assert_eq!(selected(&harness), None);
    expect_silence(&mut end).await;
}

fn ctrl_shift(harness: &mut Harness, letter: &str) -> iced::Task<Message> {
    let key = Key::Character(letter.into());
    harness.app.update(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::CTRL | Modifiers::SHIFT,
        text: None,
    }))
}

#[tokio::test]
async fn ctrl_shift_c_copies_the_selection_and_never_sends_ctrl_c() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    harness.app.route_pointer(id, press(0, 0, 2));

    let task = ctrl_shift(&mut harness, "c");

    assert!(task.units() > 0);
    assert_eq!(harness.app.terminal_copy_text(), Some("hello".to_string()));
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn ctrl_shift_c_without_a_selection_does_nothing() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;

    let task = ctrl_shift(&mut harness, "c");

    assert_eq!(task.units(), 0);
    assert_eq!(harness.app.terminal_copy_text(), None);
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn ctrl_shift_v_asks_for_the_clipboard_and_writes_nothing_itself() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;

    let task = ctrl_shift(&mut harness, "v");

    assert!(task.units() > 0);
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn shift_page_up_and_down_scroll_the_history_and_send_nothing() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;
    for number in 0..100 {
        print(&harness, &format!("row {number}\r\n"));
    }

    press_key(&mut harness, Key::Named(Named::PageUp), Modifiers::SHIFT);
    let up = offset(&harness);
    press_key(&mut harness, Key::Named(Named::PageDown), Modifiers::SHIFT);

    assert!(up > 10, "{up}");
    assert!(offset(&harness) < up);
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn the_chords_leave_an_unfocused_drawer_alone() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    harness.app.route_pointer(id, press(0, 0, 2));
    chord(&mut harness);

    let task = ctrl_shift(&mut harness, "c");

    assert_eq!(task.units(), 0);
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn the_palette_copy_action_copies_the_selection() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    harness.app.route_pointer(id, press(0, 0, 2));

    let task = harness.app.update(Message::ActionRequested(crate::shortcuts::Action::TerminalCopy));

    assert!(task.units() > 0);
}

fn selected_rows(harness: &Harness) -> Vec<u16> {
    let snapshot = harness.app.drawer().tab.as_ref().unwrap().snapshot().unwrap();
    let columns = usize::from(snapshot.size.columns);
    (0..snapshot.size.rows)
        .filter(|row| snapshot.cells[usize::from(*row) * columns..][..columns].iter().any(|cell| cell.selected))
        .collect()
}

fn open_search(harness: &mut Harness) {
    let _ = ctrl_shift(harness, "f");
}

fn type_query(harness: &mut Harness, query: &str) {
    harness.press(Message::TerminalSearchTyped(query.to_string()));
}

#[tokio::test]
async fn ctrl_shift_f_opens_the_search_bar_and_takes_the_keys() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;

    let task = ctrl_shift(&mut harness, "f");
    press_key(&mut harness, Key::Character("x".into()), Modifiers::empty());

    assert!(harness.app.drawer().search.open);
    assert!(task.units() > 0);
    assert!(!harness.app.terminal_has_keys());
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn typing_a_query_selects_the_first_match() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "alpha beta gamma");
    open_search(&mut harness);

    type_query(&mut harness, "beta");

    assert_eq!(selected(&harness), Some("beta".to_string()));
    assert!(!harness.app.drawer().search.missed);
}

#[tokio::test]
async fn next_and_previous_walk_the_matches_and_wrap() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "ab\r\nab\r\nab\r\n");
    open_search(&mut harness);

    type_query(&mut harness, "ab");
    assert_eq!(selected_rows(&harness), vec![0]);
    harness.press(Message::TerminalSearchNext);
    assert_eq!(selected_rows(&harness), vec![1]);
    harness.press(Message::TerminalSearchNext);
    assert_eq!(selected_rows(&harness), vec![2]);
    harness.press(Message::TerminalSearchNext);
    assert_eq!(selected_rows(&harness), vec![0]);
    harness.press(Message::TerminalSearchPrevious);
    assert_eq!(selected_rows(&harness), vec![2]);
}

#[tokio::test]
async fn submitting_means_next_and_shift_submit_means_previous() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "ab\r\nab\r\nab\r\n");
    open_search(&mut harness);
    type_query(&mut harness, "ab");

    harness.press(Message::TerminalSearchSubmitted);
    assert_eq!(selected_rows(&harness), vec![1]);
    harness.app.modifiers = Modifiers::SHIFT;
    harness.press(Message::TerminalSearchSubmitted);
    assert_eq!(selected_rows(&harness), vec![0]);
}

#[tokio::test]
async fn a_single_match_does_not_loop_or_miss() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "only one here");
    open_search(&mut harness);
    type_query(&mut harness, "only");

    harness.press(Message::TerminalSearchNext);
    harness.press(Message::TerminalSearchNext);

    assert_eq!(selected(&harness), Some("only".to_string()));
    assert!(!harness.app.drawer().search.missed);
}

#[tokio::test]
async fn a_missing_query_is_flagged_and_clears_the_selection() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "alpha beta");
    open_search(&mut harness);
    type_query(&mut harness, "beta");
    assert!(selected(&harness).is_some());

    type_query(&mut harness, "zzz");

    assert!(harness.app.drawer().search.missed);
    assert_eq!(selected(&harness), None);
}

#[tokio::test]
async fn an_empty_query_clears_the_selection_without_error() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "alpha beta");
    open_search(&mut harness);
    type_query(&mut harness, "beta");

    type_query(&mut harness, "");
    harness.press(Message::TerminalSearchNext);
    harness.press(Message::TerminalSearchPrevious);

    assert_eq!(selected(&harness), None);
    assert!(!harness.app.drawer().search.missed);
}

#[tokio::test]
async fn regex_metacharacters_are_searched_literally() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "axb(c\r\na.b(c");
    open_search(&mut harness);

    type_query(&mut harness, "a.b(c");

    assert_eq!(selected_rows(&harness), vec![1]);
}

#[tokio::test]
async fn a_query_with_a_newline_matches_nothing_and_does_not_panic() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "a\r\nb");
    open_search(&mut harness);

    type_query(&mut harness, "a\nb");

    assert_eq!(selected(&harness), None);
}

#[tokio::test]
async fn escape_closes_the_bar_and_returns_the_keys() {
    let mut harness = Harness::new();
    let (_id, mut end) = opened_and_ready(&mut harness).await;
    open_search(&mut harness);

    press_key(&mut harness, Key::Named(Named::Escape), Modifiers::empty());

    assert!(!harness.app.drawer().search.open);
    assert!(harness.app.terminal_has_keys());
    press_key(&mut harness, Key::Character("a".into()), Modifiers::empty());
    assert_eq!(read_text(&mut end, 1).await, "a");
}

#[tokio::test]
async fn closing_the_drawer_or_the_tab_resets_the_open_state_but_keeps_the_query() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    open_search(&mut harness);
    type_query(&mut harness, "abc");

    harness.app.toggle_terminal();
    assert!(!harness.app.drawer().search.open);
    assert_eq!(harness.app.drawer().search.query, "abc");

    harness.app.toggle_terminal();
    open_search(&mut harness);
    harness.press(Message::TerminalClosed);
    assert!(!harness.app.drawer().search.open);
    assert_eq!(harness.app.drawer().search.query, "abc");
}

#[tokio::test]
async fn clicking_the_terminal_closes_the_bar() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    open_search(&mut harness);

    harness.app.route_pointer(id, press(0, 0, 1));

    assert!(!harness.app.drawer().search.open);
    assert!(harness.app.drawer().focused);
}

#[tokio::test]
async fn a_search_in_a_large_scrollback_finishes() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    for number in 0..12_000 {
        print(&harness, &format!("line {number:05}\r\n"));
    }
    open_search(&mut harness);
    let started = std::time::Instant::now();

    type_query(&mut harness, "line 00042");
    harness.press(Message::TerminalSearchNext);
    type_query(&mut harness, "no such text");

    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
}

#[tokio::test]
async fn a_match_in_the_history_scrolls_into_view() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    for number in 0..100 {
        print(&harness, &format!("row {number:03}\r\n"));
    }
    open_search(&mut harness);

    type_query(&mut harness, "row 003");

    assert!(offset(&harness) > 0);
    assert_eq!(selected(&harness), Some("row 003".to_string()));
}

#[tokio::test]
async fn the_search_action_opens_the_bar_from_the_palette() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;

    let task = harness.app.update(Message::ActionRequested(crate::shortcuts::Action::TerminalSearch));

    assert!(harness.app.drawer().search.open);
    assert!(task.units() > 0);
}

fn reporting_on(harness: &Harness, extra: &str) {
    print(harness, &format!("\x1b[?1000h\x1b[?1006h{extra}"));
}

#[tokio::test]
async fn the_rest_of_a_focusing_click_neither_selects_nor_reports() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    harness.app.route_pointer(id, press(0, 0, 2));
    chord(&mut harness);
    assert_eq!(selected(&harness), Some("hello".to_string()));

    harness.app.route_pointer(id, press(0, 8, 1));
    harness.app.route_pointer(id, drag(0, 10));
    let outcome = harness.app.route_pointer(id, release(0, 10));

    assert!(harness.app.drawer().focused);
    assert_eq!(selected(&harness), Some("hello".to_string()));
    assert_eq!(outcome, ClipboardRequest::None);
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn the_rest_of_a_focusing_click_is_not_reported_to_a_program_either() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    reporting_on(&harness, "");
    chord(&mut harness);

    harness.app.route_pointer(id, press(0, 0, 1));
    harness.app.route_pointer(id, drag(0, 3));
    harness.app.route_pointer(id, pointer(PointerKind::Release(MouseButton::Left), 0, 3, Side::Left, 0, 0));
    expect_silence(&mut end).await;

    harness.app.route_pointer(id, press(0, 0, 1));
    assert_eq!(read_bytes(&mut end, 9).await, b"\x1b[<0;1;1M");
}

#[tokio::test]
async fn a_reported_press_keeps_reporting_when_shift_is_pressed_mid_drag() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    reporting_on(&harness, "hello");
    print(&harness, "\x1b[?1002h");

    harness.app.route_pointer(id, press(0, 0, 1));
    assert_eq!(read_bytes(&mut end, 9).await, b"\x1b[<0;1;1M");
    harness.app.modifiers = Modifiers::SHIFT;
    harness.app.route_pointer(id, drag(0, 2));
    harness.app.route_pointer(id, release(0, 2));

    assert_eq!(selected(&harness), None);
    let motion = read_bytes(&mut end, 10).await;
    assert!(motion.starts_with(b"\x1b[<"), "{motion:?}");
    assert_eq!(read_bytes(&mut end, 1).await.len(), 1);
}

#[tokio::test]
async fn a_selecting_press_keeps_selecting_when_shift_is_released_mid_drag() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    reporting_on(&harness, "hello world");
    harness.app.modifiers = Modifiers::SHIFT;

    harness.app.route_pointer(id, press(0, 0, 1));
    harness.app.modifiers = Modifiers::empty();
    harness.app.route_pointer(id, drag(0, 4));
    harness.app.route_pointer(id, release(0, 4));

    assert_eq!(selected(&harness), Some("hello".to_string()));
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn a_middle_button_drag_is_reported_with_that_button_in_drag_mode() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    reporting_on(&harness, "\x1b[?1002h");

    harness.app.route_pointer(id, pointer(PointerKind::Press(MouseButton::Middle), 0, 0, Side::Left, 1, 0));
    assert_eq!(read_bytes(&mut end, 9).await, b"\x1b[<1;1;1M");
    harness.app.route_pointer(id, drag(0, 4));

    assert_eq!(read_bytes(&mut end, 10).await, b"\x1b[<33;5;1M");
}

#[tokio::test]
async fn a_drag_report_without_a_gesture_is_ignored() {
    let mut harness = Harness::new();
    let (id, mut end) = opened_and_ready(&mut harness).await;
    print(&harness, "hello world");
    harness.app.route_pointer(id, press(0, 0, 2));
    harness.app.route_pointer(id, release(0, 0));

    harness.app.route_pointer(id, drag(0, 9));
    let outcome = harness.app.route_pointer(id, release(0, 9));

    assert_eq!(selected(&harness), Some("hello".to_string()));
    assert_eq!(outcome, ClipboardRequest::None);
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn closing_the_tab_forgets_the_gesture() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    harness.app.route_pointer(id, press(0, 0, 1));

    harness.press(Message::TerminalClosed);

    assert_eq!(harness.app.drawer().gesture, crate::drawer::Gesture::Idle);
}

#[tokio::test]
async fn stepping_through_matches_after_output_scrolled_the_screen_continues_from_the_match() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "ab\r\nab\r\nab\r\n");
    open_search(&mut harness);
    type_query(&mut harness, "ab");
    harness.press(Message::TerminalSearchNext);

    for number in 0..40 {
        print(&harness, &format!("filler {number}\r\n"));
    }
    harness.press(Message::TerminalSearchNext);

    assert_eq!(selection_line(&harness), 2 - history_of(&harness) as i32);
}

fn history_of(harness: &Harness) -> usize {
    let tab = harness.app.drawer().tab.as_ref().unwrap();
    tab.session.as_ref().unwrap().with_terminal(|terminal| terminal.history())
}

fn selection_line(harness: &Harness) -> i32 {
    let tab = harness.app.drawer().tab.as_ref().unwrap();
    tab.session.as_ref().unwrap().with_terminal(|terminal| terminal.selection_match().unwrap().start.line)
}

#[tokio::test]
async fn typing_more_of_the_query_keeps_the_current_match() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    print(&harness, "abc abd\r\nabd\r\n");
    open_search(&mut harness);
    type_query(&mut harness, "ab");
    harness.press(Message::TerminalSearchNext);
    harness.press(Message::TerminalSearchNext);
    assert_eq!(selection_line(&harness), 1);

    type_query(&mut harness, "abd");

    assert_eq!(selected(&harness), Some("abd".to_string()));
    assert_eq!(selection_line(&harness), 1);
}

#[tokio::test]
async fn the_search_buttons_hand_the_keys_back_to_the_input() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    open_search(&mut harness);

    assert!(harness.app.update(Message::TerminalSearchNext).units() > 0);
    assert!(harness.app.update(Message::TerminalSearchPrevious).units() > 0);
}

#[tokio::test]
async fn keys_the_search_input_did_not_take_never_reach_the_file_panes() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: start.clone(),
        entries: vec![Entry { name: "a.txt".to_string(), path: start.join("a.txt"), ..Entry::default() }],
    });
    let (_id, mut end) = opened_and_ready(&mut harness).await;
    open_search(&mut harness);
    harness.sent();

    press_key(&mut harness, Key::Named(Named::Delete), Modifiers::empty());
    press_key(&mut harness, Key::Named(Named::Enter), Modifiers::empty());
    press_key(&mut harness, Key::Named(Named::F5), Modifiers::empty());

    assert!(harness.app.modals.is_empty());
    assert!(harness.sent().is_empty());
    expect_silence(&mut end).await;
    assert!(harness.app.drawer().search.open);
}

#[tokio::test]
async fn escape_closes_a_modal_opened_over_the_search_bar_first() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    harness.feed(Event::Listed { location: Location::Local, path: start, entries: Vec::new() });
    let (_id, _end) = opened_and_ready(&mut harness).await;
    open_search(&mut harness);
    harness.press(Message::ActionRequested(crate::shortcuts::Action::NewFolder));
    assert!(!harness.app.modals.is_empty());
    assert!(harness.app.drawer().search.open);

    press_key(&mut harness, Key::Named(Named::Escape), Modifiers::empty());

    assert!(harness.app.modals.is_empty());
    assert!(harness.app.drawer().search.open);
}

#[tokio::test]
async fn the_chord_searches_again_by_refocusing_the_input() {
    let mut harness = Harness::new();
    let (_id, _end) = opened_and_ready(&mut harness).await;
    open_search(&mut harness);

    let task = ctrl_shift(&mut harness, "f");

    assert!(task.units() > 0);
    assert!(harness.app.drawer().search.open);
}

async fn exit_shell(harness: &mut Harness, id: u64) {
    harness.press(Message::TerminalEventReceived(id, TerminalEvent::Exited(Some(0))));
}

#[tokio::test]
async fn copy_and_page_chords_still_work_after_the_shell_exited() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    for number in 0..100 {
        print(&harness, &format!("row {number}\r\n"));
    }
    harness.app.route_pointer(id, press(0, 0, 2));
    exit_shell(&mut harness, id).await;

    let task = ctrl_shift(&mut harness, "c");
    press_key(&mut harness, Key::Named(Named::PageUp), Modifiers::SHIFT);

    assert!(task.units() > 0);
    assert!(offset(&harness) > 0);
}

#[tokio::test]
async fn ctrl_shift_v_after_the_shell_exited_does_nothing() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    exit_shell(&mut harness, id).await;

    assert_eq!(ctrl_shift(&mut harness, "v").units(), 0);
}

#[tokio::test]
async fn the_search_chord_opens_the_bar_after_the_shell_exited() {
    let mut harness = Harness::new();
    let (id, _end) = opened_and_ready(&mut harness).await;
    exit_shell(&mut harness, id).await;

    let task = ctrl_shift(&mut harness, "f");

    assert!(harness.app.drawer().search.open);
    assert!(task.units() > 0);
}
