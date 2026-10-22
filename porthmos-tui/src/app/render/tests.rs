use std::path::Path;

use chrono::TimeZone;
use porthmos_core::{
    Entry,
    transfer::{
        JobStatus, TransferQueue,
        rows::{RowState, ScanInfo},
    },
};

use super::*;
use crate::{
    app::testing::{TestApp, cell_bg, cell_fg, find_text, render_app, test_app},
    theme::{ColorDepth, ThemeSettings},
    widgets::dialog::ConfirmDialog,
};

fn app_in_temp_dir() -> ((), App) {
    let TestApp { app, .. } = test_app(Path::new("/d"));
    ((), app)
}

fn scanning(app: &mut App, names: &[&str]) {
    let scans: Vec<ScanInfo> = names
        .iter()
        .enumerate()
        .map(|(index, name)| ScanInfo {
            batch_id: index as u64,
            label: name,
            direction: Direction::Upload,
            state: RowState::Scanning,
        })
        .collect();
    app.transfers = TransferSnapshot::of(&TransferQueue::new(), &scans);
}

fn hint_bar_text(app: &App) -> String {
    use ratatui::{Terminal, backend::TestBackend};

    let backend = TestBackend::new(60, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.render_hint_bar(frame, frame.area())).unwrap();
    terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn render_status_shows_scanning_while_planning() {
    let (_dir, mut app) = app_in_temp_dir();
    scanning(&mut app, &["myfolder"]);

    assert!(hint_bar_text(&app).contains("Scanning myfolder"));
}

#[test]
fn planning_status_text_is_none_without_a_scan() {
    let (_dir, app) = app_in_temp_dir();

    assert_eq!(app.planning_status_text(), None);
}

#[test]
fn planning_status_text_names_a_single_scan() {
    let (_dir, mut app) = app_in_temp_dir();
    scanning(&mut app, &["myfolder"]);

    assert_eq!(app.planning_status_text(), Some("Scanning myfolder\u{2026}".to_string()));
}

#[test]
fn planning_status_text_counts_several_scans() {
    let (_dir, mut app) = app_in_temp_dir();
    scanning(&mut app, &["one", "two"]);

    assert_eq!(app.planning_status_text(), Some("Scanning 2 copies\u{2026}".to_string()));
}

fn indexed_theme() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn themed_app() -> (App, Theme) {
    let TestApp { app, .. } = test_app(Path::new("/d"));
    let theme = indexed_theme();
    (app.with_theme(theme.clone()), theme)
}

#[test]
fn a_confirm_dialog_border_is_drawn_in_the_strong_line_colour() {
    let (mut app, theme) = themed_app();
    app.dialog = Some(Dialog::Confirm(ConfirmDialog::new("sure?")));

    let terminal = render_app(&app, 60, 12);

    let (text_x, row) = find_text(&terminal, "sure?").expect("message is drawn");
    let border_x = (0..text_x)
        .rev()
        .find(|x| terminal.backend().buffer()[(*x, row)].symbol() == "\u{2502}")
        .expect("left border is drawn");
    assert_eq!(cell_fg(&terminal, border_x, row), theme.line_strong);
}

fn row_symbols(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>, y: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_string()).collect()
}

#[test]
fn the_whole_screen_is_painted_with_the_theme_background() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 40, 12);

    for y in 0..12 {
        for x in 0..40 {
            let background = cell_bg(&terminal, x, y);
            let focus_row = background == theme.row_focus && (2..10).contains(&y);
            assert!(background == theme.bg || focus_row, "({x}, {y}): {background:?}");
        }
    }
}

#[test]
fn the_terminal_background_setting_leaves_cells_unpainted() {
    let TestApp { app, .. } = test_app(Path::new("/d"));
    let settings = ThemeSettings { terminal_background: true, ..ThemeSettings::default() };
    let app = app.with_theme(Theme::new(ColorDepth::Indexed, &settings));

    let terminal = render_app(&app, 40, 12);

    for y in 0..12 {
        for x in 0..40 {
            let background = cell_bg(&terminal, x, y);
            let focus_row = background != ratatui::style::Color::Reset && (2..10).contains(&y);
            assert!(background == ratatui::style::Color::Reset || focus_row, "({x}, {y}): {background:?}");
        }
    }
}

#[test]
fn rules_are_drawn_under_the_top_bar_and_above_the_hint_bar() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 40, 12);

    for y in [1, 10] {
        assert_eq!(row_symbols(&terminal, y), "\u{2500}".repeat(40));
        assert_eq!(cell_fg(&terminal, 5, y), theme.line);
    }
}

#[test]
fn a_terminal_shorter_than_twelve_rows_shows_the_too_small_message() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 40, 11);

    let (x, y) = find_text(&terminal, "terminal too small").expect("message is drawn");
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    assert!(find_text(&terminal, "local").is_none());
}

#[test]
fn rendering_never_panics_at_any_size() {
    let (mut app, _) = themed_app();
    app.dialog = Some(Dialog::Confirm(ConfirmDialog::new("sure?")));
    app.notifications.push(Severity::Error, "boom");

    for (width, height) in [(1, 1), (2, 2), (5, 12), (20, 12), (3, 50), (200, 3), (0, 0), (80, 11), (80, 12)] {
        let _ = render_app(&app, width, height);
    }
}

#[test]
fn a_dialog_is_drawn_on_the_themed_background() {
    let (mut app, theme) = themed_app();
    app.dialog = Some(Dialog::Confirm(ConfirmDialog::new("sure?")));

    let terminal = render_app(&app, 60, 12);

    let (x, y) = find_text(&terminal, "sure?").expect("message is drawn");
    assert_eq!(cell_bg(&terminal, x, y), theme.bg);
}

fn add_session(app: &mut App, id: u64, name: &str) {
    app.sessions.insert(id, name.to_string(), true, PanelView::from_listing(PathBuf::from("/"), Vec::new()));
}

fn saved_connection(name: &str, protocol: &str) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: protocol.to_string(),
        host: format!("{name}.example.com"),
        port: 22,
        username: "deploy".to_string(),
        password: None,
        options: Default::default(),
        source: ConnectionSource::Profile,
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
    }
}

#[test]
fn with_no_session_the_top_bar_shows_only_the_app_name() {
    let (app, _) = themed_app();

    let terminal = render_app(&app, 80, 12);

    assert_eq!(row_symbols(&terminal, 0).trim(), "porthmos");
}

#[test]
fn one_connected_session_shows_a_tab_and_no_old_title_text() {
    let (mut app, theme) = themed_app();
    add_session(&mut app, 1, "test");

    let terminal = render_app(&app, 80, 12);

    let row = row_symbols(&terminal, 0);
    assert!(row.contains("1 test \u{2022}"), "{row:?}");
    assert!(!row.contains("SSH: Connected"));
    assert!(!row.contains("Not connected"));
    let (x, y) = find_text(&terminal, "porthmos").expect("app name is drawn");
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_muted);
}

#[test]
fn the_active_tab_is_underlined_in_the_accent_on_the_top_rule() {
    let (mut app, theme) = themed_app();
    add_session(&mut app, 1, "alpha");
    add_session(&mut app, 2, "beta");

    let terminal = render_app(&app, 80, 12);

    let (x, _) = find_text(&terminal, "2 beta").expect("active tab is drawn");
    for column in x..x + 6 {
        assert_eq!(cell_fg(&terminal, column, 1), theme.accent, "column {column}");
    }
    assert_eq!(cell_fg(&terminal, x - 1, 1), theme.line);
    assert_eq!(cell_fg(&terminal, x + 6, 1), theme.line);
    assert_eq!(cell_fg(&terminal, 0, 1), theme.line);
}

#[test]
fn switching_the_active_session_moves_the_underline() {
    let (mut app, theme) = themed_app();
    add_session(&mut app, 1, "alpha");
    add_session(&mut app, 2, "beta");
    app.sessions.cycle();

    let terminal = render_app(&app, 80, 12);

    let (x, _) = find_text(&terminal, "1 alpha").expect("tab is drawn");
    assert_eq!(cell_fg(&terminal, x, 1), theme.accent);
    let (beta_x, _) = find_text(&terminal, "2 beta").expect("tab is drawn");
    assert_eq!(cell_fg(&terminal, beta_x, 1), theme.line);
}

#[test]
fn non_files_screens_show_a_breadcrumb_instead_of_tabs() {
    let (mut app, _) = themed_app();
    add_session(&mut app, 1, "test");

    for (screen, name) in [
        (Screen::Connections, "connections"),
        (Screen::Transfers, "transfers"),
        (Screen::History, "history"),
        (Screen::Sync, "sync"),
        (Screen::Search, "search"),
    ] {
        app.screen = screen;
        let terminal = render_app(&app, 80, 12);

        let row = row_symbols(&terminal, 0);
        assert!(row.contains(&format!("porthmos / {name}")), "{row:?}");
        assert!(!row.contains("1 test"), "{row:?}");
    }
}

#[test]
fn the_protocol_comes_from_the_saved_connection_with_the_same_name() {
    let (mut app, _) = themed_app();
    app.connections.replace(vec![saved_connection("prod", "sftp")]);
    add_session(&mut app, 1, "prod");

    let terminal = render_app(&app, 80, 12);

    assert!(row_symbols(&terminal, 0).contains("1 prod sftp \u{2022}"));
}

#[test]
fn connecting_status_is_shown_on_the_right_in_the_muted_colour() {
    let (mut app, theme) = themed_app();
    app.connection_status = ConnectionStatus::Connecting("nas".to_string());

    let terminal = render_app(&app, 80, 12);

    let (x, y) = find_text(&terminal, "Connecting to nas").expect("status is drawn");
    assert_eq!(y, 0);
    assert!(x > 40);
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_muted);
}

#[test]
fn failed_status_is_shown_only_without_a_session() {
    let (mut app, theme) = themed_app();
    app.connection_status = ConnectionStatus::Failed("boom".to_string());

    let without_session = render_app(&app, 80, 12);
    let (x, y) = find_text(&without_session, "Connection failed: boom").expect("status is drawn");
    assert_eq!(cell_fg(&without_session, x, y), theme.err);

    add_session(&mut app, 1, "test");
    let with_session = render_app(&app, 80, 12);
    assert!(find_text(&with_session, "Connection failed").is_none());
}

#[test]
fn the_remote_panel_is_not_split_for_a_tab_strip() {
    let (mut app, _) = themed_app();
    add_session(&mut app, 1, "alpha");
    let one = render_app(&app, 100, 14);
    add_session(&mut app, 2, "beta");
    let two = render_app(&app, 100, 14);

    assert!(find_text(&one, "alpha:/").is_some());
    assert_eq!(find_text(&one, "alpha:/").map(|(_, y)| y), find_text(&two, "beta:/").map(|(_, y)| y));
}

fn bottom_row(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
    let height = terminal.backend().buffer().area.height;
    row_symbols(terminal, height - 1)
}

#[test]
fn the_hint_bar_lists_the_actions_of_the_screen_with_their_keys() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 120, 12);

    assert!(bottom_row(&terminal).contains("c copy   n mkdir"), "{:?}", bottom_row(&terminal));
    let (x, y) = find_text(&terminal, "c copy").expect("hint is drawn");
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, x + 3, y), theme.fg_dim);
}

#[test]
fn a_transient_status_replaces_the_whole_hint_bar_in_the_muted_colour() {
    let (mut app, theme) = themed_app();
    scanning(&mut app, &["myfolder"]);

    let terminal = render_app(&app, 80, 12);

    let row = bottom_row(&terminal);
    assert!(row.contains("Scanning myfolder"), "{row:?}");
    assert!(!row.contains("? help"));
    let (x, y) = find_text(&terminal, "Scanning myfolder").expect("status is drawn");
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_muted);
}

#[test]
fn a_narrow_terminal_keeps_the_help_pair() {
    let (app, _) = themed_app();

    for width in [60, 40, 30, 20, 12] {
        let terminal = render_app(&app, width, 12);
        assert!(bottom_row(&terminal).contains("? help"), "width {width}: {:?}", bottom_row(&terminal));
    }
}

#[test]
fn the_hint_bar_and_the_help_overlay_agree_on_a_remapped_help_key() {
    let (mut app, _) = themed_app();
    let overrides = std::collections::HashMap::from([("help".to_string(), "ctrl+g".to_string())]);
    app.key_bindings = input::KeyBindings::from_overrides(&overrides).0;

    let hints = render_app(&app, 100, 24);
    assert!(bottom_row(&hints).contains("Ctrl+G help"), "{:?}", bottom_row(&hints));

    app.help_visible = true;
    let overlay = render_app(&app, 100, 60);
    let (_, y) = find_text(&overlay, "Ctrl+G").expect("overlay lists the remapped key");
    assert!(y < 59);
    assert!(find_text(&overlay, &format!("Ctrl+G{}help", " ".repeat(6))).is_some());
}

#[test]
fn non_files_screens_show_back_at_the_right_edge() {
    let (mut app, _) = themed_app();
    app.screen = Screen::Connections;

    let terminal = render_app(&app, 100, 12);

    assert!(bottom_row(&terminal).trim_end().ends_with("Esc back"), "{:?}", bottom_row(&terminal));
}

fn dot_of(
    terminal: &ratatui::Terminal<ratatui::backend::TestBackend>, message: &str,
) -> (String, ratatui::style::Color) {
    let (x, y) = find_text(terminal, message).expect("message is drawn");
    (terminal.backend().buffer()[(x - 2, y)].symbol().to_string(), cell_fg(terminal, x - 2, y))
}

#[test]
fn an_error_toast_is_drawn_with_the_err_dot() {
    let (mut app, theme) = themed_app();
    app.notifications.push(Severity::Error, "boom");

    let terminal = render_app(&app, 80, 12);

    assert_eq!(dot_of(&terminal, "boom"), ("\u{2022}".to_string(), theme.err));
}

#[test]
fn a_warning_toast_has_a_bright_dot_not_yellow() {
    let (mut app, theme) = themed_app();
    app.notifications.push(Severity::Warning, "careful");

    let terminal = render_app(&app, 80, 12);

    let (symbol, colour) = dot_of(&terminal, "careful");
    assert_eq!(symbol, "\u{2022}");
    assert_eq!(colour, theme.fg_bright);
    assert_ne!(colour, ratatui::style::Color::Yellow);
}

#[test]
fn an_info_toast_has_a_muted_dot() {
    let (mut app, theme) = themed_app();
    app.notifications.push(Severity::Info, "fyi");

    let terminal = render_app(&app, 80, 12);

    assert_eq!(dot_of(&terminal, "fyi"), ("\u{2022}".to_string(), theme.fg_muted));
}

#[test]
fn a_notification_no_longer_replaces_the_hint_bar() {
    let (mut app, _) = themed_app();
    app.notifications.push(Severity::Error, "boom");

    let terminal = render_app(&app, 100, 12);

    assert!(bottom_row(&terminal).contains("? help"), "{:?}", bottom_row(&terminal));
    assert!(!bottom_row(&terminal).contains("boom"));
}

#[test]
fn only_the_first_queued_notification_is_shown_and_the_next_follows_after_dismissal() {
    let (mut app, _) = themed_app();
    app.notifications.push(Severity::Error, "first problem");
    app.notifications.push(Severity::Error, "second problem");

    let before = render_app(&app, 80, 12);
    app.notifications.dismiss_current();
    let after = render_app(&app, 80, 12);

    assert!(find_text(&before, "first problem").is_some());
    assert!(find_text(&before, "second problem").is_none());
    assert!(find_text(&after, "first problem").is_none());
    assert!(find_text(&after, "second problem").is_some());
}

#[test]
fn esc_dismisses_an_error_toast() {
    let (mut app, _) = themed_app();
    app.notifications.push(Severity::Error, "boom");

    app.apply_action(Action::Back);

    assert!(find_text(&render_app(&app, 80, 12), "boom").is_none());
}

#[test]
fn an_overlay_is_drawn_above_a_toast_so_an_error_never_blocks_a_dialog() {
    let (mut app, _) = themed_app();
    app.help_visible = true;
    app.notifications.push(Severity::Error, "problem ".repeat(12));

    let terminal = render_app(&app, 60, 20);

    assert!(find_text(&terminal, &format!("q / F10{}quit", " ".repeat(5))).is_some());
}

#[test]
fn a_toast_stays_visible_when_a_dialog_does_not_cover_it() {
    let (mut app, _) = themed_app();
    app.dialog = Some(Dialog::Confirm(ConfirmDialog::new("sure?")));
    app.notifications.push(Severity::Error, "late error");

    let terminal = render_app(&app, 100, 24);

    assert!(find_text(&terminal, "sure?").is_some());
    assert!(find_text(&terminal, "late error").is_some());
}

#[test]
fn a_long_connecting_status_is_truncated_so_the_active_tab_keeps_its_name() {
    let (mut app, _) = themed_app();
    add_session(&mut app, 1, "alpha");
    add_session(&mut app, 2, "beta");
    app.connection_status = ConnectionStatus::Connecting("n".repeat(80));

    let terminal = render_app(&app, 80, 12);

    let row = row_symbols(&terminal, 0);
    assert!(row.contains("2 beta"), "{row:?}");
    assert!(row.contains("Connecting to"), "{row:?}");
    assert!(row.contains('\u{2026}'), "{row:?}");
}

#[test]
fn a_long_failure_without_a_session_is_truncated_instead_of_dropped() {
    let (mut app, _) = themed_app();
    app.connection_status = ConnectionStatus::Failed(format!("handshake failed: {}", "x".repeat(200)));

    let terminal = render_app(&app, 80, 12);

    let row = row_symbols(&terminal, 0);
    assert!(row.contains("Connection failed: handshake failed"), "{row:?}");
    assert!(row.trim_end().ends_with('\u{2026}'), "{row:?}");
}

#[test]
fn the_underline_follows_a_wide_character_tab_while_a_status_is_shown() {
    let (mut app, theme) = themed_app();
    add_session(&mut app, 1, "\u{4e00}\u{4e8c}\u{4e09}");
    add_session(&mut app, 2, "beta");
    app.connection_status = ConnectionStatus::Connecting("nas".to_string());

    let terminal = render_app(&app, 100, 12);

    let (x, _) = find_text(&terminal, "2 beta").expect("active tab is drawn");
    for column in x..x + 6 {
        assert_eq!(cell_fg(&terminal, column, 1), theme.accent, "column {column}");
    }
    assert_eq!(cell_fg(&terminal, x - 1, 1), theme.line);
}

#[test]
fn rendering_with_sessions_and_a_status_never_panics_at_any_size() {
    let (mut app, _) = themed_app();
    add_session(&mut app, 1, "alpha");
    add_session(&mut app, 2, "\u{4e00}\u{4e8c}\u{4e09}\u{56db}");
    app.connection_status = ConnectionStatus::Connecting("\u{4e00}".repeat(40));

    for width in 0..=40 {
        let _ = render_app(&app, width, 12);
    }
}

#[test]
fn at_eighty_columns_the_files_hint_bar_still_shows_how_to_connect_and_quit() {
    let (app, _) = themed_app();

    let terminal = render_app(&app, 80, 12);

    let row = bottom_row(&terminal);
    assert!(row.contains("F9 connections"), "{row:?}");
    assert!(row.contains("q quit"), "{row:?}");
    assert!(row.contains("? help"), "{row:?}");
}

#[test]
fn a_multi_line_message_is_wrapped_inside_the_toast() {
    let (mut app, _) = themed_app();
    app.notifications.push(Severity::Error, "first line\nsecond line");

    let terminal = render_app(&app, 80, 12);

    let (_, first) = find_text(&terminal, "first line").expect("first line");
    let (_, second) = find_text(&terminal, "second line").expect("second line");
    assert_eq!(second, first + 1);
}

#[test]
fn with_clock_replaces_the_time_source() {
    use chrono::{Local, TimeZone};

    let moment = Local.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();
    let (app, _) = themed_app();

    let app = app.with_clock(crate::clock::Clock::fixed(moment));

    assert_eq!(app.clock.now(), moment);
}

fn listed_app(entries: Vec<Entry>) -> (App, Theme) {
    let mut test = test_app(Path::new("/d"));
    test.list_local(entries);
    let theme = indexed_theme();
    let moment = chrono::Local.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();
    (test.app.with_theme(theme.clone()).with_clock(crate::clock::Clock::fixed(moment)), theme)
}

fn deploy_script() -> Entry {
    Entry {
        name: "deploy.sh".to_string(),
        path: PathBuf::from("/d/deploy.sh"),
        is_dir: false,
        size: 2457,
        permissions: Some(0o100755),
        modified: Some(chrono::Local.with_ymd_and_hms(2026, 10, 20, 14, 2, 0).unwrap().timestamp() as u64),
    }
}

#[test]
fn the_files_screen_shows_the_local_header_rule_column_header_and_footer() {
    let (app, theme) = listed_app(vec![deploy_script()]);

    let terminal = render_app(&app, 120, 24);

    assert!(row_symbols(&terminal, 2).contains("local  /d"), "{:?}", row_symbols(&terminal, 2));
    assert_eq!(cell_fg(&terminal, 10, 3), theme.accent);
    assert!(row_symbols(&terminal, 4).contains("name"));
    assert!(row_symbols(&terminal, 19).contains("1 item"));
    assert!(find_text(&terminal, "not connected").is_some());
}

#[test]
fn the_divider_is_drawn_between_the_panels_in_the_line_colour() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 120, 24);

    for y in 2..20 {
        assert_eq!(terminal.backend().buffer()[(60, y)].symbol(), "\u{2502}", "row {y}");
        assert_eq!(cell_fg(&terminal, 60, y), theme.line);
    }
}

#[test]
fn the_info_line_describes_the_focused_entry_of_the_active_panel() {
    let (mut app, _) = listed_app(vec![deploy_script()]);
    app.local.cursor = app.local.rows().len() - 1;

    let terminal = render_app(&app, 120, 24);

    assert!(
        row_symbols(&terminal, 21).contains("deploy.sh   2.4 KB   -rwxr-xr-x   today 14:02"),
        "{:?}",
        row_symbols(&terminal, 21)
    );
}

#[test]
fn the_info_line_is_empty_on_the_parent_row() {
    let (app, _) = listed_app(vec![deploy_script()]);

    let terminal = render_app(&app, 120, 24);

    assert_eq!(row_symbols(&terminal, 21).trim(), "");
}

#[test]
fn a_rule_separates_the_panels_from_the_info_line() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 120, 24);

    assert_eq!(terminal.backend().buffer()[(10, 20)].symbol(), "\u{2500}");
    assert_eq!(cell_fg(&terminal, 10, 20), theme.line);
}

#[test]
fn switching_the_active_panel_moves_the_accent_rule() {
    let (mut app, theme) = themed_app();
    add_session(&mut app, 1, "prod");

    let local_active = render_app(&app, 120, 24);
    app.active_panel = ActivePanel::Remote;
    let remote_active = render_app(&app, 120, 24);

    assert_eq!((cell_fg(&local_active, 10, 3), cell_fg(&local_active, 80, 3)), (theme.accent, theme.line));
    assert_eq!((cell_fg(&remote_active, 10, 3), cell_fg(&remote_active, 80, 3)), (theme.line, theme.accent));
}

#[test]
fn a_connected_session_shows_the_saved_connections_protocol_and_user_at_host() {
    let (mut app, _) = themed_app();
    app.connections.replace(vec![saved_connection("prod", "sftp")]);
    add_session(&mut app, 1, "prod");

    let terminal = render_app(&app, 120, 24);

    assert!(row_symbols(&terminal, 2).contains("sftp  deploy@prod.example.com:/"), "{:?}", row_symbols(&terminal, 2));
}

#[test]
fn a_session_without_a_saved_connection_shows_its_name_and_path() {
    let (mut app, _) = themed_app();
    add_session(&mut app, 1, "prod");

    let terminal = render_app(&app, 120, 24);

    assert!(row_symbols(&terminal, 2).contains("remote  prod:/"), "{:?}", row_symbols(&terminal, 2));
}

#[test]
fn the_unconnected_remote_panel_says_not_connected_and_names_the_connect_key() {
    let (mut app, _) = themed_app();

    let default_keys = render_app(&app, 120, 24);
    let overrides = std::collections::HashMap::from([("open_connections".to_string(), "ctrl+g".to_string())]);
    app.key_bindings = input::KeyBindings::from_overrides(&overrides).0;
    let remapped = render_app(&app, 120, 24);

    assert!(find_text(&default_keys, "press F9 to connect").is_some());
    assert!(find_text(&remapped, "press Ctrl+G to connect").is_some());
}

#[test]
fn connecting_shows_the_target_name_in_the_remote_panel() {
    let (mut app, _) = themed_app();
    app.connection_status = ConnectionStatus::Connecting("nas".to_string());

    let terminal = render_app(&app, 120, 24);

    assert!(find_text(&terminal, "connecting to nas\u{2026}").is_some());
}

#[test]
fn the_home_directory_is_shown_as_a_tilde() {
    let home = Path::new("/home/me");

    assert_eq!(abbreviate_home(Path::new("/home/me/projects"), Some(home)), "~/projects");
    assert_eq!(abbreviate_home(Path::new("/home/me"), Some(home)), "~");
    assert_eq!(abbreviate_home(Path::new("/home/mex"), Some(home)), "/home/mex");
    assert_eq!(abbreviate_home(Path::new("/etc"), Some(home)), "/etc");
    assert_eq!(abbreviate_home(Path::new("/home/me/x"), None), "/home/me/x");
    assert_eq!(abbreviate_home(Path::new("/etc"), Some(Path::new("/"))), "/etc");
    assert_eq!(abbreviate_home(Path::new("/"), Some(Path::new("/"))), "/");
    assert_eq!(abbreviate_home(Path::new("/etc"), Some(Path::new(""))), "/etc");
}

#[test]
fn the_files_screen_never_panics_at_any_size() {
    let (mut app, _) = listed_app(vec![deploy_script()]);
    add_session(&mut app, 1, "prod");
    app.connections.replace(vec![saved_connection("prod", "sftp")]);

    for width in 0..=130 {
        for height in 0..=14 {
            let _ = render_app(&app, width, height);
        }
    }
}

fn connected_app() -> (App, Theme) {
    let (mut app, theme) = listed_app(vec![deploy_script()]);
    app.connections.replace(vec![saved_connection("prod", "sftp")]);
    add_session(&mut app, 1, "prod");
    (app, theme)
}

#[test]
fn at_one_hundred_columns_both_panels_are_drawn() {
    let (app, _) = listed_app(vec![deploy_script()]);

    let terminal = render_app(&app, 100, 24);

    let header = row_symbols(&terminal, 2);
    assert!(header.contains("local  /d") && header.contains("remote"), "{header:?}");
    assert!(find_text(&terminal, "not connected").is_some());
}

#[test]
fn at_ninety_nine_columns_only_the_active_panel_is_drawn_with_a_switch_hint() {
    let (app, _) = listed_app(vec![deploy_script()]);

    let terminal = render_app(&app, 99, 24);

    let header = row_symbols(&terminal, 2);
    assert!(header.contains("local \u{21c4} remote  /d"), "{header:?}");
    assert!(find_text(&terminal, "not connected").is_none());
    assert!(find_text(&terminal, "deploy.sh").is_some());
    assert_eq!(terminal.backend().buffer()[(49, 5)].symbol(), " ");
}

#[test]
fn switching_panels_in_single_mode_shows_the_other_panel() {
    let (mut app, _) = listed_app(vec![deploy_script()]);

    app.apply_action(Action::SwitchPanel);
    let terminal = render_app(&app, 80, 24);

    assert!(row_symbols(&terminal, 2).contains("remote \u{21c4} local"), "{:?}", row_symbols(&terminal, 2));
    assert!(find_text(&terminal, "not connected").is_some());
    assert!(find_text(&terminal, "deploy.sh").is_none());
}

#[test]
fn the_switch_hint_names_the_connected_protocol() {
    let (mut app, _) = connected_app();

    let local_active = render_app(&app, 80, 24);
    app.active_panel = ActivePanel::Remote;
    let remote_active = render_app(&app, 80, 24);

    assert!(row_symbols(&local_active, 2).contains("local \u{21c4} sftp  /d"));
    assert!(row_symbols(&remote_active, 2).contains("sftp \u{21c4} local  deploy@prod.example.com:/"));
}

#[test]
fn the_modified_column_disappears_below_sixty_columns() {
    let (app, _) = listed_app(vec![deploy_script()]);

    let at_sixty = render_app(&app, 60, 24);
    let at_fifty_nine = render_app(&app, 59, 24);

    assert!(row_symbols(&at_sixty, 4).contains("modified"));
    assert!(!row_symbols(&at_fifty_nine, 4).contains("modified"));
    assert!(find_text(&at_fifty_nine, "deploy.sh").is_some());
}

#[test]
fn a_twenty_column_terminal_still_draws_rows() {
    let (app, _) = listed_app(vec![deploy_script()]);

    let terminal = render_app(&app, 20, 24);

    assert!(find_text(&terminal, "deploy.sh").is_some());
}

#[test]
fn the_info_line_stays_visible_in_every_mode() {
    for width in [120, 100, 99, 60, 59, 20] {
        let (mut app, _) = listed_app(vec![deploy_script()]);
        app.local.cursor = app.local.rows().len() - 1;

        let terminal = render_app(&app, width, 24);

        assert!(row_symbols(&terminal, 21).contains("deploy.sh"), "width {width}: {:?}", row_symbols(&terminal, 21));
    }
}

#[test]
fn a_height_of_eleven_rows_shows_the_too_small_message_in_single_mode_too() {
    let (app, _) = listed_app(vec![deploy_script()]);

    let terminal = render_app(&app, 80, 11);

    assert!(find_text(&terminal, "terminal too small").is_some());
}

fn running_snapshot(session: u64, label: &str) -> porthmos_core::transfer::TransferSnapshot {
    let mut queue = TransferQueue::new();
    let id = queue.enqueue(
        session,
        Direction::Upload,
        PathBuf::from(format!("/local/{label}")),
        format!("/remote/{label}"),
        label.to_string(),
        100,
        None,
    );
    let mut job = queue.get_mut(id).unwrap();
    job.status = JobStatus::InProgress;
    job.transferred_bytes = 36;
    drop(job);
    porthmos_core::transfer::TransferSnapshot::of(&queue, &[])
}

#[test]
fn the_transfer_line_appears_under_the_info_line_while_a_row_runs() {
    let (mut app, _) = listed_app(vec![deploy_script()]);
    app.session_names.insert(1, "prod-web".to_string());
    app.apply_core_event(Event::TransfersChanged(running_snapshot(1, "dist")));

    let terminal = render_app(&app, 120, 24);

    let line = row_symbols(&terminal, 21);
    assert!(line.contains("\u{2191} dist  \u{2192} prod-web"), "{line:?}");
    assert!(line.contains("36%"), "{line:?}");
}

#[test]
fn the_panels_grow_back_when_nothing_runs() {
    let (mut app, _) = listed_app(vec![deploy_script()]);
    let with_line = {
        app.apply_core_event(Event::TransfersChanged(running_snapshot(1, "dist")));
        render_app(&app, 120, 24)
    };
    app.apply_core_event(Event::TransfersChanged(Default::default()));
    let without_line = render_app(&app, 120, 24);

    assert!(row_symbols(&with_line, 19).contains("\u{2500}") && row_symbols(&with_line, 18).contains("item"));
    assert!(row_symbols(&without_line, 20).contains("\u{2500}") && row_symbols(&without_line, 19).contains("item"));
    assert_eq!(row_symbols(&without_line, 21).trim(), "");
}

#[test]
fn progress_no_longer_replaces_the_hint_bar() {
    let (mut app, _) = themed_app();
    app.apply_core_event(Event::TransfersChanged(running_snapshot(1, "dist")));

    let terminal = render_app(&app, 120, 24);

    assert!(bottom_row(&terminal).contains("? help"), "{:?}", bottom_row(&terminal));
}

#[test]
fn the_line_uses_the_session_name_from_the_session_names_map() {
    let (mut app, _) = themed_app();
    app.session_names.insert(1, "nas".to_string());
    app.apply_core_event(Event::TransfersChanged(running_snapshot(1, "dist")));

    let terminal = render_app(&app, 120, 24);

    assert!(find_text(&terminal, "\u{2192} nas").is_some());
}

#[test]
fn the_files_screen_never_panics_at_any_size_with_a_running_transfer() {
    let (mut app, _) = connected_app();
    app.apply_core_event(Event::TransfersChanged(running_snapshot(1, "dist")));

    for width in 0..=130 {
        for height in 0..=14 {
            let _ = render_app(&app, width, height);
        }
    }
}

fn transfer_rows_snapshot() -> porthmos_core::transfer::TransferSnapshot {
    let mut queue = TransferQueue::new();
    for (name, status) in [
        ("running.bin", JobStatus::InProgress),
        ("waiting.bin", JobStatus::Queued),
        ("broken.log", JobStatus::Failed("permission denied".to_string())),
    ] {
        let id = queue.enqueue(
            1,
            Direction::Download,
            PathBuf::from(format!("/local/{name}")),
            format!("/remote/{name}"),
            name.to_string(),
            100,
            None,
        );
        let mut job = queue.get_mut(id).unwrap();
        job.attempts = 3;
        job.status = status;
    }
    porthmos_core::transfer::TransferSnapshot::of(&queue, &[])
}

fn transfers_app() -> App {
    let (mut app, _) = themed_app();
    app.session_names.insert(1, "prod".to_string());
    app.apply_core_event(Event::TransfersChanged(transfer_rows_snapshot()));
    app.screen = Screen::Transfers;
    app
}

#[test]
fn the_transfers_screen_shows_the_summary_in_the_top_bar() {
    let app = transfers_app();

    let terminal = render_app(&app, 120, 24);

    let top = row_symbols(&terminal, 0);
    assert!(top.contains("porthmos / transfers"), "{top:?}");
    assert!(top.trim_end().ends_with("1 active \u{b7} 1 queued \u{b7} 1 failed"), "{top:?}");
}

#[test]
fn the_transfers_screen_shows_route_speed_and_eta_for_a_running_row() {
    let mut app = transfers_app();
    let base = std::time::Instant::now();
    let rows = |done: u64| {
        let mut snapshot = transfer_rows_snapshot();
        snapshot.rows[0].bytes_done = done;
        snapshot.rows
    };
    app.rates.observe(base, &rows(0));
    app.rates.observe(base + std::time::Duration::from_secs(2), &rows(50));
    let mut snapshot = transfer_rows_snapshot();
    snapshot.rows[0].bytes_done = 50;
    app.transfers = snapshot;

    let terminal = render_app(&app, 120, 24);

    assert!(find_text(&terminal, "prod \u{2192} local").is_some());
    assert!(find_text(&terminal, " B/s").is_some());
    assert!(find_text(&terminal, "0:0").is_some());
}

#[test]
fn a_failed_row_shows_its_detail_area_with_the_recorded_time() {
    let mut app = transfers_app();
    app.transfers_cursor = 2;

    let terminal = render_app(&app, 120, 24);

    let detail = (0..24).map(|y| row_symbols(&terminal, y)).collect::<Vec<_>>().join("\n");
    assert!(detail.contains("broken.log  failed after 3 attempts"), "{detail}");
    assert!(detail.contains("check the file's permissions"), "{detail}");
}

#[test]
fn the_failed_time_comes_from_the_injected_clock() {
    let mut app = transfers_app()
        .with_clock(crate::clock::Clock::fixed(chrono::Local.with_ymd_and_hms(2026, 10, 21, 14, 7, 0).unwrap()));
    app.failed_at.clear();
    app.apply_core_event(Event::TransfersChanged(transfer_rows_snapshot()));
    app.transfers_cursor = 2;

    let terminal = render_app(&app, 120, 24);

    assert!(find_text(&terminal, "attempts \u{b7} 14:07").is_some());
}

#[test]
fn the_hint_bar_lists_retry_cancel_and_clear_done_on_the_transfers_screen() {
    let app = transfers_app();

    let terminal = render_app(&app, 120, 24);

    assert!(bottom_row(&terminal).contains("r retry   x cancel   c clear done"), "{:?}", bottom_row(&terminal));
}

#[test]
fn the_transfers_screen_never_panics_at_any_size() {
    let app = transfers_app();

    for width in 0..=130 {
        for height in 0..=14 {
            let _ = render_app(&app, width, height);
        }
    }
}

fn connections_app() -> (App, Theme) {
    let (mut app, theme) = themed_app();
    app.connections.replace(vec![saved_connection("prod", "sftp"), saved_connection("backup", "sftp")]);
    app.screen = Screen::Connections;
    (app, theme)
}

#[test]
fn the_connections_screen_shows_the_filter_line_and_a_row_at_120x24() {
    let (app, _) = connections_app();

    let terminal = render_app(&app, 120, 24);

    assert!(row_symbols(&terminal, 2).contains("/ filter"), "{:?}", row_symbols(&terminal, 2));
    assert!(row_symbols(&terminal, 4).contains("prod"), "{:?}", row_symbols(&terminal, 4));
    assert!(row_symbols(&terminal, 5).contains("deploy@backup.example.com"), "{:?}", row_symbols(&terminal, 5));
}

fn noon() -> chrono::DateTime<chrono::Local> {
    chrono::Local.with_ymd_and_hms(2026, 10, 22, 12, 0, 0).unwrap()
}

fn connections_app_at_noon() -> App {
    let (app, _) = connections_app();
    app.with_clock(crate::clock::Clock::fixed(noon()))
}

fn used_three_hours_ago(name: &str) -> Event {
    let at = noon().with_timezone(&chrono::Utc) - chrono::Duration::hours(3);
    Event::LastUsed(std::collections::BTreeMap::from([(name.to_string(), at)]))
}

#[test]
fn a_last_used_event_replaces_the_remembered_times() {
    let mut app = connections_app_at_noon();
    app.apply_core_event(used_three_hours_ago("prod"));

    app.apply_core_event(used_three_hours_ago("backup"));

    assert_eq!(app.last_used.keys().collect::<Vec<_>>(), ["backup"]);
}

#[test]
fn the_connections_list_shows_how_long_ago_each_connection_was_used() {
    let mut app = connections_app_at_noon();
    app.apply_core_event(used_three_hours_ago("prod"));

    let terminal = render_app(&app, 120, 24);

    let screen: Vec<String> = (0..24).map(|y| row_symbols(&terminal, y)).collect();
    let prod = screen.iter().find(|line| line.contains("deploy@prod")).expect("the prod row");
    let backup = screen.iter().find(|line| line.contains("deploy@backup")).expect("the backup row");
    assert!(prod.contains("3 h ago"), "{prod:?}");
    assert!(backup.contains("never"), "{backup:?}");
}

#[test]
fn the_details_pane_shows_the_last_used_time_of_the_focused_connection() {
    let mut app = connections_app_at_noon();
    app.apply_core_event(used_three_hours_ago("prod"));
    app.connections.cursor = 0;

    let terminal = render_app(&app, 120, 24);

    let screen = (0..24).map(|y| row_symbols(&terminal, y)).collect::<Vec<_>>().join("\n");
    assert!(screen.contains("last used") && screen.contains("\u{b7} 3 h ago"), "{screen}");
}

#[test]
fn a_connect_failure_is_remembered_for_the_list_and_cleared_by_a_later_success() {
    let (mut app, theme) = connections_app();

    app.apply_core_event(Event::ConnectFailed {
        name: "prod".to_string(),
        message: "Authentication failed for prod".to_string(),
    });

    assert_eq!(app.last_failure.get("prod").map(String::as_str), Some("Authentication failed for prod"));
    let failed = render_app(&app, 120, 24);
    let line = row_symbols(&failed, 4);
    let x = line
        .chars()
        .take_while(|_| true)
        .collect::<String>()
        .find("Authentication")
        .map(|byte| line[..byte].chars().count())
        .expect("message on the row");
    assert_eq!(cell_fg(&failed, x as u16, 4), theme.err);

    app.apply_core_event(Event::Connected { session: 1, name: "prod".to_string(), shell_available: true });

    assert!(app.last_failure.is_empty());
}

#[test]
fn a_failure_of_one_connection_does_not_touch_the_others() {
    let (mut app, _) = connections_app();
    app.apply_core_event(Event::ConnectFailed { name: "prod".to_string(), message: "boom".to_string() });

    app.apply_core_event(Event::Connected { session: 1, name: "backup".to_string(), shell_available: true });

    assert!(app.last_failure.contains_key("prod"));
}

#[test]
fn the_top_bar_on_the_connections_screen_counts_connected_and_errors() {
    let (mut app, _) = connections_app();
    add_session(&mut app, 1, "prod");
    app.last_failure.insert("backup".to_string(), "boom".to_string());

    let terminal = render_app(&app, 120, 24);

    let top = row_symbols(&terminal, 0);
    assert!(top.contains("porthmos / connections"), "{top:?}");
    assert!(top.trim_end().ends_with("1 connected \u{b7} 1 error"), "{top:?}");
}

#[test]
fn zero_parts_are_left_out_of_the_connections_summary() {
    let (mut app, _) = connections_app();
    assert!(!row_symbols(&render_app(&app, 120, 24), 0).trim_end().ends_with("error"));

    app.last_failure.insert("backup".to_string(), "boom".to_string());
    app.last_failure.insert("prod".to_string(), "boom".to_string());

    assert!(row_symbols(&render_app(&app, 120, 24), 0).trim_end().ends_with("2 errors"));
}

#[test]
fn the_connections_screen_never_panics_at_any_size() {
    let (mut app, _) = connections_app();
    add_session(&mut app, 1, "prod");
    app.last_failure.insert("backup".to_string(), "boom\nline two".to_string());

    for width in 0..=130 {
        for height in 0..=14 {
            let _ = render_app(&app, width, height);
        }
    }
}

fn plain_key(character: char) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(crossterm::event::KeyCode::Char(character), crossterm::event::KeyModifiers::NONE)
}

fn right_part(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>, y: u16, from: usize) -> String {
    row_symbols(terminal, y).chars().skip(from).collect()
}

#[test]
fn at_120_columns_the_details_follow_the_cursor() {
    let (mut app, _) = connections_app();

    let first = render_app(&app, 120, 24);
    app.connections.cursor = 1;
    let second = render_app(&app, 120, 24);

    assert!(right_part(&first, 2, 76).starts_with("prod sftp"), "{:?}", right_part(&first, 2, 76));
    assert!(right_part(&second, 2, 76).starts_with("backup sftp"), "{:?}", right_part(&second, 2, 76));
    assert!(right_part(&second, 4, 76).contains("host"), "{:?}", right_part(&second, 4, 76));
    assert_eq!(second.backend().buffer()[(73, 5)].symbol(), "\u{2502}");
}

#[test]
fn at_99_columns_the_details_are_hidden_and_the_list_is_full_width() {
    let (app, _) = connections_app();

    let terminal = render_app(&app, 99, 24);

    assert!(!row_symbols(&terminal, 5).contains('\u{2502}'));
    assert!(row_symbols(&terminal, 4).contains("deploy@prod.example.com"));
    assert!(find_text(&terminal, "username").is_none());
}

#[test]
fn space_opens_the_details_popup_below_100_columns_and_any_key_closes_it() {
    let (mut app, _) = connections_app();
    let _ = render_app(&app, 80, 24);

    app.apply_action(Action::ToggleSelect);
    let open = render_app(&app, 80, 24);

    assert!(app.details_popup);
    assert!(find_text(&open, "username").is_some());
    app.apply_key(plain_key('x'));
    assert!(!app.details_popup);
    assert!(find_text(&render_app(&app, 80, 24), "username").is_none());
}

#[test]
fn the_details_popup_dims_the_screen_behind_it() {
    let (mut app, theme) = connections_app();
    let _ = render_app(&app, 80, 24);
    app.apply_action(Action::ToggleSelect);

    let terminal = render_app(&app, 80, 24);

    let (x, y) = find_text(&terminal, "porthmos").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.line_strong);
    let (x, y) = find_text(&terminal, "username").unwrap();
    assert_ne!(cell_fg(&terminal, x, y), theme.line_strong);
}

#[test]
fn the_details_popup_closes_on_any_key_without_acting() {
    for character in crate::app::testing::TYPED.chars() {
        let (mut app, _) = connections_app();
        let _ = render_app(&app, 80, 24);
        app.apply_action(Action::ToggleSelect);
        assert!(app.details_popup);

        app.apply_key(plain_key(character));

        assert!(!app.details_popup, "{character:?}");
        assert!(!app.should_quit, "{character:?}");
        assert!(!app.help_visible, "{character:?}");
        assert_eq!(app.screen, Screen::Connections);
    }
}

#[test]
fn space_does_nothing_when_the_details_pane_is_visible() {
    let (mut app, _) = connections_app();
    let _ = render_app(&app, 120, 24);

    app.apply_action(Action::ToggleSelect);

    assert!(!app.details_popup);
}

#[test]
fn typing_a_space_in_the_filter_line_never_opens_the_popup() {
    let (mut app, _) = connections_app();
    let _ = render_app(&app, 80, 24);
    app.apply_action(Action::Filter);

    app.apply_key(plain_key('p'));
    app.apply_key(plain_key(' '));

    assert!(!app.details_popup);
    assert_eq!(crate::widgets::filter_line::FilterLine::filter(&app.connections), Some("p "));
}

#[test]
fn the_popup_never_shows_a_secret() {
    let (mut app, _) = connections_app();
    let mut entry = saved_connection("secretive", "sftp");
    entry.password = Some("hunter2".to_string());
    entry.saved_password = true;
    app.connections.replace(vec![entry]);
    let _ = render_app(&app, 80, 24);
    app.apply_action(Action::ToggleSelect);

    let terminal = render_app(&app, 80, 24);

    let everything = (0..24).map(|y| row_symbols(&terminal, y)).collect::<Vec<_>>().join("\n");
    assert!(!everything.contains("hunter2"));
    assert!(everything.contains("saved"));
}

#[test]
fn a_group_row_shows_its_name_and_count_in_the_details() {
    let (mut app, _) = connections_app();
    let mut grouped = saved_connection("web", "sftp");
    grouped.group = Some("production".to_string());
    app.connections.replace(vec![grouped]);

    let terminal = render_app(&app, 120, 24);

    assert!(right_part(&terminal, 2, 76).starts_with("production"), "{:?}", right_part(&terminal, 2, 76));
    assert!(right_part(&terminal, 2, 76).trim_end().ends_with("1 connection"));
}

fn form_protocols() -> Vec<porthmos_core::ProtocolInfo> {
    let info =
        |id: &'static str, name: &'static str, description: &'static str, port: u16| porthmos_core::ProtocolInfo {
            id,
            display_name: name,
            description,
            limits_note: None,
            form: porthmos_core::ConnectionForm::standard(port),
        };
    vec![info("sftp", "sftp", "ssh file transfer", 22), info("ftp", "ftp / ftps", "legacy hosting, tls", 21)]
}

fn form_app() -> (App, tokio::sync::mpsc::UnboundedReceiver<porthmos_core::Command>) {
    let TestApp { app, commands } = crate::app::testing::test_app_with_protocols(Path::new("/d"), form_protocols());
    let mut app = app.with_theme(indexed_theme());
    app.connections.replace(vec![saved_connection("prod", "sftp"), saved_connection("backup", "sftp")]);
    app.screen = Screen::Connections;
    (app, commands)
}

fn drain(commands: &mut tokio::sync::mpsc::UnboundedReceiver<porthmos_core::Command>) -> Vec<porthmos_core::Command> {
    std::iter::from_fn(|| commands.try_recv().ok()).collect()
}

fn ctrl_key(character: char) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(crossterm::event::KeyCode::Char(character), crossterm::event::KeyModifiers::CONTROL)
}

fn named_key(code: crossterm::event::KeyCode) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
}

#[test]
fn the_add_form_replaces_the_connections_screen_with_a_breadcrumb_and_hints() {
    let (mut app, _) = form_app();
    app.apply_action(Action::AddConnection);

    let terminal = render_app(&app, 120, 24);

    assert!(row_symbols(&terminal, 0).contains("porthmos / connections / new"), "{:?}", row_symbols(&terminal, 0));
    let hints = bottom_row(&terminal);
    assert!(hints.contains("tab next field   \u{2191}\u{2193} field   ctrl+t test   ctrl+s save"), "{hints:?}");
    assert!(hints.trim_end().ends_with("esc cancel"), "{hints:?}");
    assert!(find_text(&terminal, "ssh file transfer").is_some());
    assert!(find_text(&terminal, "/ filter").is_none());
}

#[test]
fn editing_says_edit_in_the_breadcrumb() {
    let (mut app, _) = form_app();
    app.apply_action(Action::Rename);

    let terminal = render_app(&app, 120, 24);

    assert!(row_symbols(&terminal, 0).contains("porthmos / connections / edit"), "{:?}", row_symbols(&terminal, 0));
    assert!(find_text(&terminal, "prod").is_some());
}

#[test]
fn cancelling_returns_to_the_connections_screen_with_the_same_cursor() {
    let (mut app, _) = form_app();
    app.connections.cursor = 1;
    app.apply_action(Action::AddConnection);

    app.apply_key(named_key(crossterm::event::KeyCode::Esc));
    let terminal = render_app(&app, 120, 24);

    assert!(app.dialog.is_none());
    assert_eq!(app.connections.cursor, 1);
    assert!(row_symbols(&terminal, 0).contains("porthmos / connections"));
    assert!(!row_symbols(&terminal, 0).contains("/ new"));
    assert!(find_text(&terminal, "/ filter").is_some());
}

#[test]
fn submitting_with_ctrl_s_sends_save_profile_like_enter_does() {
    let (mut app, mut commands) = form_app();
    app.apply_action(Action::AddConnection);
    app.apply_key(named_key(crossterm::event::KeyCode::Tab));
    for character in "newhost".chars() {
        app.apply_key(plain_key(character));
    }
    drain(&mut commands);

    app.apply_key(ctrl_key('s'));

    let sent = drain(&mut commands);
    assert!(
        matches!(sent.as_slice(), [porthmos_core::Command::SaveProfile { original: None, draft }] if draft.name == "newhost"),
        "{sent:?}"
    );
}

#[test]
fn a_validation_error_from_the_core_is_shown_under_the_fields() {
    let (mut app, _) = form_app();
    app.apply_action(Action::AddConnection);

    app.apply_core_event(Event::ProfileRejected { message: "Name is required".to_string() });
    let terminal = render_app(&app, 120, 24);

    let hint_and_error = (18..22).map(|y| row_symbols(&terminal, y)).collect::<Vec<_>>().join("\n");
    assert!(hint_and_error.contains("\u{2022} Name is required"), "{hint_and_error}");
}

#[test]
fn the_keyring_hint_is_shown_when_no_keyring_is_available() {
    let (mut app, _) = form_app();
    app.apply_core_event(Event::KeyringStatus { available: false });
    app.apply_action(Action::AddConnection);

    let terminal = render_app(&app, 120, 24);

    assert!(find_text(&terminal, "Passwords are not saved").is_some());
}

#[test]
fn a_popup_form_is_still_drawn_as_a_popup_over_its_screen() {
    let (mut app, _) = form_app();
    app.screen = Screen::Files;
    app.dialog = Some(Dialog::Form(crate::widgets::dialog::FormDialog::new(
        "Password for u@web",
        vec![crate::widgets::dialog::FormField::masked("password", "Password", "")],
    )));

    let terminal = render_app(&app, 100, 24);

    assert!(find_text(&terminal, "Password for u@web").is_some());
    assert!(find_text(&terminal, "local").is_some());
}

#[test]
fn the_form_screen_never_panics_at_any_size() {
    let (mut app, _) = form_app();
    app.apply_core_event(Event::KeyringStatus { available: false });
    app.apply_action(Action::AddConnection);
    app.apply_core_event(Event::ProfileRejected { message: "boom\nsecond".to_string() });

    for width in 0..=130 {
        for height in 0..=14 {
            let _ = render_app(&app, width, height);
        }
    }
}

#[test]
fn the_connections_screen_shows_a_connection_in_progress() {
    let (mut app, _) = connections_app();
    app.apply_core_event(Event::Connecting { name: "prod".to_string() });
    add_session(&mut app, 1, "backup");

    let terminal = render_app(&app, 120, 24);

    let top = row_symbols(&terminal, 0);
    assert!(top.trim_end().ends_with("connecting to prod\u{2026} \u{b7} 1 connected"), "{top:?}");
}

#[test]
fn a_failure_of_a_profile_that_no_longer_exists_is_forgotten_when_the_list_reloads() {
    let (mut app, _) = connections_app();
    app.last_failure.insert("gone".to_string(), "boom".to_string());
    app.last_failure.insert("prod".to_string(), "boom".to_string());

    app.apply_core_event(Event::Profiles(vec![saved_connection("prod", "sftp")]));

    assert_eq!(app.last_failure.keys().map(String::as_str).collect::<Vec<_>>(), vec!["prod"]);
    assert!(row_symbols(&render_app(&app, 120, 24), 0).trim_end().ends_with("1 error"));
}

#[test]
fn the_details_popup_gives_way_to_a_dialog_and_never_swallows_its_keys() {
    let (mut app, _) = connections_app();
    let _ = render_app(&app, 80, 24);
    app.apply_action(Action::ToggleSelect);
    assert!(app.details_popup);
    app.dialog = Some(Dialog::Confirm(ConfirmDialog::new("sure?")));

    let terminal = render_app(&app, 80, 24);
    assert!(find_text(&terminal, "sure?").is_some());
    assert!(find_text(&terminal, "username").is_none());

    app.apply_key(plain_key('n'));
    assert!(app.dialog.is_none());
    assert!(!app.details_popup);
}

#[test]
fn leaving_the_connections_screen_clears_a_stale_details_popup_without_eating_a_key() {
    let (mut app, _) = connections_app();
    let _ = render_app(&app, 80, 24);
    app.apply_action(Action::ToggleSelect);
    app.screen = Screen::Files;

    app.apply_key(plain_key('x'));

    assert!(!app.details_popup);
}

#[test]
fn the_popup_is_not_drawn_without_a_selection() {
    let (mut app, _) = connections_app();
    let _ = render_app(&app, 80, 24);
    app.apply_action(Action::ToggleSelect);
    app.connections.replace(Vec::new());

    let terminal = render_app(&app, 80, 24);

    assert!(!row_symbols(&terminal, 8).contains('\u{250c}') && !row_symbols(&terminal, 12).contains('\u{250c}'));
}

#[test]
fn the_hint_bar_offers_space_details_only_when_the_pane_is_hidden() {
    let (app, _) = connections_app();

    let narrow = render_app(&app, 80, 24);
    let wide = render_app(&app, 120, 24);

    assert!(bottom_row(&narrow).contains("Space details"), "{:?}", bottom_row(&narrow));
    assert!(!bottom_row(&wide).contains("Space details"), "{:?}", bottom_row(&wide));
}

fn history_entry(label: &str) -> porthmos_core::history::HistoryEntry {
    porthmos_core::history::HistoryEntry {
        finished_at: chrono::Utc.with_ymd_and_hms(2026, 9, 30, 8, 12, 44).unwrap(),
        connection: "prod".to_string(),
        direction: Direction::Upload,
        label: label.to_string(),
        local_path: format!("/home/me/{label}"),
        remote_path: format!("/srv/{label}"),
        files_done: 1,
        files_total: 1,
        bytes: 10,
        result: porthmos_core::history::HistoryResult::Done,
        failed_count: 0,
        failed_files: Vec::new(),
    }
}

#[test]
fn the_history_screen_shows_the_filter_line_a_row_and_an_entry_count_in_the_top_bar_at_120x24() {
    let (mut app, _) = themed_app();
    app.apply_core_event(Event::History(vec![history_entry("one.txt"), history_entry("two.txt")]));
    app.screen = Screen::History;

    let terminal = render_app(&app, 120, 24);

    let top = row_symbols(&terminal, 0);
    assert!(top.contains("porthmos / history"), "{top:?}");
    assert!(top.trim_end().ends_with("2 entries"), "{top:?}");
    assert!(find_text(&terminal, "/ filter").is_some());
    assert!(find_text(&terminal, "one.txt").is_some());
}

#[test]
fn the_search_screen_shows_its_hints_in_the_hint_bar_at_120x24() {
    let (mut app, _) = themed_app();
    app.apply_action(Action::OpenSearch);

    let terminal = render_app(&app, 120, 24);

    let hint_row = row_symbols(&terminal, 23);
    assert!(hint_row.contains("enter open"), "{hint_row:?}");
    assert!(hint_row.trim_end().ends_with("esc close"), "{hint_row:?}");
    assert!(find_text(&terminal, "/ type").is_none());
    assert!(find_text(&terminal, "type to search").is_some());
}
