use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use tokio::time::Instant;

use super::*;

#[test]
fn off_and_empty_and_zero_mean_no_limit() {
    for text in ["off", "OFF", " off ", "none", "no", "0", "", "  ", "0k", "0M"] {
        assert_eq!(parse_speed_limit(text), Ok(None), "{text:?}");
    }
}

#[test]
fn a_plain_number_is_bytes_per_second() {
    assert_eq!(parse_speed_limit("500"), Ok(Some(500)));
    assert_eq!(parse_speed_limit("1"), Ok(Some(1)));
}

#[test]
fn the_units_are_binary_like_the_sizes_the_screens_show() {
    assert_eq!(parse_speed_limit("500k"), Ok(Some(500 * 1024)));
    assert_eq!(parse_speed_limit("2M"), Ok(Some(2 * 1024 * 1024)));
    assert_eq!(parse_speed_limit("1g"), Ok(Some(1024 * 1024 * 1024)));
}

#[test]
fn a_unit_may_be_followed_by_b_or_a_per_second_suffix_and_spaces() {
    for text in ["2M", "2MB", "2 M", "2 MB/s", "2m/s", " 2Mb/s ", "2mib/s"] {
        assert_eq!(parse_speed_limit(text), Ok(Some(2 * 1024 * 1024)), "{text:?}");
    }
}

#[test]
fn decimals_are_allowed_and_round_down_to_whole_bytes() {
    assert_eq!(parse_speed_limit("1.5M"), Ok(Some(1_572_864)));
    assert_eq!(parse_speed_limit("0.5k"), Ok(Some(512)));
    assert_eq!(parse_speed_limit("1.0000001"), Ok(Some(1)));
}

#[test]
fn nonsense_is_rejected_with_the_text_in_the_message() {
    for text in ["fast", "-5", "1x", "k", "1..5M", "M2", "1 2", "NaN", "inf", "1e3"] {
        let error = parse_speed_limit(text).unwrap_err();
        assert!(error.contains(text.trim()), "{text:?}: {error}");
    }
}

#[test]
fn a_limit_too_large_to_hold_is_rejected() {
    assert!(parse_speed_limit("99999999999999G").is_err());
}

#[test]
fn a_value_below_one_byte_a_second_is_rejected() {
    assert!(parse_speed_limit("0.0001").is_err());
}

#[test]
fn a_new_limit_is_off_until_set() {
    let limit = SpeedLimit::new();

    assert_eq!(limit.get(), None);
    assert_eq!(limit.chunk_cap(), MAX_CHUNK);
}

#[test]
fn the_limit_can_be_set_read_and_turned_off_through_every_clone() {
    let limit = SpeedLimit::new();
    let shared = limit.clone();

    limit.set(Some(4096));
    assert_eq!(shared.get(), Some(4096));

    shared.set(None);
    assert_eq!(limit.get(), None);
}

#[test]
fn setting_zero_turns_the_limit_off() {
    let limit = SpeedLimit::with(Some(10));

    limit.set(Some(0));

    assert_eq!(limit.get(), None);
}

#[test]
fn the_chunk_is_a_tenth_of_a_second_of_data_between_one_byte_and_the_maximum() {
    let limit = SpeedLimit::new();

    limit.set(Some(100_000));
    assert_eq!(limit.chunk_cap(), 10_000);

    limit.set(Some(3));
    assert_eq!(limit.chunk_cap(), 1);

    limit.set(Some(u64::MAX));
    assert_eq!(limit.chunk_cap(), MAX_CHUNK);
}

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

#[tokio::test(start_paused = true)]
async fn without_a_limit_nothing_waits() {
    let limit = SpeedLimit::new();
    let started = Instant::now();

    for _ in 0..100 {
        assert!(limit.throttle(1_000_000, &never()).await);
    }

    assert_eq!(started.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn the_first_chunk_goes_at_once_and_each_next_one_waits_for_the_one_before() {
    let limit = SpeedLimit::with(Some(1000));
    let started = Instant::now();

    limit.throttle(100, &never()).await;
    assert_eq!(started.elapsed(), Duration::ZERO);

    limit.throttle(100, &never()).await;
    assert_eq!(started.elapsed(), Duration::from_millis(100));

    limit.throttle(100, &never()).await;
    assert_eq!(started.elapsed(), Duration::from_millis(200));
}

#[tokio::test(start_paused = true)]
async fn the_limit_is_shared_so_parallel_transfers_split_it() {
    let limit = SpeedLimit::with(Some(1000));
    let started = Instant::now();
    let (first, second) = (limit.clone(), limit.clone());

    let one = tokio::spawn(async move {
        for _ in 0..5 {
            first.throttle(100, &never()).await;
        }
    });
    let two = tokio::spawn(async move {
        for _ in 0..5 {
            second.throttle(100, &never()).await;
        }
    });
    one.await.unwrap();
    two.await.unwrap();

    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(850) && elapsed <= Duration::from_millis(950), "{elapsed:?}");
}

#[tokio::test(start_paused = true)]
async fn changing_the_limit_takes_effect_on_the_next_chunk_without_a_backlog() {
    let limit = SpeedLimit::with(Some(10));
    let started = Instant::now();
    limit.throttle(100, &never()).await;
    assert_eq!(started.elapsed(), Duration::ZERO);

    limit.set(Some(1_000_000));
    limit.throttle(100, &never()).await;
    limit.throttle(100, &never()).await;

    assert!(started.elapsed() < Duration::from_millis(10), "{:?}", started.elapsed());
}

#[tokio::test(start_paused = true)]
async fn turning_the_limit_off_releases_a_waiting_chunk_soon() {
    let limit = SpeedLimit::with(Some(1));
    limit.throttle(1, &never()).await;
    let waiting = limit.clone();
    let started = Instant::now();
    let task = tokio::spawn(async move { waiting.throttle(1000, &never()).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    limit.set(None);

    assert!(task.await.unwrap());
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
}

#[tokio::test(start_paused = true)]
async fn cancelling_stops_the_wait_within_a_fraction_of_a_second() {
    let limit = SpeedLimit::with(Some(1));
    limit.throttle(1, &never()).await;
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let waiting = limit.clone();
    let started = Instant::now();
    let task = tokio::spawn(async move { waiting.throttle(1_000_000, &flag).await });

    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.store(true, Ordering::Relaxed);

    assert!(!task.await.unwrap());
    assert!(started.elapsed() < Duration::from_millis(500), "{:?}", started.elapsed());
}

#[tokio::test(start_paused = true)]
async fn a_cancelled_wait_gives_its_time_back_to_the_others() {
    let limit = SpeedLimit::with(Some(1000));
    let started = Instant::now();
    let cancel = AtomicBool::new(true);
    limit.throttle(1000, &never()).await;

    assert!(!limit.throttle(1_000_000, &cancel).await);
    limit.throttle(500, &never()).await;

    assert!(started.elapsed() <= Duration::from_millis(1500), "{:?}", started.elapsed());
}

#[test]
fn a_limit_is_written_in_the_largest_unit_that_keeps_it_exact() {
    assert_eq!(format_speed_limit(None), "off");
    assert_eq!(format_speed_limit(Some(2 * 1024 * 1024)), "2M");
    assert_eq!(format_speed_limit(Some(512 * 1024)), "512k");
    assert_eq!(format_speed_limit(Some(1024 * 1024 * 1024)), "1G");
    assert_eq!(format_speed_limit(Some(1536)), "1536");
    assert_eq!(format_speed_limit(Some(1500)), "1500");
    assert_eq!(format_speed_limit(Some(1)), "1");
    assert_eq!(format_speed_limit(Some(3 * 1024 * 1024 + 1)), "3145729");
}

#[test]
fn what_is_written_is_read_back_as_the_same_limit() {
    for limit in [None, Some(1), Some(999), Some(1024), Some(1536), Some(2 * 1024 * 1024), Some(u64::MAX / 4)] {
        assert_eq!(parse_speed_limit(&format_speed_limit(limit)), Ok(limit), "{limit:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_big_chunk_read_before_the_limit_dropped_is_charged_one_chunk_of_the_new_rate_not_all_of_it() {
    let limit = SpeedLimit::new();
    let started = Instant::now();

    limit.set(Some(10_000));
    limit.throttle(262_144, &never()).await;
    limit.throttle(1_000, &never()).await;

    assert!(started.elapsed() <= Duration::from_millis(300), "{:?}", started.elapsed());
}

#[tokio::test(start_paused = true)]
async fn waiters_released_by_a_new_limit_are_charged_at_the_new_rate_one_after_another() {
    let limit = SpeedLimit::with(Some(1_000));
    limit.throttle(1_000, &never()).await;
    let started = Instant::now();
    let tasks: Vec<_> = (0..3)
        .map(|_| {
            let waiting = limit.clone();
            tokio::spawn(async move {
                waiting.throttle(1_000, &AtomicBool::new(false)).await;
                Instant::now()
            })
        })
        .collect();

    tokio::time::sleep(Duration::from_millis(300)).await;
    limit.set(Some(10_000));
    let mut finished = Vec::new();
    for task in tasks {
        finished.push(task.await.unwrap().duration_since(started));
    }

    finished.sort();
    assert!(finished[2] < Duration::from_millis(800), "{finished:?}");
    assert!(finished[2] >= finished[0] + Duration::from_millis(150), "{finished:?}");
}

#[test]
fn a_value_below_one_byte_a_second_and_a_value_too_large_say_why() {
    let small = parse_speed_limit("0.5").unwrap_err();
    let large = parse_speed_limit("99999999999999G").unwrap_err();

    assert!(small.contains("at least 1 byte"), "{small}");
    assert!(large.contains("too large"), "{large}");
}

#[test]
fn more_odd_inputs_are_rejected() {
    for text in ["+5", "1_000", "\u{ff15}", "5 k b", "5bb", "ib", "/s", "k/s"] {
        assert!(parse_speed_limit(text).is_err(), "{text:?}");
    }
}
