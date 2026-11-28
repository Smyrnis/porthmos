use std::time::{Duration, Instant};

use super::{
    HIGHLIGHT_BUDGET, Limits, MAX_HIGHLIGHT_BYTES, MAX_HIGHLIGHT_LINE_BYTES, SyntaxId, highlight, highlight_limited,
    syntax_for,
};
use crate::{Highlighted, TokenKind, lines_of};

fn syntax(name: &str) -> SyntaxId {
    syntax_for(name, "").unwrap_or_else(|| panic!("no syntax for {name}"))
}

fn pieces<'a>(text: &'a str, highlighted: &Highlighted, kind: TokenKind) -> Vec<&'a str> {
    let mut found = Vec::new();
    for (line, spans) in lines_of(text).into_iter().zip(&highlighted.lines) {
        for span in spans.iter().filter(|span| span.kind == kind) {
            found.push(&line[span.start..span.end]);
        }
    }
    found
}

fn only_plain(highlighted: &Highlighted) -> bool {
    highlighted.lines.iter().flatten().all(|span| span.kind == TokenKind::Plain)
}

#[test]
fn the_caps_are_the_documented_values() {
    assert_eq!(MAX_HIGHLIGHT_BYTES, 256 * 1024);
    assert_eq!(MAX_HIGHLIGHT_LINE_BYTES, 4096);
    assert_eq!(HIGHLIGHT_BUDGET, Duration::from_millis(500));
}

#[test]
fn rust_source_gets_keywords_strings_and_comments() {
    let text = "fn main() { let s = \"hi\"; } // done\n";
    let highlighted = highlight(text, syntax("main.rs"));

    assert!(pieces(text, &highlighted, TokenKind::Keyword).contains(&"fn"));
    assert!(pieces(text, &highlighted, TokenKind::String).contains(&"\"hi\""));
    assert!(pieces(text, &highlighted, TokenKind::Comment).iter().any(|piece| piece.starts_with("//")));
}

#[test]
fn toml_json_yaml_and_shell_are_recognised() {
    for name in ["a.toml", "a.json", "a.yaml", "a.yml", "a.sh"] {
        assert!(syntax_for(name, "").is_some(), "{name}");
    }
    assert!(syntax_for("run", "#!/bin/sh").is_some());
}

#[test]
fn toml_values_are_coloured() {
    let text = "name = \"x\"\nn = 3\n";
    let highlighted = highlight(text, syntax("a.toml"));

    assert!(pieces(text, &highlighted, TokenKind::String).contains(&"\"x\""));
    assert!(pieces(text, &highlighted, TokenKind::Number).contains(&"3"));
}

#[test]
fn unknown_names_have_no_syntax() {
    assert!(syntax_for("data.zzzzzz", "").is_none());
    assert!(syntax_for("notes.txt", "").is_none());
    assert!(syntax_for("", "").is_none());
}

#[test]
fn spans_partition_every_line() {
    let samples = [
        ("a.rs", "fn é() { let s = \"😀 x\"; }\n// ünï\n\nlet t = 1;\n"),
        ("a.toml", "[a]\nk = \"é😀\"\nn = 1\n"),
        ("a.json", "{\"k\": [1, true, null, \"é\"]}\n"),
        ("a.yaml", "k: v\nl:\n  - 1\n  - \"é\"\n"),
        ("a.sh", "#!/bin/sh\necho \"é $HOME\" # c\n"),
    ];
    for (name, text) in samples {
        let highlighted = highlight(text, syntax(name));
        for (line, spans) in lines_of(text).into_iter().zip(&highlighted.lines) {
            let mut at = 0;
            for span in spans {
                assert_eq!(span.start, at, "{name}: {line:?}");
                assert!(span.end > span.start, "{name}: {line:?}");
                assert!(line.is_char_boundary(span.start) && line.is_char_boundary(span.end), "{name}: {line:?}");
                at = span.end;
            }
            assert_eq!(at, line.len(), "{name}: {line:?}");
        }
    }
}

#[test]
fn line_count_matches_lines_of() {
    for text in ["", "a", "a\n", "a\r\nb\r\n", "\n\n", "a\n\nb"] {
        assert_eq!(highlight(text, syntax("a.rs")).lines.len(), lines_of(text).len(), "{text:?}");
    }
}

#[test]
fn crlf_is_not_part_of_any_span() {
    let highlighted = highlight("x = 1\r\n", syntax("a.toml"));

    assert_eq!(highlighted.lines.len(), 1);
    assert_eq!(highlighted.lines[0].last().map(|span| span.end), Some(5));
}

#[test]
fn an_empty_line_has_no_spans() {
    let highlighted = highlight("a = 1\n\nb = 2\n", syntax("a.toml"));

    assert!(highlighted.lines[1].is_empty());
}

#[test]
fn a_line_over_the_cap_is_plain_and_the_next_line_still_colours() {
    let long = format!("{{\"k\":\"{}\"}}", "a".repeat(MAX_HIGHLIGHT_LINE_BYTES + 900));
    let text = format!("{long}\n{{\"a\": 1}}\n");
    let highlighted = highlight(&text, syntax("a.json"));

    assert_eq!(highlighted.lines[0].len(), 1);
    assert_eq!(highlighted.lines[0][0].kind, TokenKind::Plain);
    assert_eq!(highlighted.lines[0][0].end, long.len());
    assert!(pieces(&text, &highlighted, TokenKind::Number).contains(&"1"));
}

#[test]
fn a_minified_200_kib_line_returns_quickly_and_plain() {
    let text = format!("[{}1]\n", "1,".repeat(100 * 1024));
    let started = Instant::now();
    let highlighted = highlight(&text, syntax("a.json"));

    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(highlighted.lines.len(), 1);
    assert!(only_plain(&highlighted));
}

#[test]
fn text_past_the_byte_cap_is_plain() {
    let text = "let a = 1;\n".repeat(30);
    let limits = Limits { bytes: 100, ..Limits::DEFAULT };
    let highlighted = highlight_limited(&text, syntax("a.rs"), limits);

    assert_eq!(highlighted.lines.len(), 30);
    assert!(highlighted.lines[0].iter().any(|span| span.kind == TokenKind::Keyword));
    let last = &highlighted.lines[29];
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].kind, TokenKind::Plain);
}

#[test]
fn an_exhausted_budget_leaves_everything_plain() {
    let text = "fn a() {}\nlet b = 1;\n";
    let limits = Limits { budget: Duration::ZERO, ..Limits::DEFAULT };
    let highlighted = highlight_limited(text, syntax("a.rs"), limits);

    assert_eq!(highlighted.lines.len(), 2);
    assert!(only_plain(&highlighted));
}

#[test]
fn highlighting_twice_gives_the_same_result() {
    let text = "fn a() { 1 }\n";
    let id = syntax("a.rs");

    assert_eq!(highlight(text, id), highlight(text, id));
}
