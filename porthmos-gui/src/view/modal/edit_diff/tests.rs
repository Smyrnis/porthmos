use iced::Color;
use porthmos_core::diff::{DiffKind, DiffLine, DiffOutcome};

use super::*;
use crate::theme::{Palette, ThemeSettings};

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

fn line(kind: DiffKind, text: &str) -> DiffLine {
    DiffLine { kind, text: text.to_string() }
}

fn changes() -> DiffOutcome {
    DiffOutcome::Changes(vec![
        line(DiffKind::Context, "same"),
        line(DiffKind::Removed, "theirs"),
        line(DiffKind::Added, "mine"),
        line(DiffKind::Skipped(12), ""),
    ])
}

#[test]
fn changes_produce_one_row_per_diff_line_with_its_marker() {
    let rows = rows(&changes());

    let texts: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(texts, ["  same", "- theirs", "+ mine", "\u{2026} 12 unchanged lines \u{2026}"]);
    let kinds: Vec<Option<DiffKind>> = rows.iter().map(|row| row.kind).collect();
    assert_eq!(
        kinds,
        [Some(DiffKind::Context), Some(DiffKind::Removed), Some(DiffKind::Added), Some(DiffKind::Skipped(12))]
    );
}

#[test]
fn a_non_changes_outcome_shows_its_note_as_the_only_row() {
    let rows = rows(&DiffOutcome::RemoteGone);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "The file no longer exists on the server.");
    assert_eq!(rows[0].kind, None);
}

#[test]
fn removed_and_added_rows_are_tinted_and_the_others_are_not() {
    let palette = palette();

    let removed = row_background(Some(DiffKind::Removed), &palette).expect("a tint");
    let added = row_background(Some(DiffKind::Added), &palette).expect("a tint");

    assert_ne!(removed, added);
    assert_ne!(removed, palette.surface);
    assert_ne!(added, palette.surface);
    assert_eq!(row_background(Some(DiffKind::Context), &palette), None);
    assert_eq!(row_background(Some(DiffKind::Skipped(3)), &palette), None);
    assert_eq!(row_background(None, &palette), None);
}

#[test]
fn text_colours_follow_the_kind() {
    let palette = palette();

    assert_eq!(row_color(Some(DiffKind::Removed), &palette), palette.err);
    assert_eq!(row_color(Some(DiffKind::Added), &palette), palette.ok);
    assert_eq!(row_color(Some(DiffKind::Context), &palette), palette.fg);
    assert_eq!(row_color(Some(DiffKind::Skipped(2)), &palette), palette.fg_dim);
    assert_eq!(row_color(None, &palette), palette.fg);
}

#[test]
fn the_tint_stays_close_to_the_dialog_surface() {
    let palette = palette();

    let Color { r, g, b, .. } = row_background(Some(DiffKind::Removed), &palette).unwrap();

    for (tinted, plain) in [(r, palette.surface.r), (g, palette.surface.g), (b, palette.surface.b)] {
        assert!((tinted - plain).abs() < 0.35, "{tinted} vs {plain}");
    }
}

#[test]
fn the_body_builds_for_every_outcome() {
    let palette = palette();
    let outcomes = [
        changes(),
        DiffOutcome::Changes(Vec::new()),
        DiffOutcome::OnlyLineEndings,
        DiffOutcome::Identical,
        DiffOutcome::NotText,
        DiffOutcome::TooLarge { limit: 1024 * 1024 },
        DiffOutcome::TooManyChanges,
        DiffOutcome::RemoteGone,
    ];

    for outcome in outcomes {
        let _ = body(&outcome, &palette);
    }
}

#[test]
fn a_failure_message_reads_on_one_line_without_question_marks() {
    let text = failed_text("Comparison failed: a.txt:\nsession disconnected");

    assert_eq!(text, "Comparison failed: a.txt: session disconnected");
    assert!(!text.contains('?'));
}

#[test]
fn a_failure_message_never_carries_control_characters() {
    let text = failed_text("bad \u{1b}[2J name\u{7}");

    assert!(!text.chars().any(char::is_control), "{text:?}");
}
