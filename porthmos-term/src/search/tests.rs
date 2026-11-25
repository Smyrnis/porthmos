use crate::{GridPoint, Match, SearchDirection, TermSize, Terminal, TerminalConfig};

fn terminal_with(text: &str) -> Terminal {
    let mut terminal = Terminal::new(TermSize::clamped(80, 24), TerminalConfig::default());
    terminal.feed(text.as_bytes());
    terminal
}

fn at(line: i32, column: u16) -> GridPoint {
    GridPoint { line, column }
}

fn span(start: GridPoint, end: GridPoint) -> Option<Match> {
    Some(Match { start, end })
}

#[test]
fn search_finds_literal_text_in_both_directions() {
    let terminal = terminal_with("alpha beta\r\nGamma delta\r\nbeta again");

    assert_eq!(terminal.search("beta", at(0, 0), SearchDirection::Forward), span(at(0, 6), at(0, 9)));
    assert_eq!(terminal.search("beta", at(0, 10), SearchDirection::Forward), span(at(2, 0), at(2, 3)));
    assert_eq!(terminal.search("beta", at(1, 0), SearchDirection::Backward), span(at(0, 6), at(0, 9)));
}

#[test]
fn regex_metacharacters_in_the_query_are_literal() {
    let terminal = terminal_with("a1b( x a.b(");

    assert_eq!(terminal.search("a.b(", at(0, 0), SearchDirection::Forward), span(at(0, 7), at(0, 10)));
}

#[test]
fn search_is_case_insensitive_unless_the_query_has_capitals() {
    let terminal = terminal_with("Gamma gamma");

    assert_eq!(terminal.search("gamma", at(0, 1), SearchDirection::Forward), span(at(0, 6), at(0, 10)));
    assert_eq!(terminal.search("Gamma", at(0, 1), SearchDirection::Forward), span(at(0, 0), at(0, 4)));
}

#[test]
fn an_empty_or_missing_query_finds_nothing() {
    let terminal = terminal_with("alpha");

    assert_eq!(terminal.search("", at(0, 0), SearchDirection::Forward), None);
    assert_eq!(terminal.search("zzz", at(0, 0), SearchDirection::Forward), None);
}

#[test]
fn reveal_scrolls_a_history_match_into_view() {
    let mut terminal = terminal_with("");
    for index in 0..200 {
        terminal.feed(format!("line {index}\r\n").as_bytes());
    }
    let history = terminal.snapshot().history as i32;

    let early = terminal.search("line 0", at(-history, 0), SearchDirection::Forward).unwrap();
    terminal.reveal(early);
    let revealed = terminal.snapshot();
    let late = terminal.search("line 199", at(0, 0), SearchDirection::Forward).unwrap();
    terminal.scroll_to_bottom();
    terminal.reveal(late);

    assert_eq!(revealed.row_text(0), "line 0");
    assert_eq!(terminal.snapshot().display_offset, 0);
}
