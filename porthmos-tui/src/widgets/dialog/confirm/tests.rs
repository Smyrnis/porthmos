use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(dialog: &ConfirmDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_confirm(frame, frame.area(), dialog, &theme)).unwrap();
    terminal
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

#[test]
fn default_focus_is_no() {
    let dialog = ConfirmDialog::new("Delete?");
    assert_eq!(dialog.focus, ConfirmFocus::No);
}

#[test]
fn tab_toggles_focus_between_yes_and_no() {
    let mut dialog = ConfirmDialog::new("Delete?");
    dialog.handle_key(key(KeyCode::Tab));
    assert_eq!(dialog.focus, ConfirmFocus::Yes);
    dialog.handle_key(key(KeyCode::Tab));
    assert_eq!(dialog.focus, ConfirmFocus::No);
}

#[test]
fn enter_confirms_only_when_yes_is_focused() {
    let mut dialog = ConfirmDialog::new("Delete?");
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), ConfirmOutcome::Cancelled);

    dialog.handle_key(key(KeyCode::Tab));
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), ConfirmOutcome::Confirmed);
}

#[test]
fn y_and_n_shortcuts_work_regardless_of_focus() {
    let mut dialog = ConfirmDialog::new("Delete?");
    assert_eq!(dialog.handle_key(key(KeyCode::Char('y'))), ConfirmOutcome::Confirmed);

    let mut dialog = ConfirmDialog::new("Delete?");
    assert_eq!(dialog.handle_key(key(KeyCode::Char('n'))), ConfirmOutcome::Cancelled);
}

fn rendered_rows(dialog: &ConfirmDialog, width: u16, height: u16) -> Vec<String> {
    screen_rows(&draw(dialog, width, height))
}

#[test]
fn every_line_of_a_multi_line_message_is_shown_on_its_own_row() {
    let dialog = ConfirmDialog::new("first line\nsecond line\nthird line");

    let rows = rendered_rows(&dialog, 40, 12);

    let row_of =
        |text: &str| rows.iter().position(|row| row.contains(text)).unwrap_or_else(|| panic!("{text} missing"));
    assert_eq!(row_of("second line"), row_of("first line") + 1);
    assert_eq!(row_of("third line"), row_of("second line") + 1);
    assert!(row_of("y yes") > row_of("third line"));
}

#[test]
fn a_line_wider_than_the_popup_wraps_instead_of_being_cut_off() {
    let dialog = ConfirmDialog::new(format!("{}\nTrust it?", "x".repeat(100)));

    let rows = rendered_rows(&dialog, 40, 14);

    let shown: usize = rows.iter().map(|row| row.matches('x').count()).sum();
    assert_eq!(shown, 100);
    assert!(rows.iter().any(|row| row.contains("Trust it?")));
    assert!(rows.iter().any(|row| row.contains("y yes")));
}

#[test]
fn the_options_read_y_yes_and_n_no_without_reversed_video() {
    let terminal = draw(&ConfirmDialog::new("Delete?"), 60, 12);

    assert!(find_text(&terminal, "y yes").is_some());
    assert!(find_text(&terminal, "n no").is_some());
    assert!(!terminal.backend().buffer().content().iter().any(|cell| cell.modifier.contains(Modifier::REVERSED)));
}

#[test]
fn focus_starts_on_no_so_the_chevron_is_before_n() {
    let terminal = draw(&ConfirmDialog::new("Delete?"), 60, 12);

    assert!(find_text(&terminal, "\u{203a} n no").is_some());
    assert!(find_text(&terminal, "\u{203a} y yes").is_none());
}

#[test]
fn tab_moves_the_chevron_to_yes() {
    let mut dialog = ConfirmDialog::new("Delete?");
    dialog.handle_key(key(KeyCode::Tab));

    let terminal = draw(&dialog, 60, 12);

    assert!(find_text(&terminal, "\u{203a} y yes").is_some());
    assert!(find_text(&terminal, "\u{203a} n no").is_none());
}

#[test]
fn the_focused_label_is_bright_and_the_other_is_dim() {
    let theme = indexed();
    let terminal = draw(&ConfirmDialog::new("Delete?"), 60, 12);

    let (yes_x, yes_y) = find_text(&terminal, "y yes").unwrap();
    let (no_x, no_y) = find_text(&terminal, "n no").unwrap();
    assert_eq!(cell_fg(&terminal, yes_x + 2, yes_y), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, no_x + 2, no_y), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, no_x - 2, no_y), theme.accent);
}

#[test]
fn an_escape_sequence_in_the_message_is_never_written_to_a_cell() {
    let dialog = ConfirmDialog::new("Delete \u{1b}[2Jevil\u{202e}name?");

    let terminal = draw(&dialog, 60, 12);

    assert!(!has_unprintable_cell(&terminal));
    assert!(find_text(&terminal, "Delete ?[2Jevil?name?").is_some());
}

#[test]
fn a_five_hundred_character_name_wraps_inside_the_popup() {
    let dialog = ConfirmDialog::new(format!("Delete {}?", "x".repeat(500)));

    let terminal = draw(&dialog, 80, 24);

    let rows = screen_rows(&terminal);
    assert_eq!(rows.iter().map(|row| row.matches('x').count()).sum::<usize>(), 500);
    let buffer = terminal.backend().buffer();
    for y in 0..24 {
        assert_ne!(buffer[(2, y)].symbol(), "x");
        assert_ne!(buffer[(77, y)].symbol(), "x");
    }
}

#[test]
fn a_wide_character_message_wraps_by_display_width() {
    let dialog = ConfirmDialog::new("\u{65e5}".repeat(100));

    let terminal = draw(&dialog, 40, 24);

    let buffer = terminal.backend().buffer();
    let glyphs = buffer.content().iter().filter(|cell| cell.symbol() == "\u{65e5}").count();
    assert_eq!(glyphs, 100);
    assert_ne!(buffer[(0, 11)].symbol(), "\u{65e5}");
}

#[test]
fn tiny_screens_do_not_panic() {
    for width in 0..=12 {
        for height in 0..=4 {
            draw(&ConfirmDialog::new("Delete everything now?"), width, height);
        }
    }
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier() {
    assert!(!include_str!("mod.rs").contains("REVERSED"));
}
