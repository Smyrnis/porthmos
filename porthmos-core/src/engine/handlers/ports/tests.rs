use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use porthmos_monitor::POLL_TIMEOUT;
use porthmos_vfs::{
    CommandOutput, CommandRunner, ErrorKind, ProtocolError, async_trait,
    testing::{FakeFs, FakeRunner},
};
use tokio::time::{Instant, sleep, timeout};

use crate::{
    Message, PortsState, Severity,
    engine::{
        Command, Event, Internal, SessionId,
        handlers::PortsUpdate,
        testing::{TestEngine, test_engine},
    },
};

const IDLE: Duration = Duration::from_secs(9);

fn token_of(input: &str) -> String {
    let start = input.find("@@").expect("the script names its token") + 2;
    input[start..start + input[start..].find(':').unwrap()].to_string()
}

fn linux_output(input: &str) -> String {
    let t = token_of(input);
    format!("@@{t}:os\nLinux\n@@{t}:uid\n1000\n@@{t}:end\n")
}

fn success(text: String) -> Result<CommandOutput, ProtocolError> {
    Ok(CommandOutput { stdout: text.into_bytes(), stderr: Vec::new(), status: Some(0) })
}

fn refused() -> Result<CommandOutput, ProtocolError> {
    Err(ProtocolError::new(ErrorKind::Connect, anyhow::anyhow!("channel refused")))
}

fn counting(
    answer: impl Fn(&str, usize) -> Result<CommandOutput, ProtocolError> + Send + Sync + 'static,
) -> (Arc<FakeRunner>, Arc<AtomicUsize>) {
    let runner = FakeRunner::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    runner.respond_with(move |input| answer(input, counter.fetch_add(1, Ordering::SeqCst) + 1));
    (runner, calls)
}

fn healthy() -> (Arc<FakeRunner>, Arc<AtomicUsize>) {
    counting(|input, _| success(linux_output(input)))
}

struct Slow {
    calls: AtomicUsize,
    delay: Duration,
}

#[async_trait]
impl CommandRunner for Slow {
    async fn run_with_input(&self, _command: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        sleep(self.delay).await;
        success(linux_output(&String::from_utf8_lossy(input)))
    }
}

struct Stalled(AtomicUsize);

#[async_trait]
impl CommandRunner for Stalled {
    async fn run_with_input(&self, _command: &str, _input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }
}

fn connect(t: &mut TestEngine, runner: Arc<dyn CommandRunner>) -> SessionId {
    t.add_session_with("prod", Arc::new(FakeFs::new().with_runner(runner)))
}

fn start(t: &mut TestEngine, session: SessionId) {
    t.engine.handle_command(Command::StartPorts { session });
}

fn refresh(t: &mut TestEngine, session: SessionId) {
    t.engine.handle_command(Command::RefreshPorts { session });
}

async fn next_state(t: &mut TestEngine) -> PortsState {
    loop {
        let internal = timeout(Duration::from_secs(300), t.internal.recv()).await.expect("progress").unwrap();
        t.engine.handle_internal(internal);
        for event in t.drain() {
            if let Event::Ports { state, .. } = event {
                return state;
            }
        }
    }
}

async fn quiet(t: &mut TestEngine, span: Duration) -> Vec<Event> {
    let mut seen = Vec::new();
    let _ = timeout(span, async {
        loop {
            tokio::select! {
                event = t.events.recv() => seen.push(event.unwrap()),
                internal = t.internal.recv() => t.engine.handle_internal(internal.unwrap()),
            }
        }
    })
    .await;
    seen
}

fn ports_events(events: &[Event]) -> usize {
    events.iter().filter(|event| matches!(event, Event::Ports { .. })).count()
}

fn notices(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|event| matches!(event, Event::Notice { .. })).collect()
}

#[tokio::test(start_paused = true)]
async fn start_fetches_exactly_once_and_publishes_a_snapshot() {
    let mut t = test_engine();
    let (runner, calls) = healthy();
    let session = connect(&mut t, runner);

    start(&mut t, session);
    let PortsState::Snapshot(snapshot) = next_state(&mut t).await else { panic!("expected a snapshot") };
    let later = quiet(&mut t, IDLE).await;

    assert_eq!(snapshot.uid, 1000);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(ports_events(&later), 0);
    assert!(!t.engine.tasks.live_names().contains(&"ports-fetch"));
}

#[tokio::test(start_paused = true)]
async fn refresh_fetches_once_more() {
    let mut t = test_engine();
    let (runner, calls) = healthy();
    let session = connect(&mut t, runner);
    start(&mut t, session);
    next_state(&mut t).await;

    refresh(&mut t, session);
    let state = next_state(&mut t).await;
    let later = quiet(&mut t, IDLE).await;

    assert!(matches!(state, PortsState::Snapshot(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(ports_events(&later), 0);
}

#[tokio::test(start_paused = true)]
async fn refresh_during_a_fetch_is_ignored() {
    let mut t = test_engine();
    let slow = Arc::new(Slow { calls: AtomicUsize::new(0), delay: Duration::from_secs(5) });
    let session = connect(&mut t, slow.clone());

    start(&mut t, session);
    refresh(&mut t, session);
    refresh(&mut t, session);
    let seen = quiet(&mut t, Duration::from_secs(30)).await;

    assert_eq!(slow.calls.load(Ordering::SeqCst), 1);
    assert_eq!(ports_events(&seen), 1);
}

#[tokio::test(start_paused = true)]
async fn a_second_start_while_started_does_not_fetch_again_but_replays_unavailable() {
    let mut t = test_engine();
    let (runner, calls) = healthy();
    let session = connect(&mut t, runner);
    start(&mut t, session);
    start(&mut t, session);
    next_state(&mut t).await;
    start(&mut t, session);
    let seen = quiet(&mut t, Duration::from_secs(30)).await;
    assert_eq!((calls.load(Ordering::SeqCst), ports_events(&seen)), (1, 0));

    let (darwin, darwin_calls) = counting(|input, _| {
        let t = token_of(input);
        success(format!("@@{t}:os\nDarwin\n@@{t}:end\n"))
    });
    let other = t.add_session_with("mac", Arc::new(FakeFs::new().with_runner(darwin)));
    start(&mut t, other);
    next_state(&mut t).await;
    start(&mut t, other);
    let replay = t.drain();

    assert!(matches!(replay.as_slice(), [Event::Ports { state: PortsState::Unavailable(_), .. }]), "{replay:?}");
    assert_eq!(darwin_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn no_runner_publishes_unavailable_and_never_runs_anything() {
    let mut t = test_engine();
    let (session, _) = t.add_session("prod");

    start(&mut t, session);
    refresh(&mut t, session);
    let events = t.drain();

    let [Event::Ports { state: PortsState::Unavailable(first), .. }] = events.as_slice() else {
        panic!("expected one Unavailable, got {events:?}");
    };
    assert!(first.to_string().contains("can't run commands"), "{first}");
    assert!(!t.engine.tasks.live_names().contains(&"ports-fetch"));
    assert!(!t.engine.ports_run.has(session));
}

#[tokio::test(start_paused = true)]
async fn non_linux_publishes_unavailable() {
    let mut t = test_engine();
    let (runner, calls) = counting(|input, _| {
        let t = token_of(input);
        success(format!("@@{t}:os\nDarwin\n@@{t}:end\n"))
    });
    let session = connect(&mut t, runner);

    start(&mut t, session);
    let PortsState::Unavailable(message) = next_state(&mut t).await else { panic!("expected Unavailable") };
    refresh(&mut t, session);
    let later = quiet(&mut t, Duration::from_secs(30)).await;

    assert!(message.to_string().contains("Darwin"), "{message}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(ports_events(&later), 1);
}

#[tokio::test(start_paused = true)]
async fn a_failed_fetch_publishes_failing_and_a_later_success_recovers() {
    let mut t = test_engine();
    let (runner, calls) = counting(|input, call| if call == 2 { refused() } else { success(linux_output(input)) });
    let session = connect(&mut t, runner);
    start(&mut t, session);
    assert!(matches!(next_state(&mut t).await, PortsState::Snapshot(_)));

    refresh(&mut t, session);
    let PortsState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };
    refresh(&mut t, session);
    let recovered = next_state(&mut t).await;

    assert!(message.to_string().contains("channel refused"), "{message}");
    assert!(matches!(recovered, PortsState::Snapshot(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(t.engine.ports_run.slot(session).unwrap().failures, 0);
}

#[tokio::test(start_paused = true)]
async fn three_failures_in_a_row_report_exactly_one_notice() {
    let mut t = test_engine();
    let (runner, _) = counting(|_, _| refused());
    let session = connect(&mut t, runner);
    let mut seen = Vec::new();

    start(&mut t, session);
    seen.extend(quiet(&mut t, Duration::from_secs(1)).await);
    for _ in 0..4 {
        refresh(&mut t, session);
        seen.extend(quiet(&mut t, Duration::from_secs(1)).await);
    }

    let failing =
        seen.iter().filter(|event| matches!(event, Event::Ports { state: PortsState::Failing(_), .. })).count();
    let notices = notices(&seen);
    assert_eq!(failing, 5);
    let [Event::Notice { severity, message }] = notices.as_slice() else { panic!("expected one notice") };
    assert_eq!(*severity, Severity::Warning);
    assert!(message.to_string().contains("channel refused"), "{message}");
}

#[tokio::test(start_paused = true)]
async fn a_hung_fetch_times_out_after_poll_timeout_and_counts_as_a_failure() {
    let mut t = test_engine();
    let stalled = Arc::new(Stalled(AtomicUsize::new(0)));
    let session = connect(&mut t, stalled.clone());
    let begun = Instant::now();

    start(&mut t, session);
    let PortsState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };

    assert!(begun.elapsed() >= POLL_TIMEOUT, "{:?}", begun.elapsed());
    assert!(begun.elapsed() < POLL_TIMEOUT * 2, "{:?}", begun.elapsed());
    assert_eq!(stalled.0.load(Ordering::SeqCst), 1);
    assert_eq!(t.engine.ports_run.slot(session).unwrap().failures, 1);
    assert!(message.to_string().contains("in time"), "{message}");
    refresh(&mut t, session);
    quiet(&mut t, Duration::from_secs(1)).await;
    assert_eq!(stalled.0.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn stop_and_disconnect_discard_a_late_result() {
    let mut t = test_engine();
    let slow = Arc::new(Slow { calls: AtomicUsize::new(0), delay: Duration::from_secs(5) });
    let other_slow = Arc::new(Slow { calls: AtomicUsize::new(0), delay: Duration::from_secs(5) });
    let one = connect(&mut t, slow.clone());
    let two = t.add_session_with("other", Arc::new(FakeFs::new().with_runner(other_slow.clone())));
    start(&mut t, one);
    start(&mut t, two);
    let stale = Internal::Ports { session: one, generation: 0, update: PortsUpdate::Failed("late".into()) };

    t.engine.handle_command(Command::StopPorts { session: one });
    t.engine.handle_command(Command::Disconnect { session: two });
    t.engine.handle_internal(stale);
    t.engine.handle_internal(Internal::Ports {
        session: two,
        generation: 1,
        update: PortsUpdate::Unavailable(Message::new("late")),
    });
    let seen = quiet(&mut t, Duration::from_secs(30)).await;

    assert_eq!(ports_events(&seen), 0);
    assert!(!t.engine.ports_run.has(one));
    assert!(!t.engine.ports_run.has(two));

    start(&mut t, one);
    let seen = quiet(&mut t, Duration::from_secs(30)).await;
    assert_eq!(ports_events(&seen), 1);
}

#[tokio::test(start_paused = true)]
async fn the_fetch_never_runs_sudo() {
    let mut t = test_engine();
    let (runner, _) = healthy();
    let session = connect(&mut t, runner.clone());

    start(&mut t, session);
    next_state(&mut t).await;
    refresh(&mut t, session);
    next_state(&mut t).await;

    let commands = runner.commands();
    assert_eq!(commands.len(), 2);
    assert!(commands.iter().all(|command| !command.contains("sudo")), "{commands:?}");
}

#[tokio::test(start_paused = true)]
async fn refresh_without_a_start_or_after_a_stop_does_nothing_and_a_later_start_still_fetches() {
    let mut t = test_engine();
    let (runner, calls) = healthy();
    let session = connect(&mut t, runner);

    refresh(&mut t, session);
    let before = quiet(&mut t, IDLE).await;
    start(&mut t, session);
    next_state(&mut t).await;
    t.engine.handle_command(Command::StopPorts { session });
    refresh(&mut t, session);
    let after_stop = quiet(&mut t, IDLE).await;
    start(&mut t, session);
    let state = next_state(&mut t).await;

    assert_eq!(ports_events(&before), 0);
    assert_eq!(ports_events(&after_stop), 0);
    assert!(matches!(state, PortsState::Snapshot(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
