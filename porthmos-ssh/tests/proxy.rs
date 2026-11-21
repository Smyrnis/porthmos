use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use porthmos_ssh::{
    connect, exec,
    testing::{self, Options, SshServer},
};
use porthmos_vfs::{Answer, ErrorKind, Target};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Clone, Copy)]
enum Dialect {
    Socks5,
    Socks4,
    Http,
    Refusing,
}

struct FakeProxy {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FakeProxy {
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

async fn read_until_nul(stream: &mut TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let byte = stream.read_u8().await.unwrap();
        if byte == 0 {
            return bytes;
        }
        bytes.push(byte);
    }
}

async fn serve_socks5(client: &mut TcpStream) -> (String, u16) {
    let mut head = [0u8; 2];
    client.read_exact(&mut head).await.unwrap();
    let mut methods = vec![0u8; usize::from(head[1])];
    client.read_exact(&mut methods).await.unwrap();
    client.write_all(&[5, 0]).await.unwrap();
    let mut request = [0u8; 4];
    client.read_exact(&mut request).await.unwrap();
    let host = match request[3] {
        1 => {
            let mut octets = [0u8; 4];
            client.read_exact(&mut octets).await.unwrap();
            std::net::Ipv4Addr::from(octets).to_string()
        }
        3 => {
            let length = client.read_u8().await.unwrap();
            let mut name = vec![0u8; usize::from(length)];
            client.read_exact(&mut name).await.unwrap();
            String::from_utf8(name).unwrap()
        }
        other => panic!("address type {other}"),
    };
    let port = client.read_u16().await.unwrap();
    client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await.unwrap();
    (host, port)
}

async fn serve_socks4(client: &mut TcpStream) -> (String, u16) {
    let mut head = [0u8; 8];
    client.read_exact(&mut head).await.unwrap();
    let port = u16::from_be_bytes([head[2], head[3]]);
    let _user = read_until_nul(client).await;
    let host = if head[4..7] == [0, 0, 0] && head[7] != 0 {
        String::from_utf8(read_until_nul(client).await).unwrap()
    } else {
        std::net::Ipv4Addr::new(head[4], head[5], head[6], head[7]).to_string()
    };
    client.write_all(&[0, 90, 0, 0, 0, 0, 0, 0]).await.unwrap();
    (host, port)
}

async fn serve_http(client: &mut TcpStream) -> (String, u16) {
    let mut seen = Vec::new();
    while !seen.ends_with(b"\r\n\r\n") {
        seen.push(client.read_u8().await.unwrap());
    }
    let text = String::from_utf8(seen).unwrap();
    let authority = text.split_whitespace().nth(1).unwrap().to_string();
    client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.unwrap();
    let (host, port) = authority.rsplit_once(':').unwrap();
    (host.to_string(), port.parse().unwrap())
}

async fn start_proxy(dialect: Dialect) -> FakeProxy {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut client, _)) = listener.accept().await else {
                return;
            };
            let seen = seen.clone();
            tokio::spawn(async move {
                let (host, requested_port) = match dialect {
                    Dialect::Socks5 => serve_socks5(&mut client).await,
                    Dialect::Socks4 => serve_socks4(&mut client).await,
                    Dialect::Http => serve_http(&mut client).await,
                    Dialect::Refusing => {
                        let mut ignored = [0u8; 3];
                        let _ = client.read_exact(&mut ignored).await;
                        let _ = client.write_all(&[5, 0]).await;
                        let mut request = [0u8; 32];
                        let _ = client.read(&mut request).await;
                        let _ = client.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).await;
                        return;
                    }
                };
                seen.lock().unwrap().push(format!("{host}:{requested_port}"));
                let mut upstream = TcpStream::connect(("127.0.0.1", requested_port)).await.unwrap();
                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            });
        }
    });
    FakeProxy { port, requests }
}

fn behind(server: &SshServer, name: &str, options: &[(&str, String)]) -> Target {
    Target {
        host: name.to_string(),
        options: options.iter().map(|(key, value)| (key.to_string(), value.clone())).collect::<BTreeMap<_, _>>(),
        ..server.target()
    }
}

async fn echo_through(session: &porthmos_ssh::Session) -> String {
    let output = exec(session, "echo proxied").await.expect("a command runs on the target");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[tokio::test]
async fn a_socks5_proxy_carries_the_session_and_is_asked_for_the_host_by_name() {
    let server = SshServer::start(Options::default()).await;
    let proxy = start_proxy(Dialect::Socks5).await;
    let target = behind(&server, "files.invalid", &[("proxy", format!("socks5://127.0.0.1:{}", proxy.port))]);
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed)]);

    let session = connect(&target, &mut prompter, &server.connect_options()).await.expect("the proxy forwards");

    assert_eq!(echo_through(&session).await, "proxied");
    assert_eq!(proxy.requests(), [format!("files.invalid:{}", server.port)]);
}

#[tokio::test]
async fn a_socks4a_proxy_carries_the_session() {
    let server = SshServer::start(Options::default()).await;
    let proxy = start_proxy(Dialect::Socks4).await;
    let target = behind(&server, "files.invalid", &[("proxy", format!("socks4://127.0.0.1:{}", proxy.port))]);
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed)]);

    let session = connect(&target, &mut prompter, &server.connect_options()).await.expect("the proxy forwards");

    assert_eq!(echo_through(&session).await, "proxied");
    assert_eq!(proxy.requests(), [format!("files.invalid:{}", server.port)]);
}

#[tokio::test]
async fn an_http_proxy_carries_the_session_and_nothing_is_lost_after_the_headers() {
    let server = SshServer::start(Options::default()).await;
    let proxy = start_proxy(Dialect::Http).await;
    let target = behind(&server, "files.invalid", &[("proxy", format!("http://127.0.0.1:{}", proxy.port))]);
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed)]);

    let session = connect(&target, &mut prompter, &server.connect_options()).await.expect("the proxy forwards");

    assert_eq!(echo_through(&session).await, "proxied");
    assert_eq!(proxy.requests(), [format!("files.invalid:{}", server.port)]);
}

#[tokio::test]
async fn a_proxy_that_refuses_is_a_connect_error_that_names_the_proxy() {
    let server = SshServer::start(Options::default()).await;
    let proxy = start_proxy(Dialect::Refusing).await;
    let target = behind(&server, "files.invalid", &[("proxy", format!("socks5://127.0.0.1:{}", proxy.port))]);

    let error = connect(&target, &mut testing::answers(vec![]), &server.connect_options())
        .await
        .err()
        .expect("the proxy refuses");

    assert_eq!(error.kind(), ErrorKind::Connect);
    let text = error.to_string();
    assert!(text.contains("proxy") && text.contains("not allowed"), "{text}");
}

#[tokio::test]
async fn a_proxy_nobody_listens_on_says_so() {
    let server = SshServer::start(Options::default()).await;
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap().port()
    };
    let target = behind(&server, "files.invalid", &[("proxy", format!("socks5://127.0.0.1:{closed}"))]);

    let error = connect(&target, &mut testing::answers(vec![]), &server.connect_options())
        .await
        .err()
        .expect("nothing listens");

    assert_eq!(error.kind(), ErrorKind::Connect);
    assert!(error.to_string().contains("could not reach the proxy"), "{error}");
}

#[tokio::test]
async fn declining_the_host_key_behind_a_proxy_cancels() {
    let server = SshServer::start(Options::default()).await;
    let proxy = start_proxy(Dialect::Socks5).await;
    let target = behind(&server, "files.invalid", &[("proxy", format!("socks5://127.0.0.1:{}", proxy.port))]);

    let error = connect(&target, &mut testing::answers(vec![None]), &server.connect_options())
        .await
        .err()
        .expect("the key was declined");

    assert_eq!(error.kind(), ErrorKind::Cancelled);
}

#[tokio::test]
async fn a_proxy_together_with_a_jump_host_is_refused_before_anything_connects() {
    let target_server = SshServer::start(Options::default()).await;
    let jump = SshServer::start(Options { forwarding: true, ..Options::default() }).await;
    let proxy = start_proxy(Dialect::Socks5).await;
    let target = Target {
        options: BTreeMap::from([
            ("proxy".to_string(), format!("socks5://127.0.0.1:{}", proxy.port)),
            ("proxy_jump".to_string(), format!("{}@jump.invalid:{}", testing::USER, jump.port)),
        ]),
        ..target_server.target()
    };

    let error = connect(&target, &mut testing::answers(vec![]), &target_server.connect_options())
        .await
        .err()
        .expect("the combination is refused");

    assert_eq!(error.kind(), ErrorKind::Connect);
    assert!(error.to_string().contains("jump host"), "{error}");
    assert!(proxy.requests().is_empty());
    assert_eq!(jump.forwarded_connections(), 0);
}

#[tokio::test]
async fn a_proxy_address_with_a_login_is_refused_before_anything_connects() {
    let server = SshServer::start(Options::default()).await;
    let target = behind(&server, "files.invalid", &[("proxy", "socks5://alice:secret@127.0.0.1:1080".to_string())]);

    let error = connect(&target, &mut testing::answers(vec![]), &server.connect_options())
        .await
        .err()
        .expect("a login is not supported");

    assert_eq!(error.kind(), ErrorKind::Connect);
    let text = error.to_string();
    assert!(text.contains("login") && !text.contains("secret"), "{text}");
}

#[tokio::test]
async fn setting_both_a_proxy_and_a_proxy_command_is_refused() {
    let server = SshServer::start(Options::default()).await;
    let target = behind(
        &server,
        "files.invalid",
        &[("proxy", "socks5://127.0.0.1:1080".to_string()), ("proxy_command", "nc %h %p".to_string())],
    );

    let error =
        connect(&target, &mut testing::answers(vec![]), &server.connect_options()).await.err().expect("both are set");

    assert_eq!(error.kind(), ErrorKind::Connect);
    assert!(error.to_string().contains("not both"), "{error}");
}

#[tokio::test]
async fn none_or_blank_proxy_settings_connect_directly() {
    for value in ["none", "", "  "] {
        let server = SshServer::start(Options::default()).await;
        let target = behind(&server, "127.0.0.1", &[("proxy", value.to_string())]);

        connect(&target, &mut testing::answers(vec![Some(Answer::Confirmed)]), &server.connect_options())
            .await
            .unwrap_or_else(|error| panic!("{value:?}: {error}"));
    }
}

#[cfg(unix)]
mod command {
    use super::*;

    fn bash_exists() -> bool {
        std::process::Command::new("bash").arg("-c").arg("true").status().is_ok_and(|status| status.success())
    }

    fn relay_to(port: u16) -> String {
        format!("bash -c 'exec 3<>/dev/tcp/127.0.0.1/{port}; cat <&3 & cat >&3; kill $!'")
    }

    #[tokio::test]
    async fn a_proxy_command_carries_the_session() {
        if !bash_exists() {
            return;
        }
        let server = SshServer::start(Options::default()).await;
        let target = Target { port: 1, ..behind(&server, "127.0.0.1", &[("proxy_command", relay_to(server.port))]) };
        let mut prompter = testing::answers(vec![Some(Answer::Confirmed)]);

        let session = connect(&target, &mut prompter, &server.connect_options()).await.expect("the command relays");

        assert_eq!(echo_through(&session).await, "proxied");
    }

    #[tokio::test]
    async fn the_host_and_port_tokens_are_filled_in_before_the_command_runs() {
        if !bash_exists() {
            return;
        }
        let server = SshServer::start(Options::default()).await;
        let seen = tempfile::tempdir().unwrap();
        let record = seen.path().join("args");
        let command = format!(
            "echo '%h %p %r' > '{}'; bash -c 'exec 3<>/dev/tcp/%h/%p; cat <&3 & cat >&3; kill $!'",
            record.display()
        );
        let target = behind(&server, "127.0.0.1", &[("proxy_command", command)]);
        let mut prompter = testing::answers(vec![Some(Answer::Confirmed)]);

        connect(&target, &mut prompter, &server.connect_options()).await.expect("the command relays");

        let line = std::fs::read_to_string(&record).unwrap();
        assert_eq!(line.trim(), format!("127.0.0.1 {} {}", server.port, testing::USER));
    }

    #[tokio::test]
    async fn what_a_failing_command_prints_is_part_of_the_error() {
        let server = SshServer::start(Options::default()).await;
        let target = behind(&server, "127.0.0.1", &[("proxy_command", "echo 'gate is down' >&2; exit 1".to_string())]);

        let error = connect(&target, &mut testing::answers(vec![]), &server.connect_options())
            .await
            .err()
            .expect("the command fails");

        assert_eq!(error.kind(), ErrorKind::Connect);
        assert!(error.to_string().contains("gate is down"), "{error}");
    }

    #[tokio::test]
    async fn a_host_the_shell_would_read_as_code_is_never_put_into_the_command() {
        let server = SshServer::start(Options::default()).await;
        let witness = tempfile::tempdir().unwrap();
        let planted = witness.path().join("planted");
        let target =
            behind(&server, &format!("a;touch {}", planted.display()), &[("proxy_command", "nc %h %p".to_string())]);

        let error = connect(&target, &mut testing::answers(vec![]), &server.connect_options())
            .await
            .err()
            .expect("the host is refused");

        assert_eq!(error.kind(), ErrorKind::Connect);
        assert!(error.to_string().contains("host"), "{error}");
        assert!(!planted.exists());
    }
}
