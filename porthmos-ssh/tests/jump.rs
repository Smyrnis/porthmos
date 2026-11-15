use std::collections::BTreeMap;

use porthmos_ssh::{
    connect, exec,
    testing::{self, Options, SshServer},
};
use porthmos_vfs::{Answer, ErrorKind, Question, Target};

fn jump_server() -> Options {
    Options { forwarding: true, ..Options::default() }
}

fn target_through(server: &SshServer, jump: &str) -> Target {
    Target { options: BTreeMap::from([("proxy_jump".to_string(), jump.to_string())]), ..server.target() }
}

fn jump_to(server: &SshServer) -> String {
    format!("{}@127.0.0.1:{}", testing::USER, server.port)
}

fn password() -> Option<Answer> {
    Some(Answer::Password(testing::PASSWORD.to_string()))
}

#[tokio::test]
async fn a_session_is_opened_through_a_jump_host() {
    let target = SshServer::start(Options::default()).await;
    let jump = SshServer::start(jump_server()).await;
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed), password(), Some(Answer::Confirmed)]);

    let session = connect(&target_through(&target, &jump_to(&jump)), &mut prompter, &target.connect_options())
        .await
        .expect("the jump host forwards to the target");

    let output = exec(&session, "echo through").await.expect("a command runs on the target");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "through");
    assert_eq!(jump.forwarded_connections(), 1);
    assert_eq!(target.started_commands(), 1);
}

#[tokio::test]
async fn the_jump_host_is_asked_about_first_and_the_target_last() {
    let target = SshServer::start(Options::default()).await;
    let jump = SshServer::start(jump_server()).await;
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed), password(), Some(Answer::Confirmed)]);

    connect(&target_through(&target, &jump_to(&jump)), &mut prompter, &target.connect_options()).await.unwrap();

    match prompter.asked.as_slice() {
        [
            Question::TrustHostKey { port: first, .. },
            Question::Password { username, .. },
            Question::TrustHostKey { port: last, .. },
        ] => {
            assert_eq!((*first, *last), (jump.port, target.port));
            assert_eq!(username, testing::USER);
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn two_jump_hosts_are_crossed_in_order() {
    let target = SshServer::start(Options::default()).await;
    let first = SshServer::start(jump_server()).await;
    let second = SshServer::start(jump_server()).await;
    let mut prompter = testing::answers(vec![
        Some(Answer::Confirmed),
        password(),
        Some(Answer::Confirmed),
        password(),
        Some(Answer::Confirmed),
    ]);
    let chain = format!("{},{}", jump_to(&first), jump_to(&second));

    connect(&target_through(&target, &chain), &mut prompter, &target.connect_options())
        .await
        .expect("both hops forward");

    assert_eq!((first.forwarded_connections(), second.forwarded_connections()), (1, 1));
}

#[tokio::test]
async fn a_jump_host_that_refuses_forwarding_is_named_in_the_error() {
    let target = SshServer::start(Options::default()).await;
    let jump = SshServer::start(Options::default()).await;
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed), password()]);

    let error = connect(&target_through(&target, &jump_to(&jump)), &mut prompter, &target.connect_options())
        .await
        .err()
        .expect("forwarding is refused");

    assert_eq!(error.kind(), ErrorKind::Connect);
    let text = error.to_string();
    assert!(text.contains("127.0.0.1") && text.contains("forward"), "{text}");
}

#[tokio::test]
async fn declining_the_jump_hosts_key_cancels_without_touching_the_target() {
    let target = SshServer::start(Options::default()).await;
    let jump = SshServer::start(jump_server()).await;
    let mut prompter = testing::answers(vec![None]);

    let error = connect(&target_through(&target, &jump_to(&jump)), &mut prompter, &target.connect_options())
        .await
        .err()
        .expect("the key was declined");

    assert_eq!(error.kind(), ErrorKind::Cancelled);
    assert_eq!(jump.forwarded_connections(), 0);
}

#[tokio::test]
async fn a_wrong_jump_password_is_rejected_and_names_the_jump_host() {
    let target = SshServer::start(Options::default()).await;
    let jump = SshServer::start(jump_server()).await;
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed), Some(Answer::Password("nope".to_string()))]);

    let error = connect(&target_through(&target, &jump_to(&jump)), &mut prompter, &target.connect_options())
        .await
        .err()
        .expect("the password is wrong");

    assert_eq!(error.kind(), ErrorKind::AuthRejected);
    assert!(error.to_string().contains("127.0.0.1"), "{error}");
}

#[tokio::test]
async fn cancelling_the_jump_password_cancels() {
    let target = SshServer::start(Options::default()).await;
    let jump = SshServer::start(jump_server()).await;
    let mut prompter = testing::answers(vec![Some(Answer::Confirmed), None]);

    let error = connect(&target_through(&target, &jump_to(&jump)), &mut prompter, &target.connect_options())
        .await
        .err()
        .expect("the prompt was cancelled");

    assert_eq!(error.kind(), ErrorKind::Cancelled);
}

#[tokio::test]
async fn none_or_an_empty_jump_connects_directly() {
    for value in ["none", "", "  "] {
        let target = SshServer::start(Options::default()).await;
        let jump = SshServer::start(jump_server()).await;
        let mut prompter = testing::answers(vec![Some(Answer::Confirmed)]);

        connect(&target_through(&target, value), &mut prompter, &target.connect_options())
            .await
            .unwrap_or_else(|error| panic!("{value:?}: {error}"));

        assert_eq!(jump.forwarded_connections(), 0, "{value:?}");
    }
}

#[tokio::test]
async fn a_malformed_jump_is_a_connect_error_that_says_what_is_wrong() {
    let target = SshServer::start(Options::default()).await;

    let error = connect(&target_through(&target, "host:abc"), &mut testing::answers(vec![]), &target.connect_options())
        .await
        .err()
        .expect("the jump can't be read");

    assert_eq!(error.kind(), ErrorKind::Connect);
    assert!(error.to_string().contains("abc"), "{error}");
}
