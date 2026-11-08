use super::*;

#[test]
fn answer_debug_redacts_the_password() {
    assert!(!format!("{:?}", Answer::Password("hunter2".into())).contains("hunter2"));
}

#[test]
fn answer_debug_shows_a_confirmation() {
    assert_eq!(format!("{:?}", Answer::Confirmed), "Confirmed");
}

fn trust_host_key() -> Question {
    Question::TrustHostKey {
        name: "box".into(),
        host: "box.example".into(),
        port: 22,
        key_type: "ssh-ed25519".into(),
        fingerprint: "SHA256:abc".into(),
    }
}

fn trust_certificate() -> Question {
    Question::TrustCertificate {
        name: "box".into(),
        host: "box.example".into(),
        port: 443,
        fingerprint: "AA:BB".into(),
        subject: "box.example".into(),
        expires: "2030-01-01".into(),
    }
}

#[test]
fn trusting_a_new_host_key_or_certificate_is_never_confirmed_by_default() {
    assert!(trust_host_key().is_risky_to_confirm());
    assert!(trust_certificate().is_risky_to_confirm());
}

#[test]
fn a_password_prompt_has_nothing_risky_to_confirm() {
    assert!(!Question::Password { username: "root".into(), name: "box".into() }.is_risky_to_confirm());
}
