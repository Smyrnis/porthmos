use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(dialog: &MessageDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_message(frame, frame.area(), dialog, &theme)).unwrap();
    terminal
}

#[test]
fn enter_and_esc_close_and_other_keys_wait() {
    let mut dialog = MessageDialog::new("T", "M");
    assert_eq!(dialog.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)), MessageOutcome::Pending);
    assert_eq!(dialog.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), MessageOutcome::Closed);
    assert_eq!(dialog.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), MessageOutcome::Closed);
}

#[test]
fn it_renders_the_title_and_wrapped_message() {
    let dialog = MessageDialog::new("Missing hosts", "These labelled hosts are no longer in ~/.ssh/config: web1.");
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(50, 12)).unwrap();
    terminal.draw(|frame| render_message(frame, frame.area(), &dialog, &Theme::default())).unwrap();
    let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
    assert!(screen.contains("Missing hosts"), "{screen}");
    assert!(screen.contains("web1"), "{screen}");
}

#[test]
fn a_message_that_wraps_at_words_still_shows_the_ok_line() {
    let words = format!("{} ", "x".repeat(25)).repeat(6);
    let dialog = MessageDialog::new("T", words.trim());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(52, 30)).unwrap();
    terminal.draw(|frame| render_message(frame, frame.area(), &dialog, &Theme::default())).unwrap();
    let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
    assert!(screen.contains("enter close"), "{screen}");
}

#[test]
fn each_line_of_a_multi_line_message_gets_its_own_row() {
    let dialog = MessageDialog::new("Details", "first line\nsecond line\nthird line");
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 14)).unwrap();

    terminal.draw(|frame| render_message(frame, frame.area(), &dialog, &Theme::default())).unwrap();

    let buffer = terminal.backend().buffer();
    let rows: Vec<String> =
        (0..14).map(|y| (0..60).map(|x| buffer[(x, y)].symbol().to_string()).collect::<String>()).collect();
    let row_of = |needle: &str| rows.iter().position(|row| row.contains(needle)).unwrap();
    assert_eq!(row_of("second line"), row_of("first line") + 1);
    assert_eq!(row_of("third line"), row_of("first line") + 2);
    assert!(rows.iter().any(|row| row.contains("enter close")));
}

#[test]
fn the_title_is_muted_in_the_border_and_the_footer_says_enter_close() {
    let theme = indexed();
    let terminal = draw(&MessageDialog::new("Missing hosts", "web1 is gone"), 60, 14);

    let (x, y) = find_text(&terminal, "Missing hosts").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_muted);
    let (x, y) = find_text(&terminal, "enter close").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, x + 6, y), theme.fg_dim);
}

#[test]
fn long_text_wraps_inside_the_popup() {
    let dialog = MessageDialog::new("T", "word ".repeat(60).trim());

    let terminal = draw(&dialog, 60, 24);

    let words =
        crate::app::testing::screen_rows(&terminal).iter().map(|row| row.matches("word").count()).sum::<usize>();
    assert_eq!(words, 60);
}

#[test]
fn hostile_title_and_text_are_sanitised() {
    let dialog = MessageDialog::new("T\u{1b}[31m", "a\u{202e}b\nc\u{7}d");

    let terminal = draw(&dialog, 60, 14);

    assert!(!has_unprintable_cell(&terminal));
    assert!(find_text(&terminal, "a?b").is_some());
    assert!(find_text(&terminal, "c?d").is_some());
}

#[test]
fn tiny_screens_do_not_panic() {
    for width in 0..=12 {
        for height in 0..=4 {
            draw(&MessageDialog::new("Title", "some message text"), width, height);
        }
    }
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier() {
    assert!(!include_str!("mod.rs").contains("REVERSED"));
}
