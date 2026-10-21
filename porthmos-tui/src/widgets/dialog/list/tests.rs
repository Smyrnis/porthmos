use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text, has_unprintable_cell},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(dialog: &ListDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_list(frame, frame.area(), dialog, &theme)).unwrap();
    terminal
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn dialog() -> ListDialog {
    ListDialog::new("Bookmarks", vec!["a".to_string(), "b".to_string(), "c".to_string()])
}

#[test]
fn down_moves_the_cursor_forward_and_clamps_at_the_end() {
    let mut dialog = dialog();
    dialog.handle_key(key(KeyCode::Down));
    dialog.handle_key(key(KeyCode::Down));
    dialog.handle_key(key(KeyCode::Down));
    assert_eq!(dialog.cursor, 2);
}

#[test]
fn up_clamps_at_zero() {
    let mut dialog = dialog();
    dialog.handle_key(key(KeyCode::Up));
    assert_eq!(dialog.cursor, 0);
}

#[test]
fn enter_selects_the_entry_under_the_cursor() {
    let mut dialog = dialog();
    dialog.handle_key(key(KeyCode::Down));
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), ListOutcome::Selected(1));
}

#[test]
fn esc_cancels() {
    let mut dialog = dialog();
    assert_eq!(dialog.handle_key(key(KeyCode::Esc)), ListOutcome::Cancelled);
}

#[test]
fn f8_removes_only_when_the_dialog_is_removable() {
    let mut dialog = dialog();
    assert_eq!(dialog.handle_key(key(KeyCode::F(8))), ListOutcome::Pending);

    let mut removable = dialog.removable(true);
    assert_eq!(removable.handle_key(key(KeyCode::F(8))), ListOutcome::Removed(0));
}

#[test]
fn enter_on_an_empty_list_does_nothing() {
    let mut dialog = ListDialog::new("Bookmarks", Vec::new());
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), ListOutcome::Pending);
}

#[test]
fn the_selected_item_has_a_chevron_and_the_focus_row_background() {
    let theme = indexed();
    let mut dialog = dialog();
    dialog.cursor = 1;

    let terminal = draw(&dialog, 60, 14);

    let (x, y) = find_text(&terminal, "\u{203a} b").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.accent);
    assert_eq!(cell_bg(&terminal, x + 2, y), theme.row_focus);
    assert!(find_text(&terminal, "\u{203a} a").is_none());
}

#[test]
fn an_empty_list_shows_a_dim_empty_row() {
    let theme = indexed();
    let terminal = draw(&ListDialog::new("Bookmarks", Vec::new()), 60, 14);

    let (x, y) = find_text(&terminal, "(empty)").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn the_footer_lists_f8_remove_only_when_the_list_is_removable() {
    let plain = draw(&dialog(), 70, 14);
    let removable = draw(&dialog().removable(true), 70, 14);

    assert!(find_text(&plain, "enter select").is_some());
    assert!(find_text(&plain, "esc cancel").is_some());
    assert!(find_text(&plain, "f8 remove").is_none());
    assert!(find_text(&removable, "f8 remove").is_some());
}

#[test]
fn a_list_taller_than_the_screen_scrolls_to_the_cursor() {
    let items = (0..30).map(|index| format!("item{index:02}")).collect();
    let mut dialog = ListDialog::new("Many", items);
    dialog.cursor = 25;

    let terminal = draw(&dialog, 40, 12);

    assert!(find_text(&terminal, "item25").is_some());
    assert!(find_text(&terminal, "item00").is_none());
    assert!(find_text(&terminal, "esc cancel").is_some());
}

#[test]
fn hostile_items_and_titles_are_sanitised() {
    let dialog = ListDialog::new("T\u{1b}[2J", vec!["a\u{1b}[31mb".to_string(), "c\u{202e}d".to_string()]);

    let terminal = draw(&dialog, 60, 14);

    assert!(!has_unprintable_cell(&terminal));
    assert!(find_text(&terminal, "a?[31mb").is_some());
}

#[test]
fn a_five_hundred_character_item_is_cut_inside_the_popup() {
    let dialog = ListDialog::new("T", vec!["y".repeat(500)]);

    let terminal = draw(&dialog, 80, 14);

    let buffer = terminal.backend().buffer();
    for y in 0..14 {
        assert_ne!(buffer[(2, y)].symbol(), "y");
        assert_ne!(buffer[(77, y)].symbol(), "y");
    }
}

#[test]
fn tiny_screens_do_not_panic() {
    for width in 0..=12 {
        for height in 0..=4 {
            draw(&dialog(), width, height);
            draw(&ListDialog::new("Empty", Vec::new()), width, height);
        }
    }
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier() {
    assert!(!include_str!("mod.rs").contains("REVERSED"));
}
