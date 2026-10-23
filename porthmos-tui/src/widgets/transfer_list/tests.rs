use porthmos_core::transfer::{
    Direction,
    rows::{QueueRow, RowFailure, RowKind, RowState},
};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn row(id: u64, label: &str, direction: Direction, bytes: (u64, u64), state: RowState) -> QueueRow {
    QueueRow {
        kind: RowKind::Single(id),
        label: label.to_string(),
        direction,
        files_done: 0,
        files_total: 1,
        bytes_done: bytes.0,
        bytes_total: bytes.1,
        state,
        paused: false,
        session_id: Some(1),
        failure: None,
    }
}

fn failure(file: &str, message: &str, attempts: u32) -> Option<RowFailure> {
    Some(RowFailure { file: file.to_string(), message: message.to_string(), attempts })
}

struct Fixture {
    rows: Vec<QueueRow>,
    cursor: usize,
    copy_key: Option<String>,
    speeds: Vec<Option<u64>>,
    etas: Vec<Option<u64>>,
    failed_time: Option<String>,
}

impl Fixture {
    fn of(rows: Vec<QueueRow>) -> Self {
        let count = rows.len();
        Self {
            rows,
            cursor: usize::MAX,
            copy_key: None,
            speeds: vec![None; count],
            etas: vec![None; count],
            failed_time: None,
        }
    }

    fn draw(&self, width: u16, height: u16) -> Terminal<TestBackend> {
        let theme = indexed();
        let routes: Vec<String> =
            self.rows.iter().map(|row| format!("local \u{2192} prod{}", row.label.len() % 2)).collect();
        let view = TransferView {
            rows: &self.rows,
            cursor: self.cursor,
            copy_key: self.copy_key.as_deref(),
            routes: &routes,
            speeds: &self.speeds,
            etas: &self.etas,
            failed_time: self.failed_time.as_deref(),
        };
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render_transfer_list(frame, frame.area(), &view, &theme)).unwrap();
        terminal
    }
}

fn row_string(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    let mut x = 0;
    while x < buffer.area.width {
        let symbol = buffer[(x, y)].symbol();
        text.push_str(symbol);
        x += symbol.width().max(1) as u16;
    }
    text
}

fn everything(terminal: &Terminal<TestBackend>) -> String {
    (0..terminal.backend().buffer().area.height).map(|y| row_string(terminal, y)).collect::<Vec<_>>().join("\n")
}

fn modifier_at(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> Modifier {
    terminal.backend().buffer()[(x, y)].modifier
}

fn bar_cells(terminal: &Terminal<TestBackend>, y: u16) -> Vec<ratatui::style::Color> {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.width).filter(|&x| buffer[(x, y)].symbol() == "\u{2501}").map(|x| cell_fg(terminal, x, y)).collect()
}

fn all_kinds() -> Vec<QueueRow> {
    vec![
        row(1, "running.bin", Direction::Upload, (50, 100), RowState::Running),
        row(2, "queued.bin", Direction::Download, (5, 10), RowState::Queued),
        row(3, "done.bin", Direction::Upload, (100, 100), RowState::Done),
        row(4, "failed.bin", Direction::Download, (40, 100), RowState::Failed),
        row(5, "partly.bin", Direction::Upload, (60, 100), RowState::PartlyFailed(2)),
        row(6, "cancelled.bin", Direction::Upload, (10, 100), RowState::Cancelled),
        QueueRow { kind: RowKind::Scan(7), ..row(7, "scan.dir", Direction::Upload, (0, 0), RowState::Scanning) },
        QueueRow {
            kind: RowKind::Scan(8),
            ..row(8, "asking.dir", Direction::Upload, (0, 0), RowState::AwaitingAnswer)
        },
    ]
}

#[test]
fn every_status_has_its_text_and_colour() {
    let theme = indexed();
    let terminal = Fixture::of(all_kinds()).draw(120, 14);

    let expected = [
        ("active", theme.fg_bright),
        ("queued", theme.fg_dim),
        ("done", theme.ok),
        ("failed", theme.err),
        ("done, 2 failed", theme.err),
        ("cancelled", theme.fg_dim),
        ("scanning\u{2026}", theme.fg_muted),
        ("waiting for you", theme.fg_bright),
    ];
    for (index, (text, colour)) in expected.into_iter().enumerate() {
        let y = 2 + index as u16;
        let row = row_string(&terminal, y);
        let column = row.rfind(text).unwrap_or_else(|| panic!("{text:?} in {row:?}"));
        let x = row[..column].width() as u16;
        assert_eq!(cell_fg(&terminal, x, y), colour, "{text}");
    }
}

#[test]
fn bar_fill_follows_the_state() {
    let theme = indexed();
    let terminal = Fixture::of(all_kinds()).draw(120, 14);

    let count = |y: u16, colour| bar_cells(&terminal, y).into_iter().filter(|c| *c == colour).count();
    assert_eq!((count(2, theme.accent), count(2, theme.line_strong)), (8, 8));
    assert_eq!((count(3, theme.accent), count(3, theme.line_strong)), (0, 16));
    assert_eq!((count(4, theme.ok_dim), count(4, theme.line_strong)), (16, 0));
    assert_eq!((count(5, theme.err), count(5, theme.line_strong)), (6, 10));
    assert_eq!((count(6, theme.err), count(6, theme.line_strong)), (10, 6));
    assert_eq!((count(7, theme.accent), count(7, theme.line_strong)), (0, 16));
    assert!(bar_cells(&terminal, 8).is_empty());
    assert!(bar_cells(&terminal, 9).is_empty());
}

#[test]
fn percent_clamps_and_an_empty_total_is_one_hundred_percent() {
    let rows = vec![
        row(1, "over.bin", Direction::Upload, (500, 100), RowState::Running),
        row(2, "empty.bin", Direction::Upload, (0, 0), RowState::Done),
    ];

    let terminal = Fixture::of(rows).draw(120, 8);

    assert!(row_string(&terminal, 2).contains("100%"));
    assert!(row_string(&terminal, 3).contains("100%"));
    assert!(!everything(&terminal).contains("500%"));
}

#[test]
fn the_focused_row_has_a_chevron_focus_background_and_bold_name() {
    let theme = indexed();
    let mut fixture = Fixture::of(all_kinds());
    fixture.cursor = 1;

    let terminal = fixture.draw(120, 14);

    assert_eq!(terminal.backend().buffer()[(0, 3)].symbol(), "\u{203a}");
    assert_eq!(cell_fg(&terminal, 0, 3), theme.accent);
    let (x, y) = find_text(&terminal, "queued.bin").unwrap();
    assert_eq!((y, cell_fg(&terminal, x, y)), (3, theme.fg_focus));
    assert!(modifier_at(&terminal, x, y).contains(Modifier::BOLD));
    assert_eq!(cell_bg(&terminal, 119, 3), theme.row_focus);
    assert_eq!(terminal.backend().buffer()[(0, 2)].symbol(), " ");
}

#[test]
fn unfocused_names_use_the_text_colour() {
    let theme = indexed();
    let terminal = Fixture::of(all_kinds()).draw(120, 14);

    let (x, y) = find_text(&terminal, "done.bin").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
}

#[test]
fn speed_and_eta_are_right_aligned_under_their_headers_and_blank_when_unknown() {
    let theme = indexed();
    let mut fixture = Fixture::of(vec![
        row(1, "running.bin", Direction::Upload, (50, 100), RowState::Running),
        row(2, "queued.bin", Direction::Upload, (0, 10), RowState::Queued),
    ]);
    fixture.speeds[0] = Some(13_002_342);
    fixture.etas[0] = Some(41);

    let terminal = fixture.draw(120, 8);

    let header = row_string(&terminal, 0);
    let speed_end = header.find("speed").map(|byte| header[..byte].width() + 5).unwrap();
    let eta_end = header.find("eta").map(|byte| header[..byte].width() + 3).unwrap();
    let running = row_string(&terminal, 2);
    let speed_text_end = running.find("12.4 MB/s").map(|byte| running[..byte].width() + 9).unwrap();
    let eta_text_end = running.find("0:41").map(|byte| running[..byte].width() + 4).unwrap();
    assert_eq!((speed_text_end, eta_text_end), (speed_end, eta_end));
    let queued = row_string(&terminal, 3);
    assert!(!queued.contains("MB/s") && !queued.contains(":"), "{queued:?}");
    let (x, y) = find_text(&terminal, "12.4 MB/s").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn route_and_headers_are_dim_and_the_rule_is_accent() {
    let theme = indexed();
    let terminal = Fixture::of(all_kinds()).draw(120, 14);

    let (route_x, route_y) = find_text(&terminal, "local \u{2192} prod").unwrap();
    assert_eq!(cell_fg(&terminal, route_x, route_y), theme.fg_dim);
    let (item_x, header_y) = find_text(&terminal, "item").unwrap();
    assert_eq!((header_y, cell_fg(&terminal, item_x, header_y)), (0, theme.fg_dim));
    assert_eq!(row_string(&terminal, 1), "\u{2500}".repeat(120));
    assert_eq!(cell_fg(&terminal, 10, 1), theme.accent);
}

#[test]
fn directions_are_arrows_in_the_muted_colour() {
    let theme = indexed();
    let terminal = Fixture::of(all_kinds()).draw(120, 14);

    assert_eq!(terminal.backend().buffer()[(2, 2)].symbol(), "\u{2191}");
    assert_eq!(terminal.backend().buffer()[(2, 3)].symbol(), "\u{2193}");
    assert_eq!(cell_fg(&terminal, 2, 2), theme.fg_muted);
}

#[test]
fn the_empty_state_names_the_copy_key_and_survives_no_key() {
    let mut fixture = Fixture::of(Vec::new());
    fixture.copy_key = Some("F5".to_string());

    assert!(everything(&fixture.draw(80, 6)).contains("No transfers \u{2014} F5 copies the selected file or folder"));
    fixture.copy_key = None;
    assert!(everything(&fixture.draw(80, 6)).contains("No transfers yet"));
}

#[test]
fn the_detail_area_shows_file_attempts_time_message_and_advice_for_a_failed_row() {
    let theme = indexed();
    let mut failed = row(1, "access.log", Direction::Download, (0, 100), RowState::Failed);
    failed.failure = failure("access.log", "permission denied \u{2014} user can't read /var/log/nginx/access.log", 3);
    let mut fixture = Fixture::of(vec![failed]);
    fixture.cursor = 0;
    fixture.failed_time = Some("14:07".to_string());

    let terminal = fixture.draw(100, 12);

    assert_eq!(row_string(&terminal, 8), "\u{2500}".repeat(100));
    assert!(row_string(&terminal, 9).starts_with("\u{2022} access.log  failed after 3 attempts \u{b7} 14:07"));
    assert!(row_string(&terminal, 10).starts_with("permission denied"));
    assert!(row_string(&terminal, 11).starts_with("check the file's permissions"));
    assert_eq!(cell_fg(&terminal, 0, 9), theme.err);
    assert_eq!(cell_fg(&terminal, 2, 9), theme.fg);
    assert_eq!(cell_fg(&terminal, 30, 9), theme.fg_muted);
    assert_eq!(cell_fg(&terminal, 0, 10), theme.fg_muted);
    assert_eq!(cell_fg(&terminal, 0, 11), theme.fg_dim);
}

#[test]
fn one_attempt_is_singular_and_a_missing_time_is_left_out() {
    let mut failed = row(1, "a.bin", Direction::Upload, (0, 10), RowState::Failed);
    failed.failure = failure("a.bin", "mystery", 1);
    let mut fixture = Fixture::of(vec![failed]);
    fixture.cursor = 0;

    let terminal = fixture.draw(100, 12);

    let line = row_string(&terminal, 9);
    assert!(
        line.contains("failed after 1 attempt") && !line.contains("attempts") && !line.contains('\u{b7}'),
        "{line:?}"
    );
    assert_eq!(row_string(&terminal, 11).trim(), "");
}

#[test]
fn no_detail_area_for_a_running_row_or_a_short_screen() {
    let mut fixture = Fixture::of(vec![row(1, "a.bin", Direction::Upload, (1, 10), RowState::Running)]);
    fixture.cursor = 0;
    assert!(!everything(&fixture.draw(100, 12)).contains("failed after"));

    let mut failed = row(1, "a.bin", Direction::Upload, (0, 10), RowState::Failed);
    failed.failure = failure("a.bin", "boom", 3);
    let mut short = Fixture::of(vec![failed]);
    short.cursor = 0;
    assert!(!everything(&short.draw(100, 7)).contains("failed after"));
}

#[test]
fn a_hostile_error_message_is_sanitised_and_cut() {
    let mut failed = row(1, "a.bin", Direction::Upload, (0, 10), RowState::Failed);
    failed.failure =
        failure("a\u{202e}.bin", &format!("Transfer failed: a:\nbad\u{1b}[31m\nline\u{202e}{}", "x".repeat(1000)), 3);
    let mut fixture = Fixture::of(vec![failed]);
    fixture.cursor = 0;

    let terminal = fixture.draw(60, 12);

    let shown = everything(&terminal);
    assert!(!shown.contains('\u{1b}') && !shown.contains('\u{202e}'));
    assert!(row_string(&terminal, 10).contains("bad?[31m?line?"));
    for y in 0..12 {
        assert!(row_string(&terminal, y).width() <= 60);
    }
}

#[test]
fn hostile_labels_are_sanitised_and_wide_names_keep_columns_aligned() {
    let rows = vec![
        row(1, &"\u{65e5}\u{672c}".repeat(100), Direction::Upload, (50, 100), RowState::Running),
        row(2, "a\u{202e}b\nc", Direction::Upload, (50, 100), RowState::Running),
    ];

    let terminal = Fixture::of(rows).draw(120, 8);

    assert!(row_string(&terminal, 3).contains("a?b?c"));
    let first = row_string(&terminal, 2);
    let second = row_string(&terminal, 3);
    assert_eq!(first.find("active").map(|b| first[..b].width()), second.find("active").map(|b| second[..b].width()));
}

#[test]
fn columns_hide_from_the_right_as_the_width_shrinks() {
    let mut fixture = Fixture::of(vec![row(1, "running-file.bin", Direction::Upload, (50, 100), RowState::Running)]);
    fixture.speeds[0] = Some(1_000_000);
    fixture.etas[0] = Some(30);
    let markers = ["0:30", "MB/s", "local", "\u{2501}", "50%", "running-fi", "active"];
    let mut gone = vec![false; markers.len()];

    for width in (24..=130).rev() {
        let shown = everything(&fixture.draw(width, 8));
        for (index, marker) in markers.iter().enumerate() {
            let present = shown.contains(marker);
            assert!(!(gone[index] && present), "{marker:?} came back at width {width}");
            gone[index] |= !present;
        }
        assert!(shown.contains("active"), "status kept at width {width}");
        assert!(shown.contains("running-fi"), "item kept at width {width}");
    }
    assert!(gone[0] && gone[1], "eta and speed hide first");
}

#[test]
fn tiny_areas_do_not_panic() {
    let mut failed = row(1, "a.bin", Direction::Upload, (0, 10), RowState::Failed);
    failed.failure = failure("a.bin", "boom", 3);
    let mut fixture = Fixture::of(vec![failed, row(2, "b.bin", Direction::Download, (1, 2), RowState::Running)]);
    fixture.cursor = 99;
    fixture.failed_time = Some("14:07".to_string());

    for width in (0..=12).chain([90]) {
        for height in 0..=14 {
            let _ = fixture.draw(width, height);
        }
    }
    let _ = Fixture::of(Vec::new()).draw(0, 0);
}

#[test]
fn an_indexed_theme_keeps_bar_fill_status_and_focus_distinct() {
    let theme = indexed();

    assert_ne!(theme.accent, theme.line_strong);
    assert_ne!(theme.ok_dim, theme.line_strong);
    assert_ne!(theme.err, theme.line_strong);
    assert_ne!(theme.ok, theme.err);
    assert_ne!(theme.row_focus, theme.bg);
}

#[test]
fn the_list_no_longer_uses_the_reversed_modifier_or_a_box() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
    assert!(!source.contains("Borders"));
}

fn summary_rows() -> Vec<QueueRow> {
    vec![
        row(1, "a", Direction::Upload, (1, 2), RowState::Running),
        row(2, "b", Direction::Upload, (0, 2), RowState::Queued),
        row(3, "c", Direction::Upload, (0, 2), RowState::Queued),
        row(4, "d", Direction::Upload, (0, 2), RowState::Failed),
        row(5, "e", Direction::Upload, (1, 2), RowState::PartlyFailed(1)),
        row(6, "f", Direction::Upload, (2, 2), RowState::Done),
    ]
}

#[test]
fn the_summary_lists_active_queued_failed_and_speed() {
    assert_eq!(
        transfer_summary(&summary_rows(), Some(13_002_342), None),
        "1 active \u{b7} 2 queued \u{b7} 2 failed \u{b7} 12.4 MB/s"
    );
}

#[test]
fn zero_parts_and_an_unknown_speed_are_omitted() {
    let rows = vec![row(1, "a", Direction::Upload, (1, 2), RowState::Queued)];

    assert_eq!(transfer_summary(&rows, None, None), "1 queued");
    assert_eq!(transfer_summary(&rows, Some(0), None), "1 queued \u{b7} 0 B/s");
}

#[test]
fn no_rows_give_an_empty_summary() {
    assert_eq!(transfer_summary(&[], None, None), "");
    assert_eq!(transfer_summary(&[row(1, "a", Direction::Upload, (1, 1), RowState::Done)], None, None), "");
}

fn failed_fixture(failure: Option<RowFailure>, width: u16) -> Terminal<TestBackend> {
    let mut failed = row(1, "x.bin", Direction::Download, (0, 10), RowState::Failed);
    failed.failure = failure;
    let mut fixture = Fixture::of(vec![failed]);
    fixture.cursor = 0;
    fixture.failed_time = Some("14:07".to_string());
    fixture.draw(width, 12)
}

#[test]
fn the_detail_shows_only_the_error_part_of_a_real_failure_message_and_advises_from_it() {
    let message = "Transfer failed: dir/quota_2026.xlsx:\nConnection reset by peer";

    let terminal = failed_fixture(failure("quota_2026.xlsx", message, 2), 100);

    assert!(row_string(&terminal, 10).starts_with("Connection reset by peer"), "{:?}", row_string(&terminal, 10));
    assert!(
        row_string(&terminal, 11).starts_with("check the connection, then retry"),
        "{:?}",
        row_string(&terminal, 11)
    );
    assert!(!everything(&terminal).contains("Transfer failed"));
    assert!(!row_string(&terminal, 10).contains('?'));
}

#[test]
fn a_message_without_a_context_line_is_shown_whole() {
    let terminal = failed_fixture(failure("a.bin", "session disconnected", 0), 100);

    assert!(row_string(&terminal, 10).starts_with("session disconnected"));
    assert!(row_string(&terminal, 11).starts_with("check the connection"));
}

#[test]
fn a_job_that_never_started_does_not_claim_zero_attempts() {
    let terminal = failed_fixture(failure("a.bin", "session disconnected", 0), 100);

    let line = row_string(&terminal, 9);
    assert!(line.contains("did not start") && !line.contains("0 attempts"), "{line:?}");
    assert!(line.contains("14:07"));
}

#[test]
fn the_detail_file_name_is_cut_to_leave_room_for_the_attempts_and_time() {
    let long = "n".repeat(80);

    for width in [40, 60, 100] {
        let terminal = failed_fixture(failure(&long, "boom", 3), width);
        let line = row_string(&terminal, 9);
        assert!(line.trim_end().ends_with("attempts \u{b7} 14:07"), "width {width}: {line:?}");
        assert!(line.contains('\u{2026}'), "width {width}: {line:?}");
    }
}

#[test]
fn a_batch_row_shows_its_file_counts_after_the_name_in_dim() {
    let theme = indexed();
    let batch = QueueRow {
        kind: RowKind::Batch(1),
        files_done: 42,
        files_total: 118,
        ..row(1, "dist/", Direction::Upload, (36, 100), RowState::Running)
    };

    let terminal = Fixture::of(vec![batch]).draw(120, 8);

    let line = row_string(&terminal, 2);
    assert!(line.contains("dist/ 42 of 118"), "{line:?}");
    let (x, y) = find_text(&terminal, "42 of 118").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn a_long_batch_name_is_cut_before_its_counts_and_columns_stay_aligned() {
    let batch = QueueRow {
        kind: RowKind::Batch(1),
        files_done: 1,
        files_total: 2,
        ..row(1, &"n".repeat(300), Direction::Upload, (1, 2), RowState::Running)
    };
    let single = row(2, "single.bin", Direction::Upload, (1, 2), RowState::Running);

    let terminal = Fixture::of(vec![batch, single]).draw(100, 8);

    let first = row_string(&terminal, 2);
    let second = row_string(&terminal, 3);
    assert!(first.contains("1 of 2") && first.contains('\u{2026}'), "{first:?}");
    assert_eq!(first.find("active").map(|b| first[..b].width()), second.find("active").map(|b| second[..b].width()));
}

#[test]
fn the_summary_ends_with_the_limit_when_one_is_set() {
    assert_eq!(
        transfer_summary(&summary_rows(), Some(13_002_342), Some(2_097_152)),
        "1 active \u{b7} 2 queued \u{b7} 2 failed \u{b7} 12.4 MB/s \u{b7} limit 2.0 MB/s"
    );
}

#[test]
fn a_limit_alone_is_still_shown_when_nothing_is_running() {
    assert_eq!(transfer_summary(&[], None, Some(512 * 1024)), "limit 512.0 KB/s");
    let rows = vec![row(1, "a", Direction::Upload, (1, 2), RowState::Queued)];
    assert_eq!(transfer_summary(&rows, None, Some(1024)), "1 queued \u{b7} limit 1.0 KB/s");
}

#[test]
fn a_paused_row_says_paused_in_the_muted_colour_and_fills_its_bar_in_the_paused_colour() {
    let theme = indexed();
    let terminal = Fixture::of(vec![row(1, "paused.bin", Direction::Upload, (50, 100), RowState::Paused)]).draw(120, 6);

    let line = row_string(&terminal, 2);
    let column = line.rfind("paused").unwrap_or_else(|| panic!("paused in {line:?}"));
    let x = line[..column].width() as u16;
    assert_eq!(cell_fg(&terminal, x, 2), theme.fg_muted);
    let count = |colour| bar_cells(&terminal, 2).into_iter().filter(|c| *c == colour).count();
    assert_eq!((count(theme.paused), count(theme.line_strong)), (8, 8));
    assert!(line.contains("50%"), "{line:?}");
}

#[test]
fn the_summary_counts_paused_rows_between_queued_and_failed() {
    let rows = vec![
        row(1, "a", Direction::Upload, (1, 2), RowState::Running),
        row(2, "b", Direction::Upload, (0, 2), RowState::Queued),
        row(3, "c", Direction::Upload, (0, 2), RowState::Paused),
        row(4, "d", Direction::Upload, (0, 2), RowState::Failed),
    ];

    assert_eq!(transfer_summary(&rows, None, None), "1 active \u{b7} 1 queued \u{b7} 1 paused \u{b7} 1 failed");
}

#[test]
fn a_paused_row_alone_is_still_summarised() {
    let rows = vec![row(1, "c", Direction::Upload, (0, 2), RowState::Paused)];

    assert_eq!(transfer_summary(&rows, None, None), "1 paused");
}
