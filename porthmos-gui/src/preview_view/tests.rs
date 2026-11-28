use std::sync::Arc;

use porthmos_core::preview::{Preview, PreviewContent};

use porthmos_core::{Highlighted, Span, TokenKind};

use super::{Piece, PreviewState, PreviewStatus, hex_row, text_rows, visible_window};
use porthmos_core::size_text;

fn text(content: &str) -> Arc<Preview> {
    Arc::new(Preview {
        size: Some(content.len() as u64),
        shown: content.len(),
        truncated: false,
        content: PreviewContent::Text(content.to_string()),
        colours: None,
    })
}

fn binary(bytes: Vec<u8>) -> Arc<Preview> {
    Arc::new(Preview {
        size: Some(bytes.len() as u64),
        shown: bytes.len(),
        truncated: false,
        content: PreviewContent::Binary(bytes),
        colours: None,
    })
}

fn joined(row: &[Piece]) -> String {
    row.iter().map(|piece| piece.text.as_str()).collect()
}

fn lines(text: &str) -> Vec<String> {
    text_rows(text, None).iter().map(|row| joined(row)).collect()
}

fn span(start: usize, end: usize, kind: TokenKind) -> Span {
    Span { start, end, kind }
}

fn coloured(text: &str, lines: Vec<Vec<Span>>) -> Vec<Vec<Piece>> {
    text_rows(text, Some(&Highlighted { lines }))
}

fn ready(preview: Arc<Preview>) -> PreviewState {
    let mut state = PreviewState::loading(1, "file".to_string());
    assert!(state.finish(1, Ok(preview)));
    state
}

#[test]
fn text_lines_split_and_strip_carriage_returns() {
    assert_eq!(lines("a\r\nb\nc"), ["a", "b", "c"]);
}

#[test]
fn a_trailing_newline_adds_no_empty_line() {
    assert_eq!(lines("a\nb\n"), ["a", "b"]);
    assert_eq!(lines("\n"), [""]);
}

#[test]
fn an_empty_text_has_no_lines() {
    assert!(lines("").is_empty());
}

#[test]
fn tabs_expand_to_four_column_stops() {
    assert_eq!(lines("\tx"), ["    x"]);
    assert_eq!(lines("ab\tx"), ["ab  x"]);
    assert_eq!(lines("abcd\tx"), ["abcd    x"]);
}

#[test]
fn control_characters_become_question_marks() {
    assert_eq!(lines("a\u{1b}[31mb\u{7}"), ["a?[31mb?"]);
}

#[test]
fn a_long_line_is_kept_whole() {
    let long = "x".repeat(5000);

    assert_eq!(lines(&long)[0].len(), 5000);
}

#[test]
fn coloured_pieces_cover_the_whole_line_in_order() {
    let rows = coloured("let x", vec![vec![span(0, 3, TokenKind::Keyword), span(3, 5, TokenKind::Plain)]]);

    assert_eq!(
        rows,
        [vec![
            Piece { text: "let".to_string(), kind: TokenKind::Keyword },
            Piece { text: " x".to_string(), kind: TokenKind::Plain },
        ]]
    );
}

#[test]
fn a_tab_after_a_coloured_piece_expands_to_the_same_column_as_before() {
    let rows = coloured("ab\tc", vec![vec![span(0, 2, TokenKind::Keyword), span(2, 4, TokenKind::Plain)]]);

    assert_eq!(joined(&rows[0]), lines("ab\tc")[0]);
    assert_eq!(joined(&rows[0]), "ab  c");
}

#[test]
fn a_gap_between_spans_becomes_a_plain_piece() {
    let rows = coloured("abcd", vec![vec![span(0, 1, TokenKind::Keyword), span(3, 4, TokenKind::String)]]);

    assert_eq!(
        rows[0].iter().map(|piece| (piece.text.as_str(), piece.kind)).collect::<Vec<_>>(),
        [("a", TokenKind::Keyword), ("bc", TokenKind::Plain), ("d", TokenKind::String),]
    );
}

#[test]
fn an_out_of_range_span_falls_back_to_one_plain_piece() {
    for bad in [span(0, 99, TokenKind::Keyword), span(3, 2, TokenKind::Keyword)] {
        let rows = coloured("abc", vec![vec![bad]]);

        assert_eq!(rows, [vec![Piece { text: "abc".to_string(), kind: TokenKind::Plain }]]);
    }
}

#[test]
fn a_span_inside_a_multibyte_char_falls_back_to_one_plain_piece() {
    let rows = coloured("é!", vec![vec![span(0, 1, TokenKind::Keyword), span(1, 3, TokenKind::Plain)]]);

    assert_eq!(rows, [vec![Piece { text: "é!".to_string(), kind: TokenKind::Plain }]]);
}

#[test]
fn a_crlf_line_has_no_carriage_return_in_any_piece() {
    let rows = coloured("x = 1\r\n", vec![vec![span(0, 5, TokenKind::Plain)]]);

    assert!(rows[0].iter().all(|piece| !piece.text.contains('\r')));
    assert_eq!(joined(&rows[0]), "x = 1");
}

#[test]
fn control_characters_inside_a_coloured_piece_become_question_marks() {
    let rows = coloured("a\u{7}b", vec![vec![span(0, 3, TokenKind::String)]]);

    assert_eq!(joined(&rows[0]), "a?b");
}

#[test]
fn colours_for_fewer_lines_than_the_text_leave_the_rest_plain() {
    let rows = coloured("a\nb\nc", vec![vec![span(0, 1, TokenKind::Keyword)]]);

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0].kind, TokenKind::Keyword);
    assert_eq!(rows[2], [Piece { text: "c".to_string(), kind: TokenKind::Plain }]);
}

#[test]
fn an_empty_line_is_one_empty_plain_piece() {
    assert_eq!(text_rows("a\n\nb", None)[1], [Piece { text: String::new(), kind: TokenKind::Plain }]);
}

#[test]
fn a_ready_preview_uses_its_colours_for_its_rows() {
    let preview = Arc::new(Preview {
        size: Some(5),
        shown: 5,
        truncated: false,
        content: PreviewContent::Text("let x".to_string()),
        colours: Some(Highlighted { lines: vec![vec![span(0, 3, TokenKind::Keyword), span(3, 5, TokenKind::Plain)]] }),
    });
    let state = ready(preview);

    assert_eq!(state.rows(0, 1)[0][0].kind, TokenKind::Keyword);
    assert_eq!(state.columns(), 5);
}

#[test]
fn columns_count_characters_across_pieces() {
    let state = ready(text("ab\tcd\nx"));

    assert_eq!(state.columns(), 6);
}

#[test]
fn hex_row_matches_the_terminal_layout() {
    let bytes: Vec<u8> = (0x41..0x51).collect();

    assert_eq!(hex_row(0, &bytes), "00000000  41 42 43 44 45 46 47 48  49 4a 4b 4c 4d 4e 4f 50  |ABCDEFGHIJKLMNOP|");
    let short = format!("00000010  00 7f 20 {} {} |.. |", "   ".repeat(5), "   ".repeat(8));
    assert_eq!(hex_row(16, &[0x00, 0x7f, 0x20]), short);
}

#[test]
fn hex_rows_cover_all_bytes() {
    let state = ready(binary(vec![1; 33]));

    assert_eq!(state.row_count(), 3);
    let rows = state.rows(0, 3);
    assert!(joined(&rows[0]).starts_with("00000000  "));
    assert!(joined(&rows[1]).starts_with("00000010  "));
    assert!(joined(&rows[2]).starts_with("00000020  01 "));
    assert!(rows.iter().flatten().all(|piece| piece.kind == TokenKind::Plain));
}

#[test]
fn info_text_for_text_binary_and_truncated() {
    assert_eq!(ready(text("hello")).info_text(), "text · 5 B");
    assert_eq!(ready(binary(vec![0; 2048])).info_text(), "binary · 2.0 KB");
    let truncated = Arc::new(Preview {
        size: Some(3 * 1024 * 1024),
        shown: 256 * 1024,
        truncated: true,
        content: PreviewContent::Text("x".to_string()),
        colours: None,
    });
    assert_eq!(ready(truncated).info_text(), "first 256.0 KB of 3.0 MB");
    let unknown = Arc::new(Preview {
        size: None,
        shown: 100,
        truncated: true,
        content: PreviewContent::Text("x".to_string()),
        colours: None,
    });
    assert_eq!(ready(unknown).info_text(), "first 100 B");
}

#[test]
fn size_text_below_and_above_a_kilobyte() {
    assert_eq!(size_text(0), "0 B");
    assert_eq!(size_text(1023), "1023 B");
    assert_eq!(size_text(1024), "1.0 KB");
}

#[test]
fn finish_ignores_a_stale_request_id() {
    let mut state = PreviewState::loading(2, "file".to_string());

    assert!(!state.finish(1, Ok(text("old"))));

    assert!(state.is_loading());
}

#[test]
fn finish_cannot_replace_a_finished_state() {
    let mut state = ready(text("first"));

    assert!(!state.finish(1, Err("late".into())));

    assert_eq!(state.row_count(), 1);
}

#[test]
fn failed_shows_no_info_and_no_rows() {
    let mut state = PreviewState::loading(1, "file".to_string());
    state.finish(1, Err("permission denied".into()));

    assert_eq!(state.status(), &PreviewStatus::Failed("permission denied".into()));
    assert_eq!(state.info_text(), "");
    assert_eq!(state.row_count(), 0);
    assert!(state.rows(0, 5).is_empty());
}

#[test]
fn loading_has_no_info_and_no_rows() {
    let state = PreviewState::loading(1, "file".to_string());

    assert_eq!(state.info_text(), "");
    assert_eq!(state.row_count(), 0);
    assert_eq!(state.request_id(), 1);
}

#[test]
fn a_binary_preview_reports_binary() {
    let state = ready(binary(vec![0, 1, 2]));

    assert!(state.is_binary());
    assert!(!ready(text("x")).is_binary());
}

#[test]
fn rows_returns_the_requested_slice_and_clamps() {
    let state = ready(text("a\nb\nc\nd"));

    assert_eq!(state.rows(1, 2).iter().map(|row| joined(row)).collect::<Vec<_>>(), ["b", "c"]);
    assert_eq!(state.rows(3, 10).iter().map(|row| joined(row)).collect::<Vec<_>>(), ["d"]);
    assert!(state.rows(9, 3).is_empty());
}

#[test]
fn visible_window_adds_overscan_and_clamps() {
    assert_eq!(visible_window(0.0, 100.0, 1000, 10.0), (0, 12));
    assert_eq!(visible_window(500.0, 100.0, 1000, 10.0), (48, 14));
    assert_eq!(visible_window(9950.0, 100.0, 1000, 10.0), (993, 7));
    assert_eq!(visible_window(0.0, 100.0, 0, 10.0), (0, 0));
    assert_eq!(visible_window(0.0, 100.0, 3, 10.0), (0, 3));
}

#[test]
fn title_makes_hostile_names_printable() {
    let state = PreviewState::loading(1, "a\u{1b}[2Jb\n".to_string());

    assert_eq!(state.title(), "a?[2Jb?");
}

#[test]
fn columns_is_the_widest_text_line_after_tab_expansion() {
    let state = ready(text("ab\n\tabcdef\nxyz"));

    assert_eq!(state.columns(), 10);
}

#[test]
fn columns_of_a_binary_preview_is_the_hex_row_width() {
    let state = ready(binary(vec![1; 40]));

    assert_eq!(state.columns(), hex_row(0, &[0; 16]).chars().count());
}

#[test]
fn columns_is_zero_until_the_preview_is_ready() {
    let mut state = PreviewState::loading(1, "file".to_string());
    assert_eq!(state.columns(), 0);

    state.finish(1, Err("denied".into()));

    assert_eq!(state.columns(), 0);
}
