use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use porthmos_vfs::{ErrorKind, ForwardStream, PortForwarder, ProtocolError, async_trait};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

use super::*;
use crate::spec::{Endpoint, Kind, Listen};

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

async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port()
}

struct Loopback {
    address: SocketAddr,
    asked: Mutex<Vec<(String, u16)>>,
}

impl Loopback {
    fn to(address: SocketAddr) -> Arc<Self> {
        Arc::new(Self { address, asked: Mutex::default() })
    }
}

#[async_trait]
impl PortForwarder for Loopback {
    async fn connect(&self, host: &str, port: u16) -> Result<ForwardStream, ProtocolError> {
        self.asked.lock().unwrap().push((host.to_string(), port));
        let stream = TcpStream::connect(self.address)
            .await
            .map_err(|err| ProtocolError::new(ErrorKind::Connect, anyhow::anyhow!(err.to_string())))?;
        Ok(Box::new(stream))
    }
}

struct Refusing;

#[async_trait]
impl PortForwarder for Refusing {
    async fn connect(&self, host: &str, port: u16) -> Result<ForwardStream, ProtocolError> {
        Err(ProtocolError::new(ErrorKind::Connect, anyhow::anyhow!("refused {host}:{port}")))
    }
}

struct Prohibiting;

#[async_trait]
impl PortForwarder for Prohibiting {
    async fn connect(&self, _host: &str, _port: u16) -> Result<ForwardStream, ProtocolError> {
        Err(ProtocolError::new(ErrorKind::Unsupported, anyhow::anyhow!("forwarding is not allowed")))
    }
}

struct Broken;

#[async_trait]
impl PortForwarder for Broken {
    async fn connect(&self, _host: &str, _port: u16) -> Result<ForwardStream, ProtocolError> {
        Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("something else")))
    }
}

struct Silent;

#[async_trait]
impl PortForwarder for Silent {
    async fn connect(&self, _host: &str, _port: u16) -> Result<ForwardStream, ProtocolError> {
        std::future::pending().await
    }
}

fn local_kind(port: u16) -> Kind {
    Kind::Local {
        listen: Listen { address: "127.0.0.1".parse().unwrap(), port },
        target: Endpoint { host: "db.internal".to_string(), port: 5432 },
    }
}

fn spec(port: u16) -> TunnelSpec {
    TunnelSpec {
        connection: "prod".to_string(),
        name: "db".to_string(),
        kind: local_kind(port),
        allow_external: false,
        auto_start: false,
    }
}

async fn serving(forwarder: Arc<dyn PortForwarder>) -> RunningTunnel {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    serve(listener, local_kind(1), forwarder)
}

async fn until(mut condition: impl FnMut() -> bool) {
    timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the condition held within five seconds");
}

async fn round_trip(address: SocketAddr, payload: &[u8]) -> Vec<u8> {
    let mut client = TcpStream::connect(address).await.unwrap();
    client.write_all(payload).await.unwrap();
    let mut received = vec![0u8; payload.len()];
    timeout(Duration::from_secs(5), client.read_exact(&mut received)).await.unwrap().unwrap();
    received
}

#[tokio::test]
async fn a_local_tunnel_carries_bytes_both_ways_to_an_echo_server() {
    let forwarder = Loopback::to(echo_server().await);
    let tunnel = serving(forwarder.clone()).await;

    let echoed = round_trip(tunnel.local_addr(), b"hello through the tunnel").await;

    assert_eq!(echoed, b"hello through the tunnel");
    until(|| tunnel.stats().bytes_down == 24).await;
    let stats = tunnel.stats();
    assert_eq!((stats.bytes_up, stats.bytes_down, stats.connections), (24, 24, 1));
    assert_eq!(forwarder.asked.lock().unwrap().as_slice(), [("db.internal".to_string(), 5432)]);
    tunnel.stop().await;
}

#[tokio::test]
async fn half_close_from_the_client_still_delivers_the_reply() {
    let tunnel = serving(Loopback::to(echo_server().await)).await;
    let mut client = TcpStream::connect(tunnel.local_addr()).await.unwrap();

    client.write_all(b"last words").await.unwrap();
    client.shutdown().await.unwrap();
    let mut received = Vec::new();
    timeout(Duration::from_secs(5), client.read_to_end(&mut received)).await.unwrap().unwrap();

    assert_eq!(received, b"last words");
    tunnel.stop().await;
}

#[tokio::test]
async fn many_parallel_connections_all_complete_and_are_counted() {
    let tunnel = serving(Loopback::to(echo_server().await)).await;
    let address = tunnel.local_addr();

    let clients: Vec<_> = (0..64u8)
        .map(|index| tokio::spawn(async move { round_trip(address, &[index; 100]).await == vec![index; 100] }))
        .collect();
    for client in clients {
        assert!(client.await.unwrap());
    }

    until(|| tunnel.stats().bytes_down == 6400).await;
    assert_eq!(tunnel.stats().connections, 64);
    tunnel.stop().await;
}

#[tokio::test]
async fn the_257th_simultaneous_connection_is_closed_and_counted_as_refused() {
    let tunnel = serving(Arc::new(Silent)).await;
    let address = tunnel.local_addr();
    let mut held = Vec::new();
    for _ in 0..MAX_CONNECTIONS {
        held.push(TcpStream::connect(address).await.unwrap());
    }
    until(|| tunnel.stats().active == MAX_CONNECTIONS).await;

    let mut extra = TcpStream::connect(address).await.unwrap();
    let mut byte = [0u8; 1];
    let read = timeout(Duration::from_secs(5), extra.read(&mut byte)).await.unwrap();

    assert!(matches!(read, Ok(0) | Err(_)));
    let stats = tunnel.stats();
    assert_eq!((stats.refused, stats.active, stats.connections), (1, MAX_CONNECTIONS, MAX_CONNECTIONS as u64));
    drop(held);
    tunnel.stop().await;
}

#[tokio::test]
async fn a_slot_is_free_again_once_a_connection_ends() {
    let tunnel = serving(Loopback::to(echo_server().await)).await;
    for _ in 0..3 {
        round_trip(tunnel.local_addr(), b"x").await;
    }

    until(|| tunnel.stats().active == 0).await;
    assert_eq!(tunnel.stats().refused, 0);
    tunnel.stop().await;
}

#[tokio::test]
async fn a_target_that_refuses_closes_only_that_connection_and_counts_an_error() {
    let tunnel = serving(Arc::new(Refusing)).await;

    for _ in 0..2 {
        let mut client = TcpStream::connect(tunnel.local_addr()).await.unwrap();
        let mut byte = [0u8; 1];
        let read = timeout(Duration::from_secs(5), client.read(&mut byte)).await.unwrap();
        assert!(matches!(read, Ok(0) | Err(_)));
    }

    until(|| tunnel.stats().errors == 2).await;
    until(|| tunnel.stats().active == 0).await;
    tunnel.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_forwarder_that_never_answers_is_given_up_on_and_counted() {
    let tunnel = serving(Arc::new(Silent)).await;
    let mut client = TcpStream::connect(tunnel.local_addr()).await.unwrap();

    let mut byte = [0u8; 1];
    let read = client.read(&mut byte).await;

    assert!(matches!(read, Ok(0) | Err(_)));
    assert_eq!(tunnel.stats().errors, 1);
    until(|| tunnel.stats().active == 0).await;
    tunnel.stop().await;
}

#[tokio::test]
async fn stop_closes_live_connections_and_frees_the_port() {
    let tunnel = serving(Loopback::to(echo_server().await)).await;
    let address = tunnel.local_addr();
    let mut client = TcpStream::connect(address).await.unwrap();
    client.write_all(b"x").await.unwrap();
    let mut byte = [0u8; 1];
    client.read_exact(&mut byte).await.unwrap();

    tunnel.stop().await;

    let read = timeout(Duration::from_secs(2), client.read(&mut byte)).await.expect("the connection was closed");
    assert!(matches!(read, Ok(0) | Err(_)));
    TcpListener::bind(address).await.expect("the port is free again");
}

#[tokio::test]
async fn dropping_a_running_tunnel_closes_its_listener() {
    let tunnel = serving(Loopback::to(echo_server().await)).await;
    let address = tunnel.local_addr();

    drop(tunnel);

    until_rebindable(address).await;
}

async fn until_rebindable(address: SocketAddr) {
    timeout(Duration::from_secs(5), async {
        while TcpListener::bind(address).await.is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the port was released");
}

#[tokio::test]
async fn starting_validates_binds_and_can_restart_on_the_same_port() {
    let port = free_port().await;
    let forwarder = Loopback::to(echo_server().await);

    let first = RunningTunnel::start(&spec(port), forwarder.clone()).await.unwrap();
    assert_eq!(first.local_addr().port(), port);
    assert_eq!(round_trip(first.local_addr(), b"one").await, b"one");
    first.stop().await;

    let second = RunningTunnel::start(&spec(port), forwarder).await.unwrap();
    assert_eq!(round_trip(second.local_addr(), b"two").await, b"two");
    second.stop().await;
}

#[tokio::test]
async fn a_port_already_in_use_is_a_bind_error_naming_the_address() {
    let holder = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = holder.local_addr().unwrap().port();

    let error = RunningTunnel::start(&spec(port), Arc::new(Refusing)).await.err().expect("the port is taken");

    match error {
        StartError::Bind { address, reason } => {
            assert_eq!(address, format!("127.0.0.1:{port}"));
            assert!(!reason.is_empty());
        }
        other => panic!("expected a bind error, got {other:?}"),
    }
}

#[tokio::test]
async fn starting_an_invalid_spec_reports_the_spec_error_and_binds_nothing() {
    let port = free_port().await;
    let mut external = spec(port);
    external.kind = Kind::Local {
        listen: Listen { address: "0.0.0.0".parse().unwrap(), port },
        target: Endpoint { host: "db".to_string(), port: 1 },
    };

    let error = RunningTunnel::start(&external, Arc::new(Refusing)).await.err().expect("must be refused");

    assert!(matches!(error, StartError::Invalid(SpecError::ExternalNotAllowed)), "{error:?}");
    TcpListener::bind(("0.0.0.0", port)).await.expect("nothing was bound");
}

fn dynamic_kind(port: u16) -> Kind {
    Kind::Dynamic { listen: Listen { address: "127.0.0.1".parse().unwrap(), port } }
}

async fn serving_dynamic(forwarder: Arc<dyn PortForwarder>) -> RunningTunnel {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    serve(listener, dynamic_kind(1), forwarder)
}

async fn socks_connect(address: SocketAddr, request: &[u8]) -> (TcpStream, [u8; 10]) {
    let mut client = TcpStream::connect(address).await.unwrap();
    client.write_all(&[5, 1, 0]).await.unwrap();
    let mut method = [0u8; 2];
    timeout(Duration::from_secs(5), client.read_exact(&mut method)).await.unwrap().unwrap();
    assert_eq!(method, [5, 0]);
    client.write_all(request).await.unwrap();
    let mut answer = [0u8; 10];
    timeout(Duration::from_secs(5), client.read_exact(&mut answer)).await.unwrap().unwrap();
    (client, answer)
}

fn domain_request(host: &str, port: u16) -> Vec<u8> {
    let mut request = vec![5, 1, 0, 3, host.len() as u8];
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&port.to_be_bytes());
    request
}

#[tokio::test]
async fn starting_a_dynamic_spec_binds_and_serves() {
    let port = free_port().await;
    let mut dynamic = spec(port);
    dynamic.name = "socks".to_string();
    dynamic.kind = dynamic_kind(port);

    let tunnel = RunningTunnel::start(&dynamic, Loopback::to(echo_server().await)).await.unwrap();

    let (mut client, answer) = socks_connect(tunnel.local_addr(), &domain_request("anything", 1)).await;
    assert_eq!(answer, [5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    client.write_all(b"hello").await.unwrap();
    let mut echoed = [0u8; 5];
    client.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"hello");
    tunnel.stop().await;
}

#[tokio::test]
async fn a_socks_client_reaches_a_target_through_a_dynamic_tunnel_and_counters_move() {
    let tunnel = serving_dynamic(Loopback::to(echo_server().await)).await;

    let (mut client, answer) = socks_connect(tunnel.local_addr(), &domain_request("db.internal", 5432)).await;
    client.write_all(b"0123456789").await.unwrap();
    let mut echoed = [0u8; 10];
    timeout(Duration::from_secs(5), client.read_exact(&mut echoed)).await.unwrap().unwrap();

    assert_eq!(answer[..2], [5, 0]);
    assert_eq!(&echoed, b"0123456789");
    until(|| tunnel.stats().bytes_down == 10).await;
    assert_eq!((tunnel.stats().bytes_up, tunnel.stats().connections, tunnel.stats().errors), (10, 1, 0));
    tunnel.stop().await;
}

#[tokio::test]
async fn a_domain_request_reaches_the_forwarder_as_a_name_and_an_address_as_text() {
    let forwarder = Loopback::to(echo_server().await);
    let tunnel = serving_dynamic(forwarder.clone()).await;

    socks_connect(tunnel.local_addr(), &domain_request("internal.example", 8080)).await;
    socks_connect(tunnel.local_addr(), &[5, 1, 0, 1, 10, 0, 0, 5, 0, 22]).await;

    assert_eq!(
        forwarder.asked.lock().unwrap().as_slice(),
        [("internal.example".to_string(), 8080), ("10.0.0.5".to_string(), 22)]
    );
    tunnel.stop().await;
}

#[tokio::test]
async fn a_refused_target_replies_connection_refused_and_closes() {
    let tunnel = serving_dynamic(Arc::new(Refusing)).await;

    let (mut client, answer) = socks_connect(tunnel.local_addr(), &domain_request("nowhere", 1)).await;

    assert_eq!(answer, [5, 5, 0, 1, 0, 0, 0, 0, 0, 0]);
    let mut byte = [0u8; 1];
    assert!(matches!(timeout(Duration::from_secs(5), client.read(&mut byte)).await.unwrap(), Ok(0) | Err(_)));
    until(|| tunnel.stats().errors == 1).await;
    tunnel.stop().await;
}

#[tokio::test]
async fn garbage_bytes_close_the_connection_and_the_listener_keeps_working() {
    let tunnel = serving_dynamic(Loopback::to(echo_server().await)).await;
    let mut garbage = TcpStream::connect(tunnel.local_addr()).await.unwrap();
    let noise: Vec<u8> = (0..1024u32).map(|index| (index.wrapping_mul(2654435761) >> 13) as u8 | 0x80).collect();
    garbage.write_all(&noise).await.unwrap();
    let mut byte = [0u8; 1];
    assert!(matches!(timeout(Duration::from_secs(5), garbage.read(&mut byte)).await.unwrap(), Ok(0) | Err(_)));

    let (_client, answer) = socks_connect(tunnel.local_addr(), &domain_request("fine", 1)).await;

    assert_eq!(answer[..2], [5, 0]);
    until(|| tunnel.stats().errors == 1).await;
    tunnel.stop().await;
}

#[tokio::test]
async fn many_slow_handshakes_do_not_block_valid_clients() {
    let tunnel = serving_dynamic(Loopback::to(echo_server().await)).await;
    let mut idle = Vec::new();
    for _ in 0..50 {
        idle.push(TcpStream::connect(tunnel.local_addr()).await.unwrap());
    }

    let (_client, answer) =
        timeout(Duration::from_secs(2), socks_connect(tunnel.local_addr(), &domain_request("fine", 1)))
            .await
            .expect("a valid client is served while others sit idle");

    assert_eq!(answer[..2], [5, 0]);
    drop(idle);
    tunnel.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_client_that_never_finishes_the_handshake_is_dropped_after_ten_seconds() {
    let tunnel = serving_dynamic(Arc::new(Refusing)).await;
    let mut client = TcpStream::connect(tunnel.local_addr()).await.unwrap();
    client.write_all(&[5, 1, 0]).await.unwrap();

    let mut buffer = [0u8; 8];
    let mut total = 0;
    while let Ok(count) = client.read(&mut buffer).await {
        if count == 0 {
            break;
        }
        total += count;
    }

    assert_eq!(total, 2);
    until(|| tunnel.stats().errors == 1).await;
    until(|| tunnel.stats().active == 0).await;
    tunnel.stop().await;
}

#[tokio::test]
async fn a_forwarder_that_prohibits_or_fails_otherwise_gets_the_matching_reply_code() {
    for (forwarder, code) in [(Arc::new(Prohibiting) as Arc<dyn PortForwarder>, 2u8), (Arc::new(Broken), 1)] {
        let tunnel = serving_dynamic(forwarder).await;

        let (_client, answer) = socks_connect(tunnel.local_addr(), &domain_request("x", 1)).await;

        assert_eq!(answer, [5, code, 0, 1, 0, 0, 0, 0, 0, 0]);
        tunnel.stop().await;
    }
}
