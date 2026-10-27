use super::*;

#[test]
fn the_embedded_keys_parse() {
    for key in TRUSTED_PUBLIC_KEYS {
        assert!(parse_public_key(key).is_ok(), "{key}");
    }
    assert_eq!(trusted_keys().len(), TRUSTED_PUBLIC_KEYS.len());
}

#[test]
fn at_least_one_key_is_embedded() {
    assert!(!TRUSTED_PUBLIC_KEYS.is_empty());
}
