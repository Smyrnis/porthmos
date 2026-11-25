use porthmos_term::{TermSize, Terminal, TerminalConfig, ViewPoint};

use super::*;

fn snapshot_of(output: &str) -> porthmos_term::Snapshot {
    let mut terminal = Terminal::new(TermSize::clamped(60, 4), TerminalConfig::default());
    terminal.feed(output.as_bytes());
    terminal.snapshot()
}

fn at(row: u16, column: u16) -> ViewPoint {
    ViewPoint { row, column }
}

fn osc8(target: &str, text: &str) -> String {
    format!("\x1b]8;;{target}\x1b\\{text}\x1b]8;;\x1b\\")
}

#[test]
fn a_plain_url_is_found_under_any_of_its_cells() {
    let snapshot = snapshot_of("see https://example.org/a now");

    for column in 4..24 {
        let link = link_at(&snapshot, at(0, column)).unwrap_or_else(|| panic!("no link at {column}"));
        assert_eq!(link.target, "https://example.org/a");
        assert_eq!(link.segments, vec![(0, 4..25)]);
        assert!(!link.from_hyperlink);
    }
    assert_eq!(link_at(&snapshot, at(0, 0)), None);
    assert_eq!(link_at(&snapshot, at(0, 26)), None);
}

#[test]
fn trailing_punctuation_is_not_part_of_a_plain_url() {
    let snapshot = snapshot_of("(https://example.org/x).");

    assert_eq!(link_at(&snapshot, at(0, 5)).unwrap().target, "https://example.org/x");
}

#[test]
fn an_osc8_cell_resolves_to_its_target_and_span() {
    let snapshot = snapshot_of(&format!("go {} end", osc8("https://example.org/real", "click")));

    let link = link_at(&snapshot, at(0, 5)).unwrap();

    assert_eq!(link.target, "https://example.org/real");
    assert_eq!(link.label, "click");
    assert_eq!(link.segments, vec![(0, 3..8)]);
    assert!(link.from_hyperlink);
    assert_eq!(link_at(&snapshot, at(0, 9)), None);
}

#[test]
fn adjacent_osc8_links_with_different_targets_are_separate() {
    let snapshot = snapshot_of(&format!("{}{}", osc8("https://a.example", "aa"), osc8("https://b.example", "bb")));

    assert_eq!(link_at(&snapshot, at(0, 1)).unwrap().target, "https://a.example");
    assert_eq!(link_at(&snapshot, at(0, 2)).unwrap().target, "https://b.example");
    assert_eq!(link_at(&snapshot, at(0, 2)).unwrap().segments, vec![(0, 2..4)]);
}

#[test]
fn only_http_https_and_osc8_mailto_are_clickable() {
    let clickable = |output: &str| link_at(&snapshot_of(output), at(0, 1)).is_some_and(|link| is_clickable(&link));

    assert!(clickable("https://example.org"));
    assert!(clickable("HTTP://example.org"));
    assert!(clickable(&osc8("mailto:me@example.org", "mail")));
    assert!(!clickable("mailto:me@example.org"));
    assert!(!clickable(&osc8("file:///etc/passwd", "file")));
    assert!(!clickable(&osc8("ssh://host", "ssh")));
    assert!(!clickable(&osc8("javascript:alert(1)", "js")));
    assert!(!clickable(&osc8("ftp://host/x", "ftp")));
}

#[test]
fn a_target_with_control_characters_is_not_clickable() {
    let link = Link {
        target: "https://example.org/\u{7}".into(),
        segments: vec![(0, 0..4)],
        from_hyperlink: true,
        label: "x".into(),
    };

    assert!(!is_clickable(&link));
}

#[test]
fn confirmation_is_needed_only_when_the_label_differs_from_the_target() {
    let same = link_at(&snapshot_of(&osc8("https://example.org", "https://example.org")), at(0, 1)).unwrap();
    let different = link_at(&snapshot_of(&osc8("https://evil.example", "https://example.org")), at(0, 1)).unwrap();
    let plain = link_at(&snapshot_of("https://example.org"), at(0, 1)).unwrap();

    assert!(!needs_confirmation(&same));
    assert!(needs_confirmation(&different));
    assert!(!needs_confirmation(&plain));
}

#[test]
fn a_blank_or_hidden_cell_has_no_link() {
    let hidden = snapshot_of(&format!("\x1b[8m{}\x1b[0m", osc8("https://example.org", "secret")));

    assert_eq!(link_at(&snapshot_of("   "), at(0, 1)), None);
    assert_eq!(link_at(&hidden, at(0, 1)), None);
}

#[test]
fn wide_characters_do_not_shift_the_column_span() {
    let snapshot = snapshot_of("你好 https://example.org/w");

    let link = link_at(&snapshot, at(0, 10)).unwrap();

    assert_eq!(link.target, "https://example.org/w");
    assert_eq!(link.segments, vec![(0, 5..26)]);
}

#[test]
fn a_point_outside_the_grid_has_no_link() {
    let snapshot = snapshot_of("https://example.org");

    assert_eq!(link_at(&snapshot, at(0, 500)), None);
    assert_eq!(link_at(&snapshot, at(50, 0)), None);
}

fn narrow_snapshot_of(output: &str) -> porthmos_term::Snapshot {
    let mut terminal = Terminal::new(TermSize::clamped(20, 4), TerminalConfig::default());
    terminal.feed(output.as_bytes());
    terminal.snapshot()
}

#[test]
fn a_url_wrapped_over_two_rows_is_one_link_from_either_row() {
    let snapshot = narrow_snapshot_of("https://example.org/a/long/path/x end");

    let top = link_at(&snapshot, at(0, 3)).unwrap();
    let bottom = link_at(&snapshot, at(1, 2)).unwrap();

    assert_eq!(top.target, "https://example.org/a/long/path/x");
    assert_eq!(bottom.target, top.target);
    assert_eq!(top.segments, vec![(0, 0..20), (1, 0..13)]);
    assert_eq!(link_at(&snapshot, at(1, 15)), None);
}

#[test]
fn rows_that_were_not_wrapped_are_never_joined() {
    let snapshot = narrow_snapshot_of("https://example.org/\r\ncontinued");

    let link = link_at(&snapshot, at(0, 3)).unwrap();

    assert_eq!(link.target, "https://example.org/");
    assert_eq!(link_at(&snapshot, at(1, 2)), None);
}
