use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::import::{Candidate, ImportPlan, ImportRow, ImportSource, Verdict};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn candidate(name: &str) -> Candidate {
    Candidate {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: format!("{name}.example"),
        port: Some(22),
        username: "alice".to_string(),
        group: Some("Work".to_string()),
        options: BTreeMap::new(),
        notes: Vec::new(),
    }
}

fn new_row(name: &str) -> ImportRow {
    ImportRow { name: name.to_string(), candidate: Some(candidate(name)), verdict: Verdict::New }
}

fn other_row(name: &str, verdict: Verdict) -> ImportRow {
    let candidate = (verdict == Verdict::Exists).then(|| candidate(name));
    ImportRow { name: name.to_string(), candidate, verdict }
}

fn plan(rows: Vec<ImportRow>) -> ImportPlan {
    ImportPlan { import_id: 1, source: ImportSource::FileZilla, rows }
}

fn mixed() -> ImportDialog {
    ImportDialog::new(&plan(vec![
        new_row("alpha"),
        other_row("beta", Verdict::Exists),
        new_row("gamma"),
        other_row("delta", Verdict::Unsupported("Storj is not supported".to_string())),
        other_row("epsilon", Verdict::Invalid("port".to_string())),
    ]))
}

fn draw(dialog: &ImportDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_import(frame, frame.area(), dialog, &theme)).unwrap();
    terminal
}

#[test]
fn new_rows_start_ticked_and_other_rows_do_not() {
    assert_eq!(mixed().ticked_rows(), [0, 2]);
}

#[test]
fn space_ticks_new_rows_only() {
    let mut dialog = mixed();
    dialog.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(dialog.ticked_rows(), [2]);
    dialog.handle_key(key(KeyCode::Down));
    dialog.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(dialog.ticked_rows(), [2]);
    dialog.handle_key(key(KeyCode::Up));
    dialog.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(dialog.ticked_rows(), [0, 2]);
}

#[test]
fn a_and_n_tick_and_untick_all_new() {
    let mut dialog = mixed();
    dialog.handle_key(key(KeyCode::Char('n')));
    assert!(dialog.ticked_rows().is_empty());
    dialog.handle_key(key(KeyCode::Char('a')));
    assert_eq!(dialog.ticked_rows(), [0, 2]);
}

#[test]
fn enter_with_nothing_ticked_stays_pending_and_says_so() {
    let mut dialog = mixed();
    dialog.handle_key(key(KeyCode::Char('n')));
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), ImportChoice::Pending);
    let terminal = draw(&dialog, 100, 20);
    assert!(find_text(&terminal, "Tick at least one connection").is_some());
}

#[test]
fn enter_returns_the_ticked_row_numbers_in_plan_order() {
    let mut dialog = mixed();
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), ImportChoice::Run(vec![0, 2]));
}

#[test]
fn escape_and_q_cancel() {
    assert_eq!(mixed().handle_key(key(KeyCode::Esc)), ImportChoice::Cancel);
    assert_eq!(mixed().handle_key(key(KeyCode::Char('q'))), ImportChoice::Cancel);
}

#[test]
fn control_and_alt_keys_do_nothing() {
    let mut dialog = mixed();
    for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
        assert_eq!(dialog.handle_key(KeyEvent::new(KeyCode::Char('n'), modifier)), ImportChoice::Pending);
    }
    assert_eq!(dialog.ticked_rows(), [0, 2]);
}

#[test]
fn the_cursor_moves_and_stays_inside_the_list() {
    let mut dialog = mixed();
    dialog.handle_key(key(KeyCode::Up));
    assert_eq!(dialog.cursor, 0);
    dialog.handle_key(key(KeyCode::Char('j')));
    dialog.handle_key(key(KeyCode::Char('j')));
    assert_eq!(dialog.cursor, 2);
    dialog.handle_key(key(KeyCode::Char('k')));
    assert_eq!(dialog.cursor, 1);
    dialog.handle_key(key(KeyCode::End));
    assert_eq!(dialog.cursor, 4);
    dialog.handle_key(key(KeyCode::Down));
    assert_eq!(dialog.cursor, 4);
    dialog.handle_key(key(KeyCode::Home));
    assert_eq!(dialog.cursor, 0);
    dialog.handle_key(key(KeyCode::PageDown));
    assert_eq!(dialog.cursor, 4);
    dialog.handle_key(key(KeyCode::PageUp));
    assert_eq!(dialog.cursor, 0);
}

#[test]
fn renders_each_verdict() {
    let terminal = draw(&mixed(), 110, 20);
    for needle in [
        "[x] alpha",
        "sftp alice@alpha.example:22",
        "beta",
        "already exists",
        "Storj is not supported",
        "can't import: port",
        "Work",
        "2 will be added",
    ] {
        assert!(
            find_text(&terminal, needle).is_some(),
            "{needle} not on screen:\n{}",
            screen_rows(&terminal).join("\n")
        );
    }
}

#[test]
fn rows_that_cannot_be_imported_have_no_tick_box() {
    let terminal = draw(&mixed(), 110, 20);
    let rows = screen_rows(&terminal);
    let delta = rows.iter().find(|row| row.contains("delta")).expect("delta row");
    assert!(!delta.contains("[ ] delta") && !delta.contains("[x] delta"), "{delta}");
}

#[test]
fn the_selected_rows_notes_show_in_the_detail_line() {
    let mut noted = candidate("keyed");
    noted.notes = vec!["key is in PuTTY format, convert it first".to_string()];
    let dialog = ImportDialog::new(&plan(vec![ImportRow {
        name: "keyed".to_string(),
        candidate: Some(noted),
        verdict: Verdict::New,
    }]));
    let terminal = draw(&dialog, 110, 20);
    assert!(find_text(&terminal, "key is in PuTTY format, convert it first").is_some());
}

#[test]
fn escape_characters_in_a_name_never_reach_the_buffer() {
    let dialog = ImportDialog::new(&plan(vec![new_row("a\u{1b}[31mb"), new_row("c\u{202e}d")]));
    let terminal = draw(&dialog, 110, 20);
    assert!(!has_unprintable_cell(&terminal));
}

#[test]
fn long_names_truncate_to_the_width() {
    let long = "n".repeat(300);
    let dialog = ImportDialog::new(&plan(vec![new_row(&long)]));
    let terminal = draw(&dialog, 60, 12);
    let rows = screen_rows(&terminal);
    assert!(rows.iter().all(|row| row.chars().count() <= 60));
    assert!(find_text(&terminal, "\u{2026}").is_some());
}

#[test]
fn the_list_scrolls_with_the_cursor() {
    let rows: Vec<ImportRow> = (0..200).map(|number| new_row(&format!("site{number:03}"))).collect();
    let mut dialog = ImportDialog::new(&plan(rows));
    let before = draw(&dialog, 100, 14);
    assert!(find_text(&before, "site000").is_some());
    assert!(find_text(&before, "site150").is_none());
    dialog.handle_key(key(KeyCode::End));
    let after = draw(&dialog, 100, 14);
    assert!(find_text(&after, "site199").is_some());
    assert!(find_text(&after, "site000").is_none());
}

#[test]
fn a_tiny_terminal_does_not_panic() {
    let dialog = mixed();
    for (width, height) in [(1, 1), (10, 3), (30, 5), (0, 0)] {
        draw(&dialog, width, height);
    }
}

#[test]
fn an_empty_plan_renders_and_imports_nothing() {
    let mut dialog = ImportDialog::new(&plan(Vec::new()));
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), ImportChoice::Pending);
    draw(&dialog, 80, 12);
}
