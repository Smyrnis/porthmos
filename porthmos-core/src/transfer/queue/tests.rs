use super::*;

fn queue_with_one_job(queue: &mut TransferQueue) -> u64 {
    queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from("/local/file.txt"),
        "/remote/file.txt".to_string(),
        "file.txt".to_string(),
        100,
        None,
    )
}

fn queue_with_a_batch_job(queue: &mut TransferQueue, batch_id: u64, name: &str, total_bytes: u64) -> u64 {
    queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from(format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        total_bytes,
        Some(batch_id),
    )
}

#[test]
fn enqueue_assigns_increasing_ids() {
    let mut queue = TransferQueue::new();
    let first = queue_with_one_job(&mut queue);
    let second = queue_with_one_job(&mut queue);
    assert_ne!(first, second);
}

#[test]
fn start_batch_assigns_increasing_ids() {
    let mut queue = TransferQueue::new();
    let first = queue.start_batch("batch".to_string());
    let second = queue.start_batch("batch".to_string());
    assert_ne!(first, second);
}

#[test]
fn retry_or_give_up_requeues_within_the_attempt_limit() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    let mut job = queue.get_mut(id).unwrap();
    job.status = JobStatus::Failed("boom".to_string());
    job.attempts = 1;
    job.transferred_bytes = 50;
    drop(job);

    assert!(queue.retry_or_give_up(id));
    let job = queue.get(id).unwrap();
    assert_eq!(job.status, JobStatus::Queued);
    assert_eq!(job.transferred_bytes, 50);
}

#[test]
fn retry_or_give_up_stops_after_max_attempts() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    let mut job = queue.get_mut(id).unwrap();
    job.status = JobStatus::Failed("boom".to_string());
    job.attempts = 3;
    drop(job);

    assert!(!queue.retry_or_give_up(id));
}

#[test]
fn fail_queued_for_session_marks_only_that_sessions_queued_jobs() {
    let mut queue = TransferQueue::new();
    let a1 = queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from("/local/a1.txt"),
        "/remote/a1.txt".to_string(),
        "a1.txt".to_string(),
        10,
        None,
    );
    let a2 = queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from("/local/a2.txt"),
        "/remote/a2.txt".to_string(),
        "a2.txt".to_string(),
        10,
        None,
    );
    let other = queue_with_one_job(&mut queue);
    queue.get_mut(other).unwrap().session_id = 2;
    let in_progress = queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from("/local/a3.txt"),
        "/remote/a3.txt".to_string(),
        "a3.txt".to_string(),
        10,
        None,
    );
    queue.get_mut(in_progress).unwrap().status = JobStatus::InProgress;

    let count = queue.fail_queued_for_session(1, "session disconnected");

    assert_eq!(count, 2);
    assert_eq!(queue.get(a1).unwrap().status, JobStatus::Failed("session disconnected".to_string()));
    assert_eq!(queue.get(a2).unwrap().status, JobStatus::Failed("session disconnected".to_string()));
    assert_eq!(queue.get(other).unwrap().status, JobStatus::Queued);
    assert_eq!(queue.get(in_progress).unwrap().status, JobStatus::InProgress);
}

#[test]
fn batch_progress_aggregates_across_every_job_in_the_batch() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("batch".to_string());
    let first = queue_with_a_batch_job(&mut queue, batch_id, "a.txt", 100);
    let second = queue_with_a_batch_job(&mut queue, batch_id, "b.txt", 200);
    let other_batch = queue.start_batch("batch".to_string());
    queue_with_a_batch_job(&mut queue, other_batch, "c.txt", 999);

    queue.get_mut(first).unwrap().status = JobStatus::Completed;
    queue.get_mut(first).unwrap().transferred_bytes = 100;
    queue.get_mut(second).unwrap().status = JobStatus::InProgress;
    queue.get_mut(second).unwrap().transferred_bytes = 50;

    let progress = queue.batch_progress(batch_id);

    assert_eq!(progress.total_files, 2);
    assert_eq!(progress.completed_files, 1);
    assert_eq!(progress.total_bytes, 300);
    assert_eq!(progress.transferred_bytes, 150);
}

fn enqueue_for(queue: &mut TransferQueue, session_id: u64, direction: Direction, name: &str) -> u64 {
    queue.enqueue(
        session_id,
        direction,
        VfsPath::from(format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        10,
        None,
    )
}

#[test]
fn startable_returns_queued_jobs_in_order_up_to_the_limit() {
    let mut queue = TransferQueue::new();
    let first = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    let second = enqueue_for(&mut queue, 1, Direction::Upload, "b.txt");
    enqueue_for(&mut queue, 1, Direction::Upload, "c.txt");

    assert_eq!(queue.startable(2), vec![first, second]);
}

#[test]
fn startable_counts_jobs_that_are_already_active() {
    let mut queue = TransferQueue::new();
    let active = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    queue.get_mut(active).unwrap().status = JobStatus::InProgress;
    let next = enqueue_for(&mut queue, 1, Direction::Upload, "b.txt");
    enqueue_for(&mut queue, 1, Direction::Upload, "c.txt");

    assert_eq!(queue.startable(2), vec![next]);
}

#[test]
fn startable_with_a_limit_of_one_waits_for_the_active_job() {
    let mut queue = TransferQueue::new();
    let active = queue_with_one_job(&mut queue);
    queue.get_mut(active).unwrap().status = JobStatus::InProgress;
    queue_with_one_job(&mut queue);

    assert!(queue.startable(1).is_empty());
}

#[test]
fn startable_skips_finished_failed_and_cancelled_jobs() {
    let mut queue = TransferQueue::new();
    for status in [JobStatus::Completed, JobStatus::Failed("boom".to_string()), JobStatus::Cancelled] {
        let id = queue_with_one_job(&mut queue);
        queue.get_mut(id).unwrap().status = status;
    }
    let queued = queue_with_one_job(&mut queue);

    assert_eq!(queue.startable(4), vec![queued]);
}

#[test]
fn active_jobs_and_active_count_cover_every_in_progress_job() {
    let mut queue = TransferQueue::new();
    let first = queue_with_one_job(&mut queue);
    let second = queue_with_one_job(&mut queue);
    queue_with_one_job(&mut queue);
    queue.get_mut(first).unwrap().status = JobStatus::InProgress;
    queue.get_mut(second).unwrap().status = JobStatus::InProgress;

    let active: Vec<u64> = queue.active_jobs().map(|job| job.id).collect();

    assert_eq!(active, vec![first, second]);
    assert_eq!(queue.active_count(), 2);
}

#[test]
fn active_ids_for_session_returns_only_that_sessions_in_progress_jobs() {
    let mut queue = TransferQueue::new();
    let mine = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    let theirs = enqueue_for(&mut queue, 2, Direction::Upload, "b.txt");
    enqueue_for(&mut queue, 1, Direction::Upload, "c.txt");
    queue.get_mut(mine).unwrap().status = JobStatus::InProgress;
    queue.get_mut(theirs).unwrap().status = JobStatus::InProgress;

    assert_eq!(queue.active_ids_for_session(1), vec![mine]);
}

#[test]
fn has_pending_is_true_for_queued_and_in_progress_jobs() {
    let mut queue = TransferQueue::new();
    let job = enqueue_for(&mut queue, 1, Direction::Download, "a.txt");

    assert!(queue.has_pending(1, Direction::Download));
    queue.get_mut(job).unwrap().status = JobStatus::InProgress;
    assert!(queue.has_pending(1, Direction::Download));
}

#[test]
fn has_pending_ignores_other_sessions_directions_and_finished_jobs() {
    let mut queue = TransferQueue::new();
    let done = enqueue_for(&mut queue, 1, Direction::Download, "done.txt");
    queue.get_mut(done).unwrap().status = JobStatus::Completed;
    enqueue_for(&mut queue, 2, Direction::Download, "other_session.txt");
    enqueue_for(&mut queue, 1, Direction::Upload, "other_direction.txt");

    assert!(!queue.has_pending(1, Direction::Download));
}

#[test]
fn cancel_all_queued_cancels_every_queued_job_and_nothing_else() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("batch".to_string());
    let batch_job = queue_with_a_batch_job(&mut queue, batch_id, "a.txt", 10);
    let loose_job = queue_with_one_job(&mut queue);
    let active = queue_with_one_job(&mut queue);
    queue.get_mut(active).unwrap().status = JobStatus::InProgress;

    let count = queue.cancel_all_queued();

    assert_eq!(count, 2);
    assert_eq!(queue.get(batch_job).unwrap().status, JobStatus::Cancelled);
    assert_eq!(queue.get(loose_job).unwrap().status, JobStatus::Cancelled);
    assert_eq!(queue.get(active).unwrap().status, JobStatus::InProgress);
}

#[test]
fn startable_holds_back_a_second_job_for_the_same_destination() {
    let mut queue = TransferQueue::new();
    let first = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    let other = enqueue_for(&mut queue, 1, Direction::Upload, "b.txt");

    assert_eq!(queue.startable(4), vec![first, other]);
}

#[test]
fn startable_waits_while_the_same_destination_is_in_progress() {
    let mut queue = TransferQueue::new();
    let first = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    queue.get_mut(first).unwrap().status = JobStatus::InProgress;
    enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");

    assert!(queue.startable(4).is_empty());
}

#[test]
fn downloads_to_the_same_local_file_from_different_sessions_do_not_overlap() {
    let mut queue = TransferQueue::new();
    let first = enqueue_for(&mut queue, 1, Direction::Download, "a.txt");
    enqueue_for(&mut queue, 2, Direction::Download, "a.txt");

    assert_eq!(queue.startable(4), vec![first]);
}

#[test]
fn uploads_to_the_same_path_on_different_sessions_run_together() {
    let mut queue = TransferQueue::new();
    let first = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    let second = enqueue_for(&mut queue, 2, Direction::Upload, "a.txt");

    assert_eq!(queue.startable(4), vec![first, second]);
}

#[test]
fn get_finds_each_job_by_id_among_many() {
    let mut queue = TransferQueue::new();
    let ids: Vec<u64> = (0..1000).map(|n| enqueue_for(&mut queue, 1, Direction::Upload, &format!("{n}.txt"))).collect();

    for id in ids {
        assert_eq!(queue.get(id).unwrap().id, id);
        assert_eq!(queue.get_mut(id).unwrap().id, id);
    }
    assert!(queue.get(1000).is_none());
}

#[test]
fn start_batch_remembers_its_label() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("photos".to_string());

    assert_eq!(queue.batch_label(batch_id), Some("photos"));
    assert_eq!(queue.batch_label(batch_id + 1), None);
}

#[test]
fn jobs_iterates_in_queue_order() {
    let mut queue = TransferQueue::new();
    let first = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    let second = enqueue_for(&mut queue, 1, Direction::Upload, "b.txt");

    let ids: Vec<u64> = queue.jobs().map(|job| job.id).collect();

    assert_eq!(ids, vec![first, second]);
}

#[test]
fn retry_jobs_requeues_only_failed_and_cancelled_jobs_and_resets_them() {
    let mut queue = TransferQueue::new();
    let failed = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    let cancelled = enqueue_for(&mut queue, 1, Direction::Upload, "b.txt");
    let completed = enqueue_for(&mut queue, 1, Direction::Upload, "c.txt");
    let running = enqueue_for(&mut queue, 1, Direction::Upload, "d.txt");
    {
        let mut job = queue.get_mut(failed).unwrap();
        job.status = JobStatus::Failed("boom".to_string());
        job.attempts = 3;
        job.transferred_bytes = 5;
    }
    queue.get_mut(cancelled).unwrap().status = JobStatus::Cancelled;
    queue.get_mut(completed).unwrap().status = JobStatus::Completed;
    queue.get_mut(running).unwrap().status = JobStatus::InProgress;

    let count = queue.retry_jobs(&[failed, cancelled, completed, running]);

    assert_eq!(count, 2);
    let job = queue.get(failed).unwrap();
    assert_eq!((job.status.clone(), job.attempts, job.transferred_bytes), (JobStatus::Queued, 0, 5));
    assert_eq!(queue.get(cancelled).unwrap().status, JobStatus::Queued);
    assert_eq!(queue.get(completed).unwrap().status, JobStatus::Completed);
    assert_eq!(queue.get(running).unwrap().status, JobStatus::InProgress);
}

#[test]
fn remove_jobs_removes_jobs_and_the_label_of_a_batch_it_emptied() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("photos".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch_id, "a.txt", 10);
    let b = queue_with_a_batch_job(&mut queue, batch_id, "b.txt", 10);

    queue.remove_jobs(&[a, b]);

    assert!(queue.get(a).is_none());
    assert!(queue.get(b).is_none());
    assert_eq!(queue.batch_label(batch_id), None);
}

#[test]
fn remove_jobs_keeps_the_label_of_a_batch_that_still_has_jobs() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("photos".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch_id, "a.txt", 10);
    queue_with_a_batch_job(&mut queue, batch_id, "b.txt", 10);

    queue.remove_jobs(&[a]);

    assert_eq!(queue.batch_label(batch_id), Some("photos"));
}

#[test]
fn remove_jobs_keeps_labels_of_batches_it_did_not_touch() {
    let mut queue = TransferQueue::new();
    let scanning = queue.start_batch("still scanning".to_string());
    let single = queue_with_one_job(&mut queue);

    queue.remove_jobs(&[single]);

    assert_eq!(queue.batch_label(scanning), Some("still scanning"));
}

#[test]
fn forget_batch_if_empty_drops_only_an_unused_label() {
    let mut queue = TransferQueue::new();
    let unused = queue.start_batch("scan failed".to_string());
    let used = queue.start_batch("photos".to_string());
    queue_with_a_batch_job(&mut queue, used, "a.txt", 10);

    queue.forget_batch_if_empty(unused);
    queue.forget_batch_if_empty(used);

    assert_eq!(queue.batch_label(unused), None);
    assert_eq!(queue.batch_label(used), Some("photos"));
}

#[test]
fn new_jobs_do_not_resume() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);

    assert!(!queue.get(id).unwrap().resume);
}

#[test]
fn an_automatic_retry_resumes() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    queue.get_mut(id).unwrap().status = JobStatus::Failed("boom".to_string());
    queue.get_mut(id).unwrap().attempts = 1;

    assert!(queue.retry_or_give_up(id));

    assert!(queue.get(id).unwrap().resume);
}

#[test]
fn a_manual_retry_resumes() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    queue.get_mut(id).unwrap().status = JobStatus::Cancelled;
    queue.get_mut(id).unwrap().attempts = 1;

    queue.retry_jobs(&[id]);

    assert!(queue.get(id).unwrap().resume);
}

#[test]
fn a_manual_retry_of_a_job_that_never_started_does_not_resume() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    queue.get_mut(id).unwrap().status = JobStatus::Cancelled;

    queue.retry_jobs(&[id]);

    assert!(!queue.get(id).unwrap().resume);
}

#[test]
fn a_session_limit_holds_back_that_sessions_jobs_but_not_others() {
    let mut queue = TransferQueue::new();
    let first = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    enqueue_for(&mut queue, 1, Direction::Upload, "b.txt");
    let other = enqueue_for(&mut queue, 2, Direction::Upload, "c.txt");

    let limits = |session: u64| (session == 1).then_some(1);

    assert_eq!(queue.startable_limited(4, limits), vec![first, other]);
}

#[test]
fn a_session_limit_counts_that_sessions_active_jobs() {
    let mut queue = TransferQueue::new();
    let active = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    queue.get_mut(active).unwrap().status = JobStatus::InProgress;
    enqueue_for(&mut queue, 1, Direction::Upload, "b.txt");

    assert!(queue.startable_limited(4, |_| Some(1)).is_empty());
    assert_eq!(queue.startable_limited(4, |_| Some(2)).len(), 1);
}

use std::time::Instant;

use super::super::rows::{QueueRow, RowKind, RowState};

fn reference_rows(queue: &TransferQueue, positions: &HashMap<RowKind, u64>) -> Vec<QueueRow> {
    let mut groups: Vec<Vec<&TransferJob>> = Vec::new();
    let mut batch_group: HashMap<u64, usize> = HashMap::new();
    for job in queue.jobs() {
        match job.batch_id {
            Some(batch_id) => match batch_group.get(&batch_id) {
                Some(&index) => groups[index].push(job),
                None => {
                    batch_group.insert(batch_id, groups.len());
                    groups.push(vec![job]);
                }
            },
            None => groups.push(vec![job]),
        }
    }
    let kind_of = |jobs: &Vec<&TransferJob>| match jobs[0].batch_id {
        Some(batch_id) => RowKind::Batch(batch_id),
        None => RowKind::Single(jobs[0].id),
    };
    groups.sort_by_key(|jobs| (positions[&kind_of(jobs)], jobs[0].id));
    groups
        .iter()
        .map(|jobs| {
            let first = jobs[0];
            let (kind, label) = match first.batch_id {
                Some(batch_id) => {
                    (RowKind::Batch(batch_id), queue.batch_label(batch_id).unwrap_or(&first.display_name).to_string())
                }
                None => (RowKind::Single(first.id), first.display_name.clone()),
            };
            let completed = jobs.iter().filter(|job| job.status == JobStatus::Completed).count();
            let failed = jobs.iter().filter(|job| matches!(job.status, JobStatus::Failed(_))).count();
            let state = if jobs.iter().any(|job| job.status == JobStatus::InProgress) {
                RowState::Running
            } else if jobs.iter().any(|job| job.status == JobStatus::Queued) {
                if jobs.iter().filter(|job| job.status == JobStatus::Queued).all(|job| job.paused) {
                    RowState::Paused
                } else {
                    RowState::Queued
                }
            } else if completed == jobs.len() {
                RowState::Done
            } else if failed == 0 {
                RowState::Cancelled
            } else if completed > 0 {
                RowState::PartlyFailed(failed)
            } else {
                RowState::Failed
            };
            QueueRow {
                kind,
                label,
                direction: first.direction,
                files_done: completed,
                files_total: jobs.len(),
                bytes_done: jobs.iter().map(|job| job.transferred_bytes).sum(),
                bytes_total: jobs.iter().map(|job| job.total_bytes).sum(),
                state,
                paused: jobs
                    .iter()
                    .any(|job| job.paused && matches!(job.status, JobStatus::Queued | JobStatus::InProgress)),
                session_id: None,
                failure: None,
            }
        })
        .collect()
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % bound
    }
}

fn add_job(queue: &mut TransferQueue, name: &str, batch: Option<u64>, session: u64, direction: Direction) -> u64 {
    queue.enqueue(
        session,
        direction,
        VfsPath::from(format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        100,
        batch,
    )
}

fn reference_startable(
    queue: &TransferQueue, positions: &HashMap<RowKind, u64>, limit: usize,
    session_limit: impl Fn(u64) -> Option<usize>,
) -> Vec<u64> {
    let running = |job: &&TransferJob| job.status == JobStatus::InProgress;
    let free_slots = limit.saturating_sub(queue.jobs().filter(running).count());
    let mut busy_destinations: HashSet<Destination> =
        queue.jobs().filter(running).map(TransferJob::destination).collect();
    let mut per_session: HashMap<u64, usize> = HashMap::new();
    for job in queue.jobs().filter(running) {
        *per_session.entry(job.session_id).or_default() += 1;
    }
    for job in queue.jobs().filter(|job| job.status == JobStatus::Queued && job.paused && job.resume) {
        busy_destinations.insert(job.destination());
    }
    let rank = |job: &TransferJob| positions[&job.batch_id.map_or(RowKind::Single(job.id), RowKind::Batch)];
    let mut waiting: Vec<&TransferJob> =
        queue.jobs().filter(|job| job.status == JobStatus::Queued && !job.paused).collect();
    waiting.sort_by_key(|job| (rank(job), job.id));
    let mut startable = Vec::new();
    for job in waiting {
        if startable.len() == free_slots {
            break;
        }
        let in_flight = per_session.get(&job.session_id).copied().unwrap_or(0);
        if session_limit(job.session_id).is_some_and(|cap| in_flight >= cap) {
            continue;
        }
        if busy_destinations.insert(job.destination()) {
            *per_session.entry(job.session_id).or_default() += 1;
            startable.push(job.id);
        }
    }
    startable
}

fn finished_map(rows: &[QueueRow]) -> HashMap<RowKind, bool> {
    rows.iter().map(|row| (row.kind, row.is_finished())).collect()
}

fn pick(random: &mut Lcg, ids: &[u64]) -> Option<u64> {
    if ids.is_empty() { None } else { Some(ids[random.next(ids.len() as u64) as usize]) }
}

#[test]
fn everything_matches_a_recount_from_the_jobs_after_every_random_operation() {
    let mut queue = TransferQueue::new();
    let mut random = Lcg(7);
    let batches: Vec<u64> = (0..3).map(|index| queue.start_batch(format!("batch {index}"))).collect();
    let mut ids: Vec<u64> = Vec::new();
    let mut removals = 0;
    let mut previous = finished_map(&queue.rows());
    let mut positions: HashMap<RowKind, u64> = HashMap::new();
    for step in 0..1200u64 {
        match random.next(13) {
            0 | 1 => {
                let batch = match random.next(4) {
                    3 => None,
                    index => Some(batches[index as usize]),
                };
                let direction = if random.next(2) == 0 { Direction::Upload } else { Direction::Download };
                ids.push(add_job(&mut queue, &format!("f{step}"), batch, random.next(3), direction));
            }
            2 | 3 => {
                if let Some(id) = pick(&mut random, &ids) {
                    let status = match random.next(5) {
                        0 => JobStatus::Queued,
                        1 => JobStatus::InProgress,
                        2 => JobStatus::Completed,
                        3 => JobStatus::Failed("boom".to_string()),
                        _ => JobStatus::Cancelled,
                    };
                    if let Some(mut job) = queue.get_mut(id) {
                        job.status = status;
                    }
                }
            }
            4 => {
                if let Some(id) = pick(&mut random, &ids)
                    && let Some(mut job) = queue.get_mut(id)
                {
                    job.transferred_bytes = random.next(101);
                }
            }
            5 => {
                let chosen: Vec<u64> = ids.iter().copied().filter(|_| random.next(4) == 0).collect();
                queue.retry_jobs(&chosen);
            }
            6 => {
                queue.cancel_all_queued();
            }
            7 => {
                queue.fail_queued_for_session(random.next(3), "gone");
            }
            8 => {
                if let Some(id) = pick(&mut random, &ids) {
                    queue.retry_or_give_up(id);
                }
            }
            10 | 11 => {
                if let Some(id) = pick(&mut random, &ids)
                    && let Some(mut job) = queue.get_mut(id)
                {
                    job.paused = random.next(2) == 0;
                    job.resume = random.next(2) == 0;
                }
            }
            9 => {
                if let Some(id) = pick(&mut random, &ids)
                    && let Some(mut job) = queue.get_mut(id)
                {
                    job.session_id = random.next(3);
                }
            }
            _ => {
                let whole_batch = random.next(3) == 0;
                let chosen: Vec<u64> = if whole_batch {
                    let batch = batches[random.next(3) as usize];
                    queue.jobs().filter(|job| job.batch_id == Some(batch)).map(|job| job.id).collect()
                } else {
                    ids.iter().copied().filter(|_| random.next(6) == 0).collect()
                };
                removals += chosen.len();
                queue.remove_jobs(&chosen);
                ids.retain(|id| queue.get(*id).is_some());
            }
        }
        let rows = queue.rows();
        positions.retain(|kind, _| rows.iter().any(|row| row.kind == *kind));
        for row in &rows {
            positions.entry(row.kind).or_insert_with(|| queue.jobs_of(row.kind)[0].id);
        }
        let without_extras: Vec<QueueRow> =
            rows.iter().map(|row| QueueRow { session_id: None, failure: None, ..row.clone() }).collect();
        assert_eq!(without_extras, reference_rows(&queue, &positions), "rows differ after step {step}");
        for row in &rows {
            match row.state {
                RowState::Failed | RowState::PartlyFailed(_) => {
                    assert!(row.failure.is_some(), "a failed row has a failure after step {step}")
                }
                RowState::Done | RowState::Cancelled => {
                    assert!(row.failure.is_none(), "a row without failed jobs has no failure after step {step}")
                }
                _ => {}
            }
        }
        queue.assert_consistent();
        let changes = queue.take_changes();
        let current = finished_map(&rows);
        for (kind, was_finished) in &previous {
            match current.get(kind) {
                Some(now) if now != was_finished => assert!(
                    changes
                        .iter()
                        .any(|change| matches!(change, RowChange::Finished(k) | RowChange::Reopened(k) if k == kind)),
                    "{kind:?} flipped without a change after step {step}: {changes:?}"
                ),
                None => assert!(
                    changes.contains(&RowChange::Removed(*kind)),
                    "{kind:?} vanished without a Removed change after step {step}: {changes:?}"
                ),
                _ => {}
            }
        }
        previous = current;
        for session in 0..3 {
            for direction in [Direction::Upload, Direction::Download] {
                let expected = queue.jobs().any(|job| {
                    job.session_id == session
                        && job.direction == direction
                        && (job.status == JobStatus::InProgress || job.status == JobStatus::Queued && !job.paused)
                });
                assert_eq!(queue.has_pending(session, direction), expected, "pending after step {step}");
            }
        }
        assert_eq!(queue.queued_count(), queue.jobs().filter(|job| job.status == JobStatus::Queued).count());
        for batch in &batches {
            let jobs: Vec<&TransferJob> = queue.jobs().filter(|job| job.batch_id == Some(*batch)).collect();
            let progress = queue.batch_progress(*batch);
            assert_eq!(progress.total_files, jobs.len(), "batch files after step {step}");
            assert_eq!(progress.completed_files, jobs.iter().filter(|job| job.status == JobStatus::Completed).count());
            assert_eq!(progress.total_bytes, jobs.iter().map(|job| job.total_bytes).sum::<u64>());
            assert_eq!(progress.transferred_bytes, jobs.iter().map(|job| job.transferred_bytes).sum::<u64>());
        }
        let limit_of = |session: u64| match session {
            0 => Some(2),
            1 => Some(4),
            _ => None,
        };
        for limit in [1, 3, 8] {
            assert_eq!(
                queue.startable_limited(limit, limit_of),
                reference_startable(&queue, &positions, limit, limit_of),
                "startable at limit {limit} after step {step}"
            );
        }
    }
    assert!(removals > 20, "the random run removed only {removals} jobs");
}

#[test]
fn removing_the_last_unfinished_job_of_a_row_finishes_it() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("docs".to_string());
    let done = add_job(&mut queue, "a", Some(batch), 1, Direction::Upload);
    let waiting = add_job(&mut queue, "b", Some(batch), 1, Direction::Upload);
    queue.get_mut(done).unwrap().status = JobStatus::Completed;
    drain(&mut queue);

    queue.remove_jobs(&[waiting]);

    assert_eq!(drain(&mut queue), vec![RowChange::Finished(RowKind::Batch(batch))]);
    assert_eq!(queue.row(RowKind::Batch(batch)).unwrap().state, RowState::Done);
}

#[test]
fn a_row_keeps_its_place_and_takes_the_label_of_its_first_remaining_job_after_a_partial_removal() {
    let mut queue = TransferQueue::new();
    let first = add_job(&mut queue, "first", Some(99), 1, Direction::Upload);
    let single = add_job(&mut queue, "single", None, 1, Direction::Upload);
    add_job(&mut queue, "second", Some(99), 1, Direction::Download);

    queue.remove_jobs(&[first]);

    let rows = queue.rows();
    assert_eq!(rows.iter().map(|row| row.kind).collect::<Vec<_>>(), vec![RowKind::Batch(99), RowKind::Single(single)]);
    assert_eq!(rows[0].label, "second");
    assert_eq!(rows[0].direction, Direction::Download);
    queue.assert_consistent();
}

#[test]
#[should_panic(expected = "row counters differ")]
fn the_consistency_check_notices_a_corrupt_counter() {
    let mut queue = TransferQueue::new();
    add_job(&mut queue, "a", None, 1, Direction::Upload);

    queue.corrupt_for_test();

    queue.assert_consistent();
}

#[test]
fn starting_does_not_walk_the_queue_of_a_capped_session() {
    fn per_call(jobs: usize) -> f64 {
        let mut queue = TransferQueue::new();
        for index in 0..2 {
            let id = add_job(&mut queue, &format!("r{index}"), None, 1, Direction::Upload);
            queue.get_mut(id).unwrap().status = JobStatus::InProgress;
        }
        for index in 0..jobs {
            add_job(&mut queue, &format!("a{index}"), None, 1, Direction::Download);
        }
        for index in 0..jobs {
            add_job(&mut queue, &format!("b{index}"), None, 2, Direction::Download);
        }
        let calls = 300;
        let start = Instant::now();
        for _ in 0..calls {
            std::hint::black_box(queue.startable_limited(4, |session| if session == 1 { Some(2) } else { None }));
        }
        start.elapsed().as_secs_f64() / calls as f64
    }

    let small = (0..3).map(|_| per_call(1_000)).fold(f64::MAX, f64::min);
    let large = (0..3).map(|_| per_call(100_000)).fold(f64::MAX, f64::min);

    assert!(large < small * 30.0, "per call: {small:.9}s with 1k queued, {large:.9}s with 100k queued");
}

#[test]
fn a_row_is_looked_up_by_kind_and_keeps_its_jobs_in_order() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("docs".to_string());
    let a = add_job(&mut queue, "a", Some(batch), 1, Direction::Upload);
    let single = add_job(&mut queue, "s", None, 1, Direction::Upload);
    let b = add_job(&mut queue, "b", Some(batch), 1, Direction::Upload);

    assert_eq!(queue.row(RowKind::Batch(batch)).unwrap().files_total, 2);
    assert_eq!(queue.row(RowKind::Single(single)).unwrap().label, "s");
    assert!(queue.row(RowKind::Single(a)).is_none());
    assert!(queue.row(RowKind::Scan(3)).is_none());
    assert_eq!(queue.jobs_of(RowKind::Batch(batch)).iter().map(|job| job.id).collect::<Vec<_>>(), vec![a, b]);
    assert!(queue.jobs_of(RowKind::Batch(99)).is_empty());
}

fn drain(queue: &mut TransferQueue) -> Vec<RowChange> {
    queue.take_changes()
}

#[test]
fn a_row_reports_when_it_finishes_and_when_it_reopens() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("docs".to_string());
    let a = add_job(&mut queue, "a", Some(batch), 1, Direction::Upload);
    let b = add_job(&mut queue, "b", Some(batch), 1, Direction::Upload);
    assert!(drain(&mut queue).is_empty());

    queue.get_mut(a).unwrap().status = JobStatus::Completed;
    assert!(drain(&mut queue).is_empty());
    queue.get_mut(b).unwrap().status = JobStatus::Failed("boom".to_string());
    assert_eq!(drain(&mut queue), vec![RowChange::Finished(RowKind::Batch(batch))]);

    queue.retry_jobs(&[b]);
    assert_eq!(drain(&mut queue), vec![RowChange::Reopened(RowKind::Batch(batch))]);
    assert!(drain(&mut queue).is_empty());
}

#[test]
fn cancelling_and_failing_queued_jobs_finish_their_rows() {
    let mut queue = TransferQueue::new();
    let single = add_job(&mut queue, "one", None, 1, Direction::Upload);
    let other = add_job(&mut queue, "two", None, 2, Direction::Upload);

    queue.cancel_all_queued();
    let mut changes = drain(&mut queue);
    changes.sort_by_key(|change| format!("{change:?}"));
    assert_eq!(
        changes,
        vec![RowChange::Finished(RowKind::Single(single)), RowChange::Finished(RowKind::Single(other))]
    );

    queue.retry_jobs(&[single, other]);
    drain(&mut queue);
    queue.fail_queued_for_session(1, "session disconnected");
    assert_eq!(drain(&mut queue), vec![RowChange::Finished(RowKind::Single(single))]);
}

#[test]
fn removing_every_job_of_a_row_reports_it_removed() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("docs".to_string());
    let a = add_job(&mut queue, "a", Some(batch), 1, Direction::Upload);
    let b = add_job(&mut queue, "b", Some(batch), 1, Direction::Upload);
    let keep = add_job(&mut queue, "k", None, 1, Direction::Upload);
    for id in [a, b] {
        queue.get_mut(id).unwrap().status = JobStatus::Completed;
    }
    drain(&mut queue);

    queue.remove_jobs(&[a]);
    assert!(drain(&mut queue).is_empty());
    queue.remove_jobs(&[b]);

    assert_eq!(drain(&mut queue), vec![RowChange::Removed(RowKind::Batch(batch))]);
    assert!(queue.row(RowKind::Batch(batch)).is_none());
    assert!(queue.batch_label(batch).is_none());
    assert_eq!(queue.rows().len(), 1);
    assert!(queue.get(keep).is_some());
}

#[test]
fn a_job_that_moves_to_another_session_keeps_the_indexes_right() {
    let mut queue = TransferQueue::new();
    let id = add_job(&mut queue, "a", None, 1, Direction::Upload);
    assert!(queue.has_pending(1, Direction::Upload));

    queue.get_mut(id).unwrap().session_id = 2;

    assert!(!queue.has_pending(1, Direction::Upload));
    assert!(queue.has_pending(2, Direction::Upload));
    queue.assert_consistent();
}

#[test]
fn starting_looks_only_at_queued_jobs_and_stops_when_every_session_is_capped() {
    let mut queue = TransferQueue::new();
    let running: Vec<u64> =
        (0..3).map(|index| add_job(&mut queue, &format!("r{index}"), None, 1, Direction::Upload)).collect();
    for id in &running {
        queue.get_mut(*id).unwrap().status = JobStatus::InProgress;
    }
    for index in 0..50 {
        add_job(&mut queue, &format!("q{index}"), None, 1, Direction::Download);
    }

    assert!(queue.startable_limited(10, |_| Some(3)).is_empty());
    assert_eq!(queue.startable_limited(10, |_| Some(5)).len(), 2);
    assert_eq!(queue.startable_limited(5, |_| None).len(), 2);
}

#[test]
fn the_cost_of_a_snapshot_does_not_grow_with_the_number_of_jobs() {
    fn per_event(jobs: usize) -> f64 {
        let mut queue = TransferQueue::new();
        let batch = queue.start_batch("big".to_string());
        let ids: Vec<u64> = (0..jobs)
            .map(|index| add_job(&mut queue, &format!("f{index}"), Some(batch), 1, Direction::Upload))
            .collect();
        let events = 2000usize;
        let start = Instant::now();
        for index in 0..events {
            let id = ids[index % ids.len()];
            let previous = ids[(index + ids.len() - 1) % ids.len()];
            queue.get_mut(previous).unwrap().status = JobStatus::Queued;
            queue.get_mut(id).unwrap().status = JobStatus::InProgress;
            queue.startable_limited(16, |_| Some(6));
            queue.has_pending(1, Direction::Upload);
            std::hint::black_box(crate::transfer::TransferSnapshot::of(&queue, &[]));
            queue.take_changes();
        }
        start.elapsed().as_secs_f64() / events as f64
    }

    let small = (0..3).map(|_| per_event(1_000)).fold(f64::MAX, f64::min);
    let large = (0..3).map(|_| per_event(100_000)).fold(f64::MAX, f64::min);

    assert!(large < small * 30.0, "per event: {small:.9}s at 1k jobs, {large:.9}s at 100k jobs");
}

#[test]
fn a_batch_is_flagged_for_unkept_times_only_the_first_time() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("sync".to_string());
    let other = queue.start_batch("again".to_string());

    assert!(queue.flag_unkept_times(batch_id));
    assert!(!queue.flag_unkept_times(batch_id));
    assert!(queue.flag_unkept_times(other));
}

#[test]
fn removing_the_jobs_of_a_batch_forgets_its_unkept_times_flag() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("sync".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch_id, "a.txt", 10);
    queue.flag_unkept_times(batch_id);

    queue.remove_jobs(&[a]);

    assert!(!queue.has_unkept_times_flag(batch_id));
}

#[test]
fn a_batch_that_still_has_jobs_keeps_its_unkept_times_flag() {
    let mut queue = TransferQueue::new();
    let batch_id = queue.start_batch("sync".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch_id, "a.txt", 10);
    queue_with_a_batch_job(&mut queue, batch_id, "b.txt", 10);
    queue.flag_unkept_times(batch_id);

    queue.remove_jobs(&[a]);

    assert!(queue.has_unkept_times_flag(batch_id));
}

fn fail_job(queue: &mut TransferQueue, id: u64, message: &str, attempts: u32) {
    let mut job = queue.get_mut(id).unwrap();
    job.attempts = attempts;
    job.status = JobStatus::Failed(message.to_string());
}

#[test]
fn a_single_row_reports_the_session_of_its_job() {
    let mut queue = TransferQueue::new();
    enqueue_for(&mut queue, 7, Direction::Upload, "a.txt");

    assert_eq!(queue.rows()[0].session_id, Some(7));
}

#[test]
fn a_batch_row_reports_the_session_of_its_files() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("folder".to_string());
    queue_with_a_batch_job(&mut queue, batch, "a", 10);
    queue_with_a_batch_job(&mut queue, batch, "b", 10);

    assert_eq!(queue.rows()[0].session_id, Some(1));
}

#[test]
fn a_failed_job_sets_the_rows_failure_with_message_file_and_attempts() {
    let mut queue = TransferQueue::new();
    let id = enqueue_for(&mut queue, 1, Direction::Download, "access.log");

    fail_job(&mut queue, id, "permission denied", 3);

    let failure = queue.rows()[0].failure.clone().expect("a failure");
    assert_eq!(
        failure,
        RowFailure { file: "access.log".to_string(), message: "permission denied".to_string(), attempts: 3 }
    );
}

#[test]
fn a_later_failure_replaces_the_earlier_one_in_a_batch() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("folder".to_string());
    let first = queue_with_a_batch_job(&mut queue, batch, "a", 10);
    let second = queue_with_a_batch_job(&mut queue, batch, "b", 10);

    fail_job(&mut queue, second, "disk full", 1);
    fail_job(&mut queue, first, "timed out", 2);

    let failure = queue.rows()[0].failure.clone().unwrap();
    assert_eq!((failure.file.as_str(), failure.message.as_str(), failure.attempts), ("a", "timed out", 2));
}

#[test]
fn a_row_without_failures_has_no_failure() {
    let mut queue = TransferQueue::new();
    let id = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    queue.get_mut(id).unwrap().status = JobStatus::Completed;

    assert_eq!(queue.rows()[0].failure, None);
}

#[test]
fn retrying_a_failed_row_clears_its_failure() {
    let mut queue = TransferQueue::new();
    let id = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    fail_job(&mut queue, id, "boom", 3);

    queue.retry_jobs(&[id]);

    assert_eq!(queue.rows()[0].failure, None);
}

#[test]
fn removing_the_failed_job_clears_the_failure() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("folder".to_string());
    let good = queue_with_a_batch_job(&mut queue, batch, "a", 10);
    let bad = queue_with_a_batch_job(&mut queue, batch, "b", 10);
    queue.get_mut(good).unwrap().status = JobStatus::Completed;
    fail_job(&mut queue, bad, "boom", 3);

    queue.remove_jobs(&[bad]);

    assert_eq!(queue.rows()[0].failure, None);
}

#[test]
fn a_partly_failed_batch_keeps_the_failure_of_the_last_failed_file() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("folder".to_string());
    let good = queue_with_a_batch_job(&mut queue, batch, "a", 10);
    let bad = queue_with_a_batch_job(&mut queue, batch, "b", 10);
    queue.get_mut(good).unwrap().status = JobStatus::Completed;
    fail_job(&mut queue, bad, "no space left", 3);

    let row = queue.rows().remove(0);

    assert_eq!(row.state, RowState::PartlyFailed(1));
    assert_eq!(row.failure.map(|failure| failure.file), Some("b".to_string()));
}

#[test]
fn failure_bookkeeping_does_not_scan_the_jobs_of_a_row() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("big".to_string());
    let ids: Vec<u64> =
        (0..20_000).map(|index| queue_with_a_batch_job(&mut queue, batch, &format!("f{index}"), 1)).collect();

    let started = Instant::now();
    for id in &ids {
        fail_job(&mut queue, *id, "boom", 1);
    }
    let rows = queue.rows();

    assert!(started.elapsed() < std::time::Duration::from_secs(3), "{:?}", started.elapsed());
    assert_eq!(rows[0].state, RowState::Failed);
}

#[test]
fn a_transient_failure_that_is_retried_never_replaces_another_jobs_failure() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("folder".to_string());
    let permanent = queue_with_a_batch_job(&mut queue, batch, "secret.conf", 10);
    let transient = queue_with_a_batch_job(&mut queue, batch, "big.iso", 10);
    fail_job(&mut queue, permanent, "permission denied", 3);
    {
        let mut job = queue.get_mut(transient).unwrap();
        job.status = JobStatus::InProgress;
        job.attempts = 1;
    }

    let retried = queue.fail_or_retry(transient, "connection reset".to_string());

    assert!(retried);
    assert_eq!(queue.get(transient).unwrap().status, JobStatus::Queued);
    let failure = queue.rows()[0].failure.clone().unwrap();
    assert_eq!((failure.file.as_str(), failure.message.as_str()), ("secret.conf", "permission denied"));
}

#[test]
fn giving_up_marks_the_job_failed_and_records_its_failure() {
    let mut queue = TransferQueue::new();
    let id = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    {
        let mut job = queue.get_mut(id).unwrap();
        job.status = JobStatus::InProgress;
        job.attempts = 3;
    }

    let retried = queue.fail_or_retry(id, "disk full".to_string());

    assert!(!retried);
    assert_eq!(queue.get(id).unwrap().status, JobStatus::Failed("disk full".to_string()));
    assert_eq!(queue.rows()[0].failure.clone().map(|failure| failure.message), Some("disk full".to_string()));
}

#[test]
fn a_retried_job_is_marked_to_resume_and_an_unknown_job_is_ignored() {
    let mut queue = TransferQueue::new();
    let id = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    {
        let mut job = queue.get_mut(id).unwrap();
        job.status = JobStatus::InProgress;
        job.attempts = 1;
    }

    assert!(queue.fail_or_retry(id, "reset".to_string()));
    assert!(queue.get(id).unwrap().resume);
    assert!(!queue.fail_or_retry(999, "x".to_string()));
}

#[test]
fn a_retried_failure_never_flips_a_row_to_finished() {
    let mut queue = TransferQueue::new();
    let id = enqueue_for(&mut queue, 1, Direction::Upload, "a.txt");
    {
        let mut job = queue.get_mut(id).unwrap();
        job.status = JobStatus::InProgress;
        job.attempts = 1;
    }
    queue.take_changes();

    queue.fail_or_retry(id, "reset".to_string());

    assert!(queue.take_changes().is_empty());
}

fn named_job(queue: &mut TransferQueue, name: &str) -> u64 {
    queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from(format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        10,
        None,
    )
}

fn pause(queue: &mut TransferQueue, id: u64) {
    queue.get_mut(id).unwrap().paused = true;
}

fn row_state(queue: &TransferQueue, kind: RowKind) -> RowState {
    queue.row(kind).unwrap().state
}

#[test]
fn a_paused_job_is_never_started_and_the_next_one_takes_its_place() {
    let mut queue = TransferQueue::new();
    let first = named_job(&mut queue, "first");
    let second = named_job(&mut queue, "second");
    pause(&mut queue, first);

    assert_eq!(queue.startable(4), vec![second]);
    assert_eq!(queue.startable(1), vec![second]);
    queue.assert_consistent();
}

#[test]
fn unpausing_makes_the_job_startable_again_in_its_old_place() {
    let mut queue = TransferQueue::new();
    let first = named_job(&mut queue, "first");
    let second = named_job(&mut queue, "second");
    pause(&mut queue, first);

    queue.get_mut(first).unwrap().paused = false;

    assert_eq!(queue.startable(4), vec![first, second]);
    queue.assert_consistent();
}

#[test]
fn nothing_starts_while_every_queued_job_is_paused() {
    let mut queue = TransferQueue::new();
    let ids: Vec<u64> = ["a", "b", "c", "d", "e"].iter().map(|name| named_job(&mut queue, name)).collect();
    for id in ids {
        pause(&mut queue, id);
    }

    assert!(queue.startable(4).is_empty());
    queue.assert_consistent();
}

#[test]
fn a_paused_job_of_another_session_does_not_block_the_first_job_of_this_one() {
    let mut queue = TransferQueue::new();
    let blocked =
        queue.enqueue(1, Direction::Upload, VfsPath::from("/l/a"), "/r/a".to_string(), "a".to_string(), 1, None);
    let other =
        queue.enqueue(2, Direction::Upload, VfsPath::from("/l/b"), "/r/b".to_string(), "b".to_string(), 1, None);
    pause(&mut queue, blocked);

    assert_eq!(queue.startable(4), vec![other]);
}

#[test]
fn a_row_whose_queued_jobs_are_all_paused_is_paused() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("b".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch, "a", 10);
    let b = queue_with_a_batch_job(&mut queue, batch, "b", 10);
    assert_eq!(row_state(&queue, RowKind::Batch(batch)), RowState::Queued);

    pause(&mut queue, a);
    assert_eq!(row_state(&queue, RowKind::Batch(batch)), RowState::Queued);

    pause(&mut queue, b);
    assert_eq!(row_state(&queue, RowKind::Batch(batch)), RowState::Paused);
    queue.assert_consistent();
}

#[test]
fn a_row_with_a_running_job_stays_running_even_if_the_rest_is_paused() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("b".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch, "a", 10);
    let b = queue_with_a_batch_job(&mut queue, batch, "b", 10);
    queue.get_mut(a).unwrap().status = JobStatus::InProgress;
    pause(&mut queue, b);

    assert_eq!(row_state(&queue, RowKind::Batch(batch)), RowState::Running);
    queue.assert_consistent();
}

#[test]
fn a_paused_job_that_is_then_cancelled_no_longer_counts_as_paused() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("b".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch, "a", 10);
    pause(&mut queue, a);

    queue.get_mut(a).unwrap().status = JobStatus::Cancelled;

    assert_eq!(row_state(&queue, RowKind::Batch(batch)), RowState::Cancelled);
    queue.assert_consistent();
}

#[test]
fn retrying_a_failed_job_clears_its_pause() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    {
        let mut job = queue.get_mut(id).unwrap();
        job.paused = true;
        job.status = JobStatus::Failed("boom".to_string());
    }

    queue.retry_jobs(&[id]);

    assert!(!queue.get(id).unwrap().paused);
    assert_eq!(queue.startable(4), vec![id]);
}

#[test]
fn a_finished_row_whose_last_job_was_paused_and_completed_is_done() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    {
        let mut job = queue.get_mut(id).unwrap();
        job.paused = true;
        job.status = JobStatus::Completed;
    }

    assert_eq!(row_state(&queue, RowKind::Single(id)), RowState::Done);
    queue.assert_consistent();
}

#[test]
fn pausing_and_unpausing_reports_no_finished_or_reopened_row() {
    let mut queue = TransferQueue::new();
    let id = queue_with_one_job(&mut queue);
    queue.take_changes();

    pause(&mut queue, id);
    queue.get_mut(id).unwrap().paused = false;

    assert!(queue.take_changes().is_empty());
}

fn row_of_files(queue: &mut TransferQueue, name: &str, session: u64, files: &[&str]) -> RowKind {
    let batch = queue.start_batch(name.to_string());
    for file in files {
        queue.enqueue(
            session,
            Direction::Upload,
            VfsPath::from(format!("/local/{name}/{file}")),
            format!("/remote/{name}/{file}"),
            file.to_string(),
            10,
            Some(batch),
        );
    }
    RowKind::Batch(batch)
}

fn labels(queue: &TransferQueue) -> Vec<String> {
    queue.rows().into_iter().map(|row| row.label).collect()
}

fn started_names(queue: &TransferQueue, limit: usize) -> Vec<String> {
    queue.startable(limit).into_iter().map(|id| queue.get(id).unwrap().display_name.clone()).collect()
}

#[test]
fn rows_start_in_the_order_they_were_added() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1"]);
    row_of_files(&mut queue, "B", 1, &["b1"]);
    row_of_files(&mut queue, "C", 1, &["c1"]);

    assert_eq!(labels(&queue), ["A", "B", "C"]);
    assert_eq!(started_names(&queue, 3), ["a1", "b1", "c1"]);
}

#[test]
fn moving_a_row_later_swaps_it_with_the_next_one_in_the_list_and_in_the_start_order() {
    let mut queue = TransferQueue::new();
    let a = row_of_files(&mut queue, "A", 1, &["a1"]);
    row_of_files(&mut queue, "B", 1, &["b1"]);
    row_of_files(&mut queue, "C", 1, &["c1"]);

    assert!(queue.move_row(a, false));

    assert_eq!(labels(&queue), ["B", "A", "C"]);
    assert_eq!(started_names(&queue, 3), ["b1", "a1", "c1"]);
    queue.assert_consistent();
}

#[test]
fn moving_a_row_earlier_puts_it_before_the_previous_one() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1"]);
    row_of_files(&mut queue, "B", 1, &["b1"]);
    let c = row_of_files(&mut queue, "C", 1, &["c1"]);

    assert!(queue.move_row(c, true));
    assert!(queue.move_row(c, true));

    assert_eq!(labels(&queue), ["C", "A", "B"]);
    assert_eq!(started_names(&queue, 1), ["c1"]);
    queue.assert_consistent();
}

#[test]
fn the_first_row_cannot_go_earlier_and_the_last_cannot_go_later() {
    let mut queue = TransferQueue::new();
    let a = row_of_files(&mut queue, "A", 1, &["a1"]);
    let b = row_of_files(&mut queue, "B", 1, &["b1"]);

    assert!(!queue.move_row(a, true));
    assert!(!queue.move_row(b, false));

    assert_eq!(labels(&queue), ["A", "B"]);
}

#[test]
fn an_unknown_row_or_a_scan_cannot_be_moved() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1"]);

    assert!(!queue.move_row(RowKind::Batch(99), true));
    assert!(!queue.move_row(RowKind::Single(99), false));
    assert!(!queue.move_row(RowKind::Scan(1), false));
}

#[test]
fn a_finished_row_is_skipped_when_looking_for_the_neighbour() {
    let mut queue = TransferQueue::new();
    let a = row_of_files(&mut queue, "A", 1, &["a1"]);
    let b = row_of_files(&mut queue, "B", 1, &["b1"]);
    let c = row_of_files(&mut queue, "C", 1, &["c1"]);
    let a_job = queue.jobs_of(a)[0].id;
    queue.get_mut(a_job).unwrap().status = JobStatus::Completed;

    assert!(!queue.move_row(b, true));
    assert!(queue.move_row(c, true));

    assert_eq!(labels(&queue), ["A", "C", "B"]);
    queue.assert_consistent();
}

#[test]
fn a_running_row_may_swap_places_with_a_waiting_one_without_disturbing_it() {
    let mut queue = TransferQueue::new();
    let a = row_of_files(&mut queue, "A", 1, &["a1"]);
    let b = row_of_files(&mut queue, "B", 1, &["b1"]);
    let a_job = queue.jobs_of(a)[0].id;
    queue.get_mut(a_job).unwrap().status = JobStatus::InProgress;

    assert!(queue.move_row(b, true));

    assert_eq!(labels(&queue), ["B", "A"]);
    assert_eq!(queue.get(a_job).unwrap().status, JobStatus::InProgress);
    assert_eq!(started_names(&queue, 4), ["b1"]);
    queue.assert_consistent();
}

#[test]
fn a_paused_row_can_be_moved_and_keeps_its_pause() {
    let mut queue = TransferQueue::new();
    let a = row_of_files(&mut queue, "A", 1, &["a1"]);
    row_of_files(&mut queue, "B", 1, &["b1"]);
    let a_job = queue.jobs_of(a)[0].id;
    pause(&mut queue, a_job);

    assert!(queue.move_row(a, false));

    assert_eq!(labels(&queue), ["B", "A"]);
    assert!(queue.get(a_job).unwrap().paused);
    queue.assert_consistent();
}

#[test]
fn every_waiting_job_of_a_moved_row_moves_with_it_in_order() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1", "a2"]);
    let b = row_of_files(&mut queue, "B", 1, &["b1", "b2", "b3"]);

    assert!(queue.move_row(b, true));

    assert_eq!(started_names(&queue, 5), ["b1", "b2", "b3", "a1", "a2"]);
    queue.assert_consistent();
}

#[test]
fn the_order_is_shared_by_sessions_so_a_moved_row_starts_before_another_sessions_row() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1"]);
    row_of_files(&mut queue, "B", 2, &["b1"]);
    let c = row_of_files(&mut queue, "C", 1, &["c1"]);

    assert!(queue.move_row(c, true));
    assert!(queue.move_row(c, true));

    assert_eq!(started_names(&queue, 3), ["c1", "a1", "b1"]);
    queue.assert_consistent();
}

#[test]
fn a_file_added_to_a_moved_row_later_keeps_that_rows_place() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1"]);
    let b = row_of_files(&mut queue, "B", 1, &["b1"]);
    assert!(queue.move_row(b, true));
    let RowKind::Batch(batch) = b else { unreachable!() };

    queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from("/local/B/b2"),
        "/remote/B/b2".to_string(),
        "b2".to_string(),
        10,
        Some(batch),
    );

    assert_eq!(started_names(&queue, 4), ["b1", "b2", "a1"]);
    queue.assert_consistent();
}

#[test]
fn cancelling_removing_and_failing_after_a_move_keep_the_queue_consistent() {
    let mut queue = TransferQueue::new();
    let a = row_of_files(&mut queue, "A", 1, &["a1", "a2"]);
    let b = row_of_files(&mut queue, "B", 2, &["b1"]);
    assert!(queue.move_row(b, true));

    assert_eq!(queue.fail_queued_for_session(2, "gone"), 1);
    queue.assert_consistent();
    let ids: Vec<u64> = queue.jobs_of(a).iter().map(|job| job.id).collect();
    assert_eq!(queue.cancel_all_queued(), 2);
    queue.remove_jobs(&ids);
    queue.assert_consistent();
    assert_eq!(queue.queued_count(), 0);
}

#[test]
fn moving_rows_reports_no_finished_or_reopened_row() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1"]);
    let b = row_of_files(&mut queue, "B", 1, &["b1"]);
    queue.take_changes();

    queue.move_row(b, true);

    assert!(queue.take_changes().is_empty());
}

#[test]
fn single_file_rows_can_be_reordered_too() {
    let mut queue = TransferQueue::new();
    let first = named_job(&mut queue, "one");
    let second = named_job(&mut queue, "two");

    assert!(queue.move_row(RowKind::Single(second), true));

    assert_eq!(started_names(&queue, 2), ["two", "one"]);
    assert_eq!(queue.rows().first().map(|row| row.kind), Some(RowKind::Single(second)));
    assert_eq!(queue.rows().last().map(|row| row.kind), Some(RowKind::Single(first)));
}

#[test]
fn moving_rows_at_random_keeps_the_queue_consistent_and_starts_jobs_in_row_order() {
    let mut queue = TransferQueue::new();
    let mut random = Lcg(11);
    let batches: Vec<u64> = (0..4).map(|index| queue.start_batch(format!("batch {index}"))).collect();
    let mut ids: Vec<u64> = Vec::new();
    let mut moves = 0;
    for step in 0..900u64 {
        match random.next(9) {
            0 | 1 => {
                let batch = match random.next(5) {
                    4 => None,
                    index => Some(batches[index as usize]),
                };
                let direction = if random.next(2) == 0 { Direction::Upload } else { Direction::Download };
                ids.push(add_job(&mut queue, &format!("f{step}"), batch, random.next(3), direction));
            }
            2 => {
                if let Some(id) = pick(&mut random, &ids)
                    && let Some(mut job) = queue.get_mut(id)
                {
                    job.status = match random.next(5) {
                        0 => JobStatus::Queued,
                        1 => JobStatus::InProgress,
                        2 => JobStatus::Completed,
                        3 => JobStatus::Failed("boom".to_string()),
                        _ => JobStatus::Cancelled,
                    };
                }
            }
            3..=5 => {
                let rows = queue.rows();
                if !rows.is_empty() {
                    let row = &rows[random.next(rows.len() as u64) as usize];
                    moves += usize::from(queue.move_row(row.kind, random.next(2) == 0));
                }
            }
            6 => {
                if let Some(id) = pick(&mut random, &ids)
                    && let Some(mut job) = queue.get_mut(id)
                {
                    job.paused = random.next(2) == 0;
                }
            }
            7 => {
                let chosen: Vec<u64> = ids.iter().copied().filter(|_| random.next(5) == 0).collect();
                queue.retry_jobs(&chosen);
            }
            _ => {
                let chosen: Vec<u64> = ids.iter().copied().filter(|_| random.next(9) == 0).collect();
                queue.remove_jobs(&chosen);
                ids.retain(|id| queue.get(*id).is_some());
            }
        }
        queue.assert_consistent();
        let rank: HashMap<RowKind, usize> =
            queue.rows().iter().enumerate().map(|(index, row)| (row.kind, index)).collect();
        let kind_of = |job: &TransferJob| job.batch_id.map_or(RowKind::Single(job.id), RowKind::Batch);
        let mut expected: Vec<&TransferJob> =
            queue.jobs().filter(|job| job.status == JobStatus::Queued && !job.paused).collect();
        expected.sort_by_key(|job| (rank[&kind_of(job)], job.id));
        let running = queue.active_count();
        let expected: Vec<u64> = expected.iter().take(1000).map(|job| job.id).collect();
        assert_eq!(queue.startable(running + 1000), expected, "start order after step {step}");
    }
    assert!(moves > 50, "only {moves} moves happened");
}

fn shared_destination_job(queue: &mut TransferQueue, local: &str) -> u64 {
    queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from(format!("/local/{local}")),
        "/remote/shared.txt".to_string(),
        local.to_string(),
        10,
        None,
    )
}

fn pause_with_partial(queue: &mut TransferQueue, id: u64) {
    let mut job = queue.get_mut(id).unwrap();
    job.paused = true;
    job.resume = true;
}

#[test]
fn a_paused_job_holding_a_partial_file_keeps_its_destination_from_other_jobs() {
    let mut queue = TransferQueue::new();
    let held = shared_destination_job(&mut queue, "one");
    let other = shared_destination_job(&mut queue, "two");
    pause_with_partial(&mut queue, held);

    assert!(queue.startable(4).is_empty());
    queue.assert_consistent();

    queue.get_mut(held).unwrap().paused = false;
    assert_eq!(queue.startable(4), vec![held]);
    assert_ne!(held, other);
}

#[test]
fn a_paused_job_without_a_partial_file_does_not_hold_its_destination() {
    let mut queue = TransferQueue::new();
    let held = shared_destination_job(&mut queue, "one");
    let other = shared_destination_job(&mut queue, "two");
    pause(&mut queue, held);

    assert_eq!(queue.startable(4), vec![other]);
    queue.assert_consistent();
}

#[test]
fn the_held_destination_is_released_when_the_paused_job_ends_in_any_way() {
    for end in [JobStatus::Cancelled, JobStatus::Completed, JobStatus::Failed("no".to_string())] {
        let mut queue = TransferQueue::new();
        let held = shared_destination_job(&mut queue, "one");
        let other = shared_destination_job(&mut queue, "two");
        pause_with_partial(&mut queue, held);

        queue.get_mut(held).unwrap().status = end;

        assert_eq!(queue.startable(4), vec![other]);
        queue.assert_consistent();
    }
}

#[test]
fn removing_a_paused_job_that_held_a_destination_releases_it() {
    let mut queue = TransferQueue::new();
    let held = shared_destination_job(&mut queue, "one");
    let other = shared_destination_job(&mut queue, "two");
    pause_with_partial(&mut queue, held);

    queue.remove_jobs(&[held]);

    assert_eq!(queue.startable(4), vec![other]);
    queue.assert_consistent();
}

#[test]
fn only_paused_jobs_waiting_is_not_pending_but_a_running_paused_job_is() {
    let mut queue = TransferQueue::new();
    let id = named_job(&mut queue, "one");
    assert!(queue.has_pending(1, Direction::Upload));

    pause(&mut queue, id);
    assert!(!queue.has_pending(1, Direction::Upload));

    queue.get_mut(id).unwrap().status = JobStatus::InProgress;
    assert!(queue.has_pending(1, Direction::Upload));

    queue.get_mut(id).unwrap().status = JobStatus::Queued;
    assert!(!queue.has_pending(1, Direction::Upload));

    named_job(&mut queue, "two");
    assert!(queue.has_pending(1, Direction::Upload));
    queue.assert_consistent();
}

#[test]
fn a_row_reports_paused_as_soon_as_any_unfinished_job_is_paused_even_while_it_is_still_stopping() {
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("b".to_string());
    let a = queue_with_a_batch_job(&mut queue, batch, "a", 10);
    queue_with_a_batch_job(&mut queue, batch, "b", 10);
    queue.get_mut(a).unwrap().status = JobStatus::InProgress;
    assert!(!queue.row(RowKind::Batch(batch)).unwrap().paused);

    queue.get_mut(a).unwrap().paused = true;
    let row = queue.row(RowKind::Batch(batch)).unwrap();

    assert_eq!(row.state, RowState::Running);
    assert!(row.paused);
    queue.assert_consistent();

    queue.get_mut(a).unwrap().status = JobStatus::Completed;
    assert!(!queue.row(RowKind::Batch(batch)).unwrap().paused);
    queue.assert_consistent();
}

#[test]
fn failing_a_sessions_queued_jobs_includes_the_paused_ones() {
    let mut queue = TransferQueue::new();
    let paused = named_job(&mut queue, "p");
    let waiting = named_job(&mut queue, "w");
    pause(&mut queue, paused);

    assert_eq!(queue.fail_queued_for_session(1, "gone"), 2);

    assert!(matches!(queue.get(paused).unwrap().status, JobStatus::Failed(_)));
    assert!(matches!(queue.get(waiting).unwrap().status, JobStatus::Failed(_)));
    queue.assert_consistent();
}

#[test]
fn cancelling_every_queued_job_includes_the_paused_ones() {
    let mut queue = TransferQueue::new();
    let paused = named_job(&mut queue, "p");
    named_job(&mut queue, "w");
    pause(&mut queue, paused);

    assert_eq!(queue.cancel_all_queued(), 2);
    assert_eq!(queue.get(paused).unwrap().status, JobStatus::Cancelled);
    queue.assert_consistent();
}

#[test]
fn a_manual_retry_resumes_a_job_that_had_written_something_even_with_no_failed_attempt() {
    let mut queue = TransferQueue::new();
    let id = named_job(&mut queue, "one");
    {
        let mut job = queue.get_mut(id).unwrap();
        job.status = JobStatus::Cancelled;
        job.attempts = 0;
        job.transferred_bytes = 5;
    }

    queue.retry_jobs(&[id]);

    assert!(queue.get(id).unwrap().resume);
}

#[test]
fn a_manual_retry_of_a_job_that_wrote_nothing_does_not_resume() {
    let mut queue = TransferQueue::new();
    let id = named_job(&mut queue, "one");
    queue.get_mut(id).unwrap().status = JobStatus::Cancelled;

    queue.retry_jobs(&[id]);

    assert!(!queue.get(id).unwrap().resume);
}

#[test]
fn a_row_keeps_its_place_when_its_first_job_is_removed_even_if_it_sits_at_its_own_key_again() {
    let mut queue = TransferQueue::new();
    row_of_files(&mut queue, "A", 1, &["a1"]);
    let b = row_of_files(&mut queue, "B", 1, &["b1"]);
    let c = row_of_files(&mut queue, "C", 1, &["c1"]);
    let RowKind::Batch(b_batch) = b else { unreachable!() };
    queue.enqueue(
        1,
        Direction::Upload,
        VfsPath::from("/local/B/b2"),
        "/remote/B/b2".to_string(),
        "b2".to_string(),
        10,
        Some(b_batch),
    );
    assert!(queue.move_row(c, true));
    assert!(queue.move_row(c, true));
    assert!(queue.move_row(b, true));
    assert_eq!(labels(&queue), ["C", "B", "A"]);
    let first_of_b = queue.jobs_of(b)[0].id;

    queue.remove_jobs(&[first_of_b]);

    assert_eq!(labels(&queue), ["C", "B", "A"]);
    assert_eq!(started_names(&queue, 4), ["c1", "b2", "a1"]);
    queue.assert_consistent();
}

#[test]
fn rows_never_share_a_position() {
    let mut queue = TransferQueue::new();
    let kinds: Vec<RowKind> =
        ["A", "B", "C", "D"].iter().map(|name| row_of_files(&mut queue, name, 1, &["f", "g"])).collect();
    for step in 0..40 {
        queue.move_row(kinds[step % 4], step % 3 == 0);
        queue.assert_consistent();
    }
}
