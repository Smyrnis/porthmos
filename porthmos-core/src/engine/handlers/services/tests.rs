use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use porthmos_monitor::{POLL_TIMEOUT, ServiceVerb, change_script};
use porthmos_vfs::{
    CommandOutput, CommandRunner, ErrorKind, ProtocolError, async_trait,
    testing::{FakeFs, FakeRunner},
};
use tokio::time::{Instant, sleep, timeout};

use crate::{
    Message, ServicesState, Severity,
    engine::{
        Command, Event, Internal, SessionId,
        handlers::ServicesUpdate,
        testing::{TestEngine, test_engine},
    },
};

const IDLE: Duration = Duration::from_secs(9);

fn token_of(input: &str) -> String {
    let start = input.find("@@").expect("the script names its token") + 2;
    input[start..start + input[start..].find(':').unwrap()].to_string()
}

fn listing(input: &str, uid: u32) -> String {
    let t = token_of(input);
    format!(
        "@@{t}:os\nLinux\n@@{t}:uid\n{uid}\n@@{t}:systemd\nyes\n@@{t}:units\n\
         nginx.service loaded active running Web server\n\
         cron.service loaded inactive dead Scheduler\n\
         dbus.service loaded active running D-Bus\n\
         weird\\x2dname.service loaded active running Escaped\n\
         @@{t}:files\n\
         nginx.service enabled\n\
         cron.service disabled\n\
         dbus.service static\n\
         weird\\x2dname.service enabled\n\
         @@{t}:end\n"
    )
}

fn success(text: String) -> Result<CommandOutput, ProtocolError> {
    Ok(CommandOutput { stdout: text.into_bytes(), stderr: Vec::new(), status: Some(0) })
}

fn exit(status: u32, stdout: &str, stderr: &str) -> Result<CommandOutput, ProtocolError> {
    Ok(CommandOutput { stdout: stdout.as_bytes().to_vec(), stderr: stderr.as_bytes().to_vec(), status: Some(status) })
}

fn refused() -> Result<CommandOutput, ProtocolError> {
    Err(ProtocolError::new(ErrorKind::Connect, anyhow::anyhow!("channel refused")))
}

fn is_listing(command: &str) -> bool {
    command.contains("@@")
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
    counting(|input, _| success(listing(input, 1000)))
}

struct Server {
    runner: Arc<FakeRunner>,
    listings: Arc<AtomicUsize>,
}

fn server(uid: u32, other: impl Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync + 'static) -> Server {
    let runner = FakeRunner::new();
    let listings = Arc::new(AtomicUsize::new(0));
    let counter = listings.clone();
    runner.respond_with(move |input| {
        if is_listing(input) {
            counter.fetch_add(1, Ordering::SeqCst);
            success(listing(input, uid))
        } else {
            other(input)
        }
    });
    Server { runner, listings }
}

fn actions(runner: &FakeRunner) -> Vec<String> {
    runner.commands().into_iter().filter(|command| !is_listing(command)).collect()
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
        success(listing(&String::from_utf8_lossy(input), 1000))
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
    t.engine.handle_command(Command::StartServices { session });
}

fn refresh(t: &mut TestEngine, session: SessionId) {
    t.engine.handle_command(Command::RefreshServices { session });
}

fn act(t: &mut TestEngine, session: SessionId, unit: &str, verb: ServiceVerb) {
    t.engine.handle_command(Command::ServiceAction { session, unit: unit.into(), verb });
}

fn status(t: &mut TestEngine, session: SessionId, unit: &str) {
    t.engine.handle_command(Command::ServiceStatus { session, unit: unit.into() });
}

async fn next_state(t: &mut TestEngine) -> ServicesState {
    loop {
        let internal = timeout(Duration::from_secs(300), t.internal.recv()).await.expect("progress").unwrap();
        t.engine.handle_internal(internal);
        for event in t.drain() {
            if let Event::Services { state, .. } = event {
                return state;
            }
        }
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

async fn next_status(t: &mut TestEngine) -> (String, String) {
    loop {
        let event = timeout(Duration::from_secs(60), async {
            tokio::select! {
                event = t.events.recv() => Some(event.unwrap()),
                internal = t.internal.recv() => { t.engine.handle_internal(internal.unwrap()); None }
            }
        })
        .await
        .expect("a status");
        if let Some(Event::ServiceStatus { unit, text, .. }) = event {
            return (unit, text);
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

fn services_events(events: &[Event]) -> usize {
    events.iter().filter(|event| matches!(event, Event::Services { .. })).count()
}

fn notices(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|event| matches!(event, Event::Notice { .. })).collect()
}

async fn with_snapshot(
    uid: u32, other: impl Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync + 'static,
) -> (TestEngine, Server, SessionId) {
    let mut t = test_engine();
    let fake = server(uid, other);
    let session = connect(&mut t, fake.runner.clone());
    start(&mut t, session);
    assert!(matches!(next_state(&mut t).await, ServicesState::Snapshot(_)));
    t.drain();
    (t, fake, session)
}

#[tokio::test(start_paused = true)]
async fn start_fetches_exactly_once_and_publishes_a_snapshot() {
    let mut t = test_engine();
    let (runner, calls) = healthy();
    let session = connect(&mut t, runner);

    start(&mut t, session);
    let ServicesState::Snapshot(snapshot) = next_state(&mut t).await else { panic!("expected a snapshot") };
    let later = quiet(&mut t, IDLE).await;

    assert_eq!(snapshot.uid, 1000);
    assert_eq!(snapshot.units.len(), 4);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(services_events(&later), 0);
    assert!(!t.engine.tasks.live_names().contains(&"services-fetch"));
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

    assert!(matches!(state, ServicesState::Snapshot(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(services_events(&later), 0);
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
    assert_eq!(services_events(&seen), 1);
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
    assert_eq!((calls.load(Ordering::SeqCst), services_events(&seen)), (1, 0));

    let (darwin, darwin_calls) = counting(|input, _| {
        let t = token_of(input);
        success(format!("@@{t}:os\nDarwin\n@@{t}:end\n"))
    });
    let other = t.add_session_with("mac", Arc::new(FakeFs::new().with_runner(darwin)));
    start(&mut t, other);
    next_state(&mut t).await;
    start(&mut t, other);
    let replay = t.drain();

    assert!(matches!(replay.as_slice(), [Event::Services { state: ServicesState::Unavailable(_), .. }]), "{replay:?}");
    assert_eq!(darwin_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn no_runner_publishes_unavailable_and_never_runs_anything() {
    let mut t = test_engine();
    let (session, _) = t.add_session("prod");

    start(&mut t, session);
    refresh(&mut t, session);
    let events = t.drain();

    let [Event::Services { state: ServicesState::Unavailable(first), .. }] = events.as_slice() else {
        panic!("expected one Unavailable, got {events:?}");
    };
    assert!(first.to_string().contains("can't run commands"), "{first}");
    assert!(!t.engine.tasks.live_names().contains(&"services-fetch"));
    assert!(!t.engine.services_run.has(session));
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
    let ServicesState::Unavailable(message) = next_state(&mut t).await else { panic!("expected Unavailable") };
    refresh(&mut t, session);
    let later = quiet(&mut t, Duration::from_secs(30)).await;

    assert!(message.to_string().contains("Darwin"), "{message}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(services_events(&later), 1);
}

#[tokio::test(start_paused = true)]
async fn a_server_without_systemd_publishes_unavailable() {
    let mut t = test_engine();
    let (runner, _) = counting(|input, _| {
        let t = token_of(input);
        success(format!("@@{t}:os\nLinux\n@@{t}:uid\n0\n@@{t}:systemd\n\n@@{t}:end\n"))
    });
    let session = connect(&mut t, runner);

    start(&mut t, session);
    let ServicesState::Unavailable(message) = next_state(&mut t).await else { panic!("expected Unavailable") };

    assert!(message.to_string().contains("systemd"), "{message}");
}

#[tokio::test(start_paused = true)]
async fn a_failed_fetch_publishes_failing_and_a_later_success_recovers() {
    let mut t = test_engine();
    let (runner, calls) = counting(|input, call| if call == 2 { refused() } else { success(listing(input, 1000)) });
    let session = connect(&mut t, runner);
    start(&mut t, session);
    assert!(matches!(next_state(&mut t).await, ServicesState::Snapshot(_)));

    refresh(&mut t, session);
    let ServicesState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };
    refresh(&mut t, session);
    let recovered = next_state(&mut t).await;

    assert!(message.to_string().contains("channel refused"), "{message}");
    assert!(matches!(recovered, ServicesState::Snapshot(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(t.engine.services_run.slot(session).unwrap().failures, 0);
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
        seen.iter().filter(|event| matches!(event, Event::Services { state: ServicesState::Failing(_), .. })).count();
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
    let ServicesState::Failing(message) = next_state(&mut t).await else { panic!("expected Failing") };

    assert!(begun.elapsed() >= POLL_TIMEOUT, "{:?}", begun.elapsed());
    assert!(begun.elapsed() < POLL_TIMEOUT * 2, "{:?}", begun.elapsed());
    assert_eq!(stalled.0.load(Ordering::SeqCst), 1);
    assert_eq!(t.engine.services_run.slot(session).unwrap().failures, 1);
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
    let stale = Internal::Services { session: one, generation: 0, update: ServicesUpdate::Failed("late".into()) };

    t.engine.handle_command(Command::StopServices { session: one });
    t.engine.handle_command(Command::Disconnect { session: two });
    t.engine.handle_internal(stale);
    t.engine.handle_internal(Internal::Services {
        session: two,
        generation: 1,
        update: ServicesUpdate::Unavailable(Message::new("late")),
    });
    let seen = quiet(&mut t, Duration::from_secs(30)).await;

    assert_eq!(services_events(&seen), 0);
    assert!(!t.engine.services_run.has(one));
    assert!(!t.engine.services_run.has(two));

    start(&mut t, one);
    let seen = quiet(&mut t, Duration::from_secs(30)).await;
    assert_eq!(services_events(&seen), 1);
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
async fn an_action_as_root_runs_the_direct_command_and_then_refreshes() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Restart);
    let (severity, text) = next_notice(&mut t).await;
    let state = next_state(&mut t).await;

    assert_eq!(actions(&fake.runner), [change_script("systemctl restart nginx.service </dev/null")]);
    assert_eq!(severity, Severity::Info);
    assert!(text.contains("nginx.service"), "{text}");
    assert!(matches!(state, ServicesState::Snapshot(_)));
    assert_eq!(fake.listings.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn an_action_as_uid_1000_runs_sudo_n() {
    let (mut t, fake, session) = with_snapshot(1000, |_| success(String::new())).await;

    act(&mut t, session, "cron.service", ServiceVerb::Start);
    next_notice(&mut t).await;

    assert_eq!(actions(&fake.runner), [change_script("sudo -n systemctl start cron.service </dev/null")]);
}

#[tokio::test(start_paused = true)]
async fn an_unknown_uid_uses_sudo() {
    let (mut t, fake, session) = with_snapshot(u32::MAX, |_| success(String::new())).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Stop);
    next_notice(&mut t).await;

    assert_eq!(actions(&fake.runner), [change_script("sudo -n systemctl stop nginx.service </dev/null")]);
}

#[tokio::test(start_paused = true)]
async fn an_action_for_a_unit_not_in_the_latest_list_is_refused_and_runs_nothing() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;

    act(&mut t, session, "ghost.service", ServiceVerb::Stop);
    let (severity, text) = next_notice(&mut t).await;
    let later = quiet(&mut t, Duration::from_secs(30)).await;

    assert_eq!(severity, Severity::Warning);
    assert!(text.contains("ghost.service"), "{text}");
    assert!(actions(&fake.runner).is_empty());
    assert_eq!(fake.listings.load(Ordering::SeqCst), 1);
    assert_eq!(services_events(&later), 0);
}

#[tokio::test(start_paused = true)]
async fn an_action_the_unit_state_does_not_allow_is_refused() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;

    act(&mut t, session, "dbus.service", ServiceVerb::Enable);
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Warning);
    assert!(text.contains("dbus.service"), "{text}");
    assert!(actions(&fake.runner).is_empty());
}

#[tokio::test(start_paused = true)]
async fn an_action_with_no_started_services_is_refused() {
    let mut t = test_engine();
    let fake = server(0, |_| success(String::new()));
    let session = connect(&mut t, fake.runner.clone());

    act(&mut t, session, "nginx.service", ServiceVerb::Start);
    let (severity, _) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Warning);
    assert!(fake.runner.commands().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_unit_with_unsafe_characters_in_the_list_gets_no_command_even_if_asked() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;

    act(&mut t, session, "weird\\x2dname.service", ServiceVerb::Restart);
    let (severity, text) = next_notice(&mut t).await;
    status(&mut t, session, "weird\\x2dname.service");
    let (_, second) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Warning);
    assert!(text.contains("weird"), "{text}");
    assert!(second.contains("weird"), "{second}");
    assert!(actions(&fake.runner).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_failed_action_reports_the_first_stderr_line_and_still_refreshes() {
    let (mut t, fake, session) =
        with_snapshot(0, |_| exit(1, "", "\nFailed to start nginx.service: Unit is masked\nsecond line\n")).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Start);
    let (severity, text) = next_notice(&mut t).await;
    let state = next_state(&mut t).await;

    assert_eq!(severity, Severity::Error);
    assert!(text.contains("Unit is masked") && !text.contains("second line"), "{text}");
    assert!(matches!(state, ServicesState::Snapshot(_)));
    assert_eq!(fake.listings.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn a_sudo_password_requirement_gets_its_own_notice() {
    let (mut t, _, session) = with_snapshot(1000, |_| exit(1, "", "sudo: a password is required\n")).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Stop);
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Error);
    assert!(text.contains("passwordless sudo"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_user_not_in_sudoers_gets_its_own_notice() {
    let (mut t, _, session) = with_snapshot(1000, |_| exit(1, "", "mine is not in the sudoers file.\n")).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Stop);
    let (_, text) = next_notice(&mut t).await;

    assert!(text.contains("isn't allowed to use sudo"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_missing_sudo_gets_its_own_notice() {
    let (mut t, _, session) = with_snapshot(1000, |_| exit(127, "", "sh: 1: sudo: not found\n")).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Stop);
    let (_, text) = next_notice(&mut t).await;

    assert!(text.contains("sudo isn't installed"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_runner_error_during_an_action_is_reported() {
    let (mut t, _, session) = with_snapshot(0, |_| refused()).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Stop);
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Error);
    assert!(text.contains("channel refused"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn an_action_after_stop_does_not_refresh() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;

    act(&mut t, session, "nginx.service", ServiceVerb::Stop);
    t.engine.handle_command(Command::StopServices { session });
    let seen = quiet(&mut t, Duration::from_secs(30)).await;

    assert_eq!(services_events(&seen), 0);
    assert_eq!(fake.listings.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn status_runs_the_status_command_directly_and_publishes_capped_cleaned_text() {
    let long = format!("● nginx.service\u{1b}[0m\r\n\tLoaded\u{7}: ok\n{}", "x".repeat(20 * 1024));
    let (mut t, fake, session) = with_snapshot(1000, move |_| exit(3, &long, "")).await;

    status(&mut t, session, "nginx.service");
    let (unit, text) = next_status(&mut t).await;

    assert_eq!(
        actions(&fake.runner),
        [change_script("systemctl status --no-pager --lines=20 nginx.service </dev/null")]
    );
    assert_eq!(unit, "nginx.service");
    assert!(text.starts_with("● nginx.service[0m\n\tLoaded: ok\n"), "{text:?}");
    assert!(!text.contains('\u{1b}') && !text.contains('\r') && !text.contains('\u{7}'));
    assert_eq!(text.len(), 16 * 1024);
    assert_eq!(fake.listings.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn status_falls_back_to_stderr_when_there_is_no_output() {
    let (mut t, _, session) = with_snapshot(1000, |_| exit(4, "", "Unit nginx.service could not be found.\n")).await;

    status(&mut t, session, "nginx.service");
    let (_, text) = next_status(&mut t).await;

    assert!(text.contains("could not be found"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_runner_error_during_status_still_answers_the_page() {
    let (mut t, _, session) = with_snapshot(1000, |_| refused()).await;

    status(&mut t, session, "nginx.service");
    let (unit, text) = next_status(&mut t).await;

    assert_eq!(unit, "nginx.service");
    assert!(text.contains("channel refused"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn status_for_an_unknown_unit_is_refused() {
    let (mut t, fake, session) = with_snapshot(0, |_| success(String::new())).await;

    status(&mut t, session, "ghost.service");
    let (severity, text) = next_notice(&mut t).await;

    assert_eq!(severity, Severity::Warning);
    assert!(text.contains("ghost.service"), "{text}");
    assert!(actions(&fake.runner).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_status_arriving_after_stop_is_dropped() {
    let (mut t, _, session) = with_snapshot(0, |_| success("late".into())).await;

    status(&mut t, session, "nginx.service");
    t.engine.handle_command(Command::StopServices { session });
    let seen = quiet(&mut t, Duration::from_secs(30)).await;

    assert!(!seen.iter().any(|event| matches!(event, Event::ServiceStatus { .. })));
}

struct Lagging {
    listings: AtomicUsize,
    delay: Duration,
}

#[async_trait]
impl CommandRunner for Lagging {
    async fn run_with_input(&self, _command: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        let input = String::from_utf8_lossy(input);
        if !is_listing(&input) {
            return success(String::new());
        }
        if self.listings.fetch_add(1, Ordering::SeqCst) >= 1 {
            sleep(self.delay).await;
        }
        success(listing(&input, 0))
    }
}

async fn with_slow_refresh() -> (TestEngine, Arc<Lagging>, SessionId) {
    let mut t = test_engine();
    let runner = Arc::new(Lagging { listings: AtomicUsize::new(0), delay: Duration::from_secs(5) });
    let session = connect(&mut t, runner.clone());
    start(&mut t, session);
    next_state(&mut t).await;
    t.drain();
    refresh(&mut t, session);
    (t, runner, session)
}

#[tokio::test(start_paused = true)]
async fn an_action_finishing_during_a_fetch_queues_one_more_fetch_after_it() {
    let (mut t, runner, session) = with_slow_refresh().await;

    act(&mut t, session, "nginx.service", ServiceVerb::Restart);
    let seen = quiet(&mut t, Duration::from_secs(60)).await;

    assert_eq!(runner.listings.load(Ordering::SeqCst), 3);
    assert_eq!(services_events(&seen), 2);
    assert!(!t.engine.services_run.take_refetch(session));
}

#[tokio::test(start_paused = true)]
async fn several_actions_finishing_during_one_fetch_cause_exactly_one_extra_fetch() {
    let (mut t, runner, session) = with_slow_refresh().await;

    act(&mut t, session, "nginx.service", ServiceVerb::Restart);
    act(&mut t, session, "cron.service", ServiceVerb::Start);
    act(&mut t, session, "nginx.service", ServiceVerb::Stop);
    let seen = quiet(&mut t, Duration::from_secs(60)).await;

    assert_eq!(runner.listings.load(Ordering::SeqCst), 3);
    assert_eq!(services_events(&seen), 2);
}

#[tokio::test(start_paused = true)]
async fn stopping_during_the_fetch_cancels_the_queued_one() {
    let (mut t, runner, session) = with_slow_refresh().await;

    act(&mut t, session, "nginx.service", ServiceVerb::Restart);
    t.engine.handle_command(Command::StopServices { session });
    let seen = quiet(&mut t, Duration::from_secs(60)).await;

    assert_eq!(runner.listings.load(Ordering::SeqCst), 2);
    assert_eq!(services_events(&seen), 0);
}

#[tokio::test(start_paused = true)]
async fn a_status_drops_hidden_and_reordering_characters_but_keeps_text_and_newlines() {
    let dirty = "● a\u{202e}b\u{200b}c\u{feff}d\u{ad}e\nsecond line\n\tindented\n";
    let (mut t, _, session) = with_snapshot(0, move |_| exit(0, dirty, "")).await;

    status(&mut t, session, "nginx.service");
    let (_, text) = next_status(&mut t).await;

    assert_eq!(text, "● abcde\nsecond line\n\tindented\n");
}

#[tokio::test(start_paused = true)]
async fn the_unknown_unit_notice_shows_a_cleaned_and_capped_name() {
    let (mut t, _, session) = with_snapshot(0, |_| success(String::new())).await;
    let hostile = format!("ev\u{202e}il\u{7}\u{1b}[2J{}.service", "z".repeat(500));

    act(&mut t, session, &hostile, ServiceVerb::Stop);
    let (_, text) = next_notice(&mut t).await;

    assert!(text.contains("evil[2J"), "{text}");
    assert!(!text.contains('\u{202e}') && !text.contains('\u{7}') && !text.contains('\u{1b}'), "{text:?}");
    assert!(text.len() < 400, "{}", text.len());
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
    t.engine.handle_command(Command::StopServices { session });
    refresh(&mut t, session);
    let after_stop = quiet(&mut t, IDLE).await;
    start(&mut t, session);
    let state = next_state(&mut t).await;

    assert_eq!(services_events(&before), 0);
    assert_eq!(services_events(&after_stop), 0);
    assert!(matches!(state, ServicesState::Snapshot(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

struct TurnsUnavailable {
    listings: AtomicUsize,
}

#[async_trait]
impl CommandRunner for TurnsUnavailable {
    async fn run_with_input(&self, _command: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        let input = String::from_utf8_lossy(input);
        if !is_listing(&input) {
            return success(String::new());
        }
        if self.listings.fetch_add(1, Ordering::SeqCst) == 0 {
            return success(listing(&input, 0));
        }
        sleep(Duration::from_secs(5)).await;
        let t = token_of(&input);
        success(format!("@@{t}:os\nDarwin\n@@{t}:end\n"))
    }
}

#[tokio::test(start_paused = true)]
async fn a_queued_refetch_is_dropped_when_the_fetch_turned_unavailable() {
    let mut t = test_engine();
    let runner = Arc::new(TurnsUnavailable { listings: AtomicUsize::new(0) });
    let session = connect(&mut t, runner.clone());
    start(&mut t, session);
    next_state(&mut t).await;
    t.drain();
    refresh(&mut t, session);

    act(&mut t, session, "nginx.service", ServiceVerb::Restart);
    let seen = quiet(&mut t, Duration::from_secs(60)).await;

    assert_eq!(runner.listings.load(Ordering::SeqCst), 2);
    assert_eq!(services_events(&seen), 1);
}
