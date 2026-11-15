use super::*;

fn compute(remote: &[u8], mine: &[u8]) -> DiffOutcome {
    super::compute(remote.to_vec(), mine.to_vec())
}

fn changes(outcome: DiffOutcome) -> Vec<(DiffKind, String)> {
    match outcome {
        DiffOutcome::Changes(lines) => lines.into_iter().map(|line| (line.kind, line.text)).collect(),
        other => panic!("expected changes, got {other:?}"),
    }
}

fn row(kind: DiffKind, text: &str) -> (DiffKind, String) {
    (kind, text.to_string())
}

fn numbered(count: usize) -> String {
    (1..=count).map(|number| format!("{number}\n")).collect()
}

fn with_line_replaced(text: &str, number: usize, replacement: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(index, line)| if index + 1 == number { format!("{replacement}\n") } else { format!("{line}\n") })
        .collect()
}

#[test]
fn identical_bytes_are_identical() {
    assert_eq!(compute(b"a\nb\n", b"a\nb\n"), DiffOutcome::Identical);
}

#[test]
fn both_empty_is_identical() {
    assert_eq!(compute(b"", b""), DiffOutcome::Identical);
}

#[test]
fn empty_against_text_is_changes_with_only_added_lines() {
    assert_eq!(changes(compute(b"", b"x\n")), vec![row(DiffKind::Added, "x")]);
}

#[test]
fn text_against_empty_is_changes_with_only_removed_lines() {
    assert_eq!(changes(compute(b"x\n", b"")), vec![row(DiffKind::Removed, "x")]);
}

#[test]
fn one_changed_line_shows_removed_then_added_with_context() {
    let remote = numbered(8);
    let mine = with_line_replaced(&remote, 4, "four");

    let rows = changes(compute(remote.as_bytes(), mine.as_bytes()));

    assert_eq!(
        rows,
        vec![
            row(DiffKind::Context, "1"),
            row(DiffKind::Context, "2"),
            row(DiffKind::Context, "3"),
            row(DiffKind::Removed, "4"),
            row(DiffKind::Added, "four"),
            row(DiffKind::Context, "5"),
            row(DiffKind::Context, "6"),
            row(DiffKind::Context, "7"),
            row(DiffKind::Skipped(1), ""),
        ]
    );
}

#[test]
fn unchanged_runs_between_hunks_collapse_to_skipped() {
    let remote = numbered(30);
    let mine = with_line_replaced(&with_line_replaced(&remote, 2, "two"), 29, "twenty-nine");

    let rows = changes(compute(remote.as_bytes(), mine.as_bytes()));

    let skipped: Vec<&DiffKind> =
        rows.iter().map(|(kind, _)| kind).filter(|kind| matches!(kind, DiffKind::Skipped(_))).collect();
    assert_eq!(skipped, vec![&DiffKind::Skipped(20)]);
    assert_eq!(rows[0], row(DiffKind::Context, "1"));
    assert!(rows.contains(&row(DiffKind::Removed, "29")));
    assert!(rows.contains(&row(DiffKind::Added, "twenty-nine")));
}

#[test]
fn hidden_lines_before_the_first_change_are_counted_in_a_leading_skipped_row() {
    let remote = numbered(20);
    let mine = with_line_replaced(&remote, 15, "fifteen");

    let rows = changes(compute(remote.as_bytes(), mine.as_bytes()));

    assert_eq!(rows[0], row(DiffKind::Skipped(11), ""));
}

#[test]
fn lines_added_at_the_start_and_at_the_end_are_found() {
    let rows = changes(compute(b"b\n", b"a\nb\nc\n"));

    assert_eq!(rows, vec![row(DiffKind::Added, "a"), row(DiffKind::Context, "b"), row(DiffKind::Added, "c")]);
}

#[test]
fn crlf_against_lf_only_is_only_line_endings() {
    assert_eq!(compute(b"a\r\nb\r\n", b"a\nb\n"), DiffOutcome::OnlyLineEndings);
}

#[test]
fn a_missing_final_newline_only_is_only_line_endings() {
    assert_eq!(compute(b"a\nb", b"a\nb\n"), DiffOutcome::OnlyLineEndings);
}

#[test]
fn crlf_files_with_a_real_change_show_just_that_line() {
    let rows = changes(compute(b"a\r\nb\r\nc\r\n", b"a\r\nB\r\nc\r\n"));

    assert_eq!(
        rows,
        vec![
            row(DiffKind::Context, "a"),
            row(DiffKind::Removed, "b"),
            row(DiffKind::Added, "B"),
            row(DiffKind::Context, "c"),
        ]
    );
}

#[test]
fn a_nul_byte_on_either_side_is_not_text() {
    assert_eq!(compute(b"a\0b", b"a\n"), DiffOutcome::NotText);
    assert_eq!(compute(b"a\n", b"a\0b"), DiffOutcome::NotText);
}

#[test]
fn invalid_utf8_on_either_side_is_not_text() {
    assert_eq!(compute(&[0xff, 0xfe, b'\n'], b"a\n"), DiffOutcome::NotText);
    assert_eq!(compute(b"a\n", &[0xff, 0xfe, b'\n']), DiffOutcome::NotText);
}

#[test]
fn a_byte_order_mark_is_ignored() {
    assert_eq!(compute(b"\xef\xbb\xbfa\n", b"a\n"), DiffOutcome::Identical);
}

#[test]
fn tabs_become_four_spaces_and_other_control_characters_become_question_marks() {
    let rows = changes(compute(b"old\n", b"\tx\x1b[2Jy\x07\n"));

    assert_eq!(rows, vec![row(DiffKind::Removed, "old"), row(DiffKind::Added, "    x?[2Jy?")]);
}

#[test]
fn more_than_max_rows_of_changes_is_too_many_changes() {
    let remote: String = (0..6000).map(|number| format!("old {number}\n")).collect();
    let mine: String = (0..6000).map(|number| format!("new {number}\n")).collect();

    assert_eq!(compute(remote.as_bytes(), mine.as_bytes()), DiffOutcome::TooManyChanges);
}

#[test]
fn a_hundred_thousand_changed_lines_finish_with_too_many_changes() {
    let remote: String = (0..100_000).map(|number| format!("old {number}\n")).collect();
    let mine: String = (0..100_000).map(|number| format!("new {number}\n")).collect();

    assert_eq!(compute(remote.as_bytes(), mine.as_bytes()), DiffOutcome::TooManyChanges);
}

#[test]
fn exactly_max_rows_of_changes_is_still_shown() {
    let half = MAX_DIFF_ROWS / 2;
    let remote: String = (0..half).map(|number| format!("old {number}\n")).collect();
    let mine: String = (0..half).map(|number| format!("new {number}\n")).collect();

    assert_eq!(changes(compute(remote.as_bytes(), mine.as_bytes())).len(), MAX_DIFF_ROWS);
}

#[test]
fn marked_text_prefixes_each_kind_and_counts_skipped_lines() {
    let marked = |kind, text: &str| DiffLine { kind, text: text.to_string() }.marked_text();

    assert_eq!(marked(DiffKind::Context, "same"), "  same");
    assert_eq!(marked(DiffKind::Removed, "theirs"), "- theirs");
    assert_eq!(marked(DiffKind::Added, "mine"), "+ mine");
    assert_eq!(marked(DiffKind::Skipped(1), ""), "\u{2026} 1 unchanged line \u{2026}");
    assert_eq!(marked(DiffKind::Skipped(12), ""), "\u{2026} 12 unchanged lines \u{2026}");
}

#[test]
fn note_is_none_for_changes_and_the_exact_sentence_for_every_other_outcome() {
    assert_eq!(DiffOutcome::Changes(Vec::new()).note(), None);
    assert_eq!(DiffOutcome::OnlyLineEndings.note().as_deref(), Some("Only line endings differ."));
    assert_eq!(DiffOutcome::Identical.note().as_deref(), Some("The server's file is now the same as yours."));
    assert_eq!(DiffOutcome::NotText.note().as_deref(), Some("Not a text file, so no comparison is shown."));
    assert_eq!(
        DiffOutcome::TooLarge { limit: 8 * 1024 * 1024 }.note().as_deref(),
        Some("Too large to compare (over 8 MiB). The limit is [edit] compare_limit_mib.")
    );
    assert_eq!(DiffOutcome::TooManyChanges.note().as_deref(), Some("Too many changes to show."));
    assert_eq!(DiffOutcome::RemoteGone.note().as_deref(), Some("The file no longer exists on the server."));
}

#[test]
fn bidi_and_zero_width_characters_are_replaced_so_a_line_cannot_be_reordered() {
    let hostile = "a\u{202e}b\u{2066}c\u{200f}d\u{061c}e\u{200b}f\u{feff}g";

    let rows = changes(compute(b"x\n", format!("{hostile}\n").as_bytes()));

    assert_eq!(rows[1], row(DiffKind::Added, "a?b?c?d?e?f?g"));
}
