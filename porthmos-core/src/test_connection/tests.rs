use std::time::Duration;

use porthmos_vfs::{ErrorKind, testing::FakeFs};

use super::*;

#[test]
fn stage_mapping_follows_the_error_kind() {
    assert_eq!(stage_of(ErrorKind::Auth), Some(TestStage::Login));
    assert_eq!(stage_of(ErrorKind::AuthRejected), Some(TestStage::Login));
    for kind in [
        ErrorKind::Connect,
        ErrorKind::SessionStart,
        ErrorKind::Other,
        ErrorKind::Unsupported,
        ErrorKind::NotFound,
        ErrorKind::PermissionDenied,
    ] {
        assert_eq!(stage_of(kind), Some(TestStage::Connect), "{kind:?}");
    }
    assert_eq!(stage_of(ErrorKind::Cancelled), None);
}

#[tokio::test]
async fn a_configured_directory_opens() {
    let fs = FakeFs::new();
    fs.dir("/srv/www");

    assert_eq!(check_start_folder(&fs, Some("/srv/www")).await, StartFolder::Opens("/srv/www".to_string()));
}

#[tokio::test]
async fn a_missing_directory_is_missing() {
    let fs = FakeFs::new();

    assert_eq!(check_start_folder(&fs, Some("/nope")).await, StartFolder::Missing("/nope".to_string()));
}

#[tokio::test]
async fn a_permission_error_is_unreadable_with_the_message() {
    let fs = FakeFs::new();
    fs.dir("/secret").denying_stat("/secret");

    match check_start_folder(&fs, Some("/secret")).await {
        StartFolder::Unreadable { path, message } => {
            assert_eq!(path, "/secret");
            assert!(message.contains("permission denied"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_file_instead_of_a_directory_is_unreadable() {
    let fs = FakeFs::new();
    fs.file("/f.txt", b"x", None);

    assert!(matches!(
        check_start_folder(&fs, Some("/f.txt")).await,
        StartFolder::Unreadable { path, .. } if path == "/f.txt"
    ));
}

#[tokio::test]
async fn no_configured_folder_uses_the_home_directory() {
    let fs = FakeFs::new();
    fs.dir("/");

    let result = check_start_folder(&fs, None).await;

    assert!(matches!(result, StartFolder::Opens(_)), "{result:?}");
}

#[tokio::test]
async fn a_failing_home_is_unchecked() {
    let fs = FakeFs::new();
    fs.fail_home();

    assert_eq!(check_start_folder(&fs, None).await, StartFolder::Unchecked);
}

#[test]
fn debug_of_every_outcome_is_free_of_secrets_by_construction() {
    let outcomes = [
        TestOutcome::Connected(TestReport {
            elapsed: Duration::from_millis(142),
            server: Some("OpenSSH_9.6".into()),
            start_folder: StartFolder::Opens("/home/u".into()),
        }),
        TestOutcome::NotTrusted(NotTrusted::HostKey {
            key_type: "ssh-ed25519".into(),
            fingerprint: "SHA256:ab".into(),
        }),
        TestOutcome::NotTrusted(NotTrusted::Certificate { fingerprint: "AA".into(), subject: "CN=x".into() }),
        TestOutcome::Failed { stage: TestStage::Login, message: "Authentication failed".into() },
        TestOutcome::TimedOut(TestStage::Connect),
        TestOutcome::Cancelled,
    ];

    for outcome in outcomes {
        let printed = format!("{outcome:?}");
        assert!(!printed.is_empty());
        assert_eq!(outcome.clone(), outcome);
    }
}
