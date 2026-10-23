use chrono::TimeZone;

use super::*;

const URL: &str = "https://github.com/Smyrnis/porthmos/releases/download/v0.4.0/porthmos-0.4.0-x86_64-unknown-linux-gnu.tar.gz";
const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn artifact() -> Artifact {
    Artifact { url: URL.to_string(), sha256: HASH.to_string(), size: 4_821_337 }
}

fn sample() -> Manifest {
    Manifest {
        version: Version::new(0, 4, 0),
        released: Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap(),
        notes: "short plain-text summary".to_string(),
        targets: BTreeMap::from([("x86_64-unknown-linux-gnu".to_string(), artifact())]),
    }
}

fn json_with(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(&sample().to_json().unwrap()).unwrap();
    edit(&mut value);
    serde_json::to_vec(&value).unwrap()
}

fn rejection_of(bytes: &[u8]) -> String {
    Manifest::parse(bytes).expect_err("expected the manifest to be rejected").to_string()
}

#[test]
fn a_manifest_survives_a_round_trip_through_json() {
    assert_eq!(Manifest::parse(&sample().to_json().unwrap()).unwrap(), sample());
}

#[test]
fn the_json_is_stable_and_ends_with_a_newline() {
    let mut manifest = sample();
    manifest.targets.insert("aarch64-apple-darwin".to_string(), artifact());

    let first = manifest.to_json().unwrap();
    let second = manifest.to_json().unwrap();

    assert_eq!(first, second);
    assert_eq!(*first.last().unwrap(), b'\n');
    let text = String::from_utf8(first).unwrap();
    assert!(text.find("aarch64-apple-darwin").unwrap() < text.find("x86_64-unknown-linux-gnu").unwrap());
}

#[test]
fn an_unknown_field_is_ignored() {
    let bytes = json_with(|value| {
        value["channel"] = serde_json::json!("stable");
    });

    assert_eq!(Manifest::parse(&bytes).unwrap(), sample());
}

#[test]
fn a_non_https_url_is_rejected() {
    let bytes = json_with(|value| {
        value["targets"]["x86_64-unknown-linux-gnu"]["url"] = serde_json::json!(URL.replace("https://", "http://"));
    });

    let message = rejection_of(&bytes);

    assert!(message.contains("x86_64-unknown-linux-gnu") && message.contains("https"), "{message}");
}

#[test]
fn a_hash_that_is_not_64_lowercase_hex_characters_is_rejected() {
    for bad in [&HASH[..63], &HASH.to_uppercase(), &HASH.replace('b', "z"), ""] {
        let bytes = json_with(|value| {
            value["targets"]["x86_64-unknown-linux-gnu"]["sha256"] = serde_json::json!(bad);
        });

        let message = rejection_of(&bytes);

        assert!(message.contains("sha256"), "{bad:?}: {message}");
    }
}

#[test]
fn a_zero_size_is_rejected() {
    let bytes = json_with(|value| {
        value["targets"]["x86_64-unknown-linux-gnu"]["size"] = serde_json::json!(0);
    });

    assert!(rejection_of(&bytes).contains("size"));
}

#[test]
fn a_manifest_without_targets_is_rejected() {
    let bytes = json_with(|value| {
        value["targets"] = serde_json::json!({});
    });

    assert!(rejection_of(&bytes).contains("targets"));
}

#[test]
fn a_version_that_is_not_semver_is_rejected() {
    let bytes = json_with(|value| {
        value["version"] = serde_json::json!("four");
    });

    assert!(rejection_of(&bytes).contains("version"));
}

#[test]
fn malformed_json_is_rejected() {
    assert!(Manifest::parse(b"{ not json").is_err());
}
