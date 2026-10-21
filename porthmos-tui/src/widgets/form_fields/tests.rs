use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text},
    theme::{ColorDepth, Theme, ThemeSettings},
    widgets::dialog::{FormDialog, FormField, TestState},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn toggle(key: &'static str, label: &str, value: &str) -> FormField {
    FormField::choice(key, label, vec![("true".into(), "Yes".into()), ("false".into(), "No".into())], value)
}

fn sample() -> FormDialog {
    FormDialog::new(
        "t",
        vec![
            FormField::choice("protocol", "Protocol", vec![("sftp".into(), "sftp".into())], "sftp"),
            FormField::text("name", "Name", "backups"),
            FormField::text("host", "Host", "example.org"),
            toggle("tls", "use tls", "true"),
        ],
    )
}

fn draw_value(width: u16, value: &str, caret: Option<usize>, focused: bool) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, 3)).unwrap();
    terminal
        .draw(|frame| render_underline_value(frame, Rect::new(0, 0, width, 1), value, caret, focused, &theme))
        .unwrap();
    terminal
}

#[test]
fn an_underline_value_draws_the_value_then_the_underline_and_sets_the_cursor() {
    let theme = indexed();
    let mut terminal = draw_value(20, "\u{65e5}\u{672c}ab", Some(1), true);

    let symbols: Vec<String> = (0..5).map(|x| terminal.backend().buffer()[(x, 0)].symbol().to_string()).collect();
    assert_eq!((symbols[0].as_str(), symbols[2].as_str(), symbols[4].as_str()), ("\u{65e5}", "\u{672c}", "a"));
    assert_eq!(terminal.backend().buffer()[(0, 1)].symbol(), "\u{2500}");
    assert_eq!(terminal.backend().buffer()[(19, 1)].symbol(), "\u{2500}");
    assert_eq!(cell_fg(&terminal, 0, 1), theme.accent);
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (2, 0));
}

#[test]
fn an_unfocused_value_has_a_line_strong_underline_and_leaves_the_cursor_alone() {
    let theme = indexed();
    let mut terminal = draw_value(20, "abc", None, false);

    assert_eq!(cell_fg(&terminal, 0, 1), theme.line_strong);
    assert_eq!(cell_fg(&terminal, 0, 0), theme.fg);
    assert_eq!(terminal.get_cursor_position().unwrap().y, 0);
}

#[test]
fn the_caret_window_keeps_the_caret_visible_in_a_narrow_field() {
    let value = "0123456789".repeat(3);
    let mut terminal = draw_value(10, &value, Some(30), true);

    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!(cursor.y, 0);
    assert!(cursor.x < 10);
    assert_eq!(terminal.backend().buffer()[(cursor.x - 1, 0)].symbol(), "9");
}

#[test]
fn a_zero_width_or_zero_height_area_does_not_panic() {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(10, 3)).unwrap();
    terminal
        .draw(|frame| {
            render_underline_value(frame, Rect::new(0, 0, 0, 1), "abc", Some(1), true, &theme);
            render_underline_value(frame, Rect::new(0, 0, 10, 0), "abc", Some(1), true, &theme);
        })
        .unwrap();
}

#[test]
fn stack_height_counts_two_rows_per_text_field_one_per_toggle_a_gap_and_the_notes() {
    let mut form = sample();
    assert_eq!(stack_height(&form, 1), 4 + 1 + 1);
    form.hint = Some("hint".into());
    form.error = Some("error".into());
    assert_eq!(stack_height(&form, 1), 4 + 1 + 1 + 2);
    assert_eq!(stack_height(&form, 0), 6 + 1 + 1 + 2);
}

#[test]
fn a_first_field_of_zero_draws_every_field_and_one_skips_the_protocol_choice() {
    let theme = indexed();
    let form = sample();
    let mut all = Terminal::new(TestBackend::new(60, 12)).unwrap();
    let mut skipped = Terminal::new(TestBackend::new(60, 12)).unwrap();
    all.draw(|frame| render_field_stack(frame, frame.area(), &form, 0, &theme)).unwrap();
    skipped.draw(|frame| render_field_stack(frame, frame.area(), &form, 1, &theme)).unwrap();

    assert!(find_text(&all, "Protocol").is_some());
    assert!(find_text(&skipped, "Protocol").is_none());
    assert_eq!(find_text(&skipped, "Name"), Some((0, 0)));
    assert!(find_text(&skipped, "[x] use tls").is_some());
}

#[test]
fn the_error_and_hint_rows_sit_at_the_bottom_of_the_stack_area() {
    let theme = indexed();
    let mut form = sample();
    form.hint = Some("a hint".into());
    form.error = Some("it broke".into());
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal.draw(|frame| render_field_stack(frame, frame.area(), &form, 1, &theme)).unwrap();

    assert_eq!(find_text(&terminal, "a hint"), Some((0, 10)));
    assert_eq!(find_text(&terminal, "\u{2022} it broke"), Some((0, 11)));
}

fn finished_report() -> TestState {
    TestState::Finished(porthmos_core::TestOutcome::Connected(porthmos_core::TestReport {
        elapsed: std::time::Duration::from_millis(142),
        server: Some("OpenSSH 9.6".into()),
        start_folder: porthmos_core::StartFolder::Opens("/srv".into()),
    }))
}

#[test]
fn stack_height_counts_the_test_line_as_one_more_note() {
    let mut form = sample();
    let before = stack_height(&form, 1);
    form.test = Some(TestState::Running);
    assert_eq!(stack_height(&form, 1), before + 1);
    form.hint = Some("hint".into());
    form.error = Some("error".into());
    assert_eq!(stack_height(&form, 1), before + 3);
}

#[test]
fn the_test_line_sits_above_the_hint_and_the_error() {
    let theme = indexed();
    let mut form = sample();
    form.hint = Some("a hint".into());
    form.error = Some("it broke".into());
    form.test = Some(finished_report());
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal.draw(|frame| render_field_stack(frame, frame.area(), &form, 1, &theme)).unwrap();

    assert_eq!(find_text(&terminal, "\u{25cf} connected"), Some((0, 9)));
    assert_eq!(find_text(&terminal, "a hint"), Some((0, 10)));
    assert_eq!(find_text(&terminal, "\u{2022} it broke"), Some((0, 11)));
}

#[test]
fn there_is_no_test_line_without_a_test() {
    let theme = indexed();
    let form = sample();
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal.draw(|frame| render_field_stack(frame, frame.area(), &form, 1, &theme)).unwrap();

    assert!(find_text(&terminal, "\u{25cf}").is_none());
    assert!(find_text(&terminal, "\u{25cb}").is_none());
}

#[test]
fn the_focused_field_stays_visible_when_the_test_line_takes_a_row() {
    let theme = indexed();
    let mut form = FormDialog::new(
        "t",
        (0..8)
            .map(|index| FormField::text(Box::leak(format!("f{index}").into_boxed_str()), format!("Field{index}"), "v"))
            .collect(),
    );
    form.focused = 7;
    form.test = Some(TestState::Running);
    let mut terminal = Terminal::new(TestBackend::new(60, 6)).unwrap();
    terminal.draw(|frame| render_field_stack(frame, frame.area(), &form, 0, &theme)).unwrap();

    assert!(find_text(&terminal, "Field7").is_some());
    assert_eq!(find_text(&terminal, "\u{25cb} testing\u{2026}"), Some((0, 5)));
}

#[test]
fn the_test_line_survives_tiny_areas() {
    let theme = indexed();
    let mut form = sample();
    form.test = Some(finished_report());
    for (width, height) in [(1u16, 1u16), (3, 1), (10, 2), (60, 1), (0, 5), (5, 0)] {
        let mut terminal = Terminal::new(TestBackend::new(width.max(1), height.max(1))).unwrap();
        terminal.draw(|frame| render_field_stack(frame, Rect::new(0, 0, width, height), &form, 1, &theme)).unwrap();
    }
}

#[test]
fn a_typed_secret_never_reaches_the_cells_or_debug_output_while_a_test_runs() {
    let theme = indexed();
    let mut form = FormDialog::new("t", vec![FormField::masked("password", "Password", "hunter2-secret")]);
    form.test = Some(TestState::Running);
    let mut terminal = Terminal::new(TestBackend::new(60, 6)).unwrap();
    terminal.draw(|frame| render_field_stack(frame, frame.area(), &form, 0, &theme)).unwrap();

    assert!(find_text(&terminal, "hunter2").is_none());
    assert!(!format!("{:?}", form.test).contains("hunter2"));
}
