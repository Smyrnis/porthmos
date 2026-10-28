use std::ffi::{OsStr, OsString};

use russh::keys::PublicKey;
use tokio::sync::mpsc;

use super::*;

const SERVER_KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA1Wxd2wRqoDKvqvCwWtQYeIkcRgUv03zkN5wnGjAEL9";
const OTHER_KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPn1WisTlluVa9LtieLqa3379O+Ljxx+4mH1KiBjXbMx";
const SERVER_FINGERPRINT: &str = "SHA256:Sh/SULaLCH5FHkfQXVuZsA5t2rQ9B719Sf/t8m2+QkI";

fn key(openssh: &str) -> PublicKey {
    PublicKey::from_openssh(openssh).unwrap()
}

fn known_hosts_with(lines: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".ssh/known_hosts");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, lines).unwrap();
    (dir, path)
}

#[test]
fn a_recorded_key_is_known() {
    let (_dir, path) = known_hosts_with(&format!("example.com {SERVER_KEY}\n"));

    assert_eq!(host_key_status(&path, "example.com", 22, &key(SERVER_KEY)).unwrap(), HostKeyStatus::Known);
}

#[test]
fn a_host_missing_from_known_hosts_is_unknown() {
    let (_dir, path) = known_hosts_with(&format!("other.example.com {SERVER_KEY}\n"));

    assert_eq!(host_key_status(&path, "example.com", 22, &key(SERVER_KEY)).unwrap(), HostKeyStatus::Unknown);
}

#[test]
fn a_missing_known_hosts_file_means_unknown() {
    let dir = tempfile::tempdir().unwrap();

    let status = host_key_status(&dir.path().join(".ssh/known_hosts"), "example.com", 22, &key(SERVER_KEY)).unwrap();

    assert_eq!(status, HostKeyStatus::Unknown);
}

#[test]
fn a_different_key_for_a_recorded_host_is_changed() {
    let (_dir, path) = known_hosts_with(&format!("# comment\nexample.com {OTHER_KEY}\n"));

    assert_eq!(
        host_key_status(&path, "example.com", 22, &key(SERVER_KEY)).unwrap(),
        HostKeyStatus::Changed { line: 2 }
    );
}

#[test]
fn the_changed_line_counts_comments_and_blank_lines() {
    let (_dir, path) = known_hosts_with(&format!("# a\n\n# b\nexample.com {OTHER_KEY}\n"));

    assert_eq!(
        host_key_status(&path, "example.com", 22, &key(SERVER_KEY)).unwrap(),
        HostKeyStatus::Changed { line: 4 }
    );
}

#[test]
fn trusting_a_key_makes_it_known_on_a_non_default_port() {
    let (_dir, path) = known_hosts_with("");

    trust_host_key(&path, "example.com", 2222, &key(SERVER_KEY)).unwrap();

    assert_eq!(host_key_status(&path, "example.com", 2222, &key(SERVER_KEY)).unwrap(), HostKeyStatus::Known);
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.lines().any(|line| line.starts_with("[example.com]:2222 ssh-ed25519 ")), "{contents}");
}

#[test]
fn trusting_a_key_creates_the_ssh_directory_and_the_known_hosts_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".ssh/known_hosts");

    trust_host_key(&path, "example.com", 22, &key(SERVER_KEY)).unwrap();

    assert!(dir.path().join(".ssh").is_dir());
    assert_eq!(host_key_status(&path, "example.com", 22, &key(SERVER_KEY)).unwrap(), HostKeyStatus::Known);
}

#[cfg(unix)]
#[test]
fn trusting_a_key_creates_a_private_ssh_directory_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".ssh/known_hosts");

    trust_host_key(&path, "example.com", 22, &key(SERVER_KEY)).unwrap();

    let mode = std::fs::metadata(dir.path().join(".ssh")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

fn check_for(known_hosts: Option<PathBuf>) -> (HostKeyCheck, mpsc::Receiver<HostKeyQuestion>) {
    let (questions, asked) = mpsc::channel(1);
    (HostKeyCheck::new("example.com", 22, known_hosts, questions), asked)
}

fn answer_with(mut asked: mpsc::Receiver<HostKeyQuestion>, trusted: bool) -> tokio::task::JoinHandle<(String, String)> {
    tokio::spawn(async move {
        let question = asked.recv().await.unwrap();
        question.reply.send(trusted).unwrap();
        (question.key_type, question.fingerprint)
    })
}

#[tokio::test]
async fn a_known_key_is_accepted_without_asking() {
    let (_dir, path) = known_hosts_with(&format!("example.com {SERVER_KEY}\n"));
    let (check, mut asked) = check_for(Some(path));

    assert!(check.verify(&key(SERVER_KEY)).await.unwrap());
    assert!(asked.try_recv().is_err());
}

#[tokio::test]
async fn a_changed_key_is_refused_without_asking() {
    let (_dir, path) = known_hosts_with(&format!("example.com {OTHER_KEY}\n"));
    let (check, mut asked) = check_for(Some(path));

    let error = check.verify(&key(SERVER_KEY)).await.unwrap_err();

    assert!(error.to_string().contains("REMOTE HOST IDENTIFICATION HAS CHANGED"), "{error}");
    assert!(asked.try_recv().is_err());
}

#[tokio::test]
async fn an_unknown_key_asks_with_its_type_and_fingerprint_and_saves_it_when_trusted() {
    let (_dir, path) = known_hosts_with("");
    let (check, asked) = check_for(Some(path.clone()));
    let answering = answer_with(asked, true);

    assert!(check.verify(&key(SERVER_KEY)).await.unwrap());

    let (key_type, fingerprint) = answering.await.unwrap();
    assert_eq!((key_type.as_str(), fingerprint.as_str()), ("ssh-ed25519", SERVER_FINGERPRINT));
    assert_eq!(host_key_status(&path, "example.com", 22, &key(SERVER_KEY)).unwrap(), HostKeyStatus::Known);
}

#[tokio::test]
async fn declining_an_unknown_key_fails_with_host_key_declined_and_saves_nothing() {
    let (_dir, path) = known_hosts_with("");
    let (check, asked) = check_for(Some(path.clone()));
    answer_with(asked, false);

    let error = check.verify(&key(SERVER_KEY)).await.unwrap_err();

    assert!(error.downcast_ref::<HostKeyDeclined>().is_some(), "{error}");
    assert_eq!(host_key_status(&path, "example.com", 22, &key(SERVER_KEY)).unwrap(), HostKeyStatus::Unknown);
}

#[tokio::test]
async fn an_unknown_key_with_nobody_to_ask_is_declined() {
    let (_dir, path) = known_hosts_with("");
    let (check, asked) = check_for(Some(path));
    drop(asked);

    let error = check.verify(&key(SERVER_KEY)).await.unwrap_err();

    assert!(error.downcast_ref::<HostKeyDeclined>().is_some(), "{error}");
}

#[tokio::test]
async fn without_a_known_hosts_location_the_key_is_refused() {
    let (check, mut asked) = check_for(None);

    let error = check.verify(&key(SERVER_KEY)).await.unwrap_err();

    assert!(error.to_string().contains("known_hosts"), "{error}");
    assert!(asked.try_recv().is_err());
}

#[test]
fn the_version_loses_the_protocol_prefix() {
    assert_eq!(
        software_version(b"SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13.4\r\n").as_deref(),
        Some("OpenSSH_9.6p1 Ubuntu-3ubuntu13.4")
    );
    assert_eq!(software_version(b"SSH-1.99-Dropbear_2022.83\n").as_deref(), Some("Dropbear_2022.83"));
}

#[test]
fn a_line_that_is_not_an_identification_gives_none() {
    for line in [&b"HTTP/1.1 400 Bad Request\r\n"[..], b"", b"SSH-2.0-\r\n", b"SSH-\r\n", b"SSH-2.0\r\n"] {
        assert_eq!(software_version(line), None, "{line:?}");
    }
}

#[test]
fn an_unfinished_line_gives_none() {
    assert_eq!(software_version(b"SSH-2.0-OpenSS"), None);
}

#[test]
fn control_characters_and_long_text_in_the_version_are_cleaned_and_cut() {
    let cleaned = software_version(b"SSH-2.0-a\x1b[2Jb\x07c\r\n").unwrap();
    assert_eq!(cleaned, "a [2Jb c");

    let long = format!("SSH-2.0-{}\r\n", "x".repeat(200));
    let cut = software_version(long.as_bytes()).unwrap();
    assert!(cut.chars().count() <= 40 && cut.ends_with('\u{2026}'), "{cut}");
}

#[tokio::test]
async fn the_sniffer_records_the_first_line_only() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (client, mut server) = tokio::io::duplex(1024);
    let seen = ServerId::default();
    let mut sniffing = Sniffing::new(client, seen.clone());
    server.write_all(b"SSH-2.0-test_1.0\r\nbinary\x00data").await.unwrap();

    let mut buffer = [0u8; 64];
    let mut total = 0;
    while total < 29 {
        total += sniffing.read(&mut buffer[total..]).await.unwrap();
    }

    assert_eq!(seen.version().as_deref(), Some("test_1.0"));
    server.write_all(b"SSH-2.0-other\r\n").await.unwrap();
    let _ = sniffing.read(&mut buffer).await.unwrap();
    assert_eq!(seen.version().as_deref(), Some("test_1.0"));
}

#[tokio::test]
async fn the_sniffer_passes_writes_through_unchanged() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (client, mut server) = tokio::io::duplex(1024);
    let mut sniffing = Sniffing::new(client, ServerId::default());

    sniffing.write_all(b"hello").await.unwrap();
    sniffing.flush().await.unwrap();

    let mut received = [0u8; 5];
    server.read_exact(&mut received).await.unwrap();
    assert_eq!(&received, b"hello");
}

#[tokio::test]
async fn the_sniffer_never_stores_more_than_a_bounded_amount_without_a_newline() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (client, mut server) = tokio::io::duplex(8192);
    let seen = ServerId::default();
    let mut sniffing = Sniffing::new(client, seen.clone());
    server.write_all(&vec![b'a'; 4000]).await.unwrap();

    let mut buffer = vec![0u8; 4000];
    let mut total = 0;
    while total < 4000 {
        total += sniffing.read(&mut buffer[total..]).await.unwrap();
    }

    assert_eq!(seen.version(), None);
    assert!(seen.len() <= 512);
}

#[test]
fn the_client_pings_a_silent_server_and_gives_up_after_a_few_misses() {
    let config = client_config();

    assert_eq!(config.keepalive_interval, Some(KEEPALIVE_INTERVAL));
    assert_eq!(config.keepalive_max, KEEPALIVE_MAX);
}

#[test]
fn a_dead_connection_is_noticed_within_a_minute() {
    assert!(KEEPALIVE_INTERVAL * (KEEPALIVE_MAX as u32 + 1) <= std::time::Duration::from_secs(60));
}

#[test]
fn without_an_auth_socket_the_agent_is_the_default_openssh_pipe() {
    assert_eq!(agent_pipe_path(None), OsString::from(r"\\.\pipe\openssh-ssh-agent"));
}

#[test]
fn an_auth_socket_that_is_a_pipe_is_used_as_it_is() {
    let custom = OsString::from(r"\\.\pipe\custom-agent");

    assert_eq!(agent_pipe_path(Some(custom.as_os_str())), custom);
}

#[test]
fn an_auth_socket_that_is_not_a_pipe_falls_back_to_the_default_pipe() {
    assert_eq!(agent_pipe_path(Some(OsStr::new("/tmp/ssh-agent.sock"))), OsString::from(r"\\.\pipe\openssh-ssh-agent"));
}

#[test]
fn an_empty_auth_socket_falls_back_to_the_default_pipe() {
    assert_eq!(agent_pipe_path(Some(OsStr::new(""))), OsString::from(r"\\.\pipe\openssh-ssh-agent"));
}
