use base64::{Engine, engine::general_purpose::STANDARD};

use super::*;

const MESSAGE: &[u8] = b"{\"version\":\"0.4.0\"}\n";

fn pair() -> KeyPair {
    generate_key_pair().unwrap()
}

fn trusted(pair: &KeyPair) -> Vec<PublicKey> {
    vec![parse_public_key(&pair.public).unwrap()]
}

#[test]
fn a_signature_verifies_with_its_own_public_key() {
    let pair = pair();
    let signature = sign(&pair.private, MESSAGE).unwrap();

    assert_eq!(verify(&trusted(&pair), MESSAGE, &signature), Ok(()));
}

#[test]
fn a_changed_message_fails_with_mismatch() {
    let pair = pair();
    let signature = sign(&pair.private, MESSAGE).unwrap();

    assert_eq!(verify(&trusted(&pair), b"{\"version\":\"0.4.1\"}\n", &signature), Err(VerifyError::Mismatch));
}

#[test]
fn a_changed_signature_character_fails() {
    let pair = pair();
    let mut bytes = STANDARD.decode(sign(&pair.private, MESSAGE).unwrap()).unwrap();
    bytes[10] ^= 1;

    assert_eq!(verify(&trusted(&pair), MESSAGE, &STANDARD.encode(bytes)), Err(VerifyError::Mismatch));
}

#[test]
fn a_signature_from_another_key_fails() {
    let signer = pair();
    let other = pair();
    let signature = sign(&signer.private, MESSAGE).unwrap();

    assert_eq!(verify(&trusted(&other), MESSAGE, &signature), Err(VerifyError::Mismatch));
}

#[test]
fn any_one_of_several_trusted_keys_is_enough() {
    let old = pair();
    let new = pair();
    let signature = sign(&new.private, MESSAGE).unwrap();
    let keys = vec![parse_public_key(&old.public).unwrap(), parse_public_key(&new.public).unwrap()];

    assert_eq!(verify(&keys, MESSAGE, &signature), Ok(()));
}

#[test]
fn no_trusted_keys_means_nothing_verifies() {
    let pair = pair();
    let signature = sign(&pair.private, MESSAGE).unwrap();

    assert_eq!(verify(&[], MESSAGE, &signature), Err(VerifyError::NoTrustedKeys));
}

#[test]
fn a_signature_that_is_not_base64_or_not_64_bytes_is_bad_encoding() {
    let pair = pair();
    let short = STANDARD.encode([7u8; 63]);

    for bad in ["not base64 !!", short.as_str(), ""] {
        assert_eq!(verify(&trusted(&pair), MESSAGE, bad), Err(VerifyError::BadEncoding), "{bad:?}");
    }
}

#[test]
fn a_signature_with_a_trailing_newline_verifies() {
    let pair = pair();
    let signature = format!("{}\n", sign(&pair.private, MESSAGE).unwrap());

    assert_eq!(verify(&trusted(&pair), MESSAGE, &signature), Ok(()));
}

#[test]
fn the_private_half_never_appears_in_debug_output() {
    let pair = pair();

    let printed = format!("{pair:?}");

    assert!(!printed.contains(&pair.private));
    assert!(printed.contains("<redacted>"));
}

#[test]
fn a_private_key_of_the_wrong_length_or_encoding_is_an_error_that_does_not_echo_it() {
    let short = STANDARD.encode([1u8; 31]);
    let long = STANDARD.encode([1u8; 33]);

    for bad in [short.as_str(), long.as_str(), "not base64 !!", ""] {
        let message = sign(bad, MESSAGE).unwrap_err().to_string();

        assert!(message.contains("private key"), "{message}");
        assert!(bad.is_empty() || !message.contains(bad), "{message}");
    }
}

#[test]
fn a_public_key_of_the_wrong_length_is_an_error() {
    assert!(parse_public_key(&STANDARD.encode([1u8; 31])).is_err());
    assert!(parse_public_key("not base64 !!").is_err());
}

#[test]
fn two_generated_key_pairs_differ() {
    let (first, second) = (pair(), pair());

    assert_ne!(first.private, second.private);
    assert_ne!(first.public, second.public);
}
