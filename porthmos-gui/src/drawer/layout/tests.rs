use super::*;

fn leaf(id: u64) -> Layout {
    Layout::Leaf(id)
}

fn split(direction: Direction, first: Layout, second: Layout) -> Layout {
    Layout::Split { direction, first: Box::new(first), second: Box::new(second) }
}

fn side(first: Layout, second: Layout) -> Layout {
    split(Direction::Right, first, second)
}

fn stacked(first: Layout, second: Layout) -> Layout {
    split(Direction::Down, first, second)
}

#[test]
fn leaves_are_listed_in_layout_order() {
    let layout = side(leaf(1), stacked(leaf(2), leaf(3)));

    assert_eq!(layout.leaves(), vec![1, 2, 3]);
}

#[test]
fn splitting_a_leaf_puts_the_new_pane_second() {
    let mut layout = leaf(1);

    assert!(layout.split(1, 2, Direction::Right));
    assert_eq!(layout, side(leaf(1), leaf(2)));

    assert!(layout.split(2, 3, Direction::Down));
    assert_eq!(layout, side(leaf(1), stacked(leaf(2), leaf(3))));
}

#[test]
fn splitting_a_missing_pane_changes_nothing() {
    let mut layout = side(leaf(1), leaf(2));
    let before = layout.clone();

    assert!(!layout.split(9, 3, Direction::Down));
    assert_eq!(layout, before);
}

#[test]
fn removing_a_pane_collapses_its_parent_into_the_sibling() {
    let mut layout = side(leaf(1), stacked(leaf(2), leaf(3)));

    assert_eq!(layout.remove(2), Removal::Removed { focus: 3 });
    assert_eq!(layout, side(leaf(1), leaf(3)));

    let mut layout = side(leaf(1), stacked(leaf(2), leaf(3)));

    assert_eq!(layout.remove(1), Removal::Removed { focus: 2 });
    assert_eq!(layout, stacked(leaf(2), leaf(3)));
}

#[test]
fn the_focus_after_removal_is_the_sibling_leaf_nearest_the_removed_pane() {
    let mut layout = side(stacked(leaf(1), leaf(2)), leaf(3));
    assert_eq!(layout.remove(3), Removal::Removed { focus: 2 });

    let mut layout = side(leaf(3), stacked(leaf(1), leaf(2)));
    assert_eq!(layout.remove(3), Removal::Removed { focus: 1 });
}

#[test]
fn removing_the_last_pane_reports_last_and_keeps_the_layout() {
    let mut layout = leaf(1);

    assert_eq!(layout.remove(1), Removal::Last);
    assert_eq!(layout, leaf(1));
}

#[test]
fn removing_a_missing_pane_reports_missing() {
    let mut layout = side(leaf(1), leaf(2));
    let before = layout.clone();

    assert_eq!(layout.remove(7), Removal::Missing);
    assert_eq!(layout, before);
}

#[test]
fn neighbours_in_a_side_by_side_layout() {
    let layout = side(leaf(1), leaf(2));

    assert_eq!(layout.neighbour(1, Toward::Right), Some(2));
    assert_eq!(layout.neighbour(2, Toward::Left), Some(1));
    assert_eq!(layout.neighbour(1, Toward::Left), None);
    assert_eq!(layout.neighbour(1, Toward::Up), None);
    assert_eq!(layout.neighbour(1, Toward::Down), None);
}

#[test]
fn neighbours_in_a_stacked_layout() {
    let layout = stacked(leaf(1), leaf(2));

    assert_eq!(layout.neighbour(1, Toward::Down), Some(2));
    assert_eq!(layout.neighbour(2, Toward::Up), Some(1));
    assert_eq!(layout.neighbour(2, Toward::Down), None);
}

#[test]
fn neighbours_in_a_nested_layout_break_ties_by_layout_order() {
    let layout = side(leaf(1), stacked(leaf(2), leaf(3)));

    assert_eq!(layout.neighbour(1, Toward::Right), Some(2));
    assert_eq!(layout.neighbour(3, Toward::Left), Some(1));
    assert_eq!(layout.neighbour(2, Toward::Left), Some(1));
    assert_eq!(layout.neighbour(2, Toward::Down), Some(3));
    assert_eq!(layout.neighbour(3, Toward::Up), Some(2));
}

#[test]
fn neighbours_in_an_l_shaped_layout() {
    let layout = stacked(side(leaf(1), leaf(2)), leaf(3));

    assert_eq!(layout.neighbour(3, Toward::Up), Some(1));
    assert_eq!(layout.neighbour(2, Toward::Down), Some(3));
    assert_eq!(layout.neighbour(1, Toward::Right), Some(2));
}

#[test]
fn neighbours_in_a_two_by_two_layout() {
    let layout = side(stacked(leaf(1), leaf(2)), stacked(leaf(3), leaf(4)));

    assert_eq!(layout.neighbour(1, Toward::Right), Some(3));
    assert_eq!(layout.neighbour(2, Toward::Right), Some(4));
    assert_eq!(layout.neighbour(4, Toward::Left), Some(2));
}

#[test]
fn only_the_pane_touching_the_edge_is_a_neighbour() {
    let layout = side(leaf(1), side(leaf(2), leaf(3)));

    assert_eq!(layout.neighbour(1, Toward::Right), Some(2));
}

#[test]
fn an_unknown_pane_has_no_neighbour() {
    let layout = side(leaf(1), leaf(2));

    assert_eq!(layout.neighbour(9, Toward::Right), None);
}
