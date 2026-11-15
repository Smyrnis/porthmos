use iced::Size;

use crate::geometry::ROW_HEIGHT as LIST_ROW_HEIGHT;
use crate::preview_view::PreviewState;
use crate::testing::Harness;
use crate::tools::Tool;
use crate::view::tools::{PREVIEW_CHROME, PREVIEW_HEIGHT, PREVIEW_WIDTH, frame_size, list_height, overlay};

fn loading_tool() -> Tool {
    Tool::Preview(PreviewState::loading(1, "a.txt".to_string()))
}

#[test]
fn the_preview_frame_is_large_and_shrinks_with_the_window() {
    assert_eq!(frame_size(&loading_tool(), Size::new(1400.0, 900.0)), Size::new(PREVIEW_WIDTH, PREVIEW_HEIGHT));
    assert_eq!(frame_size(&loading_tool(), Size::new(480.0, 320.0)), Size::new(448.0, 288.0));
}

#[test]
fn the_body_leaves_room_for_the_header_and_buttons() {
    assert_eq!(list_height(&loading_tool(), Size::new(1400.0, 900.0)), PREVIEW_HEIGHT - PREVIEW_CHROME);
    assert_eq!(list_height(&loading_tool(), Size::new(10.0, 10.0)), LIST_ROW_HEIGHT);
}

#[test]
fn the_overlay_is_present_while_a_preview_loads() {
    let mut harness = Harness::new();
    let folder = harness.start_folder();
    let entry = porthmos_core::Entry { name: "a.txt".to_string(), path: folder.join("a.txt"), ..Default::default() };
    harness.feed(porthmos_core::Event::Listed {
        location: porthmos_core::Location::Local,
        path: folder,
        entries: vec![entry],
    });
    harness.press(crate::app::Message::PanelClicked(crate::app::PanelSide::Local, 1));
    harness.press(crate::app::Message::MoreItemChosen(crate::menus::MoreItem::Preview));

    assert!(overlay(&harness.app).is_some());
}
