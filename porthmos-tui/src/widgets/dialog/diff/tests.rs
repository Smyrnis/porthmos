use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::diff::{DiffKind, DiffLine, DiffOutcome};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn line(kind: DiffKind, text: &str) -> DiffLine {
    DiffLine { kind, text: text.to_string() }
}

fn dialog_with(lines: Vec<DiffLine>) -> DiffDialog {
    DiffDialog::new("a.txt", &DiffOutcome::Changes(lines))
}

fn many_lines(count: usize) -> DiffDialog {
    dialog_with((0..count).map(|number| line(DiffKind::Context, &format!("row {number}"))).collect())
}

fn draw(dialog: &DiffDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_diff(frame, frame.area(), dialog, &theme)).unwrap();
    terminal
}

#[test]
fn u_m_c_choose_and_esc_or_q_go_back() {
    let mut dialog = many_lines(3);

    assert_eq!(dialog.handle_key(key(KeyCode::Char('u'))), Some(DiffChoice::Overwrite));
    assert_eq!(dialog.handle_key(key(KeyCode::Char('m'))), Some(DiffChoice::KeepCopy));
    assert_eq!(dialog.handle_key(key(KeyCode::Char('c'))), Some(DiffChoice::Cancel));
    assert_eq!(dialog.handle_key(key(KeyCode::Esc)), Some(DiffChoice::Back));
    assert_eq!(dialog.handle_key(key(KeyCode::Char('q'))), Some(DiffChoice::Back));
    assert_eq!(dialog.handle_key(key(KeyCode::Char('x'))), None);
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), None);
}

#[test]
fn jk_arrows_page_keys_g_and_capital_g_scroll_and_stop_at_the_ends() {
    let mut dialog = many_lines(100);
    draw(&dialog, 80, 20);
    let page = dialog.page.get();
    assert!(page > 1 && page < 20, "{page}");

    assert_eq!(dialog.handle_key(key(KeyCode::Char('k'))), None);
    assert_eq!(dialog.scroll, 0);
    dialog.handle_key(key(KeyCode::Char('j')));
    dialog.handle_key(key(KeyCode::Down));
    assert_eq!(dialog.scroll, 2);
    dialog.handle_key(key(KeyCode::Char('k')));
    dialog.handle_key(key(KeyCode::Up));
    assert_eq!(dialog.scroll, 0);
    dialog.handle_key(key(KeyCode::PageDown));
    assert_eq!(dialog.scroll, page);
    dialog.handle_key(key(KeyCode::PageUp));
    assert_eq!(dialog.scroll, 0);
    dialog.handle_key(key(KeyCode::Char('G')));
    assert_eq!(dialog.scroll, 100 - page);
    dialog.handle_key(key(KeyCode::Char('j')));
    dialog.handle_key(key(KeyCode::PageDown));
    assert_eq!(dialog.scroll, 100 - page);
    dialog.handle_key(key(KeyCode::Char('g')));
    assert_eq!(dialog.scroll, 0);
    dialog.handle_key(key(KeyCode::End));
    assert_eq!(dialog.scroll, 100 - page);
    dialog.handle_key(key(KeyCode::Home));
    assert_eq!(dialog.scroll, 0);
}

#[test]
fn a_short_diff_never_scrolls() {
    let mut dialog = many_lines(2);
    draw(&dialog, 80, 20);

    dialog.handle_key(key(KeyCode::Char('j')));
    dialog.handle_key(key(KeyCode::Char('G')));

    assert_eq!(dialog.scroll, 0);
}

#[test]
fn removed_rows_start_with_minus_in_the_error_colour_and_added_rows_with_plus_in_the_ok_colour() {
    let dialog = dialog_with(vec![
        line(DiffKind::Context, "same"),
        line(DiffKind::Removed, "theirs"),
        line(DiffKind::Added, "mine"),
    ]);
    let theme = indexed();

    let terminal = draw(&dialog, 80, 20);

    let (x, y) = find_text(&terminal, "- theirs").expect("a removed row");
    assert_eq!(cell_fg(&terminal, x, y), theme.err_text().fg.unwrap());
    let (x, y) = find_text(&terminal, "+ mine").expect("an added row");
    assert_eq!(cell_fg(&terminal, x, y), theme.ok_text().fg.unwrap());
    let (x, y) = find_text(&terminal, "  same").expect("a context row");
    assert_eq!(cell_fg(&terminal, x, y), theme.text().fg.unwrap());
}

#[test]
fn skipped_rows_read_a_count_of_unchanged_lines_in_the_dim_colour() {
    let dialog = dialog_with(vec![line(DiffKind::Skipped(12), ""), line(DiffKind::Added, "mine")]);
    let theme = indexed();

    let terminal = draw(&dialog, 80, 20);

    let (x, y) = find_text(&terminal, "12 unchanged lines").expect("a skipped row");
    assert_eq!(cell_fg(&terminal, x, y), theme.dim().fg.unwrap());
}

#[test]
fn one_skipped_line_is_singular() {
    let dialog = dialog_with(vec![line(DiffKind::Skipped(1), ""), line(DiffKind::Added, "mine")]);

    let terminal = draw(&dialog, 80, 20);

    assert!(
        find_text(&terminal, "1 unchanged line ").is_some()
            || find_text(&terminal, "1 unchanged line\u{2026}").is_some()
    );
    assert!(find_text(&terminal, "1 unchanged lines").is_none());
}

#[test]
fn a_note_outcome_shows_one_sentence_and_the_same_hint() {
    let dialog = DiffDialog::new("a.txt", &DiffOutcome::RemoteGone);

    let terminal = draw(&dialog, 80, 20);

    assert!(find_text(&terminal, "The file no longer exists on the server.").is_some());
    assert!(find_text(&terminal, "overwrite").is_some());
    assert!(find_text(&terminal, "keep copy").is_some());
    assert!(find_text(&terminal, "esc back").is_some());
}

#[test]
fn a_note_outcome_with_no_rows_never_scrolls() {
    let mut dialog = DiffDialog::new("a.txt", &DiffOutcome::NotText);
    draw(&dialog, 80, 20);

    dialog.handle_key(key(KeyCode::Char('j')));

    assert_eq!(dialog.scroll, 0);
}

#[test]
fn no_control_character_is_ever_rendered() {
    let dialog = dialog_with(vec![
        line(DiffKind::Removed, "bad \u{1b}[2J line"),
        line(DiffKind::Added, "bell \u{7} and \u{202e} bidi"),
    ]);

    let terminal = draw(&dialog, 80, 20);

    assert!(!has_unprintable_cell(&terminal));
}

#[test]
fn long_lines_are_clipped_to_the_width() {
    let dialog = dialog_with(vec![line(DiffKind::Added, &"x".repeat(500))]);

    let terminal = draw(&dialog, 60, 20);

    let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
    assert!(!screen.contains(&"x".repeat(100)));
    assert!(screen.contains("xxxx"));
}

#[test]
fn the_title_names_the_file() {
    let dialog = many_lines(1);

    let terminal = draw(&dialog, 80, 20);

    assert!(find_text(&terminal, "a.txt").is_some());
}

#[test]
fn a_tiny_terminal_does_not_panic() {
    let dialog = many_lines(50);

    for (width, height) in [(1, 1), (10, 3), (30, 5), (200, 2)] {
        draw(&dialog, width, height);
    }
}

#[test]
fn keys_with_control_or_alt_never_choose_or_scroll() {
    let mut dialog = many_lines(100);
    draw(&dialog, 80, 20);

    for letter in ['u', 'm', 'c', 'q', 'j', 'G'] {
        for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            assert_eq!(dialog.handle_key(KeyEvent::new(KeyCode::Char(letter), modifier)), None, "{letter}");
        }
    }
    assert_eq!(dialog.scroll, 0);
}

#[test]
fn a_clipped_line_ends_with_an_ellipsis() {
    let dialog = dialog_with(vec![line(DiffKind::Added, &"x".repeat(500))]);

    let terminal = draw(&dialog, 60, 20);

    let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
    assert!(screen.contains("x\u{2026}"), "{screen}");
}

#[test]
fn a_line_that_fits_has_no_ellipsis() {
    let dialog = dialog_with(vec![line(DiffKind::Added, "short")]);

    let terminal = draw(&dialog, 60, 20);

    let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
    assert!(!screen.contains('\u{2026}'), "{screen}");
}
