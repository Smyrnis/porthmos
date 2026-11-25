use porthmos_core::theme::ColorScheme;

use super::*;
use crate::testing::Harness;
use crate::theme::ThemeSettings;

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default().with_scheme(ColorScheme::dark()))
}

#[test]
fn a_tab_without_a_title_is_called_terminal() {
    assert_eq!(tab_title(""), "Terminal");
    assert_eq!(tab_title("vim notes.txt"), "vim notes.txt");
}

#[test]
fn the_exit_status_is_shown_after_the_shell_ended() {
    assert_eq!(exit_text(None), None);
    assert_eq!(exit_text(Some(None)), Some("Exited".to_string()));
    assert_eq!(exit_text(Some(Some(3))), Some("Exited (3)".to_string()));
}

#[test]
fn the_focused_drawer_uses_the_accent_edge() {
    let palette = palette();

    assert_eq!(edge_color(true, &palette), palette.accent);
    assert_eq!(edge_color(false, &palette), palette.line_strong);
}

#[test]
fn the_drawer_is_absent_when_closed() {
    let harness = Harness::new();

    assert!(view(&harness.app).is_none());
}

#[test]
fn an_open_drawer_without_a_terminal_still_has_a_body() {
    let mut harness = Harness::new();
    harness.app.toggle_terminal();

    assert!(view(&harness.app).is_some());
}

#[test]
fn the_empty_hint_names_the_terminal_shortcut() {
    let harness = Harness::new();

    assert!(empty_hint(&harness.app).contains("F4"), "{}", empty_hint(&harness.app));
}

#[test]
fn the_empty_hint_without_a_shortcut_does_not_leave_a_gap() {
    let text = hint_text("");

    assert!(!text.contains("  ") && !text.starts_with("Press"), "{text}");
    assert!(hint_text("F4").contains("F4"));
}
