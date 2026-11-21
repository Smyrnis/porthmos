use anyhow::anyhow;
use porthmos_vfs::{Answer, ErrorKind, Prompter, Question, Target};

use super::*;
use crate::client;

struct Recording {
    answer: Option<Answer>,
    asked: Vec<Question>,
}

#[async_trait::async_trait]
impl Prompter for Recording {
    async fn ask(&mut self, question: Question) -> Option<Answer> {
        self.asked.push(question);
        self.answer.clone()
    }
}

fn target() -> Target {
    Target {
        name: "web".into(),
        host: "example.com".into(),
        port: 2222,
        username: "u".into(),
        password: None,
        options: Default::default(),
    }
}

async fn ask_once_while_connecting(prompter: &mut Recording) -> bool {
    let (questions, mut asked) = tokio::sync::mpsc::channel(1);
    let connecting = async move {
        let (reply, answer) = tokio::sync::oneshot::channel();
        let question =
            client::HostKeyQuestion { key_type: "ssh-ed25519".into(), fingerprint: "SHA256:abc".into(), reply };
        questions.send(question).await.unwrap();
        answer.await.unwrap()
    };
    answer_host_key_questions(connecting, &mut asked, prompter, &target()).await
}

#[tokio::test]
async fn a_host_key_question_is_asked_through_the_prompter_while_connecting() {
    let mut prompter = Recording { answer: Some(Answer::Confirmed), asked: Vec::new() };

    assert!(ask_once_while_connecting(&mut prompter).await);
    assert_eq!(
        prompter.asked,
        [Question::TrustHostKey {
            name: "web".into(),
            host: "example.com".into(),
            port: 2222,
            key_type: "ssh-ed25519".into(),
            fingerprint: "SHA256:abc".into(),
        }]
    );
}

#[tokio::test]
async fn cancelling_the_host_key_question_declines_the_key() {
    let mut prompter = Recording { answer: None, asked: Vec::new() };

    assert!(!ask_once_while_connecting(&mut prompter).await);
}

#[test]
fn a_declined_host_key_is_reported_as_a_cancelled_connection() {
    let error = connect_error(anyhow::Error::new(client::HostKeyDeclined));

    assert_eq!(error.kind(), ErrorKind::Cancelled);
    assert_eq!(error.to_string(), "Connection cancelled");
}

#[test]
fn any_other_handshake_failure_is_a_connect_error() {
    assert_eq!(connect_error(anyhow!("connection refused")).kind(), ErrorKind::Connect);
}

#[tokio::test]
async fn a_session_reports_the_servers_version() {
    let server = crate::testing::SshServer::start(crate::testing::Options::default()).await;

    let session = server.session().await;

    let version = session.server_version().expect("the test server announces itself");
    assert!(version.starts_with("russh_"), "{version}");
}

fn gate_alias(port: u16) -> SshConfigHost {
    SshConfigHost {
        name: "gate".to_string(),
        host_name: Some("127.0.0.1".to_string()),
        user: Some(crate::testing::USER.to_string()),
        port: Some(port),
        identity_file: None,
        proxy_jump: None,
        proxy_command: None,
    }
}

#[tokio::test]
async fn a_jump_named_by_a_config_alias_uses_the_aliases_host_user_and_port() {
    let target_server = crate::testing::SshServer::start(crate::testing::Options::default()).await;
    let jump =
        crate::testing::SshServer::start(crate::testing::Options { forwarding: true, ..Default::default() }).await;
    let mut target = target_server.target();
    target.options.insert("proxy_jump".to_string(), "gate".to_string());
    let mut prompter = crate::testing::answers(vec![
        Some(Answer::Confirmed),
        Some(Answer::Password(crate::testing::PASSWORD.to_string())),
        Some(Answer::Confirmed),
    ]);

    connect_through(&target, &mut prompter, &target_server.connect_options(), &[gate_alias(jump.port)], "me")
        .await
        .expect("the alias points at the jump server");

    assert_eq!(jump.forwarded_connections(), 1);
    assert!(matches!(
        prompter.asked.first(),
        Some(Question::TrustHostKey { name, port, .. }) if name == "gate" && *port == jump.port
    ));
}

#[tokio::test]
async fn a_target_without_a_jump_ignores_the_config_aliases() {
    let target_server = crate::testing::SshServer::start(crate::testing::Options::default()).await;
    let mut prompter = crate::testing::answers(vec![Some(Answer::Confirmed)]);

    connect_through(&target_server.target(), &mut prompter, &target_server.connect_options(), &[gate_alias(1)], "me")
        .await
        .expect("a direct connection");

    assert_eq!(prompter.asked.len(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn a_jump_host_whose_config_alias_has_a_proxy_command_is_reached_through_it() {
    let target_server = crate::testing::SshServer::start(crate::testing::Options::default()).await;
    let jump =
        crate::testing::SshServer::start(crate::testing::Options { forwarding: true, ..Default::default() }).await;
    let mut gate = gate_alias(1);
    gate.proxy_command =
        Some(format!("bash -c 'exec 3<>/dev/tcp/127.0.0.1/{}; cat <&3 & cat >&3; kill $!'", jump.port));
    let mut target = target_server.target();
    target.options.insert("proxy_jump".to_string(), "gate".to_string());
    let mut prompter = crate::testing::answers(vec![
        Some(Answer::Confirmed),
        Some(Answer::Password(crate::testing::PASSWORD.to_string())),
        Some(Answer::Confirmed),
    ]);

    connect_through(&target, &mut prompter, &target_server.connect_options(), &[gate], "me")
        .await
        .expect("the jump host is reached through its proxy command, not directly");

    assert_eq!(jump.forwarded_connections(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn the_second_jump_host_is_not_given_the_first_ones_proxy_command() {
    let target_server = crate::testing::SshServer::start(crate::testing::Options::default()).await;
    let first =
        crate::testing::SshServer::start(crate::testing::Options { forwarding: true, ..Default::default() }).await;
    let second =
        crate::testing::SshServer::start(crate::testing::Options { forwarding: true, ..Default::default() }).await;
    let mut gate = gate_alias(1);
    gate.proxy_command =
        Some(format!("bash -c 'exec 3<>/dev/tcp/127.0.0.1/{}; cat <&3 & cat >&3; kill $!'", first.port));
    let mut inner = gate_alias(second.port);
    inner.name = "inner".to_string();
    let mut target = target_server.target();
    target.options.insert("proxy_jump".to_string(), format!("gate,{}@127.0.0.1:{}", crate::testing::USER, second.port));
    let mut prompter = crate::testing::answers(vec![
        Some(Answer::Confirmed),
        Some(Answer::Password(crate::testing::PASSWORD.to_string())),
        Some(Answer::Confirmed),
        Some(Answer::Password(crate::testing::PASSWORD.to_string())),
        Some(Answer::Confirmed),
    ]);

    connect_through(&target, &mut prompter, &target_server.connect_options(), &[gate, inner], "me")
        .await
        .expect("only the first hop uses the command");

    assert_eq!((first.forwarded_connections(), second.forwarded_connections()), (1, 1));
}
