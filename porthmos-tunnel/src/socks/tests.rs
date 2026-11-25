use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use super::*;

const NO_AUTH_REPLY: [u8; 2] = [5, 0];

fn domain_request(host: &str, port: u16) -> Vec<u8> {
    let mut request = vec![5, 1, 0, 3, host.len() as u8];
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&port.to_be_bytes());
    request
}

fn with_greeting(request: &[u8]) -> Vec<u8> {
    let mut bytes = vec![5, 1, 0];
    bytes.extend_from_slice(request);
    bytes
}

struct Exchange {
    outcome: Result<Request, HandshakeError>,
    written: Vec<u8>,
    left_over: Vec<u8>,
}

async fn exchange(client_bytes: &[u8]) -> Exchange {
    let (mut client, mut server) = tokio::io::duplex(4096);
    client.write_all(client_bytes).await.unwrap();
    client.shutdown().await.unwrap();
    let outcome = handshake(&mut server).await;
    let mut left_over = Vec::new();
    server.read_to_end(&mut left_over).await.unwrap();
    drop(server);
    let mut written = Vec::new();
    client.read_to_end(&mut written).await.unwrap();
    Exchange { outcome, written, left_over }
}

fn target(exchange: &Exchange) -> Endpoint {
    match &exchange.outcome {
        Ok(Request::Connect(endpoint)) => endpoint.clone(),
        Err(error) => panic!("expected a request, got {error:?}"),
    }
}

#[tokio::test]
async fn a_connect_to_a_domain_ipv4_and_ipv6_parses() {
    let domain = exchange(&with_greeting(&domain_request("db.internal", 5432))).await;
    assert_eq!(target(&domain), Endpoint { host: "db.internal".to_string(), port: 5432 });
    assert_eq!(domain.written, NO_AUTH_REPLY);

    let ipv4 = exchange(&with_greeting(&[5, 1, 0, 1, 10, 0, 0, 5, 0x01, 0xbb])).await;
    assert_eq!(target(&ipv4), Endpoint { host: "10.0.0.5".to_string(), port: 443 });

    let mut ipv6_request = vec![5, 1, 0, 4];
    ipv6_request.extend_from_slice(&[0; 15]);
    ipv6_request.extend_from_slice(&[1, 0, 80]);
    let ipv6 = exchange(&with_greeting(&ipv6_request)).await;
    assert_eq!(target(&ipv6), Endpoint { host: "::1".to_string(), port: 80 });
}

#[tokio::test]
async fn the_largest_domain_is_accepted_and_pipelined_bytes_stay_in_the_stream() {
    let host = "h".repeat(255);
    let mut bytes = with_greeting(&domain_request(&host, 1));
    bytes.extend_from_slice(b"early data");

    let result = exchange(&bytes).await;

    assert_eq!(target(&result).host, host);
    assert_eq!(result.left_over, b"early data");
}

#[tokio::test]
async fn the_method_reply_is_no_auth_or_no_acceptable() {
    let mut bytes = vec![5, 3, 1, 2, 0];
    bytes.extend_from_slice(&domain_request("a", 1));
    let accepted = exchange(&bytes).await;
    assert!(accepted.outcome.is_ok());
    assert_eq!(accepted.written, NO_AUTH_REPLY);

    let refused = exchange(&[5, 2, 1, 2]).await;
    assert_eq!(refused.outcome.err(), Some(HandshakeError::NoAcceptableMethod));
    assert_eq!(refused.written, [5, 0xff]);

    let none = exchange(&[5, 0]).await;
    assert_eq!(none.outcome.err(), Some(HandshakeError::NoAcceptableMethod));
    assert_eq!(none.written, [5, 0xff]);
}

#[tokio::test]
async fn version_four_and_other_versions_are_refused_without_a_reply() {
    for first in [4u8, 0, 6, 0x47] {
        let result = exchange(&[first, 1, 0, 1, 2, 3]).await;
        assert_eq!(result.outcome.err(), Some(HandshakeError::BadVersion), "{first}");
        assert!(result.written.is_empty());
    }
    let wrong_request_version = exchange(&with_greeting(&[4, 1, 0, 1, 1, 1, 1, 1, 0, 1])).await;
    assert_eq!(wrong_request_version.outcome.err(), Some(HandshakeError::BadVersion));
}

#[tokio::test]
async fn bind_and_udp_associate_get_command_not_supported() {
    for command in [2u8, 3, 0, 9] {
        let result = exchange(&with_greeting(&[5, command, 0, 1, 1, 2, 3, 4, 0, 80])).await;
        assert_eq!(result.outcome.err(), Some(HandshakeError::UnsupportedCommand), "{command}");
        assert_eq!(result.written, [5, 0, 5, 7, 0, 1, 0, 0, 0, 0, 0, 0]);
    }
}

#[tokio::test]
async fn an_unknown_address_type_gets_address_type_not_supported() {
    for kind in [0u8, 2, 5, 9, 255] {
        let result = exchange(&with_greeting(&[5, 1, 0, kind, 1, 2, 3, 4, 0, 80])).await;
        assert_eq!(result.outcome.err(), Some(HandshakeError::UnsupportedAddress), "{kind}");
        assert_eq!(result.written, [5, 0, 5, 8, 0, 1, 0, 0, 0, 0, 0, 0]);
    }
}

#[tokio::test]
async fn malformed_requests_are_closed_without_a_reply() {
    let mut cases: Vec<(&str, Vec<u8>)> = vec![
        ("a zero-length domain", with_greeting(&[5, 1, 0, 3, 0, 0, 80])),
        ("a non-zero reserved byte", with_greeting(&[5, 1, 1, 1, 1, 2, 3, 4, 0, 80])),
        ("port zero", with_greeting(&[5, 1, 0, 1, 1, 2, 3, 4, 0, 0])),
        ("a domain with a space", with_greeting(&domain_request("a b", 80))),
        ("a domain with a control character", with_greeting(&domain_request("a\nb", 80))),
        ("a domain that is not text", with_greeting(&[5, 1, 0, 3, 2, 0xff, 0xfe, 0, 80])),
    ];
    cases.push(("an empty stream", Vec::new()));
    cases.push(("a greeting that stops", vec![5, 3, 0]));
    cases.push(("a request that stops after the header", with_greeting(&[5, 1, 0, 3])));
    cases.push(("a domain that stops short", with_greeting(&[5, 1, 0, 3, 9, b'a', b'b'])));
    cases.push(("an ipv4 address that stops short", with_greeting(&[5, 1, 0, 1, 1, 2])));
    cases.push(("an ipv6 address that stops short", with_greeting(&[5, 1, 0, 4, 1, 2, 3])));
    cases.push(("a port that stops short", with_greeting(&[5, 1, 0, 1, 1, 2, 3, 4, 0])));
    for (label, bytes) in cases {
        let result = exchange(&bytes).await;
        assert_eq!(result.outcome.err(), Some(HandshakeError::Malformed), "{label}");
    }
}

async fn silent_client() -> (DuplexStream, DuplexStream) {
    tokio::io::duplex(4096)
}

#[tokio::test(start_paused = true)]
async fn a_client_that_sends_nothing_times_out() {
    let (_client, mut server) = silent_client().await;

    let outcome = handshake(&mut server).await;

    assert_eq!(outcome.err(), Some(HandshakeError::TimedOut));
}

#[tokio::test(start_paused = true)]
async fn a_client_that_stalls_after_the_greeting_times_out_within_the_same_deadline() {
    let (mut client, mut server) = silent_client().await;
    client.write_all(&[5, 1, 0, 5, 1]).await.unwrap();
    let started = tokio::time::Instant::now();

    let outcome = handshake(&mut server).await;

    assert_eq!(outcome.err(), Some(HandshakeError::TimedOut));
    assert!(started.elapsed() <= Duration::from_secs(11), "{:?}", started.elapsed());
}

#[tokio::test]
async fn replies_have_the_exact_wire_format() {
    let cases = [
        (Reply::Succeeded, 0u8),
        (Reply::GeneralFailure, 1),
        (Reply::ConnectionNotAllowed, 2),
        (Reply::ConnectionRefused, 5),
        (Reply::CommandNotSupported, 7),
        (Reply::AddressTypeNotSupported, 8),
    ];
    for (reply_code, byte) in cases {
        let (mut client, mut server) = tokio::io::duplex(64);
        reply(&mut server, reply_code).await;
        drop(server);
        let mut written = Vec::new();
        client.read_to_end(&mut written).await.unwrap();
        assert_eq!(written, [5, byte, 0, 1, 0, 0, 0, 0, 0, 0]);
    }
}
