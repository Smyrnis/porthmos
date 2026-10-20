use std::path::Path;

use porthmos_core::transfer::{
    JobStatus, TransferQueue,
    rows::{RowState, ScanInfo},
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
fn transfer_status_text_shows_batch_progress_for_a_batch_job() {
    let (_dir, mut app) = app_in_temp_dir();
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("batch".to_string());
    let active = queue.enqueue(
        1,
        Direction::Upload,
        PathBuf::from("/local/a.txt"),
        "/remote/a.txt".to_string(),
        "a.txt".to_string(),
        100,
        Some(batch_id),
    );
    queue.enqueue(
        1,
        Direction::Upload,
        PathBuf::from("/local/b.txt"),
        "/remote/b.txt".to_string(),
        "b.txt".to_string(),
        100,
        Some(batch_id),
    );
    {
        let mut job = queue.get_mut(active).unwrap();
        job.status = JobStatus::InProgress;
        job.transferred_bytes = 50;
    }

    app.transfers = TransferSnapshot::of(&queue, &[]);
    let text = hint_bar_text(&app);

    assert!(text.contains("0/2 files"));
    assert!(text.contains("25%"));
}

#[test]
fn transfer_status_text_is_unchanged_for_a_non_batch_job() {
    let (_dir, mut app) = app_in_temp_dir();
    let mut queue = TransferQueue::new();
    let active = queue.enqueue(
        1,
        Direction::Upload,
        PathBuf::from("/local/a.txt"),
        "/remote/a.txt".to_string(),
        "a.txt".to_string(),
        100,
        None,
    );
    {
        let mut job = queue.get_mut(active).unwrap();
        job.status = JobStatus::InProgress;
        job.transferred_bytes = 50;
    }

    app.transfers = TransferSnapshot::of(&queue, &[]);
    let text = hint_bar_text(&app);

    assert!(text.contains("a.txt: 50%"));
    assert!(!text.contains("files"));
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

fn active_job(
    queue: &mut TransferQueue, direction: Direction, name: &str, total_bytes: u64, transferred_bytes: u64,
    batch_id: Option<u64>,
) -> u64 {
    let id = queue.enqueue(
        1,
        direction,
        PathBuf::from(format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        total_bytes,
        batch_id,
    );
    let mut job = queue.get_mut(id).unwrap();
    job.status = JobStatus::InProgress;
    job.transferred_bytes = transferred_bytes;
    id
}

fn status_of_active_jobs(app: &mut App, queue: &TransferQueue) -> String {
    app.transfers = TransferSnapshot::of(queue, &[]);
    app.transfer_status_text(&app.transfers.active)
}

#[test]
fn transfer_status_text_summarizes_several_jobs_from_one_batch() {
    let (_dir, mut app) = app_in_temp_dir();
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("batch".to_string());
    active_job(&mut queue, Direction::Upload, "a.txt", 100, 50, Some(batch_id));
    active_job(&mut queue, Direction::Upload, "b.txt", 100, 50, Some(batch_id));
    queue.enqueue(
        1,
        Direction::Upload,
        PathBuf::from("/local/c.txt"),
        "/remote/c.txt".to_string(),
        "c.txt".to_string(),
        200,
        Some(batch_id),
    );

    assert_eq!(status_of_active_jobs(&mut app, &queue), "Uploading 2 files: 0/3 files, 25% (1 queued)");
}

#[test]
fn transfer_status_text_summarizes_several_unrelated_jobs() {
    let (_dir, mut app) = app_in_temp_dir();
    let mut queue = TransferQueue::new();
    active_job(&mut queue, Direction::Download, "a.txt", 100, 100, None);
    active_job(&mut queue, Direction::Download, "b.txt", 300, 0, None);

    assert_eq!(status_of_active_jobs(&mut app, &queue), "Downloading 2 files: 25%");
}

#[test]
fn transfer_status_text_says_transferring_for_mixed_directions() {
    let (_dir, mut app) = app_in_temp_dir();
    let mut queue = TransferQueue::new();
    active_job(&mut queue, Direction::Upload, "a.txt", 100, 50, None);
    active_job(&mut queue, Direction::Download, "b.txt", 100, 50, None);

    assert_eq!(status_of_active_jobs(&mut app, &queue), "Transferring 2 files: 50%");
}

#[test]
fn transfer_status_text_is_one_hundred_percent_for_several_empty_files() {
    let (_dir, mut app) = app_in_temp_dir();
    let mut queue = TransferQueue::new();
    active_job(&mut queue, Direction::Upload, "a.txt", 0, 0, None);
    active_job(&mut queue, Direction::Upload, "b.txt", 0, 0, None);

    assert_eq!(status_of_active_jobs(&mut app, &queue), "Uploading 2 files: 100%");
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

#[test]
fn the_active_panel_border_is_drawn_in_the_accent_colour() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 60, 12);

    assert_eq!(terminal.backend().buffer()[(2, 2)].symbol(), "\u{250C}");
    assert_eq!(cell_fg(&terminal, 2, 2), theme.accent);
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
            assert_eq!(cell_bg(&terminal, x, y), theme.bg, "({x}, {y})");
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
            assert_eq!(cell_bg(&terminal, x, y), ratatui::style::Color::Reset, "({x}, {y})");
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
    assert!(find_text(&terminal, "LOCAL").is_none());
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

    assert_eq!(find_text(&one, "REMOTE"), find_text(&two, "REMOTE"));
}

fn bottom_row(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
    let height = terminal.backend().buffer().area.height;
    row_symbols(terminal, height - 1)
}

#[test]
fn the_hint_bar_lists_the_actions_of_the_screen_with_their_keys() {
    let (app, theme) = themed_app();

    let terminal = render_app(&app, 120, 12);

    assert!(bottom_row(&terminal).contains("F5 copy   F7 mkdir"), "{:?}", bottom_row(&terminal));
    let (x, y) = find_text(&terminal, "F5 copy").expect("hint is drawn");
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
    assert!(!row.contains("F1 help"));
    let (x, y) = find_text(&terminal, "Scanning myfolder").expect("status is drawn");
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_muted);
}

#[test]
fn a_narrow_terminal_keeps_the_help_pair() {
    let (app, _) = themed_app();

    for width in [60, 40, 30, 20, 12] {
        let terminal = render_app(&app, width, 12);
        assert!(bottom_row(&terminal).contains("F1 help"), "width {width}: {:?}", bottom_row(&terminal));
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
    let overlay = render_app(&app, 100, 40);
    let (_, y) = find_text(&overlay, "Ctrl+G").expect("overlay lists the remapped key");
    assert!(y < 39);
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

    assert!(bottom_row(&terminal).contains("F1 help"), "{:?}", bottom_row(&terminal));
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

    assert!(find_text(&terminal, "F10        quit").is_some());
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
    assert!(row.contains("F10 quit"), "{row:?}");
    assert!(row.contains("F1 help"), "{row:?}");
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
