use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use porthmos_monitor::{SHELL_COMMAND, change_script};
use porthmos_vfs::{
    CommandOutput, CommandRunner, ErrorKind, ProtocolError, async_trait,
    testing::{FakeFs, FakeRunner},
};
use tokio::time::{Instant, timeout};

use crate::{
    HealthSnapshot, Nice, ProcessSignal, Severity,
    engine::{
        Command, Event, Internal, MonitorState, SessionId,
        handlers::MonitorUpdate,
        testing::{TestEngine, test_engine},
    },
};

const PROBE_TIMEOUT: Duration = porthmos_monitor::POLL_TIMEOUT;
const PAIR_GAP: Duration = Duration::from_secs(1);
const IDLE_MINUTE: Duration = Duration::from_secs(60);

fn token_of(command: &str) -> String {
    let start = command.find("@@").expect("the probe names its token") + 2;
    command[start..start + command[start..].find(':').unwrap()].to_string()
}

fn stat_line(pid: u32, name: &str) -> String {
    format!("{pid} ({name}) S 1 {pid} {pid} 0 -1 4194560 0 0 0 0 5 5 0 0 20 0 1 0 100 1000 100 0")
}

fn linux_output(command: &str, uid: u32, call: usize) -> String {
    let t = token_of(command);
    let busy = 10 * call;
    let idle = 80 * call;
    let (rx, tx) = (1000 * call, 2000 * call);
    format!(
        "@@{t}:os\nLinux\n@@{t}:uid\n{uid}\n@@{t}:hostname\nweb\n@@{t}:uptime\n100.0 1.0\n@@{t}:loadavg\n0.1 0.2 0.3 1/2 3\n\
         @@{t}:meminfo\nMemTotal: 1000 kB\nMemAvailable: 500 kB\n@@{t}:stat\ncpu {busy} 0 {busy} {idle} 0 0 0\ncpu0 {busy} 0 {busy} {idle} 0 0 0\n\
         @@{t}:netdev\nInter-|\n face |\n  eth0: {rx} 1 0 0 0 0 0 0 {tx} 1 0 0 0 0 0 0\n\
         @@{t}:df\nFilesystem 1024-blocks Used Available Capacity Mounted on\n/dev/vda1 1000 500 500 50% /\n\
         @@{t}:passwd\nroot:0\nmine:1000\nwww:33\n@@{t}:pagesize\n4096\n\
         @@{t}:status\n/proc/1/status:Uid:\t0\t0\t0\t0\n/proc/500/status:Uid:\t1000\t1000\t1000\t1000\n/proc/600/status:Uid:\t33\t33\t33\t33\n\
         @@{t}:cmdline\n/proc/1/cmdline:/sbin/init\n/proc/500/cmdline:mine-app --serve\n/proc/600/cmdline:nginx\n\
         @@{t}:procs\n/proc/1/stat:{}\n/proc/500/stat:{}\n/proc/600/stat:{}\n@@{t}:end\n",
        stat_line(1, "init"),
        stat_line(500, "mine-app"),
        stat_line(600, "nginx"),
    )
}

fn success(text: String) -> Result<CommandOutput, ProtocolError> {
    Ok(CommandOutput { stdout: text.into_bytes(), stderr: Vec::new(), status: Some(0) })
}

fn is_probe(command: &str) -> bool {
    command.contains("@@")
}

struct Server {
    runner: Arc<FakeRunner>,
    probes: Arc<AtomicUsize>,
}

fn server(uid: u32, other: impl Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync + 'static) -> Server {
    let runner = FakeRunner::new();
    let probes = Arc::new(AtomicUsize::new(0));
    let counter = probes.clone();
    runner.respond_with(move |command| {
        if is_probe(command) {
            success(linux_output(command, uid, counter.fetch_add(1, Ordering::SeqCst) + 1))
        } else {
            other(command)
        }
    });
    Server { runner, probes }
}

fn plain(uid: u32) -> Server {
    server(uid, |_| success(String::new()))
}

fn connect(t: &mut TestEngine, runner: Arc<dyn porthmos_vfs::CommandRunner>) -> SessionId {
    t.add_session_with("prod", Arc::new(FakeFs::new().with_runner(runner)))
}

async fn next_state(t: &mut TestEngine) -> MonitorState {
    loop {
        let internal = timeout(Duration::from_secs(300), t.internal.recv()).await.expect("progress").unwrap();
        t.engine.handle_internal(internal);
        for event in t.drain() {
            if let Event::Monitor { state, .. } = event {
                return state;
            }
        }
    }
}

async fn next_snapshot(t: &mut TestEngine) -> Box<HealthSnapshot> {
    match next_state(t).await {
        MonitorState::Snapshot(snapshot) => snapshot,
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

async fn next_notice(t: &mut TestEngine) -> (Severity, String) {
    loop {
        let event = timeout(Duration::from_secs(60), async {
            tokio::select! {
                event = t.events.recv() => Some(event.unwrap()),
                internal = t.internal.recv() => { t.engine.handle_internal(internal.unwrap()); None }
            }
        })
        .await
        .expect("a notice");
        if let Some(Event::Notice { severity, message }) = event {
            return (severity, message.to_string());
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

fn monitor_events(events: &[Event]) -> usize {
    events.iter().filter(|event| matches!(event, Event::Monitor { .. })).count()
}

fn start(t: &mut TestEngine, session: SessionId) {
    t.engine.handle_command(Command::StartMonitor { session });
}

fn commands_not_probes(runner: &FakeRunner) -> Vec<String> {
    runner.commands().into_iter().filter(|command| !is_probe(command)).collect()
}

fn refresh(t: &mut TestEngine, session: SessionId, pair: bool) {
    t.engine.handle_command(Command::RefreshMonitor { session, pair });
}

fn rx_rates(snapshot: &HealthSnapshot) -> Vec<Option<u64>> {
    snapshot.interfaces.iter().map(|interface| interface.rx_per_second).collect()
}

fn failing_count(events: &[Event]) -> usize {
    events.iter().filter(|event| matches!(event, Event::Monitor { state: MonitorState::Failing(_), .. })).count()
}

fn notice_count(events: &[Event]) -> usize {
    events.iter().filter(|event| matches!(event, Event::Notice { .. })).count()
}

#[tokio::test(start_paused = true)]
async fn start_runs_a_pair_one_second_apart_and_publishes_twice() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    let begun = Instant::now();

    start(&mut t, session);
    let first = next_snapshot(&mut t).await;
    let at_first = begun.elapsed();
    let second = next_snapshot(&mut t).await;

    assert_eq!(first.host.hostname.as_deref(), Some("web"));
    assert_eq!(first.uid, 1000);
    assert!(first.cpu.percent.is_none());
    assert_eq!(rx_rates(&first), [None]);
    assert!(second.cpu.percent.is_some());
    assert_eq!(rx_rates(&second), [Some(1000)]);
    assert!(at_first < PAIR_GAP);
    assert!(begun.elapsed() >= PAIR_GAP);
    assert_eq!(fake.probes.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn a_second_start_does_not_fetch_again() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());

    start(&mut t, session);
    start(&mut t, session);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;
    start(&mut t, session);
    let later = quiet(&mut t, IDLE_MINUTE).await;

    assert_eq!(fake.probes.load(Ordering::SeqCst), 2);
    assert_eq!(monitor_events(&later), 0);
}

#[tokio::test(start_paused = true)]
async fn no_further_fetch_happens_on_its_own() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;

    let later = quiet(&mut t, IDLE_MINUTE).await;

    assert_eq!(fake.probes.load(Ordering::SeqCst), 2);
    assert_eq!(monitor_events(&later), 0);
    assert!(!t.engine.tasks.live_names().contains(&"monitor-fetch"));
}

#[tokio::test(start_paused = true)]
async fn refresh_with_pair_true_repeats_the_pair() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;

    refresh(&mut t, session, true);
    let first = next_snapshot(&mut t).await;
    let second = next_snapshot(&mut t).await;

    assert!(first.cpu.percent.is_none());
    assert_eq!(rx_rates(&first), [None]);
    assert!(second.cpu.percent.is_some());
    assert_eq!(rx_rates(&second), [Some(1000)]);
    assert_eq!(fake.probes.load(Ordering::SeqCst), 4);
}

#[tokio::test(start_paused = true)]
async fn refresh_with_pair_false_runs_one_probe_with_the_delta_since_the_last_sample() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;
    quiet(&mut t, Duration::from_secs(5)).await;

    refresh(&mut t, session, false);
    let snapshot = next_snapshot(&mut t).await;
    let later = quiet(&mut t, IDLE_MINUTE).await;

    assert!(snapshot.cpu.percent.is_some());
    assert_eq!(rx_rates(&snapshot), [Some(200)]);
    assert_eq!(fake.probes.load(Ordering::SeqCst), 3);
    assert_eq!(monitor_events(&later), 0);
}

#[tokio::test(start_paused = true)]
async fn refresh_while_a_pair_is_running_is_ignored() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;

    refresh(&mut t, session, false);
    refresh(&mut t, session, true);
    next_snapshot(&mut t).await;
    let later = quiet(&mut t, IDLE_MINUTE).await;

    assert_eq!(fake.probes.load(Ordering::SeqCst), 2);
    assert_eq!(monitor_events(&later), 0);
}

#[tokio::test(start_paused = true)]
async fn refresh_without_a_started_monitor_is_ignored() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());

    refresh(&mut t, session, false);
    refresh(&mut t, session, true);
    let seen = quiet(&mut t, IDLE_MINUTE).await;

    assert!(fake.runner.commands().is_empty());
    assert_eq!(monitor_events(&seen), 0);
    assert!(!t.engine.monitor_run.fetches.has(session));
}

#[tokio::test(start_paused = true)]
async fn stop_during_the_one_second_gap_publishes_nothing_more() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;

    t.engine.handle_command(Command::StopMonitor { session });
    let seen = quiet(&mut t, Duration::from_secs(30)).await;

    assert_eq!(monitor_events(&seen), 0);
    assert_eq!(fake.probes.load(Ordering::SeqCst), 1);
    assert!(!t.engine.tasks.live_names().contains(&"monitor-fetch"));
    assert!(!t.engine.monitor_run.fetches.has(session));
}

#[tokio::test(start_paused = true)]
async fn a_late_result_of_a_stopped_fetch_is_discarded() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;
    t.engine.handle_command(Command::StopMonitor { session });
    start(&mut t, session);

    t.engine.handle_internal(Internal::Monitor {
        session,
        generation: 0,
        update: MonitorUpdate::Failed("late".into()),
    });

    assert_eq!(monitor_events(&t.drain()), 0);
    assert!(t.notices().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_session_without_a_runner_is_unavailable_and_never_fetches() {
    let mut t = test_engine();
    let (session, _) = t.add_session("prod");

    start(&mut t, session);
    let events = t.drain();

    let [Event::Monitor { state: MonitorState::Unavailable(message), .. }] = events.as_slice() else {
        panic!("expected one Unavailable, got {events:?}");
    };
    assert!(message.to_string().contains("can't run commands"), "{message}");
    assert!(!t.engine.tasks.live_names().contains(&"monitor-fetch"));
}

#[tokio::test(start_paused = true)]
async fn a_non_linux_server_is_unavailable_and_is_not_fetched_again() {
    let mut t = test_engine();
    let runner = FakeRunner::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    runner.respond_with(move |command| {
        counter.fetch_add(1, Ordering::SeqCst);
        let t = token_of(command);
        success(format!("@@{t}:os\nDarwin\n@@{t}:end\n"))
    });
    let session = connect(&mut t, runner);

    start(&mut t, session);
    let MonitorState::Unavailable(message) = next_state(&mut t).await else { panic!("expected Unavailable") };
    let later = quiet(&mut t, IDLE_MINUTE).await;

    assert!(message.to_string().contains("Darwin"), "{message}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(monitor_events(&later), 0);
    assert!(!t.engine.tasks.live_names().contains(&"monitor-fetch"));

    start(&mut t, session);
    assert!(matches!(t.drain().as_slice(), [Event::Monitor { state: MonitorState::Unavailable(_), .. }]));
    refresh(&mut t, session, true);
    assert!(matches!(t.drain().as_slice(), [Event::Monitor { state: MonitorState::Unavailable(_), .. }]));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

fn flaky(failing: impl Fn(usize) -> bool + Send + Sync + 'static) -> (Arc<FakeRunner>, Arc<AtomicUsize>) {
    let runner = FakeRunner::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    runner.respond_with(move |command| {
        let call = counter.fetch_add(1, Ordering::SeqCst) + 1;
        if failing(call) {
            Err(ProtocolError::new(ErrorKind::Connect, anyhow::anyhow!("channel refused")))
        } else {
            success(linux_output(command, 1000, call))
        }
    });
    (runner, calls)
}

async fn refresh_and_collect(t: &mut TestEngine, session: SessionId) -> Vec<Event> {
    refresh(t, session, false);
    quiet(t, Duration::from_secs(1)).await
}

#[tokio::test(start_paused = true)]
async fn a_failed_fetch_publishes_failing_and_keeps_the_sample() {
    let mut t = test_engine();
    let (runner, calls) = flaky(|call| call == 3);
    let session = connect(&mut t, runner);
    start(&mut t, session);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;

    refresh(&mut t, session, false);
    let MonitorState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };
    quiet(&mut t, Duration::from_secs(4)).await;
    refresh(&mut t, session, false);
    let recovered = next_snapshot(&mut t).await;

    assert!(message.to_string().contains("channel refused"), "{message}");
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert!(recovered.cpu.percent.is_some());
    assert_eq!(rx_rates(&recovered), [Some(500)]);
    assert!(t.engine.monitor_run.fetches.slot(session).unwrap().latest.is_some());
}

#[tokio::test(start_paused = true)]
async fn three_failures_report_one_notice() {
    let mut t = test_engine();
    let (runner, _) = flaky(|call| call >= 3);
    let session = connect(&mut t, runner);
    start(&mut t, session);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;
    t.drain();

    let mut events = Vec::new();
    for _ in 0..4 {
        events.extend(refresh_and_collect(&mut t, session).await);
    }

    assert_eq!(failing_count(&events), 4);
    assert_eq!(notice_count(&events), 1);
    let notices = events.iter().filter_map(|event| match event {
        Event::Notice { severity, message } => Some((*severity, message.to_string())),
        _ => None,
    });
    let notices: Vec<_> = notices.collect();
    assert_eq!(notices[0].0, Severity::Warning);
    assert!(notices[0].1.contains("channel refused"), "{notices:?}");
}

#[tokio::test(start_paused = true)]
async fn a_success_resets_the_failure_count() {
    let mut t = test_engine();
    let (runner, _) = flaky(|call| matches!(call, 3 | 4 | 6 | 7));
    let session = connect(&mut t, runner);
    start(&mut t, session);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;
    t.drain();

    let mut events = Vec::new();
    for _ in 0..5 {
        events.extend(refresh_and_collect(&mut t, session).await);
    }

    assert_eq!(failing_count(&events), 4);
    assert_eq!(notice_count(&events), 0);
}

struct Stalled(AtomicUsize);

#[async_trait]
impl CommandRunner for Stalled {
    async fn run_with_input(&self, _command: &str, _input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }
}

#[tokio::test(start_paused = true)]
async fn a_hung_probe_times_out_after_ten_seconds_and_counts_as_a_failure() {
    let mut t = test_engine();
    let stalled = Arc::new(Stalled(AtomicUsize::new(0)));
    let session = connect(&mut t, stalled.clone());
    let begun = Instant::now();

    start(&mut t, session);
    let MonitorState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };

    assert!(begun.elapsed() >= PROBE_TIMEOUT, "{:?}", begun.elapsed());
    assert_eq!(stalled.0.load(Ordering::SeqCst), 1);
    assert!(message.to_string().contains("in time"), "{message}");
}

#[tokio::test(start_paused = true)]
async fn disconnect_and_a_lost_session_stop_the_fetch() {
    let mut t = test_engine();
    let first = plain(1000);
    let one = connect(&mut t, first.runner.clone());
    let second = plain(1000);
    let two = t.add_session_with("other", Arc::new(FakeFs::new().with_runner(second.runner.clone())));
    start(&mut t, one);
    start(&mut t, two);
    next_snapshot(&mut t).await;
    next_snapshot(&mut t).await;

    t.engine.handle_command(Command::Disconnect { session: one });
    t.engine.handle_internal(Internal::SessionHealth { session: two, healthy: false });
    t.drain();
    let later = quiet(&mut t, IDLE_MINUTE).await;

    assert_eq!(monitor_events(&later), 0);
    assert_eq!((first.probes.load(Ordering::SeqCst), second.probes.load(Ordering::SeqCst)), (1, 1));
    assert!(!t.engine.monitor_run.fetches.has(one) && !t.engine.monitor_run.fetches.has(two));
}

#[tokio::test(start_paused = true)]
async fn restart_after_stop_fetches_again_with_a_fresh_sample() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;
    let second = next_snapshot(&mut t).await;
    assert!(second.cpu.percent.is_some());

    t.engine.handle_command(Command::StopMonitor { session });
    quiet(&mut t, Duration::from_secs(10)).await;
    start(&mut t, session);
    let fresh = next_snapshot(&mut t).await;

    assert!(fresh.cpu.percent.is_none());
    assert_eq!(fake.probes.load(Ordering::SeqCst), 3);
}

async fn with_snapshot(
    uid: u32, other: impl Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync + 'static,
) -> (TestEngine, Server, SessionId) {
    let mut t = test_engine();
    let fake = server(uid, other);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;
    (t, fake, session)
}

#[tokio::test(start_paused = true)]
async fn a_signal_sends_the_exact_command_and_reports_success() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        signal: ProcessSignal::Terminate,
    });
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(commands_not_probes(&fake.runner), [change_script("kill -TERM 500")]);
    assert_eq!(severity, Severity::Info);
    assert!(text.contains("500") && text.contains("mine-app"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_signal_for_another_uid_is_refused_without_running_anything() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;
    t.drain();

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 600,
        name: "nginx".into(),
        signal: ProcessSignal::Kill,
    });
    let notices = t.notices();

    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, Severity::Warning);
    assert!(commands_not_probes(&fake.runner).is_empty());
}

#[tokio::test(start_paused = true)]
async fn root_may_signal_any_process() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 600,
        name: "nginx".into(),
        signal: ProcessSignal::Kill,
    });
    next_notice(&mut t).await;

    assert_eq!(commands_not_probes(&fake.runner), [change_script("kill -KILL 600")]);
}

#[tokio::test(start_paused = true)]
async fn an_unknown_pid_is_refused() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;
    t.drain();

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 9999,
        name: "ghost".into(),
        signal: ProcessSignal::Kill,
    });
    t.engine.handle_command(Command::ReniceProcess {
        session,
        pid: 9999,
        name: "ghost".into(),
        nice: Nice::new(5).unwrap(),
    });

    assert_eq!(t.notices().len(), 2);
    assert!(commands_not_probes(&fake.runner).is_empty());
}

#[tokio::test(start_paused = true)]
async fn pid_one_is_refused() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;
    t.drain();

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 1,
        name: "init".into(),
        signal: ProcessSignal::Kill,
    });
    t.engine.handle_command(Command::ReniceProcess {
        session,
        pid: 1,
        name: "init".into(),
        nice: Nice::new(5).unwrap(),
    });

    assert_eq!(t.notices().len(), 2);
    assert!(commands_not_probes(&fake.runner).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_signal_without_a_started_monitor_is_refused() {
    let mut t = test_engine();
    let fake = plain(0);
    let session = connect(&mut t, fake.runner.clone());

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        signal: ProcessSignal::Kill,
    });

    assert_eq!(t.notices().len(), 1);
    assert!(fake.runner.commands().is_empty());
}

fn failure(status: u32, stderr: &str) -> impl Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync + 'static {
    let stderr = stderr.to_string();
    move |_| Ok(CommandOutput { stdout: Vec::new(), stderr: stderr.clone().into_bytes(), status: Some(status) })
}

#[tokio::test(start_paused = true)]
async fn a_process_that_is_already_gone_reports_no_longer_running() {
    let (mut t, _, session) = with_snapshot(1000, failure(1, "sh: 1: kill: No such process")).await;

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        signal: ProcessSignal::Terminate,
    });
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Info);
    assert!(text.contains("no longer running"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_failed_signal_reports_the_first_stderr_line() {
    let (mut t, _, session) = with_snapshot(1000, failure(1, "\nkill: Operation not permitted\nsecond line\n")).await;

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        signal: ProcessSignal::Kill,
    });
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Error);
    assert!(text.contains("Operation not permitted") && !text.contains("second line"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn renice_sends_the_exact_command() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;

    t.engine.handle_command(Command::ReniceProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        nice: Nice::new(10).unwrap(),
    });
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(commands_not_probes(&fake.runner), [change_script("renice 10 -p 500")]);
    assert_eq!(severity, Severity::Info);
    assert!(text.contains("10"), "{text}");
}

struct Slow {
    running: AtomicUsize,
    most: AtomicUsize,
    starts: std::sync::Mutex<Vec<Instant>>,
    ends: std::sync::Mutex<Vec<Instant>>,
}

#[async_trait]
impl CommandRunner for Slow {
    async fn run_with_input(&self, _shell: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        self.starts.lock().unwrap().push(Instant::now());
        tokio::time::sleep(Duration::from_secs(7)).await;
        self.ends.lock().unwrap().push(Instant::now());
        self.running.fetch_sub(1, Ordering::SeqCst);
        success(linux_output(&String::from_utf8_lossy(input), 1000, 1))
    }
}

#[tokio::test(start_paused = true)]
async fn the_probes_of_a_pair_never_overlap_even_when_slower_than_the_gap() {
    let mut t = test_engine();
    let slow = Arc::new(Slow {
        running: AtomicUsize::new(0),
        most: AtomicUsize::new(0),
        starts: Default::default(),
        ends: Default::default(),
    });
    let session = connect(&mut t, slow.clone());
    let begun = Instant::now();

    start(&mut t, session);
    next_snapshot(&mut t).await;
    refresh(&mut t, session, true);
    next_snapshot(&mut t).await;

    assert_eq!(slow.most.load(Ordering::SeqCst), 1);
    assert!(begun.elapsed() >= Duration::from_secs(2 * 7 + 1));

    let (starts, ends) = (slow.starts.lock().unwrap().clone(), slow.ends.lock().unwrap().clone());
    assert_eq!(starts.len(), 2);
    assert!(starts[1].duration_since(ends[0]) >= PAIR_GAP);
    let after_the_pair = quiet(&mut t, IDLE_MINUTE).await;
    assert_eq!(monitor_events(&after_the_pair), 0);
    assert_eq!(slow.starts.lock().unwrap().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn the_probe_and_every_change_run_in_the_fixed_shell_with_the_script_on_standard_input() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        signal: ProcessSignal::Terminate,
    });
    next_notice(&mut t).await;

    let shells = fake.runner.shells();
    assert!(shells.len() >= 2 && shells.iter().all(|shell| shell == SHELL_COMMAND), "{shells:?}");
    assert!(fake.runner.commands().iter().all(|text| !text.contains(SHELL_COMMAND)));
}

#[tokio::test(start_paused = true)]
async fn a_signal_whose_pid_now_belongs_to_a_differently_named_process_is_refused() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;
    t.drain();

    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "something-else".into(),
        signal: ProcessSignal::Kill,
    });
    t.engine.handle_command(Command::ReniceProcess {
        session,
        pid: 500,
        name: "something-else".into(),
        nice: Nice::new(5).unwrap(),
    });

    let notices = t.notices();
    assert_eq!(notices.len(), 2);
    assert!(notices.iter().all(|(severity, _)| *severity == Severity::Warning), "{notices:?}");
    assert!(commands_not_probes(&fake.runner).is_empty());
}

fn signal_500(t: &mut TestEngine, session: SessionId) {
    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        signal: ProcessSignal::Terminate,
    });
}

fn renice_500(t: &mut TestEngine, session: SessionId) {
    t.engine.handle_command(Command::ReniceProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        nice: Nice::new(5).unwrap(),
    });
}

#[tokio::test(start_paused = true)]
async fn a_process_action_is_accepted_on_a_snapshot_just_inside_the_age_limit() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;
    t.drain();

    tokio::time::advance(porthmos_monitor::PROCESS_ACTION_MAX_AGE - Duration::from_secs(1)).await;
    signal_500(&mut t, session);
    renice_500(&mut t, session);
    next_notice(&mut t).await;
    next_notice(&mut t).await;

    assert_eq!(commands_not_probes(&fake.runner), [change_script("kill -TERM 500"), change_script("renice 5 -p 500")]);
}

#[tokio::test(start_paused = true)]
async fn a_process_action_on_an_old_snapshot_is_refused_and_runs_nothing() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;
    t.drain();

    tokio::time::advance(porthmos_monitor::PROCESS_ACTION_MAX_AGE + Duration::from_secs(1)).await;
    signal_500(&mut t, session);
    renice_500(&mut t, session);
    t.engine.handle_command(Command::SignalProcess {
        session,
        pid: 500,
        name: "mine-app".into(),
        signal: ProcessSignal::Kill,
    });
    let notices = t.notices();

    assert_eq!(notices.len(), 3);
    assert!(notices.iter().all(|(severity, text)| *severity == Severity::Warning && text.contains("refresh first")));
    assert!(commands_not_probes(&fake.runner).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_fresh_snapshot_reenables_process_actions_after_the_list_went_old() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;
    next_snapshot(&mut t).await;
    t.drain();
    tokio::time::advance(porthmos_monitor::PROCESS_ACTION_MAX_AGE + Duration::from_secs(5)).await;
    signal_500(&mut t, session);
    assert_eq!(t.notices().len(), 1);

    refresh(&mut t, session, false);
    next_snapshot(&mut t).await;
    signal_500(&mut t, session);
    next_notice(&mut t).await;

    assert_eq!(commands_not_probes(&fake.runner), [change_script("kill -TERM 500")]);
}

#[tokio::test(start_paused = true)]
async fn a_pair_whose_second_half_keeps_failing_reports_one_notice_at_the_third_failure() {
    let mut t = test_engine();
    let (runner, _) = flaky(|call| call % 2 == 0);
    let session = connect(&mut t, runner);
    start(&mut t, session);
    let mut events = quiet(&mut t, Duration::from_secs(3)).await;
    for _ in 0..2 {
        refresh(&mut t, session, true);
        events.extend(quiet(&mut t, Duration::from_secs(3)).await);
    }

    assert_eq!(failing_count(&events), 3);
    assert_eq!(notice_count(&events), 1);
}
