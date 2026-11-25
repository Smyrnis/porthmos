#![cfg(unix)]

use std::time::Duration;

use porthmos_scp::Scp;
use porthmos_ssh::testing::{self, Options, SshServer};
use porthmos_vfs::{Answer, FileSystem, Protocol};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};

async fn connected(server: &SshServer) -> std::sync::Arc<dyn FileSystem> {
    Scp::default()
        .with_connect_options(server.connect_options())
        .connect(&server.target(), &mut testing::answers(vec![Some(Answer::Confirmed)]))
        .await
        .unwrap()
}

#[tokio::test]
async fn the_file_system_offers_a_forwarder_that_reaches_a_target() {
    let options = Options { forwarding: true, ..Options::default() };
    let server = SshServer::start(options).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (mut reader, mut writer) = stream.split();
        let _ = tokio::io::copy(&mut reader, &mut writer).await;
    });
    let fs = connected(&server).await;

    assert!(fs.can_forward());
    let mut stream = fs.forwarder().expect("scp forwards").connect("127.0.0.1", port).await.unwrap();
    stream.write_all(b"ping").await.unwrap();
    let mut received = [0u8; 4];
    timeout(Duration::from_secs(5), stream.read_exact(&mut received)).await.unwrap().unwrap();

    assert_eq!(&received, b"ping");
}
