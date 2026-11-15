use iced::Size;
use porthmos_core::{Entry, Event, Location};

use super::more_hint;
use crate::app::Message;
use crate::geometry::ROW_HEIGHT;
use crate::palette::PaletteState;
use crate::shortcuts::Action;
use crate::testing::Harness;
use crate::tools::Tool;
use crate::view::tools::{PALETTE_CHROME, PALETTE_HEIGHT, PALETTE_WIDTH, frame_size, list_height, overlay};

fn palette_tool() -> Tool {
    Tool::Palette(PaletteState::new(Vec::new()))
}

#[test]
fn the_palette_keeps_its_size_in_a_large_window() {
    let window = Size::new(1100.0, 700.0);

    assert_eq!(frame_size(&palette_tool(), window), Size::new(PALETTE_WIDTH, PALETTE_HEIGHT));
    assert_eq!(list_height(&palette_tool(), window), PALETTE_HEIGHT - PALETTE_CHROME);
}

#[test]
fn the_palette_frame_shrinks_with_the_window_and_keeps_one_row() {
    let small = Size::new(480.0, 320.0);

    let frame = frame_size(&palette_tool(), small);

    assert_eq!(frame, Size::new(480.0 - 32.0, 320.0 - 32.0));
    assert!(list_height(&palette_tool(), small) >= ROW_HEIGHT);
    assert_eq!(list_height(&palette_tool(), Size::new(10.0, 10.0)), ROW_HEIGHT);
}

#[test]
fn the_overlay_builds_with_many_entries() {
    let mut harness = Harness::new();
    let start = harness.start_folder();
    let entries = (0..50_000)
        .map(|index| Entry {
            name: format!("file-{index}.txt"),
            path: start.join(format!("file-{index}.txt")),
            ..Entry::default()
        })
        .collect();
    harness.feed(Event::Listed { location: Location::Local, path: start, entries });
    harness.press(Message::ActionRequested(Action::Palette));
    harness.press(Message::PaletteTyped("#".to_string()));

    assert!(overlay(&harness.app).is_some());
}

#[test]
fn the_overlay_builds_when_nothing_matches() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Palette));
    harness.press(Message::PaletteTyped("zzzzqqqq".to_string()));

    assert!(overlay(&harness.app).is_some());
}

#[test]
fn the_overflow_hint_appears_only_when_matches_were_cut() {
    assert_eq!(more_hint(100, 100), None);
    assert_eq!(more_hint(0, 0), None);
    assert_eq!(
        more_hint(100, 50_000).as_deref(),
        Some("Showing the first 100 of 50000 matches. Keep typing to narrow them.")
    );
}
