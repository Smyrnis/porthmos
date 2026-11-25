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
    HealthSnapshot, Message, Nice, POLL_INTERVAL, ProcessSignal, Severity,
    engine::{
        Command, Event, Internal, MonitorState, SessionId,
        handlers::MonitorUpdate,
        testing::{TestEngine, test_engine},
    },
};

const PROBE_TIMEOUT: Duration = porthmos_monitor::POLL_TIMEOUT;

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
    format!(
        "@@{t}:os\nLinux\n@@{t}:uid\n{uid}\n@@{t}:hostname\nweb\n@@{t}:uptime\n100.0 1.0\n@@{t}:loadavg\n0.1 0.2 0.3 1/2 3\n\
         @@{t}:meminfo\nMemTotal: 1000 kB\nMemAvailable: 500 kB\n@@{t}:stat\ncpu {busy} 0 {busy} {idle} 0 0 0\ncpu0 {busy} 0 {busy} {idle} 0 0 0\n\
         @@{t}:netdev\nInter-|\n face |\n  eth0: 1000 1 0 0 0 0 0 0 2000 1 0 0 0 0 0 0\n\
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

#[tokio::test(start_paused = true)]
async fn starting_polls_publishes_a_snapshot_and_repeats_every_interval() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    let begun = Instant::now();

    start(&mut t, session);
    let first = next_snapshot(&mut t).await;
    let second = next_snapshot(&mut t).await;

    assert_eq!(first.host.hostname.as_deref(), Some("web"));
    assert_eq!(first.uid, 1000);
    assert!(begun.elapsed() >= POLL_INTERVAL);
    assert_eq!(fake.probes.load(Ordering::SeqCst), 2);
    assert!(second.cpu.percent.is_some());
    assert!(first.cpu.percent.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_second_start_does_not_double_poll() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());

    start(&mut t, session);
    start(&mut t, session);
    next_snapshot(&mut t).await;

    assert_eq!(fake.probes.load(Ordering::SeqCst), 1);
    assert_eq!(monitor_events(&quiet(&mut t, POLL_INTERVAL - Duration::from_millis(500)).await), 0);
}

#[tokio::test(start_paused = true)]
async fn stop_ends_the_task_and_a_late_result_publishes_nothing() {
    let mut t = test_engine();
    let fake = plain(1000);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    next_snapshot(&mut t).await;
    let generation = t.engine.monitor_run.sessions[&session].generation;

    t.engine.handle_command(Command::StopMonitor { session });
    let seen = quiet(&mut t, Duration::from_secs(30)).await;
    t.engine.handle_internal(Internal::Monitor {
        session,
        generation,
        update: MonitorUpdate::Failing(Message::new("late")),
    });

    assert_eq!(monitor_events(&seen), 0);
    assert_eq!(monitor_events(&t.drain()), 0);
    assert_eq!(fake.probes.load(Ordering::SeqCst), 1);
    assert!(!t.engine.tasks.live_names().contains(&"monitor-poll"));
}

#[tokio::test(start_paused = true)]
async fn a_session_without_a_runner_is_unavailable_and_never_polls() {
    let mut t = test_engine();
    let (session, _) = t.add_session("prod");

    start(&mut t, session);
    let events = t.drain();

    let [Event::Monitor { state: MonitorState::Unavailable(message), .. }] = events.as_slice() else {
        panic!("expected one Unavailable, got {events:?}");
    };
    assert!(message.to_string().contains("can't run commands"), "{message}");
    assert!(!t.engine.tasks.live_names().contains(&"monitor-poll"));
}

#[tokio::test(start_paused = true)]
async fn a_non_linux_server_is_unavailable_and_polling_stops() {
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
    let later = quiet(&mut t, Duration::from_secs(60)).await;

    assert!(message.to_string().contains("Darwin"), "{message}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(monitor_events(&later), 0);
    assert!(!t.engine.tasks.live_names().contains(&"monitor-poll"));

    start(&mut t, session);
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

#[tokio::test(start_paused = true)]
async fn one_failed_poll_keeps_the_last_snapshot_and_publishes_nothing_new() {
    let mut t = test_engine();
    let (runner, calls) = flaky(|call| call == 2);
    let session = connect(&mut t, runner);

    start(&mut t, session);
    next_snapshot(&mut t).await;
    let after = next_snapshot(&mut t).await;

    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(after.host.hostname.as_deref(), Some("web"));
    assert!(t.engine.monitor_run.sessions[&session].latest.is_some());
}

#[tokio::test(start_paused = true)]
async fn three_failures_in_a_row_publish_failing_once_and_a_success_recovers() {
    let mut t = test_engine();
    let (runner, calls) = flaky(|call| (2..=6).contains(&call));
    let session = connect(&mut t, runner);

    start(&mut t, session);
    next_snapshot(&mut t).await;
    let MonitorState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };
    let recovered = next_state(&mut t).await;

    assert!(message.to_string().contains("channel refused"), "{message}");
    assert!(matches!(recovered, MonitorState::Snapshot(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 7);
}

#[tokio::test(start_paused = true)]
async fn failing_is_announced_by_exactly_one_notice() {
    let mut t = test_engine();
    let (runner, _) = flaky(|call| call >= 2);
    let session = connect(&mut t, runner);

    start(&mut t, session);
    next_snapshot(&mut t).await;
    let events = quiet(&mut t, Duration::from_secs(120)).await;

    let failing =
        events.iter().filter(|event| matches!(event, Event::Monitor { state: MonitorState::Failing(_), .. })).count();
    let notices = events.iter().filter(|event| matches!(event, Event::Notice { .. })).count();
    assert_eq!((failing, notices), (1, 1));
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
async fn a_hung_poll_times_out_after_ten_seconds_and_counts_as_a_failure() {
    let mut t = test_engine();
    let stalled = Arc::new(Stalled(AtomicUsize::new(0)));
    let session = connect(&mut t, stalled.clone());
    let begun = Instant::now();

    start(&mut t, session);
    let MonitorState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };

    assert!(begun.elapsed() >= PROBE_TIMEOUT * 3, "{:?}", begun.elapsed());
    assert_eq!(stalled.0.load(Ordering::SeqCst), 3);
    assert!(message.to_string().contains("in time"), "{message}");
}

#[tokio::test(start_paused = true)]
async fn disconnect_and_a_lost_session_stop_the_poll() {
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
    let later = quiet(&mut t, Duration::from_secs(60)).await;

    assert_eq!(monitor_events(&later), 0);
    assert_eq!((first.probes.load(Ordering::SeqCst), second.probes.load(Ordering::SeqCst)), (1, 1));
    assert!(t.engine.monitor_run.sessions.is_empty());
}

#[tokio::test(start_paused = true)]
async fn restart_after_stop_polls_again_with_a_fresh_sample() {
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
}

#[async_trait]
impl CommandRunner for Slow {
    async fn run_with_input(&self, _shell: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(7)).await;
        self.running.fetch_sub(1, Ordering::SeqCst);
        success(linux_output(&String::from_utf8_lossy(input), 1000, 1))
    }
}

#[tokio::test(start_paused = true)]
async fn only_one_poll_is_ever_in_flight_even_when_the_probe_is_slower_than_the_interval() {
    let mut t = test_engine();
    let slow = Arc::new(Slow { running: AtomicUsize::new(0), most: AtomicUsize::new(0) });
    let session = connect(&mut t, slow.clone());
    let begun = Instant::now();

    start(&mut t, session);
    for _ in 0..3 {
        next_snapshot(&mut t).await;
    }

    assert_eq!(slow.most.load(Ordering::SeqCst), 1);
    assert!(begun.elapsed() >= Duration::from_secs(3 * 7 + 2 * 3));
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

#[tokio::test(start_paused = true)]
async fn once_failing_the_poll_slows_down_instead_of_hammering_the_server() {
    let mut t = test_engine();
    let (runner, calls) = flaky(|call| call >= 2);
    let session = connect(&mut t, runner);
    start(&mut t, session);
    next_snapshot(&mut t).await;
    let MonitorState::Failing(_) = next_state(&mut t).await else { panic!("expected Failing") };
    let at_failing = calls.load(Ordering::SeqCst);

    quiet(&mut t, Duration::from_secs(60)).await;

    let since = calls.load(Ordering::SeqCst) - at_failing;
    assert!((1..=3).contains(&since), "{since} polls in a minute while failing");
}
