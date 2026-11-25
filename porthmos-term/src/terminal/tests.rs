use super::*;
use crate::{CellWidth, Color, SelectionKind, Side, Style, TermSize, ViewPoint};

fn terminal(columns: u16, rows: u16) -> Terminal {
    Terminal::new(TermSize::clamped(columns, rows), TerminalConfig::default())
}

struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 24) as u8
    }
}

#[test]
fn text_lands_in_the_snapshot_cells() {
    let mut terminal = terminal(80, 24);

    terminal.feed(b"hi");

    assert_eq!(terminal.snapshot().row_text(0), "hi");
}

#[test]
fn colours_map_to_default_indexed_and_rgb() {
    let mut terminal = terminal(80, 24);

    terminal.feed(b"\x1b[31mr\x1b[0m \x1b[1;4;38;2;1;2;3mt\x1b[0m \x1b[48;5;200mi");

    let snapshot = terminal.snapshot();
    assert_eq!(snapshot.cells[0].foreground, Color::Indexed(1));
    assert_eq!(snapshot.cells[1].foreground, Color::Default);
    assert_eq!(snapshot.cells[2].foreground, Color::Rgb(1, 2, 3));
    assert!(snapshot.cells[2].style.contains(Style::BOLD | Style::UNDERLINE));
    assert_eq!(snapshot.cells[4].background, Color::Indexed(200));
}

#[test]
fn wide_characters_take_a_wide_cell_then_a_spacer() {
    let mut terminal = terminal(80, 24);

    terminal.feed("日😀".as_bytes());

    let widths: Vec<CellWidth> = terminal.snapshot().cells[..4].iter().map(|cell| cell.width).collect();
    assert_eq!(widths, [CellWidth::Wide, CellWidth::Spacer, CellWidth::Wide, CellWidth::Spacer]);
}

#[test]
fn combining_marks_attach_to_the_previous_cell() {
    let mut terminal = terminal(80, 24);

    terminal.feed("e\u{301}".as_bytes());

    let cell = &terminal.snapshot().cells[0];
    assert_eq!(cell.character, 'e');
    assert_eq!(cell.combining.as_deref(), Some(&['\u{301}'][..]));
}

#[test]
fn reflow_keeps_every_character_across_narrow_and_wide() {
    let line: String = (0..100).map(|index| char::from(b'0' + (index % 10) as u8)).collect();
    let mut terminal = terminal(80, 24);
    terminal.feed(line.as_bytes());

    terminal.resize(TermSize::clamped(40, 24));
    terminal.resize(TermSize::clamped(80, 24));

    let snapshot = terminal.snapshot();
    assert_eq!(snapshot.row_text(0), line[..80]);
    assert_eq!(snapshot.row_text(1), line[80..]);
}

#[test]
fn the_alternate_screen_restores_the_main_screen() {
    let mut terminal = terminal(80, 24);

    terminal.feed(b"main\x1b[?1049h\x1b[2J\x1b[Halt");
    let inside = terminal.snapshot();
    terminal.feed(b"\x1b[?1049l");
    let outside = terminal.snapshot();

    assert!(inside.modes.alternate_screen);
    assert_eq!(inside.row_text(0), "alt");
    assert!(!outside.modes.alternate_screen);
    assert_eq!(outside.row_text(0), "main");
}

#[test]
fn modes_follow_set_and_reset() {
    let mut terminal = terminal(80, 24);

    terminal.feed(b"\x1b[?2004h\x1b[?1000h\x1b[?1006h\x1b[?1h\x1b=");
    let on = terminal.snapshot().modes;
    terminal.feed(b"\x1b[?2004l\x1b[?1000l\x1b[?1006l\x1b[?1l\x1b>");
    let off = terminal.snapshot().modes;

    assert!(on.bracketed_paste && on.mouse_sgr && on.application_cursor && on.application_keypad);
    assert_eq!(on.mouse, MouseMode::Click);
    assert!(!off.bracketed_paste && !off.mouse_sgr && !off.application_cursor && !off.application_keypad);
    assert_eq!(off.mouse, MouseMode::Off);
}

#[test]
fn garbage_never_panics_and_the_grid_stays_consistent() {
    let mut random = Xorshift(0x2545_F491_4F6C_DD1D);
    let data: Vec<u8> = (0..1 << 20).map(|_| random.next()).collect();
    for chunk_size in [7, 4096] {
        let mut terminal = terminal(80, 24);
        for chunk in data.chunks(chunk_size) {
            terminal.feed(chunk);
        }
        assert_eq!(terminal.snapshot().cells.len(), 80 * 24);
    }
    let mut terminal = terminal(80, 24);
    for byte in &data[..1 << 16] {
        terminal.feed(std::slice::from_ref(byte));
    }

    terminal.feed(b"\x1b[0m\x1b[H\x1b[2Jok");

    assert_eq!(terminal.snapshot().row_text(0), "ok");
}

#[test]
fn sizes_are_clamped_and_never_panic() {
    assert_eq!(TermSize::clamped(0, 0), TermSize { columns: 1, rows: 1 });
    let mut terminal = Terminal::new(TermSize::clamped(0, 0), TerminalConfig::default());
    terminal.feed(b"abc\r\ndef");
    assert_eq!(terminal.snapshot().cells.len(), 1);

    terminal.resize(TermSize::clamped(0, 0));
    terminal.resize(TermSize::clamped(65535, 65535));

    let snapshot = terminal.snapshot();
    assert_eq!(terminal.size(), TermSize { columns: 512, rows: 256 });
    assert_eq!(snapshot.cells.len(), 512 * 256);
}

fn numbered_lines(terminal: &mut Terminal, count: usize) {
    for index in 0..count {
        terminal.feed(format!("line {index}\r\n").as_bytes());
    }
}

#[test]
fn selection_extracts_text_across_rows() {
    let mut terminal = terminal(80, 24);
    terminal.feed(b"hello world\r\nsecond line");

    terminal.start_selection(SelectionKind::Simple, ViewPoint { row: 0, column: 6 }, Side::Left);
    terminal.update_selection(ViewPoint { row: 1, column: 5 }, Side::Right);

    assert_eq!(terminal.selected_text().as_deref(), Some("world\nsecond"));
}

#[test]
fn words_and_lines_selection_expand() {
    let mut terminal = terminal(80, 24);
    terminal.feed(b"foo bar baz\r\nnext");

    terminal.start_selection(SelectionKind::Words, ViewPoint { row: 0, column: 5 }, Side::Left);
    assert_eq!(terminal.selected_text().as_deref(), Some("bar"));

    terminal.start_selection(SelectionKind::Lines, ViewPoint { row: 0, column: 2 }, Side::Left);
    assert_eq!(terminal.selected_text().map(|text| text.trim_end().to_string()).as_deref(), Some("foo bar baz"));
}

#[test]
fn selected_cells_are_flagged_in_the_snapshot() {
    let mut terminal = terminal(80, 24);
    terminal.feed(b"hello world");

    terminal.start_selection(SelectionKind::Simple, ViewPoint { row: 0, column: 6 }, Side::Left);
    terminal.update_selection(ViewPoint { row: 0, column: 10 }, Side::Right);
    let selected: Vec<bool> = terminal.snapshot().cells[..12].iter().map(|cell| cell.selected).collect();
    terminal.clear_selection();

    assert_eq!(selected, [false, false, false, false, false, false, true, true, true, true, true, false]);
    assert!(terminal.snapshot().cells.iter().all(|cell| !cell.selected));
    assert_eq!(terminal.selected_text(), None);
}

#[test]
fn scrolling_clamps_to_history_and_returns_to_the_bottom() {
    let mut terminal = terminal(80, 24);
    numbered_lines(&mut terminal, 200);
    let history = terminal.snapshot().history;

    terminal.scroll(1000);
    let top = terminal.snapshot();
    terminal.scroll(-5000);
    let bottom = terminal.snapshot();
    terminal.scroll(10);
    terminal.scroll_to_bottom();

    assert_eq!(top.display_offset, history);
    assert_eq!(top.row_text(0), "line 0");
    assert_eq!(bottom.display_offset, 0);
    assert_eq!(terminal.snapshot().display_offset, 0);
}

#[test]
fn selection_with_resize_and_evicted_history_does_not_panic() {
    let mut terminal =
        Terminal::new(TermSize::clamped(80, 24), TerminalConfig { scrollback: 50, ..TerminalConfig::default() });
    terminal.feed(b"first line");
    terminal.start_selection(SelectionKind::Simple, ViewPoint { row: 0, column: 0 }, Side::Left);
    terminal.update_selection(ViewPoint { row: 5, column: 3 }, Side::Right);

    numbered_lines(&mut terminal, 500);
    terminal.resize(TermSize::clamped(60, 10));
    let _ = terminal.selected_text();

    assert_eq!(terminal.snapshot().cells.len(), 600);
}

#[test]
fn modes_match_the_snapshot_without_copying_the_screen() {
    let mut terminal = terminal(80, 24);
    assert_eq!(terminal.modes(), terminal.snapshot().modes);

    terminal.feed(b"\x1b[?1h\x1b[?2004h\x1b[?1049h");

    let modes = terminal.modes();
    assert!(modes.application_cursor && modes.bracketed_paste && modes.alternate_screen);
    assert_eq!(modes, terminal.snapshot().modes);
}

#[test]
fn select_match_highlights_the_found_text() {
    let mut terminal = terminal(40, 5);
    terminal.feed(b"alpha beta gamma\r\n");

    let found = terminal.search("beta", GridPoint { line: 0, column: 0 }, SearchDirection::Forward).unwrap();
    terminal.select_match(found);

    assert_eq!(terminal.selection_match(), Some(found));
    assert_eq!(terminal.selected_text(), Some("beta".to_string()));
    assert!(terminal.snapshot().cells.iter().any(|cell| cell.selected));
}

#[test]
fn select_match_scrolls_a_match_in_the_history_into_view() {
    let mut terminal = terminal(20, 5);
    for number in 0..100 {
        terminal.feed(format!("row {number:03}\r\n").as_bytes());
    }

    let found = terminal.search("row 003", GridPoint { line: -90, column: 0 }, SearchDirection::Forward).unwrap();
    terminal.select_match(found);

    let snapshot = terminal.snapshot();
    assert!(snapshot.display_offset > 0);
    assert!(snapshot.cells.iter().any(|cell| cell.selected));
    assert_eq!(terminal.selected_text(), Some("row 003".to_string()));
}

#[test]
fn selection_match_is_none_without_a_selection_and_after_clearing() {
    let mut terminal = terminal(20, 5);
    terminal.feed(b"hello\r\n");
    assert_eq!(terminal.selection_match(), None);

    terminal.start_selection(SelectionKind::Simple, ViewPoint { row: 0, column: 0 }, Side::Left);
    terminal.update_selection(ViewPoint { row: 0, column: 3 }, Side::Right);
    let range = terminal.selection_match().unwrap();
    assert_eq!((range.start, range.end), (GridPoint { line: 0, column: 0 }, GridPoint { line: 0, column: 3 }));

    terminal.clear_selection();
    assert_eq!(terminal.selection_match(), None);
}

#[test]
fn the_selection_match_follows_the_text_when_output_scrolls_it() {
    let mut terminal = terminal(20, 5);
    terminal.feed(b"alpha beta\r\n");
    let found = terminal.search("beta", GridPoint { line: 0, column: 0 }, SearchDirection::Forward).unwrap();
    terminal.select_match(found);

    terminal.feed(b"x\r\ny\r\nz\r\nw\r\nv\r\n");

    let moved = terminal.selection_match().unwrap();
    assert_eq!(moved.start.line, found.start.line - 2);
    assert_eq!(terminal.selected_text(), Some("beta".to_string()));
}

#[test]
fn display_offset_and_history_report_the_scroll_state_without_copying_the_screen() {
    let mut terminal = terminal(20, 5);
    assert_eq!((terminal.display_offset(), terminal.history()), (0, 0));
    for number in 0..30 {
        terminal.feed(format!("row {number}\r\n").as_bytes());
    }
    terminal.scroll(4);

    assert_eq!(terminal.display_offset(), 4);
    assert_eq!(terminal.history(), terminal.snapshot().history);
    assert!(terminal.history() >= 25);
}

#[test]
fn a_row_that_overflowed_into_the_next_is_marked_wrapped() {
    let mut terminal = terminal(10, 4);

    terminal.feed(b"0123456789abc\r\nnext");

    let snapshot = terminal.snapshot();
    assert_eq!(snapshot.wrapped, vec![true, false, false, false]);
}

#[test]
fn a_row_that_ends_exactly_at_the_edge_without_overflow_is_not_wrapped() {
    let mut terminal = terminal(10, 3);

    terminal.feed(b"0123456789\r\nnext");

    assert_eq!(terminal.snapshot().wrapped, vec![false, false, false]);
}
