use porthmos_core::sync::SyncDirection;

use super::direction_titles;

#[test]
fn both_is_offered_as_the_third_choice_only_when_supported() {
    let with_both = direction_titles(true);
    assert_eq!(with_both.len(), 3);
    assert_eq!(with_both[2].1, SyncDirection::Both);

    let without = direction_titles(false);
    assert_eq!(without.len(), 2);
    assert!(without.iter().all(|(_, direction)| *direction != SyncDirection::Both));
}
