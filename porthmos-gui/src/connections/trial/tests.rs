use std::time::Duration;

use porthmos_core::{NotTrusted, StartFolder, TestOutcome, TestReport, TestStage};

use super::*;

fn connected(server: Option<&str>, start_folder: StartFolder) -> TestOutcome {
    TestOutcome::Connected(TestReport {
        elapsed: Duration::from_millis(120),
        server: server.map(str::to_string),
        start_folder,
    })
}

#[test]
fn the_connected_line_includes_elapsed_server_and_start_folder() {
    let line = describe(&connected(Some("OpenSSH_9.6"), StartFolder::Opens("/srv".to_string()))).unwrap();

    assert_eq!(line.text, "Connected in 120ms \u{b7} OpenSSH_9.6. Start folder /srv opens.");
    assert!(line.good);
}

#[test]
fn the_connected_line_without_server_or_folder_check_is_short() {
    let line = describe(&connected(None, StartFolder::Unchecked)).unwrap();

    assert_eq!(line.text, "Connected in 120ms");
    assert!(line.good);
}

#[test]
fn a_missing_start_folder_is_not_good() {
    let line = describe(&connected(None, StartFolder::Missing("/nope".to_string()))).unwrap();

    assert_eq!(line.text, "Connected in 120ms. Start folder /nope does not exist.");
    assert!(!line.good);
}

#[test]
fn an_unreadable_start_folder_says_why() {
    let outcome = connected(None, StartFolder::Unreadable { path: "/root".to_string(), message: "denied".to_string() });

    let line = describe(&outcome).unwrap();

    assert_eq!(line.text, "Connected in 120ms. Start folder /root can't be read: denied.");
    assert!(!line.good);
}

#[test]
fn an_untrusted_host_key_explains_what_to_do() {
    let outcome = TestOutcome::NotTrusted(NotTrusted::HostKey {
        key_type: "ssh-ed25519".to_string(),
        fingerprint: "SHA256:abc".to_string(),
    });

    let line = describe(&outcome).unwrap();

    assert_eq!(
        line.text,
        "Host key not trusted yet (ssh-ed25519 SHA256:abc). Connect once and accept it, then test again."
    );
    assert!(!line.good);
}

#[test]
fn an_untrusted_certificate_explains_what_to_do() {
    let outcome = TestOutcome::NotTrusted(NotTrusted::Certificate {
        fingerprint: "AA:BB".to_string(),
        subject: "example.com".to_string(),
    });

    let line = describe(&outcome).unwrap();

    assert_eq!(
        line.text,
        "Certificate not trusted yet (example.com, AA:BB). Connect once and accept it, then test again."
    );
}

#[test]
fn a_failure_names_its_stage() {
    let cases = [
        (TestStage::Settings, "Settings"),
        (TestStage::Connect, "Connect"),
        (TestStage::Login, "Login"),
        (TestStage::StartFolder, "Start folder"),
    ];
    for (stage, name) in cases {
        let line = describe(&TestOutcome::Failed { stage, message: "boom".to_string() }).unwrap();

        assert_eq!(line.text, format!("{name} failed: boom"));
        assert!(!line.good);
    }
}

#[test]
fn a_timeout_names_its_stage() {
    let line = describe(&TestOutcome::TimedOut(TestStage::Login)).unwrap();

    assert_eq!(line.text, "Timed out at Login");
    assert!(!line.good);
}

#[test]
fn a_cancelled_test_has_no_line() {
    assert!(describe(&TestOutcome::Cancelled).is_none());
}

#[test]
fn the_prompt_debug_never_prints_the_typed_password() {
    let mut prompt = TestPrompt::new(7, "me".to_string(), "prod".to_string());
    prompt.text = Typed::new("hunter2-marker".to_string());

    assert!(!format!("{prompt:?}").contains("hunter2-marker"));
}
