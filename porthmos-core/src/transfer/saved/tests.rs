use porthmos_vfs::VfsPath;

use super::{INTERRUPTED_REASON, SavedJob, load, render, unfinished};
use crate::{
    Paths,
    persist::Loaded,
    transfer::{Direction, JobStatus, TransferQueue},
};

fn queue_with(statuses: &[JobStatus]) -> (TransferQueue, Vec<u64>) {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("photos".to_string());
    let ids: Vec<u64> = statuses
        .iter()
        .enumerate()
        .map(|(index, _)| {
            queue.enqueue(
                1,
                Direction::Upload,
                VfsPath::new(&format!("/home/u/{index}.jpg")),
                format!("/srv/{index}.jpg"),
                format!("{index}.jpg"),
                100 + index as u64,
                Some(batch),
            )
        })
        .collect();
    for (id, status) in ids.iter().zip(statuses) {
        queue.get_mut(*id).unwrap().status = status.clone();
    }
    (queue, ids)
}

fn paths_in(dir: &tempfile::TempDir) -> Paths {
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(paths.unfinished_transfers_file().parent().unwrap()).unwrap();
    paths
}

fn names(_: u64) -> String {
    "srv".to_string()
}

#[test]
fn only_work_that_is_not_done_is_kept() {
    let (queue, _) = queue_with(&[
        JobStatus::Queued,
        JobStatus::InProgress,
        JobStatus::Completed,
        JobStatus::Cancelled,
        JobStatus::Failed("boom".to_string()),
    ]);

    let saved = unfinished(&queue, names);

    let kept: Vec<&str> = saved.iter().map(|job| job.display_name.as_str()).collect();
    assert_eq!(kept, ["0.jpg", "1.jpg", "4.jpg"]);
}

#[test]
fn a_saved_job_remembers_what_a_retry_needs() {
    let (mut queue, ids) = queue_with(&[JobStatus::InProgress]);
    {
        let mut job = queue.get_mut(ids[0]).unwrap();
        job.transferred_bytes = 40;
        job.modified = Some(1_700_000_000);
        job.remove_source = true;
    }

    let saved = unfinished(&queue, names);

    assert_eq!(
        saved,
        [SavedJob {
            connection: "srv".to_string(),
            direction: Direction::Upload,
            local_path: "/home/u/0.jpg".to_string(),
            remote_path: "/srv/0.jpg".to_string(),
            display_name: "0.jpg".to_string(),
            total_bytes: 100,
            transferred_bytes: 40,
            modified: Some(1_700_000_000),
            remove_source: true,
            batch: Some(0),
            label: Some("photos".to_string()),
            failure: None,
        }]
    );
}

#[test]
fn a_failed_job_keeps_its_reason_and_a_running_one_has_none() {
    let (queue, _) = queue_with(&[JobStatus::Failed("no space left".to_string()), JobStatus::Queued]);

    let saved = unfinished(&queue, names);

    assert_eq!(saved[0].failure.as_deref(), Some("no space left"));
    assert_eq!(saved[1].failure, None);
}

#[test]
fn each_job_is_filed_under_the_connection_its_session_had() {
    let mut queue = TransferQueue::new();
    queue.enqueue(7, Direction::Download, VfsPath::new("/d/a"), "/a".to_string(), "a".to_string(), 1, None);
    queue.enqueue(8, Direction::Download, VfsPath::new("/d/b"), "/b".to_string(), "b".to_string(), 1, None);

    let saved = unfinished(&queue, |session| format!("host-{session}"));

    assert_eq!(saved.iter().map(|job| job.connection.as_str()).collect::<Vec<_>>(), ["host-7", "host-8"]);
    assert!(saved.iter().all(|job| job.batch.is_none() && job.label.is_none()));
}

#[test]
fn nothing_unfinished_saves_nothing() {
    let (queue, _) = queue_with(&[JobStatus::Completed, JobStatus::Cancelled]);

    assert!(unfinished(&queue, names).is_empty());
}

#[test]
fn saved_jobs_survive_a_round_trip_through_the_file_text() {
    let (queue, _) = queue_with(&[JobStatus::Queued, JobStatus::Failed("boom".to_string())]);
    let saved = unfinished(&queue, names);

    let dir = tempfile::tempdir().unwrap();
    let paths = paths_in(&dir);
    std::fs::write(paths.unfinished_transfers_file(), render(&saved).unwrap()).unwrap();

    match load(&paths) {
        Loaded::Ready(loaded) => assert_eq!(loaded, saved),
        _ => panic!("the file should load"),
    }
}

#[test]
fn a_missing_file_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();

    assert!(matches!(load(&Paths::in_dir(dir.path())), Loaded::Missing));
}

#[test]
fn an_unreadable_file_is_set_aside_with_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_in(&dir);
    std::fs::write(paths.unfinished_transfers_file(), "jobs = [[[").unwrap();

    match load(&paths) {
        Loaded::SetAside { warning, .. } => {
            assert!(warning.starts_with("The unfinished transfers file was unreadable"))
        }
        _ => panic!("expected the file to be set aside"),
    }
    assert!(!paths.unfinished_transfers_file().exists());
}

#[test]
fn a_file_with_unknown_extra_fields_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_in(&dir);
    let text = "[[jobs]]\nconnection = \"srv\"\ndirection = \"download\"\nlocal_path = \"/d/a\"\nremote_path = \"/a\"\ndisplay_name = \"a\"\ntotal_bytes = 5\nfuture_field = true\n";
    std::fs::write(paths.unfinished_transfers_file(), text).unwrap();

    match load(&paths) {
        Loaded::Ready(jobs) => {
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].transferred_bytes, 0);
            assert!(!jobs[0].remove_source);
        }
        _ => panic!("the file should load"),
    }
}

#[test]
fn the_reason_shown_for_a_restored_job_names_the_cause() {
    assert_eq!(INTERRUPTED_REASON, "Porthmos was closed before this finished");
}
