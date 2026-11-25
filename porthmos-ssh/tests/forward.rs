use std::{sync::Arc, time::Duration};

use porthmos_ssh::{
    SshForwarder,
    testing::{Options, SshServer},
};
use porthmos_vfs::{ErrorKind, PortForwarder};
use std::net::IpAddr;
use tokio::net::TcpStream;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};

const FORWARDING: Options = Options {
    hide_scp: false,
    refuse_exec: false,
    refuse_shell: false,
    max_sessions: None,
    forwarding: true,
    replace: &[],
};

const NO_FORWARDING: Options = Options {
    hide_scp: false,
    refuse_exec: false,
    refuse_shell: false,
    max_sessions: None,
    forwarding: false,
    replace: &[],
};

async fn echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut reader, mut writer) = stream.split();
                let _ = tokio::io::copy(&mut reader, &mut writer).await;
            });
        }
    });
    port
}

async fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn a_forwarded_connection_reaches_a_local_echo_server() {
    let server = SshServer::start(FORWARDING).await;
    let echo = echo_server().await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));

    let mut stream = forwarder.connect("127.0.0.1", echo).await.unwrap();
    stream.write_all(b"ping").await.unwrap();
    let mut received = [0u8; 4];
    timeout(Duration::from_secs(5), stream.read_exact(&mut received)).await.unwrap().unwrap();

    assert_eq!(&received, b"ping");
    assert_eq!(server.forwarded_connections(), 1);
}

#[tokio::test]
async fn a_refused_target_is_an_error_naming_the_target() {
    let server = SshServer::start(FORWARDING).await;
    let port = closed_port().await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));

    let error = forwarder.connect("127.0.0.1", port).await.err().expect("nothing listens there");

    assert_ne!(error.kind(), ErrorKind::Unsupported);
    assert!(error.to_string().contains(&format!("127.0.0.1:{port}")), "{error}");
}

#[tokio::test]
async fn a_server_that_prohibits_forwarding_reports_unsupported() {
    let server = SshServer::start(NO_FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));

    let error = forwarder.connect("127.0.0.1", 9).await.err().expect("forwarding is prohibited");

    assert_eq!(error.kind(), ErrorKind::Unsupported);
}

#[tokio::test]
async fn a_dropped_server_is_an_error_not_a_hang() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    drop(server);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = timeout(Duration::from_secs(5), forwarder.connect("127.0.0.1", 9)).await;

    assert!(result.expect("must not hang").is_err());
}

fn loopback() -> IpAddr {
    IpAddr::from([127, 0, 0, 1])
}

async fn read_exactly(stream: &mut (impl AsyncReadExt + Unpin), length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    timeout(Duration::from_secs(5), stream.read_exact(&mut bytes)).await.expect("bytes within five seconds").unwrap();
    bytes
}

#[tokio::test]
async fn a_remote_forward_delivers_a_connection_with_its_origin() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    let mut listener = forwarder.listen(loopback(), 0).await.unwrap();

    let mut dialler = TcpStream::connect(("127.0.0.1", listener.port)).await.unwrap();
    dialler.write_all(b"ping").await.unwrap();
    let mut incoming = timeout(Duration::from_secs(5), listener.incoming.recv()).await.unwrap().unwrap();

    assert!(incoming.origin.starts_with("127.0.0.1:"), "{}", incoming.origin);
    assert_eq!(read_exactly(&mut incoming.stream, 4).await, b"ping");
    incoming.stream.write_all(b"pong").await.unwrap();
    assert_eq!(read_exactly(&mut dialler, 4).await, b"pong");
}

#[tokio::test]
async fn port_zero_reports_the_port_the_server_chose() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));

    let listener = forwarder.listen(loopback(), 0).await.unwrap();

    assert_ne!(listener.port, 0);
}

#[tokio::test]
async fn a_requested_port_is_returned_as_is() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    let port = closed_port().await;

    let listener = forwarder.listen(loopback(), port).await.unwrap();

    assert_eq!(listener.port, port);
}

#[tokio::test]
async fn a_server_that_refuses_remote_forwarding_reports_a_clear_error() {
    let server = SshServer::start(NO_FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));

    let error = forwarder.listen(loopback(), 0).await.err().expect("the server refuses");

    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert!(error.to_string().contains("did not allow"), "{error}");
}

#[tokio::test]
async fn a_port_already_taken_on_the_server_is_an_error_not_a_fallback() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    let first = forwarder.listen(loopback(), 0).await.unwrap();

    let error = forwarder.listen(loopback(), first.port).await.err().expect("the port is taken");

    assert_eq!(error.kind(), ErrorKind::Unsupported);
    let mut dialler = TcpStream::connect(("127.0.0.1", first.port)).await.unwrap();
    dialler.write_all(b"x").await.unwrap();
    let mut first = first;
    assert!(timeout(Duration::from_secs(5), first.incoming.recv()).await.unwrap().is_some());
}

#[tokio::test]
async fn close_cancels_the_forward_on_the_server_and_the_port_can_be_listened_on_again() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    let listener = forwarder.listen(loopback(), 0).await.unwrap();
    let port = listener.port;
    assert_eq!(server.remote_listeners(), 1);

    listener.close().await;

    assert_eq!(server.remote_listeners(), 0);
    let again = forwarder.listen(loopback(), port).await.unwrap();
    assert_eq!(again.port, port);
}

#[tokio::test]
async fn a_forwarded_channel_for_an_unregistered_address_is_rejected() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    let mut listener = forwarder.listen(loopback(), 0).await.unwrap();
    let stranger = closed_port().await;

    assert!(!server.open_forwarded_channel("127.0.0.1", stranger).await);
    assert!(!server.open_forwarded_channel("10.9.8.7", listener.port).await);
    assert!(timeout(Duration::from_millis(300), listener.incoming.recv()).await.is_err());
    assert!(server.open_forwarded_channel("127.0.0.1", listener.port).await);
    assert!(timeout(Duration::from_secs(5), listener.incoming.recv()).await.unwrap().is_some());
}

#[tokio::test]
async fn many_parallel_remote_connections_are_delivered_independently() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    let mut listener = forwarder.listen(loopback(), 0).await.unwrap();
    let port = listener.port;
    let serving = tokio::spawn(async move {
        let mut tasks = Vec::new();
        for _ in 0..20 {
            let Some(mut incoming) = listener.incoming.recv().await else { break };
            tasks.push(tokio::spawn(async move {
                let mut buffer = [0u8; 8];
                let count = incoming.stream.read(&mut buffer).await.unwrap();
                incoming.stream.write_all(&buffer[..count]).await.unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
    });

    let mut dials = Vec::new();
    for index in 0..20u8 {
        dials.push(tokio::spawn(async move {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            stream.write_all(&[index; 4]).await.unwrap();
            read_exactly(&mut stream, 4).await
        }));
    }

    for (index, dial) in dials.into_iter().enumerate() {
        assert_eq!(dial.await.unwrap(), vec![index as u8; 4]);
    }
    timeout(Duration::from_secs(5), serving).await.unwrap().unwrap();
}

#[tokio::test]
async fn a_dropped_server_closes_the_incoming_channel() {
    let server = SshServer::start(FORWARDING).await;
    let forwarder = SshForwarder::new(Arc::new(server.session().await));
    let mut listener = forwarder.listen(loopback(), 0).await.unwrap();

    server.disconnect_clients().await;

    let ended = timeout(Duration::from_secs(5), listener.incoming.recv()).await.expect("must not hang");
    assert!(ended.is_none());
}
