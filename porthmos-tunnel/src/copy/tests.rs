use std::sync::atomic::Ordering;

use porthmos_vfs::ForwardStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use super::*;

fn ends() -> (DuplexStream, DuplexStream, ForwardStream, DuplexStream) {
    let (client_peer, client) = tokio::io::duplex(4096);
    let (remote, remote_peer) = tokio::io::duplex(4096);
    (client_peer, client, Box::new(remote), remote_peer)
}

#[tokio::test]
async fn counters_follow_the_bytes_in_each_direction() {
    let (mut client_peer, client, remote, mut remote_peer) = ends();
    let stats = Stats::default();
    let piping = tokio::spawn({
        let stats = stats.clone();
        async move { pipe(client, remote, &stats).await }
    });

    client_peer.write_all(&[1u8; 1000]).await.unwrap();
    remote_peer.write_all(&[2u8; 400]).await.unwrap();
    let mut from_client = [0u8; 1000];
    remote_peer.read_exact(&mut from_client).await.unwrap();
    let mut from_remote = [0u8; 400];
    client_peer.read_exact(&mut from_remote).await.unwrap();
    client_peer.shutdown().await.unwrap();
    remote_peer.shutdown().await.unwrap();
    piping.await.unwrap().unwrap();

    assert_eq!(stats.bytes_up.load(Ordering::Relaxed), 1000);
    assert_eq!(stats.bytes_down.load(Ordering::Relaxed), 400);
}

#[tokio::test]
async fn a_large_transfer_arrives_complete_and_in_order() {
    let (mut client_peer, client, remote, mut remote_peer) = ends();
    let stats = Stats::default();
    let piping = tokio::spawn({
        let stats = stats.clone();
        async move { pipe(client, remote, &stats).await }
    });
    let size = 4 * 1024 * 1024;
    let up: Vec<u8> = (0..size).map(|index| (index % 251) as u8).collect();
    let down: Vec<u8> = (0..size).map(|index| (index % 241) as u8).collect();

    let (mut client_read, mut client_write) = tokio::io::split(&mut client_peer);
    let (mut remote_read, mut remote_write) = tokio::io::split(&mut remote_peer);
    let sending_up = async {
        client_write.write_all(&up).await.unwrap();
        client_write.shutdown().await.unwrap();
    };
    let sending_down = async {
        remote_write.write_all(&down).await.unwrap();
        remote_write.shutdown().await.unwrap();
    };
    let receiving_up = async {
        let mut received = Vec::new();
        remote_read.read_to_end(&mut received).await.unwrap();
        received
    };
    let receiving_down = async {
        let mut received = Vec::new();
        client_read.read_to_end(&mut received).await.unwrap();
        received
    };
    let (_, _, got_up, got_down) = tokio::join!(sending_up, sending_down, receiving_up, receiving_down);

    assert!(got_up == up, "upstream bytes differ");
    assert!(got_down == down, "downstream bytes differ");
    piping.await.unwrap().unwrap();
    assert_eq!(stats.bytes_up.load(Ordering::Relaxed), size as u64);
    assert_eq!(stats.bytes_down.load(Ordering::Relaxed), size as u64);
}

#[tokio::test]
async fn half_close_from_the_client_keeps_the_reply_direction_open() {
    let (mut client_peer, client, remote, mut remote_peer) = ends();
    let stats = Stats::default();
    let piping = tokio::spawn(async move { pipe(client, remote, &stats).await });

    client_peer.write_all(b"request").await.unwrap();
    client_peer.shutdown().await.unwrap();
    let mut received = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(2), remote_peer.read_to_end(&mut received))
        .await
        .expect("the end of the request reached the remote side")
        .unwrap();
    remote_peer.write_all(b"reply").await.unwrap();
    remote_peer.shutdown().await.unwrap();
    let mut answer = Vec::new();
    client_peer.read_to_end(&mut answer).await.unwrap();

    assert_eq!((received.as_slice(), answer.as_slice()), (&b"request"[..], &b"reply"[..]));
    piping.await.unwrap().unwrap();
}

#[tokio::test]
async fn dropping_both_ends_ends_the_pipe() {
    let (client_peer, client, remote, _remote_peer) = ends();
    let stats = Stats::default();
    let piping = tokio::spawn(async move { pipe(client, remote, &stats).await });

    drop(client_peer);
    drop(_remote_peer);

    tokio::time::timeout(std::time::Duration::from_secs(2), piping).await.expect("the pipe ended").unwrap().unwrap();
}
