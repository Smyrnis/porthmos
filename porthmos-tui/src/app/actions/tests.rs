use porthmos_core::VfsPath;

use super::*;
use crate::app::testing::{TestApp, entry, test_app};

fn app_in(dir: &str) -> TestApp {
    test_app(&VfsPath::new(dir))
}

fn connection(name: &str, source: ConnectionSource) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: format!("{name}.example.com"),
        port: 22,
        username: "user".to_string(),
        password: None,
        options: Default::default(),
        source,
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
    }
}

#[test]
fn quit_action_sets_should_quit() {
    let mut test = app_in("/d");
    test.app.apply_action(Action::Quit);
    assert!(test.app.should_quit);
}

#[test]
fn help_action_shows_the_overlay() {
    let mut test = app_in("/d");
    test.app.apply_action(Action::Help);
    assert!(test.app.help_visible);
}

#[test]
fn switch_panel_action_toggles_active_panel() {
    let mut test = app_in("/d");
    assert_eq!(test.app.active_panel, ActivePanel::Local);
    test.app.apply_action(Action::SwitchPanel);
    assert_eq!(test.app.active_panel, ActivePanel::Remote);
}

#[test]
fn noop_action_does_not_change_state() {
    let mut test = app_in("/d");
    test.app.apply_action(Action::Noop);
    assert!(!test.app.should_quit);
    assert_eq!(test.app.active_panel, ActivePanel::Local);
    assert!(test.sent().is_empty());
}

#[test]
fn panel_actions_are_ignored_when_remote_panel_has_no_listing_yet() {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), "child", true)]);
    test.app.apply_action(Action::SwitchPanel);

    let cursor_before = test.app.local.cursor;
    test.app.apply_action(Action::Down);

    assert_eq!(test.app.local.cursor, cursor_before);
    assert!(test.sent().is_empty());
}

#[test]
fn remote_panel_navigation_works_once_a_listing_exists() {
    let mut test = app_in("/d");
    let session = test.connect(1, "test");
    test.list_remote(session, "/home/user", vec![entry(&VfsPath::new("/home/user"), "child", true)]);
    test.app.active_panel = ActivePanel::Remote;

    test.app.apply_action(Action::Down);

    assert_eq!(test.app.sessions.active().unwrap().panel.cursor, 1);
}

#[test]
fn opening_a_local_folder_asks_for_its_listing() {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), "child", true)]);
    test.app.local.cursor = 1;

    test.app.apply_action(Action::Open);

    assert_eq!(test.sent(), vec![Command::List { location: Location::Local, path: Some(VfsPath::from("/d/child")) }]);
}

#[test]
fn opening_a_remote_folder_asks_that_session_for_its_listing() {
    let mut test = app_in("/d");
    let session = test.connect(4, "test");
    test.list_remote(session, "/home/user", vec![entry(&VfsPath::new("/home/user"), "child", true)]);
    test.app.active_panel = ActivePanel::Remote;
    test.app.sessions.active_mut().unwrap().panel.cursor = 1;

    test.app.apply_action(Action::Open);

    assert_eq!(
        test.sent(),
        vec![Command::List { location: Location::Session(session), path: Some(VfsPath::from("/home/user/child")) }]
    );
}

#[test]
fn refresh_relists_the_current_folder() {
    let mut test = app_in("/d");

    test.app.apply_action(Action::Refresh);

    assert_eq!(test.sent(), vec![Command::List { location: Location::Local, path: Some(VfsPath::from("/d")) }]);
}

#[test]
fn a_listing_of_a_new_local_folder_resets_the_cursor() {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), "a", false), entry(&VfsPath::new("/d"), "b", false)]);
    test.app.local.cursor = 2;

    test.app.apply_core_event(Event::Listed {
        location: Location::Local,
        path: VfsPath::from("/d/sub"),
        entries: vec![entry(&VfsPath::new("/d/sub"), "x", false), entry(&VfsPath::new("/d/sub"), "y", false)],
    });

    assert_eq!(test.app.local.cursor, 0);
}

#[test]
fn a_failed_listing_leaves_the_panel_where_it_was() {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), "child", true)]);
    test.app.local.cursor = 1;
    test.app.apply_action(Action::Open);

    test.app.apply_core_event(Event::Notice {
        severity: Severity::Error,
        message: "Permission denied (os error 13)".to_string(),
    });

    assert_eq!(test.app.local.path(), &VfsPath::new("/d"));
    assert_eq!(test.notification().as_deref(), Some("Permission denied (os error 13)"));
}

#[test]
fn back_action_returns_to_files_screen() {
    let mut test = app_in("/d");
    test.app.apply_action(Action::OpenConnections);
    test.app.apply_action(Action::Back);
    assert_eq!(test.app.screen, Screen::Files);
}

#[test]
fn connections_cursor_moves_within_bounds() {
    let mut test = app_in("/d");
    test.app.screen = Screen::Connections;
    test.app
        .connections
        .replace(vec![connection("a", ConnectionSource::Profile), connection("b", ConnectionSource::Profile)]);

    test.app.apply_action(Action::Up);
    assert_eq!(test.app.connections.cursor, 0);

    test.app.apply_action(Action::Down);
    assert_eq!(test.app.connections.cursor, 1);

    test.app.apply_action(Action::Down);
    assert_eq!(test.app.connections.cursor, 1);
}

#[test]
fn cycle_session_action_is_a_silent_no_op_with_no_sessions() {
    let mut test = app_in("/d");
    test.app.apply_action(Action::CycleSession);
    assert!(test.app.sessions.is_empty());
}

#[test]
fn cycle_session_action_advances_the_active_session() {
    let mut test = app_in("/d");
    let a = test.connect(1, "test");
    let b = test.connect(2, "other");
    assert_eq!(test.app.sessions.active().unwrap().id, b);

    test.app.apply_action(Action::CycleSession);

    assert_eq!(test.app.sessions.active().unwrap().id, a);
}

#[test]
fn disconnect_on_the_connections_screen_disconnects_the_selected_session() {
    let mut test = app_in("/d");
    let session = test.connect(5, "test");
    test.app.connections.replace(vec![connection("test", ConnectionSource::Profile)]);
    test.app.connections.cursor = 0;
    test.app.screen = Screen::Connections;

    test.app.apply_action(Action::Disconnect);
    test.app.apply_core_event(Event::Disconnected { session, name: "test".to_string() });

    assert!(test.sent().contains(&Command::Disconnect { session }));
    assert!(test.app.sessions.is_empty());
}

#[test]
fn delete_on_the_files_screen_still_opens_the_delete_dialog() {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), "doomed.txt", false)]);
    test.app.local.cursor = 1;

    test.app.apply_action(Action::Delete);

    assert!(test.app.dialog.is_some());
}

#[test]
fn toggle_hidden_action_reveals_dotfiles_in_the_local_panel() {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), ".secret", false)]);
    let before = test.app.local.rows().len();

    test.app.apply_action(Action::ToggleHidden);

    assert_eq!(test.app.local.rows().len(), before + 1);
}

#[test]
fn cycle_sort_action_changes_the_local_panel_sort_spec() {
    let mut test = app_in("/d");
    let before = test.app.local.sort_spec();

    test.app.apply_action(Action::CycleSort);

    assert_ne!(test.app.local.sort_spec(), before);
}

#[test]
fn back_action_dismisses_an_error_notification_before_changing_screens() {
    let mut test = app_in("/d");
    test.app.screen = Screen::Connections;
    test.app.notifications.push(Severity::Error, "oops");

    test.app.apply_action(Action::Back);

    assert!(test.app.notifications.current().is_none());
    assert_eq!(test.app.screen, Screen::Connections);
}

#[test]
fn back_action_returns_to_files_screen_when_there_is_no_error() {
    let mut test = app_in("/d");
    test.app.screen = Screen::Connections;

    test.app.apply_action(Action::Back);

    assert_eq!(test.app.screen, Screen::Files);
}

#[test]
fn rename_action_on_connections_screen_opens_the_edit_connection_form() {
    let mut test = app_in("/d");
    test.app.screen = Screen::Connections;
    let mut prod = connection("prod", ConnectionSource::Profile);
    prod.host = "server.example.com".to_string();
    prod.port = 2222;
    prod.username = "deploy".to_string();
    prod.password = Some("hunter2".to_string());
    test.app.connections.replace(vec![prod]);
    test.app.connections.cursor = 0;

    test.app.apply_action(Action::Rename);

    match test.app.dialog {
        Some(Dialog::Form(ref form)) => {
            assert_eq!(form.value("name").as_deref(), Some("prod"));
            assert_eq!(form.value("host").as_deref(), Some("server.example.com"));
            assert_eq!(form.value("port").as_deref(), Some("2222"));
            assert_eq!(form.value("username").as_deref(), Some("deploy"));
            assert_eq!(form.value("password").as_deref(), Some(""));
        }
        _ => panic!("expected the edit form to open"),
    }
}

#[test]
fn f4_without_a_session_warns() {
    let mut test = app_in("/d");

    test.app.apply_action(Action::OpenTerminal);

    assert_eq!(test.notification().as_deref(), Some("Connect to a server first"));
    assert!(test.sent().is_empty());
}

#[test]
fn f4_on_a_connection_without_a_shell_warns_and_sends_nothing() {
    let mut test = app_in("/d");
    test.app.apply_core_event(Event::Connected { session: 1, name: "bucket".to_string(), shell_available: false });

    test.app.apply_action(Action::OpenTerminal);

    assert_eq!(test.notification().as_deref(), Some("This connection doesn't support a terminal session"));
    assert!(test.sent().is_empty());
}

#[test]
fn f4_on_a_connection_with_a_shell_asks_the_core_for_it() {
    let mut test = app_in("/d");
    let session = test.connect(8, "srv");

    test.app.apply_action(Action::OpenTerminal);

    assert_eq!(test.sent(), vec![Command::PrepareShell { session, directory: Some(VfsPath::from("/")) }]);
}

#[test]
fn f4_opens_the_shell_in_the_folder_shown_on_the_remote_panel() {
    let mut test = app_in("/d");
    let session = test.connect(8, "srv");
    test.list_remote(session, "/home/tester/docs", Vec::new());

    test.app.apply_action(Action::OpenTerminal);

    assert_eq!(
        test.sent(),
        vec![Command::PrepareShell { session, directory: Some(VfsPath::from("/home/tester/docs")) }]
    );
}

#[test]
fn opening_the_help_starts_at_the_top() {
    let mut test = app_in("/d");
    test.app.help_scroll = 7;

    test.app.apply_action(Action::Help);

    assert_eq!(test.app.help_scroll, 0);
}

fn press_key(test: &mut TestApp, code: KeyCode) {
    test.app.apply_key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn the_arrow_page_home_and_end_keys_scroll_the_help_and_any_other_key_closes_it() {
    let mut test = app_in("/d");
    test.app.apply_action(Action::Help);
    crate::app::testing::render_app(&test.app, 80, 12);
    let max_scroll = test.app.help_max_scroll.get();
    assert!(max_scroll > 2);

    press_key(&mut test, KeyCode::Down);
    assert_eq!((test.app.help_scroll, test.app.help_visible), (1, true));
    press_key(&mut test, KeyCode::Up);
    press_key(&mut test, KeyCode::Up);
    assert_eq!(test.app.help_scroll, 0);
    press_key(&mut test, KeyCode::PageDown);
    assert!(test.app.help_scroll > 1);
    press_key(&mut test, KeyCode::End);
    assert_eq!(test.app.help_scroll, max_scroll);
    press_key(&mut test, KeyCode::Down);
    assert_eq!(test.app.help_scroll, max_scroll);
    press_key(&mut test, KeyCode::Home);
    assert_eq!(test.app.help_scroll, 0);

    press_key(&mut test, KeyCode::Char('x'));
    assert!(!test.app.help_visible);
}

#[test]
fn the_help_never_triggers_the_actions_it_lists() {
    let mut test = app_in("/d");
    test.app.apply_action(Action::Help);

    press_key(&mut test, KeyCode::F(10));

    assert!(!test.app.help_visible);
    assert!(!test.app.should_quit);
    assert_eq!(test.app.screen, Screen::Files);
}

#[test]
fn file_operation_keys_do_nothing_outside_the_files_screen() {
    let mut test = app_in("/d");

    test.app.screen = Screen::Connections;
    press_key(&mut test, KeyCode::Tab);
    assert_eq!(test.app.active_panel, ActivePanel::Local);
    press_key(&mut test, KeyCode::F(7));
    assert!(test.app.dialog.is_none());

    test.app.screen = Screen::Transfers;
    test.sent();
    press_key(&mut test, KeyCode::F(5));
    press_key(&mut test, KeyCode::F(3));
    assert!(test.sent().is_empty());
    assert!(test.app.dialog.is_none());
}

fn letter_key(test: &mut TestApp, character: char) {
    press_key(test, KeyCode::Char(character));
}

fn sessions_abc() -> TestApp {
    let mut test = app_in("/d");
    test.connect(1, "a");
    test.connect(2, "b");
    test.connect(3, "c");
    test.sent();
    test
}

#[test]
fn digits_jump_to_the_nth_session_on_the_files_screen() {
    let mut test = sessions_abc();

    letter_key(&mut test, '2');
    assert_eq!(test.app.sessions.active_id(), Some(2));
    assert_eq!(test.app.active_panel, ActivePanel::Remote);

    test.app.active_panel = ActivePanel::Local;
    letter_key(&mut test, '1');
    assert_eq!(test.app.sessions.active_id(), Some(1));
    assert_eq!(test.app.active_panel, ActivePanel::Remote);
    letter_key(&mut test, '3');
    assert_eq!(test.app.sessions.active_id(), Some(3));
}

#[test]
fn a_digit_without_that_session_does_nothing() {
    let mut none = app_in("/d");
    for digit in '1'..='9' {
        letter_key(&mut none, digit);
    }
    assert_eq!(none.app.active_panel, ActivePanel::Local);
    assert!(none.app.sessions.is_empty());

    let mut test = sessions_abc();
    letter_key(&mut test, '2');
    test.app.active_panel = ActivePanel::Local;
    for digit in '4'..='9' {
        letter_key(&mut test, digit);
    }
    assert_eq!(test.app.sessions.active_id(), Some(2));
    assert_eq!(test.app.active_panel, ActivePanel::Local);
}

#[test]
fn nine_sessions_can_all_be_reached() {
    let mut test = app_in("/d");
    for session in 1..=9u64 {
        test.connect(session, &format!("s{session}"));
    }

    for digit in 1..=9u64 {
        letter_key(&mut test, char::from(b'0' + digit as u8));
        assert_eq!(test.app.sessions.active_id(), Some(digit));
    }
}

#[test]
fn a_digit_after_a_disconnect_follows_the_tab_order() {
    let mut test = sessions_abc();
    test.app.apply_core_event(Event::Disconnected { session: 2, name: "b".to_string() });

    letter_key(&mut test, '2');

    assert_eq!(test.app.sessions.active_id(), Some(3));
    letter_key(&mut test, '3');
    assert_eq!(test.app.sessions.active_id(), Some(3));
}

#[test]
fn digits_do_nothing_on_other_screens() {
    let mut test = sessions_abc();
    letter_key(&mut test, '3');
    for screen in [Screen::Connections, Screen::Transfers, Screen::History, Screen::Sync] {
        test.app.screen = screen;
        letter_key(&mut test, '1');
        assert_eq!(test.app.sessions.active_id(), Some(3), "{screen:?}");
    }
}

fn outcome(test: &mut TestApp) -> (&'static str, Vec<Command>, Option<String>, Screen) {
    let dialog = match &test.app.dialog {
        None => "none",
        Some(Dialog::Confirm(_)) => "confirm",
        Some(Dialog::TextInput(_)) => "text input",
        Some(Dialog::List(_)) => "list",
        Some(Dialog::Form(_)) => "form",
        Some(Dialog::Conflict(_)) => "conflict",
        Some(Dialog::Message(_)) => "message",
    };
    (dialog, test.sent(), test.notification(), test.app.screen)
}

fn with_a_local_file() -> TestApp {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), "a.txt", false)]);
    test.app.local.cursor = 1;
    test.sent();
    test
}

#[test]
fn the_letters_do_what_the_function_keys_do_on_the_files_screen() {
    let pairs =
        [('c', KeyCode::F(5)), ('d', KeyCode::F(8)), ('n', KeyCode::F(7)), ('r', KeyCode::F(2)), ('e', KeyCode::F(3))];
    for (letter, function) in pairs {
        let mut by_letter = with_a_local_file();
        let mut by_function = with_a_local_file();
        letter_key(&mut by_letter, letter);
        press_key(&mut by_function, function);
        assert_eq!(outcome(&mut by_letter), outcome(&mut by_function), "{letter}");
    }

    let mut mkdir = with_a_local_file();
    letter_key(&mut mkdir, 'n');
    assert!(matches!(mkdir.app.dialog, Some(Dialog::TextInput(_))));
    let mut delete = with_a_local_file();
    letter_key(&mut delete, 'd');
    assert!(matches!(delete.app.dialog, Some(Dialog::Confirm(_))));
    let mut rename = with_a_local_file();
    letter_key(&mut rename, 'r');
    assert!(matches!(rename.app.dialog, Some(Dialog::TextInput(_))));
}

#[test]
fn t_opens_the_transfers_screen_like_ctrl_t() {
    let mut test = app_in("/d");

    letter_key(&mut test, 't');

    assert_eq!(test.app.screen, Screen::Transfers);
}

#[test]
fn q_quits_like_f10() {
    let mut test = app_in("/d");

    letter_key(&mut test, 'q');

    assert!(test.app.should_quit);
}

#[test]
fn the_question_mark_opens_the_help_with_and_without_shift() {
    let mut plain = app_in("/d");
    letter_key(&mut plain, '?');
    assert!(plain.app.help_visible);

    let mut shifted = app_in("/d");
    shifted.app.apply_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
    assert!(shifted.app.help_visible);
}

#[test]
fn j_and_k_move_the_cursor() {
    let mut test = app_in("/d");
    test.list_local(vec![entry(&VfsPath::new("/d"), "a", false), entry(&VfsPath::new("/d"), "b", false)]);
    test.app.local.cursor = 0;

    letter_key(&mut test, 'j');
    assert_eq!(test.app.local.cursor, 1);
    letter_key(&mut test, 'j');
    assert_eq!(test.app.local.cursor, 2);
    letter_key(&mut test, 'k');
    assert_eq!(test.app.local.cursor, 1);
}

#[test]
fn the_help_overlay_closes_on_any_other_key_without_acting() {
    for character in crate::app::testing::TYPED.chars() {
        let mut test = app_in("/d");
        test.app.apply_action(Action::Help);
        test.sent();

        letter_key(&mut test, character);

        assert!(!test.app.help_visible, "{character:?}");
        test.assert_inert(Screen::Files);
    }
}
