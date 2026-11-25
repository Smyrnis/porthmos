use std::{net::SocketAddr, sync::Arc, time::Duration};

use porthmos_tunnel::{Endpoint, Kind, Listen, TunnelKey, TunnelSpec};
use porthmos_vfs::{
    FileSystem, ForwardStream, PortForwarder, ProtocolError, async_trait,
    testing::{FakeFs, FakeProtocol},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

use crate::{
    config::tunnels::Tunnels,
    engine::{
        Command, Event, Internal, TunnelInfo, TunnelStatus,
        testing::{TestEngine, sample_entry, test_engine},
    },
    profiles::ProfileDraft,
};

async fn echo_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut reader, mut writer) = stream.split();
                let _ = tokio::io::copy(&mut reader, &mut writer).await;
            });
        }
    });
    address
}

struct Loopback(SocketAddr);

#[async_trait]
impl PortForwarder for Loopback {
    async fn connect(&self, _host: &str, _port: u16) -> Result<ForwardStream, ProtocolError> {
        let stream = TcpStream::connect(self.0)
            .await
            .map_err(|err| ProtocolError::new(porthmos_vfs::ErrorKind::Connect, anyhow::anyhow!(err.to_string())))?;
        Ok(Box::new(stream))
    }
}

async fn forwarding_fs() -> Arc<dyn FileSystem> {
    let echo = echo_server().await;
    Arc::new(FakeFs::new().with_forwarder(Arc::new(Loopback(echo))))
}

async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port()
}

fn key(connection: &str, name: &str) -> TunnelKey {
    TunnelKey { connection: connection.to_string(), name: name.to_string() }
}

fn local(connection: &str, name: &str, port: u16) -> TunnelSpec {
    TunnelSpec {
        connection: connection.to_string(),
        name: name.to_string(),
        kind: Kind::Local {
            listen: Listen { address: "127.0.0.1".parse().unwrap(), port },
            target: Endpoint { host: "db.internal".to_string(), port: 5432 },
        },
        allow_external: false,
        auto_start: false,
    }
}

fn auto(mut spec: TunnelSpec) -> TunnelSpec {
    spec.auto_start = true;
    spec
}

fn save(t: &mut TestEngine, spec: TunnelSpec) {
    t.engine.handle_command(Command::SaveTunnel { original: None, spec: Box::new(spec) });
}

fn status(t: &TestEngine, key: &TunnelKey) -> TunnelStatus {
    t.engine.tunnels.get(key).map(|spec| t.engine.tunnel_status(spec)).expect("the tunnel is saved")
}

async fn pump(t: &mut TestEngine, mut done: impl FnMut(&mut TestEngine) -> bool) {
    loop {
        if done(t) {
            return;
        }
        let internal =
            timeout(Duration::from_secs(5), t.internal.recv()).await.expect("progress within five seconds").unwrap();
        t.engine.handle_internal(internal);
    }
}

async fn running(t: &mut TestEngine, key: &TunnelKey) -> SocketAddr {
    let wanted = key.clone();
    pump(t, |t| matches!(status(t, &wanted), TunnelStatus::Running { .. } | TunnelStatus::Failed(_))).await;
    match status(t, key) {
        TunnelStatus::Running { local } => local,
        other => panic!("expected a running tunnel, got {other:?}"),
    }
}

async fn echoes(address: SocketAddr, payload: &[u8]) -> bool {
    let Ok(mut client) = TcpStream::connect(address).await else {
        return false;
    };
    client.write_all(payload).await.unwrap();
    let mut received = vec![0u8; payload.len()];
    timeout(Duration::from_secs(5), client.read_exact(&mut received)).await.unwrap().unwrap();
    received == payload
}

async fn port_is_released(address: SocketAddr) {
    timeout(Duration::from_secs(5), async {
        while TcpListener::bind(address).await.is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the port was released");
}

fn infos(events: Vec<Event>) -> Option<Vec<TunnelInfo>> {
    events.into_iter().rev().find_map(|event| match event {
        Event::Tunnels(list) => Some(list),
        _ => None,
    })
}

async fn connected(t: &mut TestEngine, name: &str) -> crate::SessionId {
    t.add_session_with(name, forwarding_fs().await)
}

fn file_text(t: &TestEngine) -> String {
    std::fs::read_to_string(t.engine.host.paths.tunnels_file()).unwrap_or_default()
}

#[tokio::test]
async fn starting_without_a_connected_session_reports_offline_and_binds_nothing() {
    let mut t = test_engine();
    let port = free_port().await;
    save(&mut t, local("prod", "db", port));
    t.drain();

    t.engine.handle_command(Command::StartTunnel { key: key("prod", "db") });

    assert_eq!(status(&t, &key("prod", "db")), TunnelStatus::Offline);
    let notices = t.notices();
    assert!(notices.iter().any(|(severity, text)| *severity == crate::Severity::Warning && text.contains("prod")));
    TcpListener::bind(("127.0.0.1", port)).await.expect("nothing was bound");
}

#[tokio::test]
async fn starting_binds_reports_running_and_carries_bytes() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    save(&mut t, local("prod", "db", free_port().await));
    let wanted = key("prod", "db");
    t.drain();

    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    assert_eq!(status(&t, &wanted), TunnelStatus::Starting);
    let address = running(&mut t, &wanted).await;

    assert!(echoes(address, b"hello tunnel").await);
    let events = t.drain();
    assert!(events.iter().any(|event| matches!(event, Event::TunnelStatus { status: TunnelStatus::Starting, .. })));
    assert!(
        events.iter().any(|event| matches!(event, Event::TunnelStatus { status: TunnelStatus::Running { .. }, .. }))
    );
    timeout(Duration::from_secs(5), async {
        loop {
            t.engine.publish_tunnels();
            let list = infos(t.drain()).unwrap();
            if list[0].stats.bytes_down == 12 {
                assert_eq!((list[0].stats.bytes_up, list[0].stats.connections), (12, 1));
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the byte counters arrive");
}

#[tokio::test]
async fn starting_on_a_file_system_that_cannot_forward_fails_with_a_clear_notice() {
    let mut t = test_engine();
    t.add_session("prod");
    save(&mut t, local("prod", "db", free_port().await));
    let wanted = key("prod", "db");
    t.drain();

    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });

    let TunnelStatus::Failed(message) = status(&t, &wanted) else { panic!("expected a failure") };
    assert!(message.to_string().contains("can't forward"), "{message}");
    assert!(t.notices().iter().any(|(severity, _)| *severity == crate::Severity::Error));
}

#[tokio::test]
async fn stop_and_restart_free_and_retake_the_port() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    save(&mut t, local("prod", "db", free_port().await));
    let wanted = key("prod", "db");
    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    let address = running(&mut t, &wanted).await;

    t.engine.handle_command(Command::StopTunnel { key: wanted.clone() });
    assert_eq!(status(&t, &wanted), TunnelStatus::Stopped);
    port_is_released(address).await;

    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    t.engine.handle_command(Command::RestartTunnel { key: wanted.clone() });
    let again = running(&mut t, &wanted).await;
    assert_eq!(again, address);
    assert!(echoes(again, b"back").await);
}

#[tokio::test]
async fn a_restart_picks_up_an_edit() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    let first = free_port().await;
    save(&mut t, local("prod", "db", first));
    let wanted = key("prod", "db");
    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    running(&mut t, &wanted).await;
    let second = free_port().await;

    t.engine.handle_command(Command::SaveTunnel {
        original: Some(wanted.clone()),
        spec: Box::new(local("prod", "db", second)),
    });
    assert!(matches!(status(&t, &wanted), TunnelStatus::Running { local } if local.port() == first));
    t.engine.handle_command(Command::RestartTunnel { key: wanted.clone() });
    let address = running(&mut t, &wanted).await;

    assert_eq!(address.port(), second);
    assert!(echoes(address, b"edited").await);
}

#[tokio::test]
async fn a_port_in_use_ends_failed_with_the_address_and_reason_and_never_another_port() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    let holder = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = holder.local_addr().unwrap().port();
    save(&mut t, local("prod", "db", port));
    let wanted = key("prod", "db");
    t.drain();

    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    running_or_failed(&mut t, &wanted).await;

    let TunnelStatus::Failed(message) = status(&t, &wanted) else { panic!("expected a failure") };
    assert!(message.to_string().contains(&format!("127.0.0.1:{port}")), "{message}");
    assert!(t.notices().iter().any(|(severity, _)| *severity == crate::Severity::Error));
}

async fn running_or_failed(t: &mut TestEngine, key: &TunnelKey) {
    let wanted = key.clone();
    pump(t, |t| !matches!(status(t, &wanted), TunnelStatus::Starting)).await;
}

#[tokio::test]
async fn an_invalid_or_external_spec_is_refused_when_saving_and_nothing_is_written() {
    let mut t = test_engine();
    let mut external = local("prod", "open", 8080);
    external.kind = Kind::Dynamic { listen: Listen { address: "0.0.0.0".parse().unwrap(), port: 8080 } };
    t.drain();

    save(&mut t, external);
    save(&mut t, local("prod", "", 8081));

    assert_eq!(t.engine.tunnels.iter().count(), 0);
    assert_eq!(file_text(&t), "");
    let errors = t.notices().into_iter().filter(|(severity, _)| *severity == crate::Severity::Error).count();
    assert_eq!(errors, 2);
}

#[tokio::test]
async fn a_duplicate_name_on_one_connection_is_refused() {
    let mut t = test_engine();
    save(&mut t, local("prod", "db", 7001));
    t.drain();

    save(&mut t, local("prod", "db", 7002));

    assert_eq!(t.engine.tunnels.iter().count(), 1);
    assert!(t.notices().iter().any(|(severity, text)| *severity == crate::Severity::Error && text.contains("db")));
    save(&mut t, local("other", "db", 7003));
    assert_eq!(t.engine.tunnels.iter().count(), 2);
}

#[tokio::test]
async fn saving_writes_the_file_and_publishes_the_list() {
    let mut t = test_engine();
    t.drain();

    save(&mut t, local("prod", "db", 7004));

    assert!(file_text(&t).contains("name = \"db\""));
    let list = infos(t.drain()).expect("the list was published");
    assert_eq!((list.len(), list[0].status.clone()), (1, TunnelStatus::Offline));
}

#[tokio::test]
async fn disconnecting_stops_that_sessions_tunnels_and_leaves_other_sessions_alone() {
    let mut t = test_engine();
    let first = connected(&mut t, "alpha").await;
    connected(&mut t, "beta").await;
    save(&mut t, local("alpha", "a", free_port().await));
    save(&mut t, local("beta", "b", free_port().await));
    let (a, b) = (key("alpha", "a"), key("beta", "b"));
    t.engine.handle_command(Command::StartTunnel { key: a.clone() });
    t.engine.handle_command(Command::StartTunnel { key: b.clone() });
    let a_address = running(&mut t, &a).await;
    let b_address = running(&mut t, &b).await;

    t.engine.disconnect(first);

    assert_eq!(status(&t, &a), TunnelStatus::Lost);
    port_is_released(a_address).await;
    assert!(matches!(status(&t, &b), TunnelStatus::Running { .. }));
    assert!(echoes(b_address, b"still").await);
}

#[tokio::test]
async fn a_lost_connection_stops_its_tunnels_and_marks_them_lost() {
    let mut t = test_engine();
    let session = connected(&mut t, "prod").await;
    save(&mut t, local("prod", "db", free_port().await));
    let wanted = key("prod", "db");
    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    let address = running(&mut t, &wanted).await;

    t.engine.handle_internal(Internal::SessionHealth { session, healthy: false });

    assert_eq!(status(&t, &wanted), TunnelStatus::Lost);
    port_is_released(address).await;
}

#[tokio::test]
async fn a_disconnect_while_starting_leaves_nothing_listening() {
    let mut t = test_engine();
    let session = connected(&mut t, "prod").await;
    let port = free_port().await;
    save(&mut t, local("prod", "db", port));
    let wanted = key("prod", "db");

    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    t.engine.disconnect(session);
    tokio::time::sleep(Duration::from_millis(200)).await;
    while let Ok(internal) = t.internal.try_recv() {
        t.engine.handle_internal(internal);
    }

    assert_eq!(status(&t, &wanted), TunnelStatus::Lost);
    port_is_released("127.0.0.1:0".parse::<SocketAddr>().map(|_| SocketAddr::from(([127, 0, 0, 1], port))).unwrap())
        .await;
}

#[tokio::test]
async fn a_stop_while_starting_leaves_nothing_listening_and_reports_nothing_later() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    let port = free_port().await;
    save(&mut t, local("prod", "db", port));
    let wanted = key("prod", "db");

    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    t.engine.handle_command(Command::StopTunnel { key: wanted.clone() });
    assert_eq!(status(&t, &wanted), TunnelStatus::Stopped);
    tokio::time::sleep(Duration::from_millis(200)).await;
    while let Ok(internal) = t.internal.try_recv() {
        t.engine.handle_internal(internal);
    }

    assert_eq!(status(&t, &wanted), TunnelStatus::Stopped);
    port_is_released(SocketAddr::from(([127, 0, 0, 1], port))).await;
}

#[tokio::test]
async fn connecting_starts_only_the_auto_start_tunnels_of_that_connection() {
    let mut t = test_engine();
    save(&mut t, auto(local("srv", "auto", free_port().await)));
    save(&mut t, local("srv", "manual", free_port().await));
    save(&mut t, auto(local("elsewhere", "other", free_port().await)));
    let fs = forwarding_fs().await;

    t.engine.finish_connect(sample_entry("srv"), Arc::new(FakeProtocol::new(FakeFs::new())), fs, None);
    let started = key("srv", "auto");
    running(&mut t, &started).await;

    assert_eq!(status(&t, &key("srv", "manual")), TunnelStatus::Stopped);
    assert_eq!(status(&t, &key("elsewhere", "other")), TunnelStatus::Offline);
}

#[tokio::test]
async fn reconnecting_clears_the_lost_marker_and_restarts_only_auto_start() {
    let mut t = test_engine();
    let session = connected(&mut t, "srv").await;
    save(&mut t, auto(local("srv", "auto", free_port().await)));
    save(&mut t, local("srv", "manual", free_port().await));
    let (auto_key, manual_key) = (key("srv", "auto"), key("srv", "manual"));
    t.engine.handle_command(Command::StartTunnel { key: auto_key.clone() });
    t.engine.handle_command(Command::StartTunnel { key: manual_key.clone() });
    running(&mut t, &auto_key).await;
    running(&mut t, &manual_key).await;
    t.engine.disconnect(session);
    assert_eq!((status(&t, &auto_key), status(&t, &manual_key)), (TunnelStatus::Lost, TunnelStatus::Lost));

    let fs = forwarding_fs().await;
    t.engine.finish_connect(sample_entry("srv"), Arc::new(FakeProtocol::new(FakeFs::new())), fs, None);

    running(&mut t, &auto_key).await;
    assert_eq!(status(&t, &manual_key), TunnelStatus::Stopped);
}

#[tokio::test]
async fn deleting_a_running_tunnel_stops_it_first_and_removes_it_from_the_file() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    save(&mut t, local("prod", "db", free_port().await));
    let wanted = key("prod", "db");
    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    let address = running(&mut t, &wanted).await;

    t.engine.handle_command(Command::DeleteTunnel { key: wanted.clone() });

    assert!(t.engine.tunnels.get(&wanted).is_none());
    assert!(!file_text(&t).contains("db"));
    port_is_released(address).await;
}

#[tokio::test]
async fn renaming_a_running_tunnel_keeps_it_running_under_the_new_name() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    let port = free_port().await;
    save(&mut t, local("prod", "db", port));
    let old = key("prod", "db");
    t.engine.handle_command(Command::StartTunnel { key: old.clone() });
    running(&mut t, &old).await;

    t.engine.handle_command(Command::SaveTunnel {
        original: Some(old.clone()),
        spec: Box::new(local("prod", "database", port)),
    });

    let renamed = key("prod", "database");
    assert!(matches!(status(&t, &renamed), TunnelStatus::Running { .. }));
    assert!(t.engine.tunnels.get(&old).is_none());
}

fn engine_with_profiles() -> TestEngine {
    let mut t = test_engine();
    let protocol =
        FakeProtocol::new(FakeFs::new()).with_id("sftp").with_form(porthmos_vfs::ConnectionForm::standard(22));
    t.engine.host.protocols = vec![Arc::new(protocol)];
    t
}

fn draft(name: &str) -> ProfileDraft {
    ProfileDraft {
        name: name.to_string(),
        host: "h".to_string(),
        port: "22".to_string(),
        username: "u".to_string(),
        protocol: "sftp".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn renaming_and_deleting_a_connection_updates_the_file_and_the_list() {
    let mut t = engine_with_profiles();
    t.engine.handle_command(Command::SaveProfile { original: None, draft: Box::new(draft("old")) });
    save(&mut t, local("old", "db", 7010));
    save(&mut t, local("keep", "x", 7011));
    t.drain();

    t.engine.handle_command(Command::SaveProfile { original: Some("old".to_string()), draft: Box::new(draft("new")) });

    assert!(t.engine.tunnels.get(&key("new", "db")).is_some());
    assert!(t.engine.tunnels.get(&key("old", "db")).is_none());
    assert!(file_text(&t).contains("connection = \"new\""));
    assert_eq!(infos(t.drain()).unwrap().len(), 2);

    t.engine.handle_command(Command::DeleteProfile { name: "new".to_string() });

    assert_eq!(t.engine.tunnels.iter().count(), 1);
    assert!(!file_text(&t).contains("\"new\""));
    assert_eq!(infos(t.drain()).unwrap().len(), 1);
}

#[tokio::test]
async fn renaming_a_connection_with_a_running_tunnel_keeps_it_running() {
    let mut t = engine_with_profiles();
    t.engine.handle_command(Command::SaveProfile { original: None, draft: Box::new(draft("old")) });
    t.add_session_with("old", forwarding_fs().await);
    save(&mut t, local("old", "db", free_port().await));
    t.engine.handle_command(Command::StartTunnel { key: key("old", "db") });
    running(&mut t, &key("old", "db")).await;

    t.engine.handle_command(Command::SaveProfile { original: Some("old".to_string()), draft: Box::new(draft("new")) });

    assert!(
        matches!(status(&t, &key("new", "db")), TunnelStatus::Running { .. })
            || status(&t, &key("new", "db")) == TunnelStatus::Stopped
    );
}

#[tokio::test(start_paused = true)]
async fn stats_batches_arrive_about_once_a_second_only_while_something_runs() {
    let mut t = test_engine();
    connected(&mut t, "prod").await;
    save(&mut t, local("prod", "db", free_port().await));
    let wanted = key("prod", "db");
    t.engine.handle_command(Command::StartTunnel { key: wanted.clone() });
    running(&mut t, &wanted).await;
    t.drain();

    let mut batches = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(3500);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(internal)) = timeout(Duration::from_millis(100), t.internal.recv()).await {
            t.engine.handle_internal(internal);
        }
        batches += t.drain().iter().filter(|event| matches!(event, Event::TunnelStats(_))).count();
    }
    assert!((2..=4).contains(&batches), "{batches} batches in 3.5 seconds");

    t.engine.handle_command(Command::StopTunnel { key: wanted });
    t.drain();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(internal)) = timeout(Duration::from_millis(100), t.internal.recv()).await {
            t.engine.handle_internal(internal);
        }
    }
    assert!(!t.drain().iter().any(|event| matches!(event, Event::TunnelStats(_))));
    assert!(t.engine.tunnel_run.ticker.is_none());
}

#[tokio::test]
async fn a_protected_file_refuses_every_change() {
    let mut t = test_engine();
    t.engine.tunnels = Tunnels::protected_for_test();
    t.drain();

    save(&mut t, local("prod", "db", 7020));
    t.engine.handle_command(Command::DeleteTunnel { key: key("prod", "db") });

    assert_eq!(t.engine.tunnels.iter().count(), 0);
    assert_eq!(file_text(&t), "");
    assert_eq!(t.notices().iter().filter(|(severity, _)| *severity == crate::Severity::Error).count(), 1);
}

#[tokio::test]
async fn startup_publishes_the_saved_tunnels_as_stopped_or_offline() {
    let mut t = test_engine();
    t.engine.tunnels.upsert(None, local("online", "a", 7030)).unwrap();
    t.engine.tunnels.upsert(None, local("away", "b", 7031)).unwrap();
    t.add_session("online");
    t.drain();

    t.engine.publish_tunnels();

    let list = infos(t.drain()).unwrap();
    let states: Vec<_> = list.iter().map(|info| (info.spec.name.clone(), info.status.clone())).collect();
    assert_eq!(states, [("a".to_string(), TunnelStatus::Stopped), ("b".to_string(), TunnelStatus::Offline)]);
}

#[tokio::test]
async fn starting_an_unknown_tunnel_says_so() {
    let mut t = test_engine();
    t.drain();

    t.engine.handle_command(Command::StartTunnel { key: key("prod", "ghost") });

    assert!(t.notices().iter().any(|(severity, text)| *severity == crate::Severity::Warning && text.contains("ghost")));
}

#[tokio::test]
async fn a_connection_that_recovers_on_the_same_session_restarts_only_auto_start_tunnels() {
    let mut t = test_engine();
    let session = connected(&mut t, "srv").await;
    save(&mut t, auto(local("srv", "auto", free_port().await)));
    save(&mut t, local("srv", "manual", free_port().await));
    let (auto_key, manual_key) = (key("srv", "auto"), key("srv", "manual"));
    t.engine.handle_command(Command::StartTunnel { key: auto_key.clone() });
    t.engine.handle_command(Command::StartTunnel { key: manual_key.clone() });
    running(&mut t, &auto_key).await;
    running(&mut t, &manual_key).await;
    t.engine.handle_internal(Internal::SessionHealth { session, healthy: false });
    assert_eq!((status(&t, &auto_key), status(&t, &manual_key)), (TunnelStatus::Lost, TunnelStatus::Lost));

    t.engine.handle_internal(Internal::SessionHealth { session, healthy: true });

    running(&mut t, &auto_key).await;
    assert_eq!(status(&t, &manual_key), TunnelStatus::Stopped);
}
