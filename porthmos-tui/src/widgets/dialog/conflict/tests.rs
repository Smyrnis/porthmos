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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn dialog(is_dir: bool, index: usize, total: usize) -> ConflictDialog {
    ConflictDialog {
        file_name: "report.pdf".to_string(),
        existing: Some(ExistingFile { size: 2_200_000, modified: Some(1_000_000 - 3 * 86_400), is_dir }),
        partial: None,
        new_size: 2_500_000,
        new_modified: Some(1_000_000 - 120),
        index,
        total,
        apply_to_rest: false,
        now: 1_000_000,
    }
}

fn resolved(resolution: Option<Resolution>, apply_to_rest: bool) -> ConflictOutcome {
    ConflictOutcome::Resolved { resolution, apply_to_rest }
}

#[test]
fn letters_choose_a_resolution() {
    assert_eq!(dialog(false, 0, 1).handle_key(key(KeyCode::Char('o'))), resolved(Some(Resolution::Overwrite), false));
    assert_eq!(dialog(false, 0, 1).handle_key(key(KeyCode::Char('s'))), resolved(Some(Resolution::Skip), false));
    assert_eq!(dialog(false, 0, 1).handle_key(key(KeyCode::Char('r'))), resolved(Some(Resolution::Rename), false));
    assert_eq!(dialog(false, 0, 1).handle_key(key(KeyCode::Char('c'))), resolved(None, false));
    assert_eq!(dialog(false, 0, 1).handle_key(key(KeyCode::Esc)), resolved(None, false));
}

#[test]
fn enter_does_nothing() {
    assert_eq!(dialog(false, 0, 1).handle_key(key(KeyCode::Enter)), ConflictOutcome::Pending);
}

#[test]
fn o_is_ignored_when_the_existing_item_is_a_folder() {
    assert_eq!(dialog(true, 0, 1).handle_key(key(KeyCode::Char('o'))), ConflictOutcome::Pending);
}

#[test]
fn a_toggles_the_same_answer_for_the_rest() {
    let mut conflict = dialog(false, 0, 3);

    assert_eq!(conflict.handle_key(key(KeyCode::Char('a'))), ConflictOutcome::Pending);
    assert_eq!(conflict.handle_key(key(KeyCode::Char('s'))), resolved(Some(Resolution::Skip), true));
}

#[test]
fn a_does_nothing_on_the_last_conflict() {
    let mut conflict = dialog(false, 2, 3);

    conflict.handle_key(key(KeyCode::Char('a')));

    assert!(!conflict.apply_to_rest);
}

#[test]
fn format_age_buckets() {
    assert_eq!(format_age(-5), "just now");
    assert_eq!(format_age(59), "just now");
    assert_eq!(format_age(60), "1 minute ago");
    assert_eq!(format_age(7_200), "2 hours ago");
    assert_eq!(format_age(3 * 86_400), "3 days ago");
}

fn render(conflict: &ConflictDialog) -> String {
    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render_conflict(frame, frame.area(), conflict, &Theme::default())).unwrap();
    terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
}

fn draw(conflict: &ConflictDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_conflict(frame, frame.area(), conflict, &theme)).unwrap();
    terminal
}

#[test]
fn renders_both_files_the_counter_and_the_options() {
    let content = render(&dialog(false, 0, 3));

    assert!(content.contains("\"report.pdf\" already exists (1 of 3)"));
    assert!(content.contains("modified 3 days ago"));
    assert!(content.contains("modified 2 minutes ago"));
    assert!(content.contains("o overwrite"));
    assert!(content.contains("same answer for the other 2"));
}

#[test]
fn a_folder_conflict_hides_overwrite() {
    let content = render(&dialog(true, 0, 1));

    assert!(content.contains("folder"));
    assert!(!content.contains("o overwrite"));
    assert!(!content.contains("same answer"));
}

#[test]
fn ctrl_shortcuts_do_not_answer_except_ctrl_c() {
    let ctrl = |code| KeyEvent::new(code, KeyModifiers::CONTROL);

    assert_eq!(dialog(false, 0, 3).handle_key(ctrl(KeyCode::Char('r'))), ConflictOutcome::Pending);
    assert_eq!(dialog(false, 0, 3).handle_key(ctrl(KeyCode::Char('s'))), ConflictOutcome::Pending);
    assert_eq!(dialog(false, 0, 3).handle_key(ctrl(KeyCode::Char('o'))), ConflictOutcome::Pending);
    assert_eq!(dialog(false, 0, 3).handle_key(ctrl(KeyCode::Char('a'))), ConflictOutcome::Pending);
    assert_eq!(dialog(false, 0, 3).handle_key(ctrl(KeyCode::Char('c'))), resolved(None, false));
}

fn partial_dialog(with_existing: bool) -> ConflictDialog {
    let mut conflict = dialog(false, 0, 1);
    if !with_existing {
        conflict.existing = None;
    }
    conflict.partial = Some(ExistingFile { size: 1_300_000, modified: Some(1_000_000 - 300), is_dir: false });
    conflict
}

#[test]
fn u_resumes_only_when_there_is_a_partial() {
    assert_eq!(partial_dialog(false).handle_key(key(KeyCode::Char('u'))), resolved(Some(Resolution::Resume), false));
    assert_eq!(dialog(false, 0, 1).handle_key(key(KeyCode::Char('u'))), ConflictOutcome::Pending);
}

#[test]
fn r_renames_only_when_something_exists() {
    assert_eq!(partial_dialog(false).handle_key(key(KeyCode::Char('r'))), ConflictOutcome::Pending);
    assert_eq!(partial_dialog(true).handle_key(key(KeyCode::Char('r'))), resolved(Some(Resolution::Rename), false));
}

#[test]
fn a_partial_only_prompt_offers_resume_and_start_over() {
    let content = render(&partial_dialog(false));

    assert!(content.contains("\"report.pdf\" was partly copied before (1 of 1)"));
    assert!(content.contains("partial"));
    assert!(content.contains("u resume"));
    assert!(content.contains("o start over"));
    assert!(!content.contains("r rename"));
    assert!(!content.contains("existing"));
}

#[test]
fn a_prompt_with_both_offers_everything() {
    let content = render(&partial_dialog(true));

    assert!(content.contains("already exists"));
    assert!(content.contains("u resume"));
    assert!(content.contains("o overwrite"));
    assert!(content.contains("r rename"));
}

#[test]
fn resume_is_not_offered_when_a_folder_is_in_the_way() {
    let mut conflict = partial_dialog(true);
    conflict.existing = Some(ExistingFile { size: 0, modified: None, is_dir: true });

    assert_eq!(conflict.handle_key(key(KeyCode::Char('u'))), ConflictOutcome::Pending);
    assert!(!render(&conflict).contains("u resume"));
}

#[test]
fn rows_show_existing_and_new_with_sizes_and_ages_in_that_order() {
    let terminal = draw(&dialog(false, 0, 1), 80, 16);

    let (_, existing_row) = find_text(&terminal, "existing").unwrap();
    let (_, new_row) = find_text(&terminal, "new").unwrap();
    assert!(new_row > existing_row);
    assert!(find_text(&terminal, "2.1M").is_some());
    assert!(find_text(&terminal, "2.4M").is_some());
    assert!(find_text(&terminal, "modified 3 days ago").is_some());
}

#[test]
fn only_the_offered_options_are_listed() {
    let folder = render(&dialog(true, 0, 1));
    assert!(!folder.contains("o overwrite"));
    assert!(folder.contains("s skip"));
    assert!(folder.contains("r rename"));
    assert!(folder.contains("c cancel copy"));

    let partial_only = render(&partial_dialog(false));
    assert!(partial_only.contains("u resume"));
    assert!(partial_only.contains("o start over"));
    assert!(!partial_only.contains("r rename"));

    assert!(!render(&dialog(false, 0, 1)).contains("u resume"));
}

#[test]
fn option_keys_are_bright_and_labels_are_dim() {
    let theme = indexed();
    let terminal = draw(&dialog(false, 0, 1), 80, 16);

    let (x, y) = find_text(&terminal, "s skip").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, x + 2, y), theme.fg_dim);
}

#[test]
fn the_same_answer_row_shows_an_accent_checkbox_that_follows_the_toggle() {
    let theme = indexed();
    let mut conflict = dialog(false, 0, 3);
    let before = draw(&conflict, 80, 16);
    let (x, y) = find_text(&before, "[ ] same answer for the other 2").unwrap();
    assert_eq!(cell_fg(&before, x, y), theme.fg_dim);

    conflict.handle_key(key(KeyCode::Char('a')));
    let after = draw(&conflict, 80, 16);
    let (x, y) = find_text(&after, "[x] same answer for the other 2").unwrap();
    assert_eq!(cell_fg(&after, x, y), theme.accent);
}

#[test]
fn a_hostile_file_name_is_sanitised_and_cut_inside_the_popup() {
    let mut conflict = dialog(false, 0, 1);
    conflict.file_name = "\u{1b}[2J\u{202e}".repeat(60);

    let terminal = draw(&conflict, 80, 16);

    assert!(!has_unprintable_cell(&terminal));
    let buffer = terminal.backend().buffer();
    assert!((0..16).all(|y| buffer[(0, y)].symbol() == " " && buffer[(79, y)].symbol() == " "));
}

#[test]
fn tiny_screens_do_not_panic() {
    for width in 0..=12 {
        for height in 0..=4 {
            draw(&dialog(false, 0, 3), width, height);
            draw(&partial_dialog(true), width, height);
        }
    }
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier() {
    assert!(!include_str!("mod.rs").contains("REVERSED"));
}
