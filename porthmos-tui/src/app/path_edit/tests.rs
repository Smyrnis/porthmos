use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::VfsPath;

use super::*;
use crate::app::testing::{TestApp, test_app};
use crate::palette::Target;

fn ctrl_apostrophe() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('\''), KeyModifiers::CONTROL)
}

fn press(test: &mut TestApp, code: KeyCode) {
    test.app.apply_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn type_text(test: &mut TestApp, text: &str) {
    for character in text.chars() {
        press(test, KeyCode::Char(character));
    }
}

fn field_text(test: &TestApp) -> Option<String> {
    test.app.path_edit.as_ref().map(|edit| edit.field.text.clone())
}

fn with_remote() -> (TestApp, u64) {
    let mut test = test_app(&VfsPath::new("/d"));
    let session = test.connect(3, "srv");
    test.list_remote(session, "/srv/data", Vec::new());
    test.app.active_panel = ActivePanel::Remote;
    test.sent();
    (test, session)
}

fn go_to_paths(commands: Vec<Command>) -> Vec<(Location, VfsPath, String)> {
    commands
        .into_iter()
        .filter_map(|command| match command {
            Command::GoToPath { location, from, text } => Some((location, from, text)),
            _ => None,
        })
        .collect()
}

#[test]
fn ctrl_apostrophe_opens_the_field_filled_with_the_local_folder() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_key(ctrl_apostrophe());

    assert_eq!(field_text(&test).as_deref(), Some("/d"));
    assert_eq!(test.app.path_edit.as_ref().unwrap().location, Location::Local);
}

#[test]
fn the_remote_panel_field_is_filled_with_the_remote_folder() {
    let (mut test, session) = with_remote();

    test.app.apply_key(ctrl_apostrophe());

    assert_eq!(field_text(&test).as_deref(), Some("/srv/data"));
    assert_eq!(test.app.path_edit.as_ref().unwrap().location, Location::Session(session));
}

#[test]
fn opening_on_the_remote_panel_without_a_connection_warns_and_opens_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.active_panel = ActivePanel::Remote;

    test.app.apply_key(ctrl_apostrophe());

    assert!(test.app.path_edit.is_none());
    assert!(test.notification().is_some());
}

#[test]
fn typing_edits_the_text_and_sends_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());

    type_text(&mut test, "/x");

    assert_eq!(field_text(&test).as_deref(), Some("/d/x"));
    assert!(test.sent().is_empty());
}

#[test]
fn enter_sends_go_to_path_with_the_local_folder_as_from() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/sub");

    press(&mut test, KeyCode::Enter);

    assert_eq!(go_to_paths(test.sent()), vec![(Location::Local, VfsPath::new("/d"), "/d/sub".to_string())]);
    assert!(test.app.path_edit.as_ref().unwrap().waiting);
}

#[test]
fn enter_on_a_remote_panel_sends_the_session_location() {
    let (mut test, session) = with_remote();
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/x");

    press(&mut test, KeyCode::Enter);

    assert_eq!(
        go_to_paths(test.sent()),
        vec![(Location::Session(session), VfsPath::new("/srv/data"), "/srv/data/x".to_string())]
    );
}

#[test]
fn enter_on_blank_text_closes_the_field_and_sends_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    for _ in 0..2 {
        press(&mut test, KeyCode::Backspace);
    }
    assert_eq!(field_text(&test).as_deref(), Some(""));

    press(&mut test, KeyCode::Enter);

    assert!(test.app.path_edit.is_none());
    assert!(test.sent().is_empty());
}

#[test]
fn escape_closes_the_field_and_leaves_the_panel_unchanged() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "zzz");

    press(&mut test, KeyCode::Esc);

    assert!(test.app.path_edit.is_none());
    assert!(test.sent().is_empty());
    assert_eq!(test.app.local.path(), &VfsPath::new("/d"));
}

#[test]
fn while_waiting_the_field_ignores_a_second_enter() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Enter);
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert!(go_to_paths(test.sent()).is_empty());
}

#[test]
fn the_folder_opening_closes_the_field() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Enter);

    test.app.apply_core_event(Event::PathOpened { location: Location::Local });

    assert!(test.app.path_edit.is_none());
}

#[test]
fn a_refresh_listing_while_waiting_keeps_the_field_for_a_rejection() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/nowhere");
    press(&mut test, KeyCode::Enter);

    test.app.apply_core_event(Event::Listed {
        location: Location::Local,
        path: VfsPath::new("/d"),
        entries: Vec::new(),
    });
    test.app.apply_core_event(Event::PathRejected { location: Location::Local });

    let edit = test.app.path_edit.as_ref().expect("the field survives the refresh");
    assert_eq!(edit.field.text, "/d/nowhere");
    assert!(!edit.waiting);
}

#[test]
fn a_late_opening_after_escape_does_not_touch_a_newer_field() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Enter);
    press(&mut test, KeyCode::Esc);
    test.app.apply_key(ctrl_apostrophe());

    test.app.apply_core_event(Event::PathOpened { location: Location::Local });

    assert!(test.app.path_edit.is_some());
}

#[test]
fn an_opening_for_the_other_location_keeps_the_field_open() {
    let (mut test, session) = with_remote();
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Enter);

    test.app.apply_core_event(Event::PathOpened { location: Location::Local });

    assert_eq!(test.app.path_edit.as_ref().unwrap().location, Location::Session(session));
}

#[test]
fn an_opening_before_enter_does_not_close_the_field() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());

    test.app.apply_core_event(Event::PathOpened { location: Location::Local });

    assert!(test.app.path_edit.is_some());
}

#[test]
fn path_rejected_keeps_the_field_with_its_text_and_allows_another_try() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/oops");
    press(&mut test, KeyCode::Enter);
    test.sent();

    test.app.apply_core_event(Event::PathRejected { location: Location::Local });

    let edit = test.app.path_edit.as_ref().unwrap();
    assert!(!edit.waiting);
    assert_eq!(edit.field.text, "/d/oops");
    press(&mut test, KeyCode::Backspace);
    press(&mut test, KeyCode::Enter);
    assert_eq!(go_to_paths(test.sent()).len(), 1);
}

#[test]
fn path_rejected_for_the_other_location_is_ignored() {
    let (mut test, _) = with_remote();
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Enter);

    test.app.apply_core_event(Event::PathRejected { location: Location::Local });

    assert!(test.app.path_edit.as_ref().unwrap().waiting);
}

#[test]
fn leaving_the_files_screen_drops_the_field() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());

    test.app.screen = Screen::Connections;
    press(&mut test, KeyCode::Down);

    assert!(test.app.path_edit.is_none());
}

#[test]
fn the_session_disconnecting_closes_a_remote_field() {
    let (mut test, session) = with_remote();
    test.app.apply_key(ctrl_apostrophe());

    test.app.apply_core_event(Event::Disconnected { session, name: "srv".to_string() });

    assert!(test.app.path_edit.is_none());
}

#[test]
fn switching_the_active_session_drops_a_remote_field() {
    let (mut test, _) = with_remote();
    test.app.apply_key(ctrl_apostrophe());
    let second = test.connect(4, "other");
    test.list_remote(second, "/o", Vec::new());

    assert!(test.app.path_edit.is_none());
}

#[test]
fn a_key_that_finds_the_field_stale_is_swallowed_and_not_run_as_a_command() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    test.app.screen = Screen::Connections;

    press(&mut test, KeyCode::Char('q'));

    assert!(test.app.path_edit.is_none());
    assert!(!test.app.should_quit);
}

#[test]
fn ordinary_keys_do_not_reach_the_panel_while_the_field_is_open() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());

    type_text(&mut test, "qjd");

    assert!(!test.app.should_quit);
    assert_eq!(field_text(&test).as_deref(), Some("/dqjd"));
    assert!(test.sent().is_empty());
}

#[test]
fn the_action_is_in_the_palette_on_the_files_screen_only() {
    let mut test = test_app(&VfsPath::new("/d"));

    let on_files =
        test.app.palette_entries().iter().any(|entry| matches!(entry.target, Target::Action(Action::GoToPath)));
    test.app.screen = Screen::Connections;
    let on_connections =
        test.app.palette_entries().iter().any(|entry| matches!(entry.target, Target::Action(Action::GoToPath)));

    assert!(on_files);
    assert!(!on_connections);
}

#[test]
fn choosing_the_palette_entry_opens_the_field() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_action(Action::GoToPath);

    assert!(test.app.path_edit.is_some());
}

fn complete_paths(commands: Vec<Command>) -> Vec<(Location, VfsPath, String)> {
    commands
        .into_iter()
        .filter_map(|command| match command {
            Command::CompletePath { location, from, text } => Some((location, from, text)),
            _ => None,
        })
        .collect()
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn reply(test: &mut TestApp, location: Location, text: &str, prefix: &str, candidates: &[&str], completed: &str) {
    let completion = porthmos_core::Completion {
        prefix: prefix.to_string(),
        candidates: names(candidates),
        completed: completed.to_string(),
    };
    test.app.apply_core_event(Event::PathCompleted { location, text: text.to_string(), completion });
}

#[test]
fn tab_sends_complete_path_with_the_current_text_and_panel_folder() {
    let (mut test, session) = with_remote();
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/x");

    press(&mut test, KeyCode::Tab);

    assert_eq!(
        complete_paths(test.sent()),
        vec![(Location::Session(session), VfsPath::new("/srv/data"), "/srv/data/x".to_string())]
    );
}

#[test]
fn a_second_tab_while_the_first_is_pending_sends_nothing() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Tab);
    test.sent();

    press(&mut test, KeyCode::Tab);

    assert!(test.sent().is_empty());
}

#[test]
fn a_reply_for_the_current_text_replaces_the_field_and_moves_the_cursor_to_the_end() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/su");
    press(&mut test, KeyCode::Tab);

    reply(&mut test, Location::Local, "/d/su", "/d/", &["sub"], "/d/sub/");

    let edit = test.app.path_edit.as_ref().unwrap();
    assert_eq!(edit.field.text, "/d/sub/");
    assert_eq!(edit.field.cursor, 7);
    assert!(edit.listing.is_none());
}

#[test]
fn a_stuck_reply_with_several_candidates_stores_them_for_display() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/s");
    press(&mut test, KeyCode::Tab);

    reply(&mut test, Location::Local, "/d/s", "/d/", &["sub", "subway"], "/d/sub");
    assert_eq!(field_text(&test).as_deref(), Some("/d/sub"));
    press(&mut test, KeyCode::Tab);
    reply(&mut test, Location::Local, "/d/sub", "/d/", &["sub", "subway"], "/d/sub");

    let listing = test.app.path_edit.as_ref().unwrap().listing.as_ref().unwrap();
    assert_eq!(listing.candidates, ["sub", "subway"]);
}

#[test]
fn a_reply_for_text_the_user_has_since_changed_is_ignored() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    type_text(&mut test, "/su");
    press(&mut test, KeyCode::Tab);
    type_text(&mut test, "b");

    reply(&mut test, Location::Local, "/d/su", "/d/", &["sub"], "/d/sub/");

    assert_eq!(field_text(&test).as_deref(), Some("/d/sub"));
    assert!(test.app.path_edit.as_ref().unwrap().listing.is_none());
}

#[test]
fn a_reply_after_the_edit_was_cancelled_is_ignored() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Tab);
    press(&mut test, KeyCode::Esc);

    reply(&mut test, Location::Local, "/d", "/", &["x"], "/x/");

    assert!(test.app.path_edit.is_none());
}

#[test]
fn typing_enter_or_esc_clears_the_candidates() {
    for dismissing in [KeyCode::Char('x'), KeyCode::Backspace] {
        let mut test = test_app(&VfsPath::new("/d"));
        test.app.apply_key(ctrl_apostrophe());
        press(&mut test, KeyCode::Tab);
        reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");
        press(&mut test, KeyCode::Tab);
        reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");
        assert!(test.app.path_edit.as_ref().unwrap().listing.is_some());

        press(&mut test, dismissing);

        assert!(test.app.path_edit.as_ref().unwrap().listing.is_none());
    }

    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Tab);
    reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");
    press(&mut test, KeyCode::Tab);
    reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");

    press(&mut test, KeyCode::Esc);
    assert!(test.app.path_edit.as_ref().unwrap().listing.is_none());
    press(&mut test, KeyCode::Esc);
    assert!(test.app.path_edit.is_none());
}

#[test]
fn enter_with_a_list_shown_still_submits_the_typed_text() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Tab);
    reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");
    press(&mut test, KeyCode::Tab);
    reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert_eq!(go_to_paths(test.sent()), vec![(Location::Local, VfsPath::new("/d"), "/d".to_string())]);
}

#[test]
fn pending_is_cleared_when_the_reply_arrives_even_when_it_is_stale() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Tab);
    type_text(&mut test, "z");
    test.sent();

    reply(&mut test, Location::Local, "/d", "/", &[], "/d");
    press(&mut test, KeyCode::Tab);

    assert_eq!(complete_paths(test.sent()).len(), 1);
}

#[test]
fn enter_clears_the_candidates_and_a_rejected_path_does_not_bring_them_back() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_key(ctrl_apostrophe());
    press(&mut test, KeyCode::Tab);
    reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");
    press(&mut test, KeyCode::Tab);
    reply(&mut test, Location::Local, "/d", "/", &["a", "b"], "/d");
    assert!(test.app.path_edit.as_ref().unwrap().listing.is_some());

    press(&mut test, KeyCode::Enter);
    assert!(test.app.path_edit.as_ref().unwrap().listing.is_none());

    test.app.apply_core_event(Event::PathRejected { location: Location::Local });
    assert!(test.app.path_edit.as_ref().unwrap().listing.is_none());
}
