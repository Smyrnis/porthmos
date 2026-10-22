use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use porthmos_core::transfer::{
    Direction,
    rows::{QueueRow, RowKind, RowState},
};
use ratatui::{Terminal, backend::TestBackend};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_fg, find_text},
    rate::RateTracker,
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn row(
    kind: RowKind, label: &str, direction: Direction, bytes: (u64, u64), files: (usize, usize), state: RowState,
) -> QueueRow {
    QueueRow {
        kind,
        label: label.to_string(),
        direction,
        files_done: files.0,
        files_total: files.1,
        bytes_done: bytes.0,
        bytes_total: bytes.1,
        state,
        paused: false,
        session_id: Some(1),
        failure: None,
    }
}

fn running(id: u64, label: &str, direction: Direction, bytes: (u64, u64)) -> QueueRow {
    row(RowKind::Single(id), label, direction, bytes, (0, 1), RowState::Running)
}

fn names() -> HashMap<u64, String> {
    HashMap::from([(1, "prod-web".to_string())])
}

fn data_of(rows: &[QueueRow]) -> Option<LineData> {
    line_data(rows, &names(), &RateTracker::default())
}

fn full_data() -> LineData {
    LineData {
        arrow: "\u{2191}".to_string(),
        label: "dist/".to_string(),
        target: Some("prod-web".to_string()),
        percent: 36,
        counts: Some((42, 118)),
        speed: Some(13_002_342),
        eta: Some(41),
        queued: 2,
        paused: 0,
        incoming: false,
        waiting: false,
    }
}

fn draw(data: &LineData, width: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal.draw(|frame| render_transfer_line(frame, frame.area(), data, &theme)).unwrap();
    terminal
}

fn text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    let mut x = 0;
    while x < buffer.area.width {
        let symbol = buffer[(x, 0)].symbol();
        text.push_str(symbol);
        x += symbol.width().max(1) as u16;
    }
    text
}

#[test]
fn no_rows_means_no_line() {
    assert!(data_of(&[]).is_none());
    assert!(data_of(&[row(RowKind::Single(1), "a", Direction::Upload, (1, 1), (1, 1), RowState::Done)]).is_none());
}

#[test]
fn a_single_running_row_gives_its_label_target_percent_and_arrow() {
    let data = data_of(&[running(1, "dist/", Direction::Upload, (36, 100))]).unwrap();

    assert_eq!((data.arrow.as_str(), data.label.as_str()), ("\u{2191}", "dist/"));
    assert_eq!(data.target.as_deref(), Some("prod-web"));
    assert_eq!((data.percent, data.counts, data.queued), (36, None, 0));
}

#[test]
fn a_downloading_row_uses_the_down_arrow_and_the_same_target() {
    let data = data_of(&[running(1, "db.sql.gz", Direction::Download, (1, 2))]).unwrap();

    assert_eq!(data.arrow, "\u{2193}");
    assert_eq!(data.target.as_deref(), Some("prod-web"));
    assert!(data.incoming);
}

#[test]
fn a_batch_row_gives_file_counts() {
    let batch = row(RowKind::Batch(1), "photos", Direction::Upload, (10, 100), (42, 118), RowState::Running);

    assert_eq!(data_of(&[batch]).unwrap().counts, Some((42, 118)));
}

#[test]
fn several_running_rows_give_a_combined_line() {
    let rows = [
        running(1, "a", Direction::Upload, (50, 100)),
        running(2, "b", Direction::Upload, (150, 300)),
        row(RowKind::Single(3), "c", Direction::Upload, (0, 10), (0, 1), RowState::Queued),
    ];

    let data = data_of(&rows).unwrap();

    assert_eq!(data.label, "2 transfers");
    assert_eq!((data.target, data.counts, data.percent, data.queued), (None, None, 50, 1));
    assert_eq!(data.arrow, "\u{2191}");
}

#[test]
fn mixed_directions_use_both_arrows() {
    let rows = [running(1, "a", Direction::Upload, (1, 2)), running(2, "b", Direction::Download, (1, 2))];

    assert_eq!(data_of(&rows).unwrap().arrow, "\u{2191}\u{2193}");
}

#[test]
fn queued_rows_are_counted_and_the_first_is_shown_when_nothing_runs() {
    let queued =
        |id: u64| row(RowKind::Single(id), &format!("f{id}"), Direction::Upload, (0, 10), (0, 1), RowState::Queued);

    let data = data_of(&[queued(1), queued(2), queued(3)]).unwrap();

    assert_eq!((data.label.as_str(), data.percent, data.queued), ("f1", 0, 2));
}

#[test]
fn zero_bytes_total_is_one_hundred_percent_and_oversized_progress_clamps() {
    assert_eq!(data_of(&[running(1, "empty", Direction::Upload, (0, 0))]).unwrap().percent, 100);
    assert_eq!(data_of(&[running(1, "over", Direction::Upload, (500, 100))]).unwrap().percent, 100);
}

#[test]
fn scan_rows_are_ignored() {
    let scan = row(RowKind::Scan(1), "folder", Direction::Upload, (0, 0), (0, 0), RowState::Scanning);

    assert!(data_of(&[scan]).is_none());
}

#[test]
fn speed_and_eta_come_from_the_rate_tracker() {
    let base = Instant::now();
    let mut rates = RateTracker::default();
    rates.observe(base, &[running(1, "a", Direction::Upload, (0, 100_000_000))]);
    rates.observe(base + Duration::from_secs(2), &[running(1, "a", Direction::Upload, (20_000_000, 100_000_000))]);

    let data = line_data(&[running(1, "a", Direction::Upload, (20_000_000, 100_000_000))], &names(), &rates).unwrap();

    assert_eq!((data.speed, data.eta), (Some(10_000_000), Some(8)));
}

#[test]
fn an_unknown_session_is_named_remote() {
    let mut orphan = running(1, "a", Direction::Upload, (1, 2));
    orphan.session_id = Some(99);

    assert_eq!(line_data(&[orphan], &names(), &RateTracker::default()).unwrap().target.as_deref(), Some("remote"));
}

#[test]
fn the_line_shows_arrow_label_route_bar_percent_counts_speed_eta_and_queue() {
    let terminal = draw(&full_data(), 120);

    let shown = text(&terminal);
    let expected_left = format!(
        "\u{2191} dist/  \u{2192} prod-web  {}{}  36%  42/118 \u{b7} 12.4 MB/s \u{b7} 0:41",
        "\u{2501}".repeat(22),
        ""
    );
    assert!(shown.starts_with(&expected_left), "{shown:?}");
    assert!(shown.trim_end().ends_with("+2 queued"), "{shown:?}");
    assert_eq!(shown.width(), 120);
}

#[test]
fn the_bar_is_twenty_two_cells_with_the_filled_part_in_accent_and_the_rest_in_line_strong() {
    let theme = indexed();
    let terminal = draw(&full_data(), 120);

    let start = text(&terminal).find('\u{2501}').map(|byte| text(&terminal)[..byte].width()).unwrap() as u16;
    for offset in 0..8 {
        assert_eq!(cell_fg(&terminal, start + offset, 0), theme.accent, "cell {offset}");
    }
    for offset in 8..22 {
        assert_eq!(cell_fg(&terminal, start + offset, 0), theme.line_strong, "cell {offset}");
    }
}

#[test]
fn an_empty_bar_is_all_line_strong_and_a_full_bar_all_accent() {
    let theme = indexed();
    let mut data = full_data();
    data.percent = 0;
    let empty = draw(&data, 120);
    data.percent = 100;
    let full = draw(&data, 120);

    let start = text(&empty).find('\u{2501}').map(|byte| text(&empty)[..byte].width()).unwrap() as u16;
    assert_eq!(cell_fg(&empty, start, 0), theme.line_strong);
    assert_eq!(cell_fg(&empty, start + 21, 0), theme.line_strong);
    assert_eq!(cell_fg(&full, start, 0), theme.accent);
    assert_eq!(cell_fg(&full, start + 21, 0), theme.accent);
}

#[test]
fn text_colours_follow_the_spec() {
    let theme = indexed();
    let terminal = draw(&full_data(), 120);

    let (label_x, y) = find_text(&terminal, "dist/").unwrap();
    assert_eq!(cell_fg(&terminal, label_x, y), theme.fg_bright);
    let (route_x, _) = find_text(&terminal, "prod-web").unwrap();
    assert_eq!(cell_fg(&terminal, route_x, y), theme.fg_dim);
    let (percent_x, _) = find_text(&terminal, "36%").unwrap();
    assert_eq!(cell_fg(&terminal, percent_x, y), theme.fg_bright);
    let (speed_x, _) = find_text(&terminal, "12.4 MB/s").unwrap();
    assert_eq!(cell_fg(&terminal, speed_x, y), theme.fg_muted);
    let (queue_x, _) = find_text(&terminal, "+2 queued").unwrap();
    assert_eq!(cell_fg(&terminal, queue_x, y), theme.fg_muted);
}

#[test]
fn narrow_widths_drop_parts_in_order() {
    let data = full_data();
    let shown = |width: u16| text(&draw(&data, width));

    let wide = shown(120);
    assert!(wide.contains("+2 queued") && wide.contains("0:41"));
    let without_queue = shown(80);
    assert!(!without_queue.contains("queued") && without_queue.contains("0:41"), "{without_queue:?}");
    let without_eta = shown(70);
    assert!(!without_eta.contains("0:41") && without_eta.contains("12.4 MB/s"), "{without_eta:?}");
    let without_speed = shown(60);
    assert!(!without_speed.contains("MB/s") && without_speed.contains("42/118"), "{without_speed:?}");
    let without_counts = shown(50);
    assert!(!without_counts.contains("42/118") && without_counts.contains('\u{2501}'), "{without_counts:?}");
    let without_bar = shown(30);
    assert!(
        !without_bar.contains('\u{2501}') && without_bar.contains("36%") && without_bar.contains("prod-web"),
        "{without_bar:?}"
    );
    let without_route = shown(20);
    assert!(
        !without_route.contains("prod-web") && without_route.contains("36%") && without_route.contains("dist/"),
        "{without_route:?}"
    );
}

#[test]
fn hostile_labels_are_sanitised_and_cut() {
    let mut data = full_data();
    data.label = format!("a\u{202e}b\nc{}", "\u{65e5}".repeat(300));

    for width in [120, 60, 30, 10, 1, 0] {
        let terminal = draw(&data, width);
        let shown = text(&terminal);
        assert!(shown.width() <= usize::from(width), "width {width}");
        assert!(!shown.contains('\u{202e}') && !shown.contains('\n'));
    }
    assert!(text(&draw(&data, 60)).contains("a?b?c"));
}

#[test]
fn tiny_areas_do_not_panic() {
    let theme = indexed();
    for width in 0..=12 {
        for height in 0..=2 {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render_transfer_line(frame, frame.area(), &full_data(), &theme)).unwrap();
        }
    }
}

#[test]
fn a_download_points_the_target_arrow_at_us_and_an_upload_away() {
    let mut data = full_data();
    assert!(text(&draw(&data, 120)).contains("\u{2192} prod-web"));

    data.incoming = true;
    let shown = text(&draw(&data, 120));

    assert!(shown.contains("\u{2190} prod-web") && !shown.contains("\u{2192} prod-web"), "{shown:?}");
}

#[test]
fn a_queued_only_line_says_queued_instead_of_showing_a_bar() {
    let queued = row(RowKind::Single(1), "f1", Direction::Upload, (0, 10), (0, 1), RowState::Queued);
    let data = data_of(&[queued]).unwrap();
    assert!(data.waiting);

    let shown = text(&draw(&data, 80));

    assert!(shown.contains("queued") && !shown.contains('\u{2501}') && !shown.contains('%'), "{shown:?}");
}

#[test]
fn a_running_line_is_not_marked_waiting() {
    assert!(!data_of(&[running(1, "a", Direction::Upload, (1, 2))]).unwrap().waiting);
}

fn paused_row(id: u64) -> QueueRow {
    row(RowKind::Single(id), "p", Direction::Upload, (1, 2), (0, 1), RowState::Paused)
}

#[test]
fn paused_rows_are_counted_beside_the_running_one_and_not_shown_as_queued() {
    let data = data_of(&[running(1, "a", Direction::Upload, (1, 2)), paused_row(2), paused_row(3)]).unwrap();

    assert_eq!((data.queued, data.paused), (0, 2));
}

#[test]
fn the_right_end_lists_queued_and_paused_counts_together() {
    let mut data = full_data();
    data.paused = 3;

    let shown = text(&draw(&data, 100));

    assert!(shown.trim_end().ends_with("+2 queued \u{b7} 3 paused"), "{shown:?}");
}

#[test]
fn the_paused_count_alone_is_shown_at_the_right_end() {
    let mut data = full_data();
    data.queued = 0;
    data.paused = 1;

    let shown = text(&draw(&data, 100));

    assert!(shown.trim_end().ends_with("1 paused"), "{shown:?}");
    assert!(!shown.contains("queued"), "{shown:?}");
}

#[test]
fn only_paused_rows_give_no_progress_line_but_a_paused_summary() {
    assert!(data_of(&[paused_row(1)]).is_none());
    assert_eq!(paused_summary(&[paused_row(1), paused_row(2)]).as_deref(), Some("2 paused"));
    assert_eq!(paused_summary(&[running(1, "a", Direction::Upload, (1, 2))]), None);
    assert_eq!(paused_summary(&[]), None);
}
