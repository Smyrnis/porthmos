use std::time::Duration;

use super::TransferWork;

#[test]
fn a_new_group_has_no_work_and_keeps_the_publish_interval() {
    let work = TransferWork::new(Duration::from_millis(40));

    assert!(work.planning.is_empty());
    assert!(work.moves.is_empty());
    assert!(work.reviews.is_empty());
    assert!(work.pause_stops.is_empty());
    assert!(work.source_removals.is_empty());
    assert!(work.last_progress_publish.is_none());
    assert!(!work.transfers_dirty);
    assert_eq!(work.publish_interval, Duration::from_millis(40));
}
