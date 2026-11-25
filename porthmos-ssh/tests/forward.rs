use std::{sync::Arc, time::Duration};

use porthmos_ssh::{
    SshForwarder,
    testing::{Options, SshServer},
};
use porthmos_vfs::{ErrorKind, PortForwarder};
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
    let server = SshServer::start(Options::default()).await;
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
