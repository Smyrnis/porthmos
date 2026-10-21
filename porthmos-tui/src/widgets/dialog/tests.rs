use ratatui::{Terminal, backend::TestBackend, text::Line, widgets::Paragraph};

use super::*;
use crate::{
    app::testing::cell_fg,
    theme::{ColorDepth, ThemeSettings},
};

#[test]
fn a_dialog_outcome_never_prints_submitted_values() {
    let form = DialogOutcome::FormSubmitted(vec![("password", "hunter2".to_string())]);
    let text = DialogOutcome::Submitted("s3cret".to_string());

    assert!(!format!("{form:?}").contains("hunter2"));
    assert!(!format!("{text:?}").contains("s3cret"));
    assert!(format!("{form:?}").contains("password"));
}

#[test]
fn a_dialog_dims_the_screen_behind_it() {
    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default());
    let dialog = Dialog::Confirm(ConfirmDialog::new("Delete?"));
    let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();

    terminal
        .draw(|frame| {
            let rows: Vec<Line> = (0..14).map(|_| Line::from("x".repeat(60))).collect();
            frame.render_widget(Paragraph::new(rows).style(theme.text()), frame.area());
            dialog.render(frame, frame.area(), &theme);
        })
        .unwrap();

    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "x");
    assert_eq!(cell_fg(&terminal, 0, 0), theme.line_strong);
    assert_eq!(cell_fg(&terminal, 59, 13), theme.line_strong);
}
