use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::preview::{Preview, PreviewContent};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::app::testing::{TestApp, entry, screen_rows, test_app};

fn app_on_a_local_file() -> TestApp {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(vec![entry(&VfsPath::new("/d"), "notes.txt", false), entry(&VfsPath::new("/d"), "sub", true)]);
    test
}

fn move_to(test: &mut TestApp, name: &str) {
    let position = test
        .app
        .local
        .rows()
        .iter()
        .position(|row| matches!(row, porthmos_core::listing::Row::Entry(entry) if entry.name == name))
        .unwrap();
    test.app.local.cursor = position;
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn press(test: &mut TestApp, code: KeyCode) {
    test.app.apply_key(key(code));
}

fn text_result(content: &str) -> Result<Arc<Preview>, String> {
    Ok(Arc::new(Preview {
        size: Some(content.len() as u64),
        shown: content.len(),
        truncated: false,
        content: PreviewContent::Text(content.to_string()),
    }))
}

fn lines(count: usize) -> String {
    (1..=count).map(|index| format!("line {index}\n")).collect()
}

fn draw(test: &TestApp) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal.draw(|frame| test.app.render(frame)).unwrap();
    screen_rows(&terminal)
}

fn opened(test: &mut TestApp) -> u64 {
    move_to(test, "notes.txt");
    test.app.apply_action(Action::Preview);
    match test.sent().as_slice() {
        [Command::Preview { request_id, .. }] => *request_id,
        other => panic!("{other:?}"),
    }
}

#[test]
fn p_on_a_local_file_asks_the_core_and_shows_a_loading_viewer() {
    let mut test = app_on_a_local_file();
    move_to(&mut test, "notes.txt");

    test.app.apply_action(Action::Preview);

    assert_eq!(
        test.sent(),
        vec![Command::Preview { request_id: 1, location: Location::Local, path: VfsPath::new("/d/notes.txt") }]
    );
    assert!(draw(&test)[0].contains("preview \u{b7} notes.txt"));
    assert!(draw(&test).iter().any(|row| row.contains("loading")));
}

#[test]
fn p_on_the_remote_panel_previews_through_the_active_session() {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(4, "srv");
    test.list_remote(session, "/srv", vec![entry(&VfsPath::new("/srv"), "a.log", false)]);
    test.app.active_panel = ActivePanel::Remote;
    test.app.sessions.active_mut().unwrap().panel.cursor = 1;

    test.app.apply_action(Action::Preview);

    assert_eq!(
        test.sent(),
        vec![Command::Preview {
            request_id: 1,
            location: Location::Session(session),
            path: VfsPath::new("/srv/a.log")
        }]
    );
}

#[test]
fn p_with_the_remote_panel_but_no_session_warns_and_sends_nothing() {
    let mut test = app_on_a_local_file();
    test.app.active_panel = ActivePanel::Remote;

    test.app.apply_action(Action::Preview);

    assert!(test.sent().is_empty());
    assert_eq!(test.notification().as_deref(), Some("Connect to a remote server first"));
}

#[test]
fn p_on_a_folder_the_parent_row_or_an_empty_panel_does_not_open_a_viewer() {
    let mut test = app_on_a_local_file();
    move_to(&mut test, "sub");
    test.app.apply_action(Action::Preview);
    assert_eq!(test.notification().as_deref(), Some("Can't preview a folder"));
    test.app.local.cursor = 0;
    test.app.apply_action(Action::Preview);
    let mut empty = test_app(&VfsPath::new("/e"));
    empty.app.apply_action(Action::Preview);

    assert!(test.sent().is_empty() && empty.sent().is_empty());
    assert!(test.app.preview.is_none() && empty.app.preview.is_none());
}

#[test]
fn previewing_is_only_available_on_the_files_screen() {
    let mut test = app_on_a_local_file();
    move_to(&mut test, "notes.txt");
    test.app.screen = Screen::Transfers;

    test.app.apply_action(Action::Preview);

    assert!(test.sent().is_empty() && test.app.preview.is_none());
}

#[test]
fn the_answer_for_the_current_request_fills_the_viewer() {
    let mut test = app_on_a_local_file();
    let id = opened(&mut test);

    test.app.apply_core_event(Event::PreviewReady {
        request_id: id,
        name: "notes.txt".to_string(),
        result: text_result("hello world\n"),
    });

    let rows = draw(&test);
    assert!(rows.iter().any(|row| row.contains("1  hello world")), "{rows:?}");
}

#[test]
fn a_failure_is_shown_in_the_viewer() {
    let mut test = app_on_a_local_file();
    let id = opened(&mut test);

    test.app.apply_core_event(Event::PreviewReady {
        request_id: id,
        name: "notes.txt".to_string(),
        result: Err("Couldn't preview notes.txt:\nPermission denied".to_string()),
    });

    assert!(draw(&test).iter().any(|row| row.contains("Permission denied")));
}

#[test]
fn an_answer_for_an_older_request_or_after_closing_is_ignored() {
    let mut test = app_on_a_local_file();
    let first = opened(&mut test);
    press(&mut test, KeyCode::Esc);
    test.sent();
    let second = opened(&mut test);
    assert_ne!(first, second);

    test.app.apply_core_event(Event::PreviewReady {
        request_id: first,
        name: "notes.txt".to_string(),
        result: text_result("stale"),
    });
    assert!(draw(&test).iter().all(|row| !row.contains("stale")));

    press(&mut test, KeyCode::Esc);
    test.app.apply_core_event(Event::PreviewReady {
        request_id: second,
        name: "notes.txt".to_string(),
        result: text_result("late"),
    });
    assert!(test.app.preview.is_none());
}

#[test]
fn escape_q_and_p_close_the_viewer() {
    for code in [KeyCode::Esc, KeyCode::Char('q'), KeyCode::Char('p')] {
        let mut test = app_on_a_local_file();
        let id = opened(&mut test);
        test.app.apply_core_event(Event::PreviewReady {
            request_id: id,
            name: "notes.txt".to_string(),
            result: text_result("x"),
        });

        press(&mut test, code);

        assert!(test.app.preview.is_none(), "{code:?}");
        assert!(!test.app.should_quit, "{code:?}");
        assert!(test.sent().is_empty(), "{code:?}");
    }
}

#[test]
fn closing_while_it_is_still_loading_cancels_the_read() {
    let mut test = app_on_a_local_file();
    opened(&mut test);

    press(&mut test, KeyCode::Esc);

    assert_eq!(test.sent(), vec![Command::CancelPreview]);
    assert!(test.app.preview.is_none());
}

#[test]
fn opening_a_second_viewer_after_closing_uses_a_new_request_id() {
    let mut test = app_on_a_local_file();
    let first = opened(&mut test);
    press(&mut test, KeyCode::Esc);
    test.sent();

    assert!(opened(&mut test) > first);
}

fn viewer_with_many_lines() -> TestApp {
    let mut test = app_on_a_local_file();
    let id = opened(&mut test);
    test.app.apply_core_event(Event::PreviewReady {
        request_id: id,
        name: "notes.txt".to_string(),
        result: text_result(&lines(200)),
    });
    draw(&test);
    test
}

fn top(test: &TestApp) -> usize {
    test.app.preview.as_ref().unwrap().top()
}

#[test]
fn the_scroll_keys_move_the_viewer() {
    let mut test = viewer_with_many_lines();

    press(&mut test, KeyCode::Down);
    press(&mut test, KeyCode::Char('j'));
    assert_eq!(top(&test), 2);
    press(&mut test, KeyCode::Up);
    press(&mut test, KeyCode::Char('k'));
    assert_eq!(top(&test), 0);
    press(&mut test, KeyCode::PageDown);
    let page = top(&test);
    assert!(page > 5);
    press(&mut test, KeyCode::PageUp);
    assert_eq!(top(&test), 0);
    press(&mut test, KeyCode::Char(' '));
    assert_eq!(top(&test), page);
    press(&mut test, KeyCode::Char('b'));
    assert_eq!(top(&test), 0);
}

#[test]
fn the_end_and_home_keys_jump_to_the_ends() {
    let mut test = viewer_with_many_lines();

    test.app.apply_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
    let bottom = top(&test);
    assert!(bottom > 100);
    press(&mut test, KeyCode::Char('g'));
    assert_eq!(top(&test), 0);
    press(&mut test, KeyCode::End);
    assert_eq!(top(&test), bottom);
    press(&mut test, KeyCode::Home);
    assert_eq!(top(&test), 0);
}

#[test]
fn w_toggles_wrapping() {
    let mut test = viewer_with_many_lines();

    press(&mut test, KeyCode::Char('w'));
    assert!(test.app.preview.as_ref().unwrap().wrap());
    press(&mut test, KeyCode::Char('w'));
    assert!(!test.app.preview.as_ref().unwrap().wrap());
}

#[test]
fn other_keys_do_nothing_while_the_viewer_is_open() {
    let mut test = viewer_with_many_lines();
    let before = test.app.screen;

    for code in [
        KeyCode::Char('c'),
        KeyCode::Char('d'),
        KeyCode::Char('m'),
        KeyCode::Char('e'),
        KeyCode::Char('t'),
        KeyCode::Char('?'),
        KeyCode::Char('/'),
        KeyCode::Tab,
        KeyCode::Enter,
        KeyCode::F(5),
        KeyCode::F(9),
        KeyCode::F(10),
    ] {
        press(&mut test, code);
    }
    test.app.apply_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    test.app.apply_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert!(test.sent().is_empty());
    assert!(test.app.preview.is_some() && test.app.dialog.is_none() && test.app.palette.is_none());
    assert!(!test.app.help_visible && !test.app.should_quit);
    assert_eq!(test.app.screen, before);
}

#[test]
fn the_viewer_covers_the_screen_and_the_files_screen_returns_after_closing() {
    let mut test = viewer_with_many_lines();

    let open = draw(&test);
    assert!(open[0].contains("preview \u{b7} notes.txt"));
    for row in &open {
        assert!(!row.contains("name") && !row.contains("items") && !row.contains("remote"), "{row:?}");
    }

    press(&mut test, KeyCode::Esc);
    let closed = draw(&test);
    assert!(closed.iter().all(|row| !row.contains("preview \u{b7}")), "{closed:?}");
    assert!(closed.iter().any(|row| row.contains("name")), "{closed:?}");
}

#[test]
fn a_toast_still_shows_over_the_viewer() {
    let mut test = viewer_with_many_lines();
    test.app.apply_core_event(Event::Notice { severity: Severity::Warning, message: "heads up".to_string() });

    assert!(draw(&test).iter().any(|row| row.contains("heads up")));
}

fn conflict() -> porthmos_core::transfer::conflicts::ConflictInfo {
    porthmos_core::transfer::conflicts::ConflictInfo {
        display_name: "b.txt".to_string(),
        existing: Some(porthmos_core::transfer::plan::ExistingFile { size: 2, modified: None, is_dir: false }),
        partial: None,
        new_size: 1,
        new_modified: None,
    }
}

#[test]
fn a_conflict_prompt_that_arrives_while_the_viewer_is_open_can_be_answered() {
    let mut test = viewer_with_many_lines();

    test.app.apply_core_event(Event::ConflictsFound { batch_id: 7, files: vec![conflict()] });
    press(&mut test, KeyCode::Char('o'));
    press(&mut test, KeyCode::Enter);

    assert!(test.sent().iter().any(|command| matches!(command, Command::ResolveConflicts { batch_id: 7, .. })));
    assert!(test.app.dialog.is_none());
    assert!(test.app.preview.is_some());
}

#[test]
fn a_password_question_that_arrives_while_the_viewer_is_open_can_be_answered() {
    let mut test = viewer_with_many_lines();

    test.app.apply_core_event(Event::Question {
        request_id: 9,
        question: porthmos_core::Question::Password { username: "u".to_string(), name: "srv".to_string() },
    });
    for character in "pw".chars() {
        press(&mut test, KeyCode::Char(character));
    }
    press(&mut test, KeyCode::Enter);

    assert!(test.sent().iter().any(|command| matches!(command, Command::Answer { request_id: 9, .. })));
    assert!(test.app.preview.is_some());
}

#[test]
fn an_edit_question_that_arrives_while_the_viewer_is_open_can_be_answered() {
    let mut test = viewer_with_many_lines();

    test.app.apply_core_event(Event::EditQuestion {
        edit_id: 4,
        name: "app.conf".to_string(),
        kind: porthmos_core::edit::EditQuestionKind::Upload,
    });
    press(&mut test, KeyCode::Char('y'));

    assert_eq!(test.sent(), vec![Command::ResolveEdit { edit_id: 4, choice: porthmos_core::edit::EditChoice::Upload }]);
    assert!(test.app.preview.is_some());
}

#[test]
fn escape_closes_the_dialog_first_and_the_viewer_stays() {
    let mut test = viewer_with_many_lines();
    test.app.apply_core_event(Event::EditQuestion {
        edit_id: 4,
        name: "app.conf".to_string(),
        kind: porthmos_core::edit::EditQuestionKind::Upload,
    });

    press(&mut test, KeyCode::Esc);

    assert!(test.app.preview.is_some());
}

#[test]
fn a_terminal_too_small_for_the_files_screen_still_shows_the_viewer() {
    let test = viewer_with_many_lines();
    let mut terminal = Terminal::new(TestBackend::new(30, 5)).unwrap();

    terminal.draw(|frame| test.app.render(frame)).unwrap();

    let rows = screen_rows(&terminal);
    assert!(rows[0].contains("preview"), "{rows:?}");
    assert!(rows.iter().all(|row| !row.contains("too small")), "{rows:?}");
}
