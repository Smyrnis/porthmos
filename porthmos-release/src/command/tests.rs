use std::path::Path;

use porthmos_update::{Manifest, sha256_hex};

use super::*;

const KEY_VARIABLE: &str = "SIGNING_KEY";

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

fn path(directory: &tempfile::TempDir, name: &str) -> String {
    directory.path().join(name).to_str().unwrap().to_string()
}

fn no_env(_name: &str) -> Option<String> {
    None
}

fn key_pair() -> (String, String) {
    let output = run(&args(&["keygen"]), &no_env).unwrap();
    let field = |label: &str| {
        output
            .lines()
            .find_map(|line| line.strip_prefix(label))
            .unwrap_or_else(|| panic!("no {label} line in {output:?}"))
            .to_string()
    };
    (field("private: "), field("public: "))
}

fn env_with(key: String) -> impl Fn(&str) -> Option<String> {
    move |name| (name == KEY_VARIABLE).then(|| key.clone())
}

struct Signed {
    directory: tempfile::TempDir,
    public: String,
}

impl Signed {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("latest.json"), b"{\"version\": \"0.4.0\"}\n").unwrap();
        let (private, public) = key_pair();
        run(
            &args(&[
                "sign",
                "--key-env",
                KEY_VARIABLE,
                "--in",
                &path(&directory, "latest.json"),
                "--out",
                &path(&directory, "latest.json.sig"),
            ]),
            &env_with(private),
        )
        .unwrap();
        Self { directory, public }
    }

    fn verify(&self) -> Result<String> {
        run(
            &args(&[
                "verify",
                "--in",
                &path(&self.directory, "latest.json"),
                "--sig",
                &path(&self.directory, "latest.json.sig"),
                "--public-key",
                &self.public,
            ]),
            &no_env,
        )
    }
}

#[test]
fn keygen_prints_a_private_and_a_public_line_that_belong_together() {
    let (private, public) = key_pair();
    let signature = porthmos_update::sign(&private, b"message").unwrap();

    let key = porthmos_update::parse_public_key(&public).unwrap();

    assert_eq!(porthmos_update::verify(&[key], b"message", &signature), Ok(()));
}

#[test]
fn sign_then_verify_round_trips_with_the_matching_public_key() {
    let signed = Signed::new();

    assert_eq!(signed.verify().unwrap(), "ok");
}

#[test]
fn the_signature_file_is_one_line_with_a_trailing_newline() {
    let signed = Signed::new();

    let text = std::fs::read_to_string(signed.directory.path().join("latest.json.sig")).unwrap();

    assert!(text.ends_with('\n') && text.trim_end().lines().count() == 1, "{text:?}");
}

#[test]
fn a_manifest_changed_by_one_character_after_signing_fails_verification() {
    let signed = Signed::new();
    let file = signed.directory.path().join("latest.json");
    let text = std::fs::read_to_string(&file).unwrap().replace("0.4.0", "0.4.1");
    std::fs::write(&file, text).unwrap();

    let message = signed.verify().unwrap_err().to_string();

    assert!(message.contains("does not match"), "{message}");
}

#[test]
fn a_changed_hash_inside_the_manifest_fails_verification() {
    let directory = tempfile::tempdir().unwrap();
    let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let original = format!("{{\"sha256\": \"{hash}\"}}\n");
    std::fs::write(directory.path().join("latest.json"), &original).unwrap();
    let (private, public) = key_pair();
    run(
        &args(&[
            "sign",
            "--key-env",
            KEY_VARIABLE,
            "--in",
            &path(&directory, "latest.json"),
            "--out",
            &path(&directory, "latest.json.sig"),
        ]),
        &env_with(private),
    )
    .unwrap();
    std::fs::write(directory.path().join("latest.json"), original.replace("ba78", "ca78")).unwrap();

    let result = run(
        &args(&[
            "verify",
            "--in",
            &path(&directory, "latest.json"),
            "--sig",
            &path(&directory, "latest.json.sig"),
            "--public-key",
            &public,
        ]),
        &no_env,
    );

    assert!(result.is_err());
}

#[test]
fn sign_with_a_missing_empty_or_malformed_key_variable_names_the_variable_and_writes_no_file() {
    let secret = "this-is-not-a-key-but-must-not-be-echoed";
    for value in [None, Some(String::new()), Some(secret.to_string())] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("latest.json"), b"{}\n").unwrap();
        let env = move |name: &str| if name == KEY_VARIABLE { value.clone() } else { None };

        let message = run(
            &args(&[
                "sign",
                "--key-env",
                KEY_VARIABLE,
                "--in",
                &path(&directory, "latest.json"),
                "--out",
                &path(&directory, "latest.json.sig"),
            ]),
            &env,
        )
        .unwrap_err()
        .to_string();

        assert!(message.contains(KEY_VARIABLE), "{message}");
        assert!(!message.contains(secret), "{message}");
        assert!(!directory.path().join("latest.json.sig").exists());
    }
}

#[test]
fn a_missing_flag_is_an_error_naming_the_flag() {
    let message = run(&args(&["sign", "--key-env", KEY_VARIABLE]), &no_env).unwrap_err().to_string();

    assert!(message.contains("--in"), "{message}");
}

#[test]
fn an_unknown_subcommand_lists_the_known_ones() {
    let message = run(&args(&["frobnicate"]), &no_env).unwrap_err().to_string();

    for name in ["keygen", "package", "manifest", "sign", "verify"] {
        assert!(message.contains(name), "{message}");
    }
}

#[test]
fn no_subcommand_is_an_error_that_lists_the_known_ones() {
    assert!(run(&[], &no_env).unwrap_err().to_string().contains("keygen"));
}

#[test]
fn a_flag_without_a_value_is_an_error() {
    let message = run(&args(&["verify", "--in"]), &no_env).unwrap_err().to_string();

    assert!(message.contains("--in"), "{message}");
}

#[test]
fn the_manifest_subcommand_lists_what_the_package_subcommand_wrote() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("porthmos"), b"binary").unwrap();
    std::fs::write(directory.path().join("LICENSE"), b"license").unwrap();
    std::fs::write(directory.path().join("notes.txt"), b"the notes\n").unwrap();
    let out = path(&directory, "dist");
    for target in ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"] {
        run(
            &args(&[
                "package",
                "--target",
                target,
                "--version",
                "0.4.0",
                "--binary",
                &path(&directory, "porthmos"),
                "--license",
                &path(&directory, "LICENSE"),
                "--out",
                &out,
            ]),
            &no_env,
        )
        .unwrap();
    }

    run(
        &args(&[
            "manifest",
            "--version",
            "0.4.0",
            "--released",
            "2026-11-02T10:00:00Z",
            "--notes-file",
            &path(&directory, "notes.txt"),
            "--base-url",
            "https://github.com/Smyrnis/porthmos/releases/download/v0.4.0",
            "--artifacts",
            &out,
            "--out",
            &path(&directory, "latest.json"),
        ]),
        &no_env,
    )
    .unwrap();

    let manifest = Manifest::parse(&std::fs::read(directory.path().join("latest.json")).unwrap()).unwrap();
    assert_eq!(manifest.notes, "the notes");
    assert_eq!(manifest.targets.len(), 2);
    for (target, artifact) in &manifest.targets {
        let archive = std::fs::read(Path::new(&out).join(format!("porthmos-0.4.0-{target}.tar.gz"))).unwrap();
        assert_eq!(artifact.sha256, sha256_hex(&archive));
    }
}

#[test]
fn verify_without_a_public_key_uses_the_embedded_keys_and_refuses_a_stranger() {
    let signed = Signed::new();

    let result = run(
        &args(&[
            "verify",
            "--in",
            &path(&signed.directory, "latest.json"),
            "--sig",
            &path(&signed.directory, "latest.json.sig"),
        ]),
        &no_env,
    );

    assert!(result.is_err());
}
