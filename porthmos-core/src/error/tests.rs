use porthmos_vfs::{ErrorKind, ProtocolError};

use super::*;

#[test]
fn user_message_formats_context_and_error_on_separate_lines() {
    let err = anyhow::anyhow!("connection timed out");
    assert_eq!(
        user_message("Unable to connect to production", &err),
        "Unable to connect to production:\nconnection timed out"
    );
}

fn failure(kind: ErrorKind) -> ProtocolError {
    ProtocolError::new(kind, anyhow::anyhow!("detail"))
}

#[test]
fn connect_failures_keep_their_established_wording() {
    assert_eq!(
        connect_failure_message(&failure(ErrorKind::Connect), "web", "SFTP"),
        "Unable to connect to web:\ndetail"
    );
    assert_eq!(
        connect_failure_message(&failure(ErrorKind::Auth), "web", "SFTP"),
        "Authentication error for web:\ndetail"
    );
    assert_eq!(
        connect_failure_message(&failure(ErrorKind::AuthRejected), "web", "SFTP"),
        "Authentication failed for web"
    );
    assert_eq!(
        connect_failure_message(&failure(ErrorKind::SessionStart), "web", "SFTP"),
        "Connected to web but failed to start SFTP:\ndetail"
    );
    assert_eq!(connect_failure_message(&failure(ErrorKind::Cancelled), "web", "SFTP"), "Connection cancelled");
    assert_eq!(connect_failure_message(&failure(ErrorKind::Other), "web", "SFTP"), "Unable to connect to web:\ndetail");
}

#[test]
fn notices_stay_longer_the_more_serious_they_are() {
    use std::time::Duration;

    assert_eq!(Severity::Info.lifetime(), Duration::from_secs(4));
    assert_eq!(Severity::Warning.lifetime(), Duration::from_secs(8));
    assert_eq!(Severity::Error.lifetime(), Duration::from_secs(15));
}
