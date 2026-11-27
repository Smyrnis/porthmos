use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use porthmos_monitor::{LogLevel, LogLine, LogSource, SHELL_COMMAND, UnitName, log_script};
use porthmos_vfs::{
    CommandOutput, CommandRunner, CommandStream, ErrorKind, ProtocolError, StreamEnd, async_trait,
    testing::{FakeFs, FakeRunner, ScriptedStream},
};
use tokio::{
    io::{AsyncRead, ReadBuf},
    time::{Instant, sleep, timeout},
};

use super::{FLUSH_INTERVAL, MAX_BATCH_LINES, MAX_OPEN_LOGS, MAX_PENDING_LINES};
use crate::{
    engine::{
        Command, Event, Internal, LogId, LogState, SessionId,
        handlers::first_line,
        testing::{TestEngine, test_engine},
    },
    i18n,
};

const HELD: Duration = Duration::from_secs(3600);

fn file(path: &str) -> LogSource {
    LogSource::File(path.to_string())
}

fn chunks(parts: &[&str]) -> Vec<Vec<u8>> {
    parts.iter().map(|part| part.as_bytes().to_vec()).collect()
}

fn held(parts: &[&str]) -> ScriptedStream {
    let mut chunks = chunks(parts);
    chunks.push(Vec::new());
    ScriptedStream { chunks, delay: Some(HELD), end: StreamEnd { status: Some(0), stderr: Vec::new() } }
}

fn ending(parts: &[&str], status: Option<u32>, stderr: &str) -> ScriptedStream {
    ScriptedStream { chunks: chunks(parts), delay: None, end: StreamEnd { status, stderr: stderr.as_bytes().to_vec() } }
}

fn connect(t: &mut TestEngine, runner: Arc<dyn CommandRunner>) -> SessionId {
    t.add_session_with("prod", Arc::new(FakeFs::new().with_runner(runner)))
}

fn start(t: &mut TestEngine, id: LogId, session: SessionId, source: LogSource) {
    t.engine.handle_command(Command::StartLog { id, session, source, initial_lines: 500 });
}

fn is_last(state: &LogState) -> bool {
    matches!(state, LogState::Ended(_) | LogState::Unavailable(_))
}

fn logs_in(events: Vec<Event>) -> Vec<(LogId, LogState)> {
    events
        .into_iter()
        .filter_map(|event| match event {
            Event::Log { id, state } => Some((id, state)),
            _ => None,
        })
        .collect()
}

async fn until(t: &mut TestEngine, id: LogId, wanted: impl Fn(&LogState) -> bool) -> Vec<(Instant, LogState)> {
    let mut seen = Vec::new();
    loop {
        for (got, state) in logs_in(t.drain()) {
            if got != id {
                continue;
            }
            let done = wanted(&state);
            seen.push((Instant::now(), state));
            if done {
                return seen;
            }
        }
        let internal = timeout(Duration::from_secs(600), t.internal.recv()).await.expect("progress").unwrap();
        t.engine.handle_internal(internal);
    }
}

async fn until_end(t: &mut TestEngine, id: LogId) -> Vec<(Instant, LogState)> {
    until(t, id, is_last).await
}

async fn quiet(t: &mut TestEngine, span: Duration) -> Vec<(LogId, LogState)> {
    let mut seen = Vec::new();
    let _ = timeout(span, async {
        loop {
            let internal = t.internal.recv().await.unwrap();
            t.engine.handle_internal(internal);
            seen.extend(logs_in(t.drain()));
        }
    })
    .await;
    seen.extend(logs_in(t.drain()));
    seen
}

fn lines_of(states: &[(Instant, LogState)]) -> Vec<LogLine> {
    states
        .iter()
        .filter_map(|(_, state)| match state {
            LogState::Lines { lines, .. } => Some(lines.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn text_of(state: &LogState) -> String {
    match state {
        LogState::Ended(message) | LogState::Unavailable(message) => message.to_string(),
        other => panic!("not an end state: {other:?}"),
    }
}

fn nothing_follows(t: &TestEngine) -> bool {
    !t.engine.tasks.live_names().contains(&"log-follow") && t.engine.logs_run.runs.is_empty()
}

fn numbered(count: usize, per_chunk: usize) -> Vec<Vec<u8>> {
    (0..count)
        .collect::<Vec<_>>()
        .chunks(per_chunk)
        .map(|numbers| numbers.iter().map(|n| format!("line {n}\n")).collect::<String>().into_bytes())
        .collect()
}

fn check_numbering(states: &[(Instant, LogState)], total: u64) -> u64 {
    let mut next = 0u64;
    let mut skipped_total = 0u64;
    for (_, state) in states {
        if let LogState::Lines { lines, skipped } = state {
            assert!(lines.len() <= MAX_BATCH_LINES, "a batch of {}", lines.len());
            assert!(!lines.is_empty());
            next += skipped;
            skipped_total += skipped;
            for line in lines {
                assert_eq!(line.text, format!("line {next}"));
                next += 1;
            }
        }
    }
    assert_eq!(next, total);
    skipped_total
}

fn assert_at_most_ten_a_second(states: &[(Instant, LogState)]) {
    let times: Vec<Instant> =
        states.iter().filter(|(_, state)| matches!(state, LogState::Lines { .. })).map(|(time, _)| *time).collect();
    for window in times.windows(11) {
        assert!(window[10] - window[0] >= Duration::from_secs(1), "eleven batches within {:?}", window[10] - window[0]);
    }
}

#[test]
fn the_caps_are_pinned() {
    assert_eq!(FLUSH_INTERVAL, Duration::from_millis(100));
    assert_eq!(MAX_BATCH_LINES, 2000);
    assert_eq!(MAX_PENDING_LINES, 20_000);
    assert_eq!(MAX_OPEN_LOGS, 6);
}

#[tokio::test(start_paused = true)]
async fn start_emits_started_then_the_lines_in_order_with_levels_then_the_end() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(ending(&["<3>disk ERROR full\nINFO ready\n", "plain tail"], Some(0), ""));
    let session = connect(&mut t, runner.clone());

    start(&mut t, 4, session, file("/var/log/syslog"));
    let states = until_end(&mut t, 4).await;

    assert_eq!(states[0].1, LogState::Started);
    let lines = lines_of(&states);
    let texts: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(texts, ["<3>disk ERROR full", "INFO ready", "plain tail"]);
    let levels: Vec<LogLevel> = lines.iter().map(|line| line.level).collect();
    assert_eq!(levels, [LogLevel::Error, LogLevel::Info, LogLevel::Unknown]);
    assert_eq!(text_of(&states.last().unwrap().1), i18n::text("logs.ended"));
    assert_eq!(states.iter().filter(|(_, state)| is_last(state)).count(), 1);
    assert_eq!(runner.shells(), [SHELL_COMMAND]);
    assert_eq!(runner.commands(), [log_script(&file("/var/log/syslog"), 500).unwrap()]);
    quiet(&mut t, Duration::from_secs(1)).await;
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn the_journal_of_one_unit_is_followed_through_its_script() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(ending(&["one\n"], Some(0), ""));
    let session = connect(&mut t, runner.clone());
    let source = LogSource::Journal(Some(UnitName::new("nginx.service").unwrap()));

    t.engine.handle_command(Command::StartLog { id: 1, session, source: source.clone(), initial_lines: 20 });
    until_end(&mut t, 1).await;

    assert_eq!(runner.commands(), [log_script(&source, 20).unwrap()]);
}

#[tokio::test(start_paused = true)]
async fn an_instant_flood_keeps_only_the_newest_pending_lines_and_counts_the_rest() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(ScriptedStream {
        chunks: numbered(100_000, 100_000),
        delay: None,
        end: StreamEnd { status: Some(0), stderr: Vec::new() },
    });
    let session = connect(&mut t, runner);

    start(&mut t, 1, session, file("/var/log/flood"));
    let states = until_end(&mut t, 1).await;

    let skipped = check_numbering(&states, 100_000);
    assert_eq!(skipped, 100_000 - MAX_PENDING_LINES as u64);
    assert_eq!(lines_of(&states).len(), MAX_PENDING_LINES);
    let batches = states.iter().filter(|(_, state)| matches!(state, LogState::Lines { .. })).count();
    assert_eq!(batches, MAX_PENDING_LINES / MAX_BATCH_LINES);
    assert_at_most_ten_a_second(&states);
    assert_eq!(text_of(&states.last().unwrap().1), i18n::text("logs.ended"));
}

#[tokio::test(start_paused = true)]
async fn a_steady_flood_never_sends_more_than_ten_batches_a_second() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(ScriptedStream {
        chunks: numbered(100_000, 1000),
        delay: Some(Duration::from_millis(10)),
        end: StreamEnd { status: Some(0), stderr: Vec::new() },
    });
    let session = connect(&mut t, runner);
    let began = Instant::now();

    start(&mut t, 1, session, file("/var/log/flood"));
    let states = until_end(&mut t, 1).await;

    let skipped = check_numbering(&states, 100_000);
    assert!(skipped > 0);
    assert_at_most_ten_a_second(&states);
    let batches = states.iter().filter(|(_, state)| matches!(state, LogState::Lines { .. })).count() as u32;
    let seconds = (Instant::now() - began).as_secs_f64().ceil() as u32;
    assert!(batches <= seconds * 10, "{batches} batches in {seconds} s");
}

#[tokio::test(start_paused = true)]
async fn stop_closes_the_channel_and_sends_nothing_more() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let closed = runner.push_stream(held(&["one\n"]));
    let session = connect(&mut t, runner);
    start(&mut t, 1, session, file("/var/log/syslog"));
    until(&mut t, 1, |state| matches!(state, LogState::Lines { .. })).await;

    t.engine.handle_command(Command::StopLog { id: 1 });
    t.engine.handle_command(Command::StopLog { id: 99 });
    let seen = quiet(&mut t, FLUSH_INTERVAL * 3).await;

    assert!(closed.load(Ordering::SeqCst));
    assert_eq!(seen, []);
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn stop_while_the_channel_is_opening_closes_nothing_and_leaves_nothing_running() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let closed = runner.push_stream(held(&["one\n"]));
    let session = connect(&mut t, runner.clone());
    start(&mut t, 1, session, file("/var/log/syslog"));

    t.engine.handle_command(Command::StopLog { id: 1 });
    let seen = quiet(&mut t, FLUSH_INTERVAL * 3).await;

    assert_eq!(seen, []);
    assert!(runner.calls() == 0 || closed.load(Ordering::SeqCst));
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn disconnect_closes_the_channel_and_reports_the_lost_connection() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let closed = runner.push_stream(held(&["one\n"]));
    let session = connect(&mut t, runner);
    start(&mut t, 1, session, file("/var/log/syslog"));
    until(&mut t, 1, |state| matches!(state, LogState::Lines { .. })).await;

    t.engine.handle_command(Command::Disconnect { session });
    let seen = quiet(&mut t, FLUSH_INTERVAL * 3).await;

    assert_eq!(seen, [(1, LogState::Ended(crate::Message::new(i18n::text("logs.connection_lost"))))]);
    assert!(closed.load(Ordering::SeqCst));
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn losing_the_session_closes_only_its_logs_and_reports_the_lost_connection() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let lost = runner.push_stream(held(&["one\n"]));
    let session = connect(&mut t, runner);
    let other_runner = FakeRunner::new();
    let kept = other_runner.push_stream(held(&["two\n"]));
    let other = t.add_session_with("other", Arc::new(FakeFs::new().with_runner(other_runner)));
    start(&mut t, 1, session, file("/var/log/syslog"));
    start(&mut t, 2, other, file("/var/log/syslog"));
    until(&mut t, 1, |state| matches!(state, LogState::Lines { .. })).await;

    t.engine.handle_internal(Internal::SessionHealth { session, healthy: false });
    let seen = quiet(&mut t, FLUSH_INTERVAL * 3).await;

    let ended: Vec<&(LogId, LogState)> = seen.iter().filter(|(_, state)| is_last(state)).collect();
    assert_eq!(ended, [&(1, LogState::Ended(crate::Message::new(i18n::text("logs.connection_lost"))))]);
    assert!(lost.load(Ordering::SeqCst));
    assert!(!kept.load(Ordering::SeqCst));
    assert_eq!(t.engine.logs_run.runs.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn losing_the_session_then_disconnecting_reports_the_end_once() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(held(&["one\n"]));
    let session = connect(&mut t, runner);
    start(&mut t, 1, session, file("/var/log/syslog"));
    until(&mut t, 1, |state| matches!(state, LogState::Lines { .. })).await;

    t.engine.handle_internal(Internal::SessionHealth { session, healthy: false });
    t.engine.handle_command(Command::Disconnect { session });
    let seen = quiet(&mut t, FLUSH_INTERVAL * 3).await;

    let ended: Vec<&(LogId, LogState)> = seen.iter().filter(|(_, state)| is_last(state)).collect();
    assert_eq!(ended, [&(1, LogState::Ended(crate::Message::new(i18n::text("logs.connection_lost"))))]);
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn a_second_start_with_the_same_id_closes_the_first_channel() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let first = runner.push_stream(held(&["one\n"]));
    let second = runner.push_stream(held(&["two\n"]));
    let session = connect(&mut t, runner);
    start(&mut t, 1, session, file("/var/log/syslog"));
    until(&mut t, 1, |state| matches!(state, LogState::Lines { .. })).await;

    start(&mut t, 1, session, file("/var/log/auth.log"));
    let states = until(&mut t, 1, |state| matches!(state, LogState::Lines { .. })).await;

    assert_eq!(states[0].1, LogState::Started);
    let texts: Vec<String> = lines_of(&states).into_iter().map(|line| line.text).collect();
    assert_eq!(texts, ["two"]);
    sleep(FLUSH_INTERVAL * 2).await;
    assert!(first.load(Ordering::SeqCst));
    assert!(!second.load(Ordering::SeqCst));
    assert_eq!(t.engine.logs_run.runs.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn engine_shutdown_closes_the_channel() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let closed = runner.push_stream(held(&["one\n"]));
    let session = connect(&mut t, runner);
    start(&mut t, 1, session, file("/var/log/syslog"));
    until(&mut t, 1, |state| matches!(state, LogState::Lines { .. })).await;

    t.engine.tasks.cancel_all();
    t.engine.tasks.shutdown(FLUSH_INTERVAL * 3).await;

    assert!(closed.load(Ordering::SeqCst));
    assert!(t.engine.tasks.live_names().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_connection_without_commands_is_unavailable_and_runs_nothing() {
    let mut t = test_engine();
    let (session, _) = t.add_session("ftp");

    start(&mut t, 1, session, file("/var/log/syslog"));

    assert_eq!(
        logs_in(t.drain()),
        [(1, LogState::Unavailable(crate::Message::new(i18n::text("logs.unavailable_no_runner"))))]
    );
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn an_unknown_session_is_unavailable() {
    let mut t = test_engine();

    start(&mut t, 1, 42, file("/var/log/syslog"));

    assert_eq!(
        logs_in(t.drain()),
        [(1, LogState::Unavailable(crate::Message::new(i18n::text("logs.unavailable_no_session"))))]
    );
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn a_runner_that_cannot_follow_is_unavailable_once() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream_failure(ProtocolError::new(ErrorKind::Unsupported, anyhow::anyhow!("no streams")));
    let session = connect(&mut t, runner);

    start(&mut t, 1, session, file("/var/log/syslog"));
    let seen = quiet(&mut t, Duration::from_secs(1)).await;

    assert_eq!(seen, [(1, LogState::Unavailable(crate::Message::new(i18n::text("logs.unavailable_no_runner"))))]);
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn a_refused_channel_ends_the_log_with_the_reason() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream_failure(ProtocolError::new(ErrorKind::Connect, anyhow::anyhow!("channel refused")));
    let session = connect(&mut t, runner);

    start(&mut t, 1, session, file("/var/log/syslog"));
    let states = until_end(&mut t, 1).await;

    assert_eq!(states.len(), 1);
    assert!(matches!(states[0].1, LogState::Ended(_)));
    assert!(text_of(&states[0].1).contains("channel refused"), "{:?}", states[0].1);
}

#[tokio::test(start_paused = true)]
async fn a_missing_tail_or_journalctl_is_unavailable() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(ending(&[], Some(127), "sh: 1: exec: tail: not found\n"));
    runner.push_stream(ending(&[], Some(127), "sh: 1: exec: journalctl: not found\n"));
    let session = connect(&mut t, runner);

    start(&mut t, 1, session, file("/var/log/syslog"));
    let file_states = until_end(&mut t, 1).await;
    start(&mut t, 2, session, LogSource::Journal(None));
    let journal_states = until_end(&mut t, 2).await;

    assert_eq!(file_states.last().unwrap().1, LogState::Unavailable(crate::Message::new(i18n::text("logs.no_tail"))));
    assert_eq!(
        journal_states.last().unwrap().1,
        LogState::Unavailable(crate::Message::new(i18n::text("logs.no_journal")))
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_tail_flushes_its_lines_then_ends_with_the_cleaned_first_stderr_line() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let stderr = "\n  tail: cannot open '/var/log/\u{202E}gol.x' for reading: Permission denied\nmore\n";
    runner.push_stream(ending(&["kept\n"], Some(1), stderr));
    let session = connect(&mut t, runner);

    start(&mut t, 1, session, file("/var/log/x"));
    let states = until_end(&mut t, 1).await;

    let texts: Vec<String> = lines_of(&states).into_iter().map(|line| line.text).collect();
    assert_eq!(texts, ["kept"]);
    let LogState::Ended(message) = &states.last().unwrap().1 else { panic!("{:?}", states.last()) };
    let text = message.to_string();
    assert_eq!(text, first_line(stderr.as_bytes()).unwrap());
    assert!(!text.contains('\u{202E}'));
    assert!(text.starts_with("tail: cannot open"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_killed_tail_without_stderr_ends_with_a_plain_reason() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(ending(&[], None, ""));
    let session = connect(&mut t, runner);

    start(&mut t, 1, session, file("/var/log/x"));
    let states = until_end(&mut t, 1).await;

    assert_eq!(states.last().unwrap().1, LogState::Ended(crate::Message::new(i18n::text("logs.stopped"))));
}

struct Broken;

impl AsyncRead for Broken {
    fn poll_read(self: Pin<&mut Self>, _: &mut Context<'_>, _: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Err(std::io::Error::other("wire cut")))
    }
}

struct BrokenRunner(Arc<AtomicBool>);

#[async_trait]
impl CommandRunner for BrokenRunner {
    async fn run_with_input(&self, _command: &str, _input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        Ok(CommandOutput::default())
    }

    async fn open_stream(&self, _command: &str, _input: &[u8]) -> Result<CommandStream, ProtocolError> {
        self.0.store(true, Ordering::SeqCst);
        let (_, ended) = tokio::sync::oneshot::channel();
        Ok(CommandStream { output: Box::new(Broken), ended })
    }
}

#[tokio::test(start_paused = true)]
async fn a_read_error_ends_the_log_with_the_error() {
    let mut t = test_engine();
    let opened = Arc::new(AtomicBool::new(false));
    let session = connect(&mut t, Arc::new(BrokenRunner(opened.clone())));

    start(&mut t, 1, session, file("/var/log/x"));
    let states = until_end(&mut t, 1).await;

    assert!(opened.load(Ordering::SeqCst));
    let LogState::Ended(message) = &states.last().unwrap().1 else { panic!("{:?}", states.last()) };
    assert!(message.to_string().contains("wire cut"), "{message}");
}

#[tokio::test(start_paused = true)]
async fn the_seventh_open_log_is_refused_until_one_stops() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    for _ in 0..=MAX_OPEN_LOGS {
        runner.push_stream(held(&["x\n"]));
    }
    let session = connect(&mut t, runner.clone());
    for id in 1..=MAX_OPEN_LOGS as LogId {
        start(&mut t, id, session, file("/var/log/syslog"));
    }

    start(&mut t, 7, session, file("/var/log/syslog"));

    let count = MAX_OPEN_LOGS.to_string();
    let refused = crate::Message::new(i18n::text_with("logs.too_many", &[("count", &count)]));
    assert_eq!(logs_in(t.drain()), [(7, LogState::Unavailable(refused))]);
    quiet(&mut t, Duration::from_secs(1)).await;
    assert_eq!(runner.calls(), MAX_OPEN_LOGS);

    t.engine.handle_command(Command::StopLog { id: 1 });
    start(&mut t, 7, session, file("/var/log/syslog"));
    until(&mut t, 7, |state| matches!(state, LogState::Lines { .. })).await;
    assert_eq!(runner.calls(), MAX_OPEN_LOGS + 1);
}

#[tokio::test(start_paused = true)]
async fn an_invalid_source_is_unavailable_and_runs_nothing() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let session = connect(&mut t, runner.clone());

    start(&mut t, 1, session, file(""));
    start(&mut t, 2, session, file("/var/log/a\nb"));
    start(&mut t, 3, session, file(&"x".repeat(5000)));

    assert_eq!(
        logs_in(t.drain()),
        [
            (1, LogState::Unavailable(crate::Message::new(i18n::text("logs.empty_path")))),
            (2, LogState::Unavailable(crate::Message::new(i18n::text("logs.bad_character")))),
            (3, LogState::Unavailable(crate::Message::new(i18n::text("logs.path_too_long")))),
        ]
    );
    quiet(&mut t, Duration::from_secs(1)).await;
    assert_eq!(runner.calls(), 0);
    assert!(nothing_follows(&t));
}

#[tokio::test(start_paused = true)]
async fn a_late_event_from_a_replaced_run_is_ignored() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    runner.push_stream(held(&["one\n"]));
    let session = connect(&mut t, runner);
    start(&mut t, 1, session, file("/var/log/syslog"));
    let run = t.engine.logs_run.runs[&1].run;

    t.engine.handle_internal(Internal::Log {
        id: 1,
        run: run + 100,
        state: LogState::Lines { lines: vec![LogLine { text: "stale".into(), level: LogLevel::Unknown }], skipped: 0 },
    });
    t.engine.handle_internal(Internal::LogEnded {
        id: 1,
        run: run + 100,
        state: LogState::Ended(crate::Message::new("stale")),
    });

    assert_eq!(logs_in(t.drain()), []);
    assert_eq!(t.engine.logs_run.runs.len(), 1);
}
