use crate::{
    engine::{
        Command, Event,
        testing::{TestEngine, test_engine},
    },
    transfer::{
        Direction, JobStatus,
        rows::{RowKind, RowState},
    },
};

fn three_waiting_rows() -> (TestEngine, Vec<RowKind>) {
    let mut t = test_engine();
    t.engine.max_parallel = 0;
    let (session, _remote) = t.add_session("srv");
    let kinds = ["a", "b", "c"]
        .iter()
        .map(|name| {
            let batch = t.engine.transfers.start_batch(name.to_string());
            t.engine.transfers.enqueue(
                session,
                Direction::Upload,
                t.dir.path().join(name),
                format!("/up/{name}"),
                name.to_string(),
                10,
                Some(batch),
            );
            RowKind::Batch(batch)
        })
        .collect();
    t.engine.publish_transfers();
    t.drain();
    (t, kinds)
}

fn published_labels(events: &[Event]) -> Vec<Vec<String>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::TransfersChanged(snapshot) => Some(snapshot.rows.iter().map(|row| row.label.clone()).collect()),
            _ => None,
        })
        .collect()
}

#[test]
fn moving_a_row_later_publishes_the_new_order() {
    let (mut t, kinds) = three_waiting_rows();
    t.drain();

    t.engine.handle_command(Command::ReorderRow { kind: kinds[0], earlier: false });

    assert_eq!(published_labels(&t.drain()), [["b", "a", "c"]]);
}

#[test]
fn moving_a_row_earlier_publishes_the_new_order() {
    let (mut t, kinds) = three_waiting_rows();
    t.drain();

    t.engine.handle_command(Command::ReorderRow { kind: kinds[2], earlier: true });

    assert_eq!(published_labels(&t.drain()), [["a", "c", "b"]]);
}

#[test]
fn a_move_that_cannot_happen_publishes_nothing() {
    let (mut t, kinds) = three_waiting_rows();
    t.drain();

    t.engine.handle_command(Command::ReorderRow { kind: kinds[0], earlier: true });
    t.engine.handle_command(Command::ReorderRow { kind: kinds[2], earlier: false });
    t.engine.handle_command(Command::ReorderRow { kind: RowKind::Batch(99), earlier: true });
    t.engine.handle_command(Command::ReorderRow { kind: RowKind::Scan(1), earlier: true });

    assert!(t.drain().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_row_moved_to_the_top_is_the_one_that_starts_when_a_slot_frees() {
    let (mut t, kinds) = three_waiting_rows();
    std::fs::write(t.dir.path().join("c"), b"data").unwrap();
    t.engine.handle_command(Command::ReorderRow { kind: kinds[2], earlier: true });
    t.engine.handle_command(Command::ReorderRow { kind: kinds[2], earlier: true });
    t.engine.max_parallel = 1;

    t.engine.fill_transfer_slots();

    let running: Vec<String> = t
        .engine
        .transfers
        .jobs()
        .filter(|job| job.status == JobStatus::InProgress)
        .map(|job| job.display_name.clone())
        .collect();
    assert_eq!(running, ["c"]);
}

#[test]
fn reordering_does_not_start_anything_by_itself() {
    let (mut t, kinds) = three_waiting_rows();

    t.engine.handle_command(Command::ReorderRow { kind: kinds[0], earlier: false });

    assert!(t.engine.transfers.jobs().all(|job| job.status == JobStatus::Queued));
    assert!(t.engine.transfers.rows().iter().all(|row| row.state == RowState::Queued));
}
