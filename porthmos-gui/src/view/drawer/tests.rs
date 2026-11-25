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

    let labels: Vec<String> = harness.app.drawer().panes.iter().map(|pane| tab_label(pane, 1)).collect();

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

fn split_open(harness: &mut Harness, direction: crate::drawer::layout::Direction) -> u64 {
    use porthmos_core::{Event, Handover};
    let action = match direction {
        crate::drawer::layout::Direction::Right => crate::shortcuts::Action::TerminalSplitRight,
        crate::drawer::layout::Direction::Down => crate::shortcuts::Action::TerminalSplitDown,
    };
    harness.press(crate::app::Message::ActionRequested(action));
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
    id
}

#[test]
fn a_single_pane_has_no_ring_and_several_do() {
    assert_eq!(ring_width(0), 0.0);
    assert_eq!(ring_width(1), 0.0);
    assert_eq!(ring_width(2), RING_WIDTH);
    assert_eq!(ring_width(5), RING_WIDTH);
}

#[test]
fn the_focused_pane_ring_is_the_accent_and_the_others_the_plain_line() {
    let palette = palette();

    assert_eq!(ring_color(true, &palette), palette.accent);
    assert_eq!(ring_color(false, &palette), palette.line_strong);
}

#[tokio::test]
async fn a_tab_label_counts_panes_only_when_there_are_several() {
    let (mut harness, first, _second) = open_two_tabs();
    harness.press(crate::app::Message::TerminalEventReceived(first, porthmos_term::TerminalEvent::Title("vim".into())));
    let pane = harness.app.drawer().pane(first).unwrap();

    assert_eq!(tab_label(pane, 1), "vim");
    assert_eq!(tab_label(pane, 2), "vim [2]");

    harness.press(crate::app::Message::TerminalEventReceived(first, porthmos_term::TerminalEvent::Exited(Some(3))));
    let pane = harness.app.drawer().pane(first).unwrap();
    assert_eq!(tab_label(pane, 3), "vim · Exited (3) [3]");
}

#[tokio::test]
async fn a_split_drawer_builds_with_nested_panes() {
    use crate::drawer::layout::Direction;
    let (mut harness, first, _second) = open_two_tabs();
    harness.press(crate::app::Message::TerminalTabSelected(first));
    split_open(&mut harness, Direction::Right);
    split_open(&mut harness, Direction::Down);

    assert_eq!(harness.app.drawer().tabs[0].layout.leaves().len(), 3);
    assert!(view(&harness.app).is_some());
}

#[tokio::test]
async fn the_tab_entry_uses_the_focused_panes_title() {
    use crate::drawer::layout::Direction;
    let (mut harness, first, _second) = open_two_tabs();
    harness.press(crate::app::Message::TerminalTabSelected(first));
    let added = split_open(&mut harness, Direction::Right);
    harness
        .press(crate::app::Message::TerminalEventReceived(added, porthmos_term::TerminalEvent::Title("htop".into())));
    harness.press(crate::app::Message::TerminalEventReceived(first, porthmos_term::TerminalEvent::Title("vim".into())));

    let entry = &tab_entries(harness.app.drawer())[0];

    assert_eq!(entry.label, "htop [2]");
}

#[tokio::test]
async fn the_header_shows_the_link_hovered_in_any_pane_of_the_tab() {
    use crate::drawer::layout::Direction;
    let (mut harness, first, _second) = open_two_tabs();
    harness.press(crate::app::Message::TerminalTabSelected(first));
    let added = split_open(&mut harness, Direction::Right);
    assert!(hovered_link(harness.app.drawer()).is_none());
    harness.press(crate::app::Message::ModifiersChanged(iced::keyboard::Modifiers::CTRL));
    harness.app.drawer().pane(first).unwrap().session.as_ref().unwrap().with_terminal(|terminal| {
        terminal.feed(b"see https://example.org/x");
    });
    harness.press(crate::app::Message::TerminalPointer(
        first,
        crate::view::terminal_grid::PointerEvent {
            kind: crate::drawer::pointer::PointerKind::Move { dragging: false },
            point: porthmos_term::ViewPoint { row: 0, column: 6 },
            side: porthmos_term::Side::Left,
            clicks: 0,
            lines: 0,
        },
    ));

    assert_eq!(harness.app.drawer().active_tab().unwrap().focus, added);
    assert_eq!(hovered_link(harness.app.drawer()).unwrap().target, "https://example.org/x");
}

#[tokio::test]
async fn the_focused_pane_keeps_the_accent_ring_after_its_shell_exited() {
    use crate::drawer::layout::Direction;
    let (mut harness, first, _second) = open_two_tabs();
    harness.press(crate::app::Message::TerminalTabSelected(first));
    let added = split_open(&mut harness, Direction::Right);
    let tab = harness.app.drawer().active_tab().unwrap();
    assert!(pane_has_focus(harness.app.drawer(), tab, added));
    assert!(!pane_has_focus(harness.app.drawer(), tab, first));

    harness.press(crate::app::Message::TerminalEventReceived(added, porthmos_term::TerminalEvent::Exited(Some(0))));

    let tab = harness.app.drawer().active_tab().unwrap();
    assert!(pane_has_focus(harness.app.drawer(), tab, added));
}
