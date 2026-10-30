use porthmos_core::Question;

use super::{Dialog, Typed};

fn trust_host() -> Question {
    Question::TrustHostKey {
        name: "alpha".to_string(),
        host: "alpha.example".to_string(),
        port: 22,
        key_type: "ssh-ed25519".to_string(),
        fingerprint: "SHA256:abc".to_string(),
    }
}

#[test]
fn a_password_question_becomes_a_password_dialog_for_that_request() {
    let dialog =
        Dialog::from_question(7, Question::Password { username: "me".to_string(), name: "alpha".to_string() }, None);

    assert_eq!(dialog.request_id(), 7);
    match dialog {
        Dialog::Password { prompt, text, .. } => {
            assert!(prompt.contains("me") && prompt.contains("alpha"), "{prompt}");
            assert_eq!(text.as_str(), "");
        }
        other => panic!("expected a password dialog, got {other:?}"),
    }
}

#[test]
fn a_host_key_question_shows_the_host_and_the_fingerprint() {
    match Dialog::from_question(1, trust_host(), None) {
        Dialog::Confirm { body, .. } => {
            assert!(body.contains("alpha.example:22"), "{body}");
            assert!(body.contains("ssh-ed25519") && body.contains("SHA256:abc"), "{body}");
        }
        other => panic!("expected a confirmation, got {other:?}"),
    }
}

#[test]
fn a_certificate_question_shows_the_subject_and_the_expiry() {
    let question = Question::TrustCertificate {
        name: "dav".to_string(),
        host: "dav.example".to_string(),
        port: 443,
        fingerprint: "AA:BB".to_string(),
        subject: "CN=dav.example".to_string(),
        expires: "2030-01-01".to_string(),
    };

    match Dialog::from_question(2, question, None) {
        Dialog::Confirm { body, .. } => {
            assert!(body.contains("CN=dav.example") && body.contains("2030-01-01") && body.contains("AA:BB"), "{body}")
        }
        other => panic!("expected a confirmation, got {other:?}"),
    }
}

#[test]
fn typed_secrets_never_appear_in_debug_output() {
    let typed = Typed::new("hunter2".to_string());
    let dialog = Dialog::Password {
        request_id: 1,
        title: "Password".to_string(),
        prompt: "p".to_string(),
        text: typed.clone(),
        save: None,
    };

    assert!(!format!("{typed:?}").contains("hunter2"));
    assert!(!format!("{dialog:?}").contains("hunter2"));
}

fn password_question() -> Question {
    Question::Password { username: "me".to_string(), name: "alpha".to_string() }
}

#[test]
fn a_password_dialog_carries_the_save_choice_it_was_given() {
    match Dialog::from_question(1, password_question(), Some(true)) {
        Dialog::Password { save, .. } => assert_eq!(save, Some(true)),
        other => panic!("{other:?}"),
    }
    match Dialog::from_question(1, password_question(), None) {
        Dialog::Password { save, .. } => assert_eq!(save, None),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_trust_dialog_has_no_save_choice() {
    assert!(matches!(Dialog::from_question(1, trust_host(), Some(true)), Dialog::Confirm { .. }));
}

#[test]
fn the_password_dialog_debug_never_prints_the_typed_text() {
    let mut dialog = Dialog::from_question(1, password_question(), Some(false));
    if let Dialog::Password { text, .. } = &mut dialog {
        *text = Typed::new("hunter2-marker".to_string());
    }

    assert!(!format!("{dialog:?}").contains("hunter2-marker"));
}
