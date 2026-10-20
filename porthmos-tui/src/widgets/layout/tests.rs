use ratatui::layout::Rect;

use super::*;

#[test]
fn split_frame_stacks_top_bar_rule_body_rule_hint_bar() {
    let frame = split_frame(Rect::new(0, 0, 100, 40)).unwrap();

    assert_eq!((frame.top_bar.y, frame.top_bar.height), (0, 1));
    assert_eq!((frame.top_rule.y, frame.top_rule.height), (1, 1));
    assert_eq!((frame.body.y, frame.body.height), (2, 36));
    assert_eq!((frame.bottom_rule.y, frame.bottom_rule.height), (38, 1));
    assert_eq!((frame.hint_bar.y, frame.hint_bar.height), (39, 1));
}

#[test]
fn split_frame_insets_content_but_not_the_rules() {
    let frame = split_frame(Rect::new(0, 0, 100, 40)).unwrap();

    for content in [frame.top_bar, frame.body, frame.hint_bar] {
        assert_eq!((content.x, content.width), (2, 96));
    }
    for rule in [frame.top_rule, frame.bottom_rule] {
        assert_eq!((rule.x, rule.width), (0, 100));
    }
}

#[test]
fn split_frame_gives_no_padding_to_a_very_narrow_area() {
    let narrow = split_frame(Rect::new(0, 0, 4, 12)).unwrap();
    let barely = split_frame(Rect::new(0, 0, 5, 12)).unwrap();

    assert_eq!((narrow.top_bar.x, narrow.top_bar.width), (0, 4));
    assert_eq!((barely.top_bar.x, barely.top_bar.width), (2, 1));
}

#[test]
fn split_frame_refuses_areas_shorter_than_twelve_rows() {
    assert_eq!(split_frame(Rect::new(0, 0, 80, 11)), None);
    assert_eq!(split_frame(Rect::new(0, 0, 80, 12)).unwrap().body.height, 8);
}

#[test]
fn split_frame_keeps_every_rectangle_inside_the_area() {
    for width in 0..=10 {
        for height in 0..=14 {
            let area = Rect::new(3, 2, width, height);
            let Some(frame) = split_frame(area) else { continue };
            for rect in [frame.top_bar, frame.top_rule, frame.body, frame.bottom_rule, frame.hint_bar] {
                assert!(
                    rect.x >= area.x
                        && rect.y >= area.y
                        && rect.right() <= area.right()
                        && rect.bottom() <= area.bottom(),
                    "{width}x{height}: {rect:?}"
                );
            }
        }
    }
}

#[test]
fn files_layout_puts_the_info_line_last_and_the_rule_above_it() {
    let layout = files_layout(Rect::new(2, 2, 96, 8), FilesMode::TwoPanels, false);

    assert_eq!((layout.info.y, layout.info.height, layout.info.x, layout.info.width), (9, 1, 2, 96));
    assert_eq!((layout.rule.y, layout.rule.height, layout.rule.x, layout.rule.width), (8, 1, 2, 96));
    assert_eq!((layout.local.y, layout.local.height), (2, 6));
    assert_eq!((layout.remote.y, layout.remote.height), (2, 6));
}

#[test]
fn two_panels_leave_a_gutter_of_two_cells_each_side_of_the_divider() {
    let layout = files_layout(Rect::new(0, 0, 96, 8), FilesMode::TwoPanels, false);

    let divider = layout.divider.expect("two panels have a divider");
    assert_eq!((divider.x, divider.width, divider.height), (48, 1, 6));
    assert_eq!(layout.local.right(), 46);
    assert_eq!(layout.remote.x, 51);
    assert_eq!(layout.remote.right(), 96);
}

#[test]
fn no_rectangle_leaves_the_body_for_any_size() {
    for width in (0..=12).chain([96, 97]) {
        for height in 0..=8 {
            let body = Rect::new(3, 2, width, height);
            let layout = files_layout(body, files_mode(width), false);
            let rects = [layout.local, layout.remote, layout.rule, layout.info].into_iter().chain(layout.divider);
            for rect in rects {
                assert!(
                    rect.x >= body.x
                        && rect.y >= body.y
                        && rect.right() <= body.right()
                        && rect.bottom() <= body.bottom(),
                    "{width}x{height}: {rect:?}"
                );
            }
        }
    }
}

#[test]
fn files_mode_switches_at_96_and_56_body_columns() {
    assert_eq!(files_mode(96), FilesMode::TwoPanels);
    assert_eq!(files_mode(200), FilesMode::TwoPanels);
    assert_eq!(files_mode(95), FilesMode::SinglePanel { show_modified: true });
    assert_eq!(files_mode(56), FilesMode::SinglePanel { show_modified: true });
    assert_eq!(files_mode(55), FilesMode::SinglePanel { show_modified: false });
    assert_eq!(files_mode(0), FilesMode::SinglePanel { show_modified: false });
}

#[test]
fn single_mode_uses_the_whole_panel_area_and_no_divider() {
    let mode = FilesMode::SinglePanel { show_modified: true };

    let layout = files_layout(Rect::new(2, 2, 80, 8), mode, false);

    assert_eq!(layout.divider, None);
    assert_eq!(layout.local, Rect::new(2, 2, 80, 6));
    assert_eq!(layout.remote, layout.local);
    assert_eq!((layout.rule.y, layout.info.y), (8, 9));
}

#[test]
fn files_layout_reserves_a_row_for_the_transfer_line_only_when_asked() {
    let body = Rect::new(2, 2, 96, 8);

    let without = files_layout(body, FilesMode::TwoPanels, false);
    let with = files_layout(body, FilesMode::TwoPanels, true);

    assert_eq!(without.transfer.height, 0);
    assert_eq!((with.transfer.y, with.transfer.height, with.transfer.width), (9, 1, 96));
    assert_eq!((with.info.y, with.rule.y, with.local.height), (8, 7, 5));
    assert_eq!((without.info.y, without.rule.y, without.local.height), (9, 8, 6));
}

#[test]
fn the_transfer_line_never_leaves_the_body() {
    for width in [0, 5, 96] {
        for height in 0..=8 {
            let body = Rect::new(3, 2, width, height);
            let layout = files_layout(body, files_mode(width), true);
            let rects = [layout.local, layout.remote, layout.rule, layout.info, layout.transfer];
            for rect in rects {
                assert!(
                    rect.x >= body.x
                        && rect.y >= body.y
                        && rect.right() <= body.right()
                        && rect.bottom() <= body.bottom(),
                    "{width}x{height}: {rect:?}"
                );
            }
        }
    }
}
