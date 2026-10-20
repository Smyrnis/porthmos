use ratatui::layout::Rect;

use super::*;

#[test]
fn splits_area_into_two_equal_halves() {
    let area = Rect::new(0, 0, 100, 40);
    let (local, remote) = split_panels(area);

    assert_eq!(local.x, 0);
    assert_eq!(local.width, 50);
    assert_eq!(local.height, 40);

    assert_eq!(remote.x, 50);
    assert_eq!(remote.width, 50);
    assert_eq!(remote.height, 40);
}

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
