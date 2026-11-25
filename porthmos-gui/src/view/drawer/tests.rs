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

#[test]
fn the_search_bar_builds_open_and_with_a_missed_query() {
    let mut harness = Harness::new();
    harness.app.toggle_terminal();
    harness.press(crate::app::Message::ActionRequested(Action::TerminalSearch));

    assert!(view(&harness.app).is_some());
    let _ = search_bar(&harness.app);

    harness.press(crate::app::Message::TerminalSearchTyped("nothing".to_string()));
    let _ = search_bar(&harness.app);
}

#[test]
fn the_floating_surface_uses_the_surface_fill_and_a_strong_border() {
    let palette = palette();
    let style = crate::view::floating(&palette)(&iced::Theme::Dark);

    assert_eq!(style.background, Some(iced::Background::Color(palette.surface)));
    assert_eq!(style.border.color, palette.line_strong);
    assert_eq!(style.border.width, 1.0);
    assert_eq!(style.border.radius, 6.0.into());
}

fn open_two_tabs() -> (Harness, u64, u64) {
    use porthmos_core::{Entry, Event, Handover, Location, VfsPath};
    let mut harness = Harness::new();
    harness.feed(Event::Connected { session: 3, name: "alpha".to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(3),
        path: VfsPath::new("/srv"),
        entries: Vec::<Entry>::new(),
    });
    let mut ids = Vec::new();
    for _ in 0..2 {
        harness.app.open_embedded_terminal(crate::drawer::TerminalSource::Remote(3));
        let id = harness
            .sent()
            .into_iter()
            .find_map(|command| match command {
                porthmos_core::Command::OpenTerminal { id, .. } => Some(id),
                _ => None,
            })
            .unwrap();
        let (stream, end) = crate::testing::shell_pipe();
        std::mem::forget(end);
        harness.feed(Event::TerminalOpened { id, session: 3, stream: Handover::new(stream) });
        ids.push(id);
    }
    (harness, ids[0], ids[1])
}

#[tokio::test]
async fn a_tab_label_shows_the_title_or_terminal_and_the_exit_status() {
    let (mut harness, first, second) = open_two_tabs();
    harness.press(crate::app::Message::TerminalEventReceived(first, porthmos_term::TerminalEvent::Title("vim".into())));
    harness.press(crate::app::Message::TerminalEventReceived(second, porthmos_term::TerminalEvent::Exited(Some(3))));

    let labels: Vec<String> = harness.app.drawer().tabs.iter().map(tab_label).collect();

    assert_eq!(labels, vec!["vim".to_string(), "Terminal · Exited (3)".to_string()]);
}

#[tokio::test]
async fn the_strip_lists_one_entry_per_tab_and_marks_the_active_one() {
    let (mut harness, first, second) = open_two_tabs();

    let entries = tab_entries(harness.app.drawer());
    assert_eq!(
        entries.iter().map(|entry| (entry.id, entry.active)).collect::<Vec<_>>(),
        vec![(first, false), (second, true)]
    );

    harness.press(crate::app::Message::TerminalTabSelected(first));
    let entries = tab_entries(harness.app.drawer());
    assert_eq!(entries.iter().map(|entry| entry.active).collect::<Vec<_>>(), vec![true, false]);
}

#[tokio::test]
async fn the_drawer_with_several_tabs_builds() {
    let (harness, _first, _second) = open_two_tabs();

    assert!(view(&harness.app).is_some());
    let _ = tab_strip(&harness.app);
}

#[test]
fn an_inactive_tab_uses_the_muted_title_colour() {
    let palette = palette();

    assert_eq!(tab_title_color(true, true, &palette), palette.fg_bright);
    assert_eq!(tab_title_color(false, true, &palette), palette.fg_muted);
    assert_eq!(tab_title_color(true, false, &palette), palette.fg_muted);
}
