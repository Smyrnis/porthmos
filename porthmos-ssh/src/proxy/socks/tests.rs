use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

use super::{socks4a, socks5};
use crate::proxy::Login;

async fn serve(mut server: tokio::io::DuplexStream, expect: Vec<u8>, reply: Vec<u8>) -> Vec<u8> {
    let mut seen = vec![0u8; expect.len()];
    server.read_exact(&mut seen).await.unwrap();
    server.write_all(&reply).await.unwrap();
    seen
}

fn request_v5(atyp: u8, address: &[u8], port: u16) -> Vec<u8> {
    let mut bytes = vec![5, 1, 0, atyp];
    bytes.extend_from_slice(address);
    bytes.extend_from_slice(&port.to_be_bytes());
    bytes
}

fn domain(name: &str) -> Vec<u8> {
    let mut bytes = vec![name.len() as u8];
    bytes.extend_from_slice(name.as_bytes());
    bytes
}

async fn run_v5(host: &str, port: u16, method_reply: [u8; 2], request: Vec<u8>, reply: Vec<u8>) -> Result<(), String> {
    let (mut client, mut server) = duplex(1024);
    let task = tokio::spawn(async move {
        let mut greeting = [0u8; 3];
        server.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [5, 1, 0]);
        server.write_all(&method_reply).await.unwrap();
        if method_reply[1] != 0 {
            return;
        }
        let mut seen = vec![0u8; request.len()];
        server.read_exact(&mut seen).await.unwrap();
        assert_eq!(seen, request);
        server.write_all(&reply).await.unwrap();
    });
    let result = socks5(&mut client, host, port, None).await;
    task.await.unwrap();
    result
}

fn ok_reply() -> Vec<u8> {
    vec![5, 0, 0, 1, 0, 0, 0, 0, 0, 0]
}

#[tokio::test]
async fn socks5_asks_for_a_host_name_by_name() {
    let request = request_v5(3, &domain("files.example"), 22);

    assert_eq!(run_v5("files.example", 22, [5, 0], request, ok_reply()).await, Ok(()));
}

#[tokio::test]
async fn socks5_sends_an_ipv4_address_as_four_bytes() {
    let request = request_v5(1, &[10, 1, 2, 3], 2222);

    assert_eq!(run_v5("10.1.2.3", 2222, [5, 0], request, ok_reply()).await, Ok(()));
}

#[tokio::test]
async fn socks5_sends_an_ipv6_address_as_sixteen_bytes() {
    let address = "2001:db8::1".parse::<std::net::Ipv6Addr>().unwrap().octets();
    let request = request_v5(4, &address, 22);

    assert_eq!(run_v5("2001:db8::1", 22, [5, 0], request, ok_reply()).await, Ok(()));
}

#[tokio::test]
async fn socks5_reads_a_whole_reply_with_a_host_name_so_nothing_after_it_is_lost() {
    let (mut client, mut server) = duplex(1024);
    let request = request_v5(3, &domain("h"), 22);
    let task = tokio::spawn(async move {
        let mut greeting = [0u8; 3];
        server.read_exact(&mut greeting).await.unwrap();
        server.write_all(&[5, 0]).await.unwrap();
        let mut seen = vec![0u8; request.len()];
        server.read_exact(&mut seen).await.unwrap();
        let mut reply = vec![5, 0, 0, 3, 4];
        reply.extend_from_slice(b"bind");
        reply.extend_from_slice(&[0, 80]);
        reply.extend_from_slice(b"SSH-2.0-after");
        server.write_all(&reply).await.unwrap();
    });

    socks5(&mut client, "h", 22, None).await.unwrap();
    task.await.unwrap();

    let mut rest = vec![0u8; 13];
    client.read_exact(&mut rest).await.unwrap();
    assert_eq!(rest, b"SSH-2.0-after");
}

#[tokio::test]
async fn socks5_tells_where_to_fill_in_a_login_when_the_proxy_wants_one_and_none_is_set() {
    let error = run_v5("h", 22, [5, 2], Vec::new(), Vec::new()).await.unwrap_err();

    assert!(error.contains("Proxy user") && error.contains("Proxy password"), "{error}");
}

#[tokio::test]
async fn socks5_reports_a_proxy_that_accepts_no_method() {
    let error = run_v5("h", 22, [5, 0xff], Vec::new(), Vec::new()).await.unwrap_err();

    assert!(error.contains("authentication") || error.contains("login"), "{error}");
}

#[tokio::test]
async fn socks5_explains_each_failure_code() {
    let cases = [
        (1, "general failure"),
        (2, "not allowed"),
        (3, "network"),
        (4, "host"),
        (5, "refused"),
        (6, "timed out"),
        (7, "command"),
        (8, "address type"),
        (99, "99"),
    ];
    for (code, word) in cases {
        let request = request_v5(3, &domain("h"), 22);
        let reply = vec![5, code, 0, 1, 0, 0, 0, 0, 0, 0];

        let error = run_v5("h", 22, [5, 0], request, reply).await.unwrap_err();

        assert!(error.contains(word), "{code}: {error}");
    }
}

#[tokio::test]
async fn socks5_rejects_a_reply_from_the_wrong_protocol_version() {
    let request = request_v5(3, &domain("h"), 22);
    let reply = vec![4, 0, 0, 1, 0, 0, 0, 0, 0, 0];

    let error = run_v5("h", 22, [5, 0], request, reply).await.unwrap_err();

    assert!(error.contains("not a SOCKS5"), "{error}");
}

#[tokio::test]
async fn socks5_reports_a_proxy_that_hangs_up() {
    let (mut client, server) = duplex(64);
    drop(server);

    let error = socks5(&mut client, "h", 22, None).await.unwrap_err();

    assert!(!error.is_empty());
}

#[tokio::test]
async fn socks5_refuses_a_host_name_too_long_to_send_without_writing_anything() {
    let (mut client, mut server) = duplex(1024);
    let long = "a".repeat(256);

    let error = socks5(&mut client, &long, 22, None).await.unwrap_err();

    assert!(error.contains("too long"), "{error}");
    drop(client);
    let mut sent = Vec::new();
    server.read_to_end(&mut sent).await.unwrap();
    assert!(sent.is_empty());
}

#[tokio::test]
async fn socks4a_sends_the_host_name_after_a_placeholder_address() {
    let (mut client, server) = duplex(1024);
    let mut expect = vec![4, 1, 0, 22, 0, 0, 0, 1, 0];
    expect.extend_from_slice(b"files.example");
    expect.push(0);
    let task = tokio::spawn(serve(server, expect.clone(), vec![0, 90, 0, 0, 0, 0, 0, 0]));

    assert_eq!(socks4a(&mut client, "files.example", 22, None).await, Ok(()));
    assert_eq!(task.await.unwrap(), expect);
}

#[tokio::test]
async fn socks4a_sends_an_ipv4_address_directly() {
    let (mut client, server) = duplex(1024);
    let expect = vec![4, 1, 0x08, 0xae, 10, 1, 2, 3, 0];
    let task = tokio::spawn(serve(server, expect.clone(), vec![0, 90, 0, 0, 0, 0, 0, 0]));

    assert_eq!(socks4a(&mut client, "10.1.2.3", 2222, None).await, Ok(()));
    assert_eq!(task.await.unwrap(), expect);
}

#[tokio::test]
async fn socks4a_cannot_carry_an_ipv6_address() {
    let (mut client, _server) = duplex(1024);

    let error = socks4a(&mut client, "::1", 22, None).await.unwrap_err();

    assert!(error.contains("IPv6"), "{error}");
}

#[tokio::test]
async fn socks4a_reports_a_rejected_request() {
    for code in [91, 92, 93] {
        let (mut client, server) = duplex(1024);
        let mut expect = vec![4, 1, 0, 22, 0, 0, 0, 1, 0];
        expect.extend_from_slice(b"h");
        expect.push(0);
        let task = tokio::spawn(serve(server, expect, vec![0, code, 0, 0, 0, 0, 0, 0]));

        let error = socks4a(&mut client, "h", 22, None).await.unwrap_err();
        task.await.unwrap();

        assert!(error.contains("rejected") || error.contains("refused") || error.contains("identd"), "{code}: {error}");
    }
}

#[tokio::test]
async fn socks4a_rejects_a_reply_that_is_not_socks4() {
    let (mut client, server) = duplex(1024);
    let mut expect = vec![4, 1, 0, 22, 0, 0, 0, 1, 0];
    expect.extend_from_slice(b"h");
    expect.push(0);
    let task = tokio::spawn(serve(server, expect, vec![5, 90, 0, 0, 0, 0, 0, 0]));

    let error = socks4a(&mut client, "h", 22, None).await.unwrap_err();
    task.await.unwrap();

    assert!(error.contains("not a SOCKS4"), "{error}");
}

fn login(user: &str, password: Option<&str>) -> Login {
    Login { user: user.to_string(), password: password.map(str::to_string) }
}

async fn run_v5_with_login(login: Login, method_reply: [u8; 2], auth_reply: [u8; 2]) -> (Result<(), String>, Vec<u8>) {
    let (mut client, mut server) = duplex(1024);
    let task = tokio::spawn(async move {
        let mut greeting = [0u8; 4];
        server.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [5, 2, 0, 2]);
        server.write_all(&method_reply).await.unwrap();
        if method_reply[1] != 2 {
            return Vec::new();
        }
        let mut head = [0u8; 2];
        server.read_exact(&mut head).await.unwrap();
        let mut user = vec![0u8; usize::from(head[1])];
        server.read_exact(&mut user).await.unwrap();
        let mut length = [0u8; 1];
        server.read_exact(&mut length).await.unwrap();
        let mut password = vec![0u8; usize::from(length[0])];
        server.read_exact(&mut password).await.unwrap();
        server.write_all(&auth_reply).await.unwrap();
        if auth_reply[1] != 0 {
            return [head.to_vec(), user, length.to_vec(), password].concat();
        }
        let mut request = vec![0u8; 4 + 1 + 1 + 2];
        server.read_exact(&mut request).await.unwrap();
        server.write_all(&ok_reply()).await.unwrap();
        [head.to_vec(), user, length.to_vec(), password].concat()
    });
    let result = socks5(&mut client, "h", 22, Some(&login)).await;
    (result, task.await.unwrap())
}

#[tokio::test]
async fn socks5_offers_the_login_method_and_sends_the_user_and_password() {
    let (result, sent) = run_v5_with_login(login("alice", Some("secret")), [5, 2], [1, 0]).await;

    assert_eq!(result, Ok(()));
    assert_eq!(sent, [&[1u8, 5][..], b"alice", &[6], b"secret"].concat());
}

#[tokio::test]
async fn socks5_sends_an_empty_password_when_only_a_user_is_set() {
    let (result, sent) = run_v5_with_login(login("alice", None), [5, 2], [1, 0]).await;

    assert_eq!(result, Ok(()));
    assert_eq!(sent, [&[1u8, 5][..], b"alice", &[0]].concat());
}

#[tokio::test]
async fn socks5_says_the_login_was_refused_and_never_repeats_the_password() {
    let (result, _) = run_v5_with_login(login("alice", Some("hunter2")), [5, 2], [1, 1]).await;

    let error = result.unwrap_err();
    assert!(error.contains("refused the login"), "{error}");
    assert!(!error.contains("hunter2") && !error.contains("alice"), "{error}");
}

#[tokio::test]
async fn socks5_goes_on_without_a_login_when_the_proxy_picks_no_authentication() {
    let (client, mut server) = duplex(1024);
    let mut client = client;
    let task = tokio::spawn(async move {
        let mut greeting = [0u8; 4];
        server.read_exact(&mut greeting).await.unwrap();
        server.write_all(&[5, 0]).await.unwrap();
        let mut request = vec![0u8; 8];
        server.read_exact(&mut request).await.unwrap();
        server.write_all(&ok_reply()).await.unwrap();
    });

    let result = socks5(&mut client, "h", 22, Some(&login("alice", Some("secret")))).await;

    task.await.unwrap();
    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn socks5_refuses_a_user_or_password_longer_than_the_protocol_allows_without_writing() {
    let (mut client, mut server) = duplex(1024);

    let long_user = socks5(&mut client, "h", 22, Some(&login(&"u".repeat(256), Some("p")))).await.unwrap_err();
    let long_password = socks5(&mut client, "h", 22, Some(&login("u", Some(&"p".repeat(256))))).await.unwrap_err();

    assert!(long_user.contains("too long"), "{long_user}");
    assert!(long_password.contains("too long"), "{long_password}");
    drop(client);
    let mut written = Vec::new();
    server.read_to_end(&mut written).await.unwrap();
    assert!(written.is_empty());
}

#[tokio::test]
async fn socks4a_sends_the_user_as_the_user_id() {
    let (mut client, server) = duplex(1024);
    let mut expect = vec![4, 1, 0, 22, 10, 1, 2, 3];
    expect.extend_from_slice(b"alice");
    expect.push(0);
    let task = tokio::spawn(serve(server, expect.clone(), vec![0, 90, 0, 0, 0, 0, 0, 0]));

    let result = socks4a(&mut client, "10.1.2.3", 22, Some(&login("alice", None))).await;

    assert_eq!(result, Ok(()));
    assert_eq!(task.await.unwrap(), expect);
}

#[tokio::test]
async fn socks4a_refuses_a_user_with_a_nul_byte_or_one_that_is_too_long_and_sends_nothing() {
    for user in ["al\0ice".to_string(), "a".repeat(256)] {
        let (mut client, mut server) = duplex(1024);

        let error = socks4a(&mut client, "10.1.2.3", 22, Some(&login(&user, None))).await.unwrap_err();

        assert!(error.contains("user"), "{error}");
        drop(client);
        let mut written = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut server, &mut written).await.unwrap();
        assert!(written.is_empty());
    }
}
