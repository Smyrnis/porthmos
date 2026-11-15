use porthmos_core::VfsPath;

use super::{candidates, frame_border, more_text, visible_crumbs};
use crate::breadcrumbs::crumbs;

#[test]
fn trailing_crumbs_are_kept_when_the_bar_is_narrow() {
    let all = crumbs(&VfsPath::new("/home/user/projects/porthmos"));

    let (clipped, shown) = visible_crumbs(&all, 20);

    assert!(clipped);
    assert_eq!(shown.last().unwrap().label, "porthmos");
    assert!(shown.len() < all.len());
}

#[test]
fn every_crumb_is_kept_when_there_is_room() {
    let all = crumbs(&VfsPath::new("/a/b"));

    let (clipped, shown) = visible_crumbs(&all, 80);

    assert!(!clipped);
    assert_eq!(shown.len(), all.len());
}

#[test]
fn the_last_crumb_is_kept_even_when_it_alone_is_too_long() {
    let all = crumbs(&VfsPath::new("/a/an_extremely_long_folder_name"));

    let (clipped, shown) = visible_crumbs(&all, 5);

    assert!(clipped);
    assert_eq!(shown.len(), 1);
}

#[test]
fn the_path_field_frame_takes_the_accent_edge_only_while_editing() {
    let palette = crate::theme::Palette::new(&crate::theme::ThemeSettings::default());

    assert_eq!(frame_border(true, &palette).color, palette.accent);
    assert_eq!(frame_border(false, &palette).color, palette.line_strong);
    assert!(frame_border(true, &palette).width > frame_border(false, &palette).width);
}

#[test]
fn the_more_row_counts_what_the_list_leaves_out() {
    assert_eq!(more_text(5).as_deref(), Some("and 5 more"));
    assert_eq!(more_text(0), None);
}

#[test]
fn the_candidate_view_exists_only_while_a_list_is_shown() {
    use crate::app::Message;
    use crate::testing::Harness;

    let mut harness = Harness::new();
    harness.press(Message::PathEditStarted(crate::app::PanelSide::Local));
    assert!(candidates(&harness.app).is_none());

    harness.press(Message::PathTyped("/".to_string()));
    harness.feed(porthmos_core::Event::PathCompleted {
        location: porthmos_core::Location::Local,
        text: "/".to_string(),
        completion: porthmos_core::Completion {
            prefix: "/".to_string(),
            candidates: vec!["a".to_string(), "b".to_string()],
            completed: "/".to_string(),
        },
    });

    assert!(candidates(&harness.app).is_some());
}
