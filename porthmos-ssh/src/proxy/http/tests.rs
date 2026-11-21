use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

use super::connect;

async fn run(host: &str, port: u16, reply: &'static [u8]) -> (Result<(), String>, String, tokio::io::DuplexStream) {
    let (mut client, mut server) = duplex(4096);
    let task = tokio::spawn(async move {
        let mut seen = Vec::new();
        let mut byte = [0u8; 1];
        while !seen.ends_with(b"\r\n\r\n") {
            server.read_exact(&mut byte).await.unwrap();
            seen.push(byte[0]);
        }
        server.write_all(reply).await.unwrap();
        (String::from_utf8(seen).unwrap(), server)
    });
    let result = connect(&mut client, host, port).await;
    let (request, _server) = task.await.unwrap();
    (result, request, client)
}

#[tokio::test]
async fn the_request_names_the_host_and_port() {
    let (result, request, _) = run("files.example", 22, b"HTTP/1.1 200 Connection established\r\n\r\n").await;

    assert_eq!(result, Ok(()));
    assert_eq!(request, "CONNECT files.example:22 HTTP/1.1\r\nHost: files.example:22\r\n\r\n");
}

#[tokio::test]
async fn an_ipv6_address_is_written_in_brackets() {
    let (result, request, _) = run("2001:db8::1", 2222, b"HTTP/1.0 200 OK\r\n\r\n").await;

    assert_eq!(result, Ok(()));
    assert!(request.starts_with("CONNECT [2001:db8::1]:2222 HTTP/1.1\r\n"), "{request}");
}

#[tokio::test]
async fn any_2xx_status_is_a_success() {
    for status in ["200", "201", "299"] {
        let reply: &'static [u8] = Box::leak(format!("HTTP/1.1 {status} Fine\r\n\r\n").into_bytes().into_boxed_slice());

        let (result, _, _) = run("h", 22, reply).await;

        assert_eq!(result, Ok(()), "{status}");
    }
}

#[tokio::test]
async fn headers_after_the_status_line_are_skipped() {
    let (result, _, _) = run("h", 22, b"HTTP/1.1 200 OK\r\nProxy-Agent: x\r\nVia: 1.1 gate\r\n\r\n").await;

    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn bytes_after_the_headers_stay_for_the_ssh_handshake() {
    let (result, _, mut client) = run("h", 22, b"HTTP/1.1 200 OK\r\n\r\nSSH-2.0-OpenSSH_9\r\n").await;

    assert_eq!(result, Ok(()));
    let mut banner = vec![0u8; 8];
    client.read_exact(&mut banner).await.unwrap();
    assert_eq!(banner, b"SSH-2.0-");
}

#[tokio::test]
async fn a_refusal_names_the_status_the_proxy_gave() {
    let (result, _, _) = run("h", 22, b"HTTP/1.1 403 Forbidden\r\n\r\n").await;

    let error = result.unwrap_err();
    assert!(error.contains("403") && error.contains("Forbidden"), "{error}");
}

#[tokio::test]
async fn a_proxy_that_wants_a_login_is_explained() {
    let (result, _, _) = run("h", 22, b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n").await;

    let error = result.unwrap_err();
    assert!(error.contains("407") && error.contains("login"), "{error}");
}

#[tokio::test]
async fn an_answer_that_is_not_http_is_refused() {
    let (result, _, _) = run("h", 22, b"SSH-2.0-OpenSSH_9\r\n\r\n").await;

    let error = result.unwrap_err();
    assert!(error.contains("not an HTTP proxy"), "{error}");
}

#[tokio::test]
async fn control_characters_in_the_status_text_are_not_passed_on() {
    let (result, _, _) = run("h", 22, b"HTTP/1.1 502 Bad\x1b[31m Gateway\r\n\r\n").await;

    let error = result.unwrap_err();
    assert!(!error.contains('\u{1b}'), "{error:?}");
}

#[tokio::test]
async fn a_host_with_a_line_break_is_never_sent() {
    let (mut client, mut server) = duplex(1024);

    let error = connect(&mut client, "evil\r\nHost: x", 22).await.unwrap_err();

    assert!(error.contains("host"), "{error}");
    drop(client);
    let mut sent = Vec::new();
    server.read_to_end(&mut sent).await.unwrap();
    assert!(sent.is_empty());
}

#[tokio::test]
async fn headers_that_never_end_are_cut_off() {
    let (mut client, mut server) = duplex(64 * 1024);
    let task = tokio::spawn(async move {
        let mut request = vec![0u8; 20];
        server.read_exact(&mut request).await.unwrap();
        let mut junk = b"HTTP/1.1 200 OK\r\n".to_vec();
        junk.extend(std::iter::repeat_n(b'a', 40 * 1024));
        let _ = server.write_all(&junk).await;
        server
    });

    let error = connect(&mut client, "h", 22).await.unwrap_err();
    let _server = task.await.unwrap();

    assert!(error.contains("too long"), "{error}");
}

#[tokio::test]
async fn a_proxy_that_hangs_up_is_reported() {
    let (mut client, server) = duplex(64);
    drop(server);

    let error = connect(&mut client, "h", 22).await.unwrap_err();

    assert!(!error.is_empty());
}
