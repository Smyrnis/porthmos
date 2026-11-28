use porthmos_highlight::{Highlighted, TokenKind};

use super::{colour_text, looks_like_log};

fn kinds(highlighted: &Highlighted) -> Vec<Vec<TokenKind>> {
    highlighted.lines.iter().map(|spans| spans.iter().map(|span| span.kind).collect()).collect()
}

#[test]
fn rotated_and_plain_log_names_look_like_logs() {
    for name in [
        "app.log",
        "APP.LOG",
        "app.log.1",
        "app.log.2026-11-28",
        "syslog",
        "messages",
        "auth.log",
        "kern.log",
        "journal",
    ] {
        assert!(looks_like_log(name), "{name}");
    }
}

#[test]
fn other_names_do_not_look_like_logs() {
    for name in ["catalog.txt", "log.rs", "blog", "logger.toml", "dialog", "main.rs", ""] {
        assert!(!looks_like_log(name), "{name}");
    }
}

#[test]
fn log_lines_are_coloured_by_level_over_the_whole_line() {
    let text = "ERROR boom\nok fine\nWARN slow\n";
    let highlighted = colour_text("app.log", text).unwrap();

    assert_eq!(kinds(&highlighted), [vec![TokenKind::Error], vec![TokenKind::Plain], vec![TokenKind::Warn]]);
    for (line, spans) in text.lines().zip(&highlighted.lines) {
        assert_eq!((spans[0].start, spans[0].end), (0, line.len()));
    }
}

#[test]
fn a_level_word_inside_a_longer_word_does_not_colour_the_line() {
    let highlighted = colour_text("app.log", "3 errors found\nwarnings: 0\n").unwrap();

    assert_eq!(kinds(&highlighted), [vec![TokenKind::Plain], vec![TokenKind::Plain]]);
}

#[test]
fn an_empty_log_has_no_lines_and_a_blank_log_line_has_no_span() {
    assert_eq!(colour_text("app.log", "").unwrap().lines.len(), 0);
    assert!(colour_text("app.log", "ERROR x\n\nok\n").unwrap().lines[1].is_empty());
}

#[test]
fn a_log_line_with_carriage_return_excludes_it_from_the_span() {
    let highlighted = colour_text("app.log", "ERROR x\r\n").unwrap();

    assert_eq!(highlighted.lines[0][0].end, 7);
}

#[test]
fn source_files_get_syntax_colours() {
    let highlighted = colour_text("main.rs", "fn x() {}\n").unwrap();

    assert!(highlighted.lines[0].iter().any(|span| span.kind == TokenKind::Keyword));
}

#[test]
fn a_log_name_wins_over_a_grammar() {
    let highlighted = colour_text("a.log", "ERROR fn x() {}\n").unwrap();

    assert_eq!(kinds(&highlighted), [vec![TokenKind::Error]]);
}

#[test]
fn an_unknown_text_file_has_no_colours() {
    assert!(colour_text("data.zzzzzz", "a\n").is_none());
    assert!(colour_text("", "a\n").is_none());
}
