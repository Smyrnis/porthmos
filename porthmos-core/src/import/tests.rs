use std::collections::BTreeMap;

use super::*;

fn candidate(name: &str) -> Candidate {
    Candidate {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: "example.com".to_string(),
        port: Some(22),
        username: "user".to_string(),
        group: None,
        options: BTreeMap::new(),
        notes: Vec::new(),
    }
}

fn row(name: &str, verdict: Verdict) -> ImportRow {
    ImportRow { name: name.to_string(), candidate: Some(candidate(name)), verdict }
}

#[test]
fn candidate_has_no_secret_field() {
    let Candidate { name, protocol, host, port, username, group, options, notes } = candidate("a");
    assert_eq!(
        (name, protocol, host, port, username),
        ("a".into(), "sftp".into(), "example.com".into(), Some(22), "user".into())
    );
    assert_eq!((group, options.len(), notes.len()), (None, 0, 0));
}

#[test]
fn new_count_counts_only_new_rows() {
    let plan = ImportPlan {
        import_id: 1,
        source: ImportSource::FileZilla,
        rows: vec![
            row("a", Verdict::New),
            row("b", Verdict::Exists),
            row("c", Verdict::Unsupported("Storj".into())),
            row("d", Verdict::Invalid("port".into())),
            row("e", Verdict::New),
        ],
    };
    assert_eq!(plan.new_count(), 2);
}

#[test]
fn sources_have_labels() {
    let labels: Vec<&str> = ImportSource::ALL.iter().map(|source| source.label()).collect();
    assert_eq!(labels, ["FileZilla", "WinSCP", "PuTTY"]);
}

#[test]
fn errors_read_as_short_messages() {
    assert_eq!(ImportError::TooLarge.to_string(), "The file is larger than 8 MiB");
    assert_eq!(ImportError::Unreadable("no such file".into()).to_string(), "Can't read the file: no such file");
    assert_eq!(
        ImportError::WrongFormat("import.not_filezilla").to_string(),
        "This is not a FileZilla site manager file"
    );
}

#[test]
fn every_wrong_format_message_exists_in_the_language_file() {
    for key in [
        "import.not_filezilla",
        "import.not_winscp",
        "import.not_utf8",
        "import.too_many_files",
        "import.no_sessions",
        "import.not_reg",
        "import.not_text_reg",
    ] {
        assert_ne!(ImportError::WrongFormat(key).to_string(), key);
    }
}

#[test]
fn a_row_summary_shows_protocol_user_host_and_port() {
    let mut full = candidate("a");
    full.port = Some(2222);
    let row = ImportRow { name: "a".into(), candidate: Some(full), verdict: Verdict::New };
    assert_eq!(row.summary(), "sftp user@example.com:2222");
    let mut no_port = candidate("a");
    no_port.port = None;
    let row = ImportRow { name: "a".into(), candidate: Some(no_port), verdict: Verdict::New };
    assert_eq!(row.summary(), "sftp user@example.com");
    let mut anonymous = candidate("a");
    anonymous.username.clear();
    anonymous.protocol = "ftp".into();
    anonymous.port = None;
    let row = ImportRow { name: "a".into(), candidate: Some(anonymous), verdict: Verdict::New };
    assert_eq!(row.summary(), "ftp example.com");
}

#[test]
fn a_row_without_a_candidate_has_an_empty_summary() {
    let row = ImportRow { name: "a".into(), candidate: None, verdict: Verdict::Unsupported("Storj".into()) };
    assert_eq!(row.summary(), "");
}

#[test]
fn a_row_status_says_why_it_cannot_be_imported() {
    let status = |verdict| ImportRow { name: "a".into(), candidate: None, verdict }.status();
    assert_eq!(status(Verdict::New), None);
    assert_eq!(status(Verdict::Exists), Some("already exists".to_string()));
    assert_eq!(
        status(Verdict::Unsupported("Storj is not supported".into())),
        Some("Storj is not supported".to_string())
    );
    assert_eq!(status(Verdict::Invalid("port".into())), Some("can't import: port".to_string()));
}

#[test]
fn a_row_note_line_joins_the_notes() {
    let mut noted = candidate("a");
    noted.notes = vec!["remote folder not imported".into(), "key is in PuTTY format, convert it first".into()];
    let row = ImportRow { name: "a".into(), candidate: Some(noted), verdict: Verdict::New };
    assert_eq!(
        row.notes_line(),
        Some("remote folder not imported; key is in PuTTY format, convert it first".to_string())
    );
    let plain = ImportRow { name: "a".into(), candidate: Some(candidate("a")), verdict: Verdict::New };
    assert_eq!(plain.notes_line(), None);
}

#[test]
fn every_source_has_a_hint_that_names_it_and_says_passwords_are_not_imported() {
    for source in ImportSource::ALL {
        let hint = source.hint();
        assert!(hint.starts_with(source.label()), "{hint}");
        assert!(hint.contains("Passwords are not imported"), "{hint}");
    }
}

#[test]
fn only_putty_asks_for_a_folder_when_the_path_is_missing() {
    assert_eq!(ImportSource::FileZilla.missing_path_message(), "Enter the path of the file");
    assert_eq!(ImportSource::WinScp.missing_path_message(), "Enter the path of the file");
    assert_eq!(ImportSource::Putty.missing_path_message(), "Enter the path of the sessions folder or .reg file");
}

#[test]
fn the_done_message_counts_connections_in_the_singular_and_plural() {
    assert_eq!(done_message(1, 0), "Imported 1 connection");
    assert_eq!(done_message(0, 0), "Imported 0 connections");
    assert_eq!(done_message(2, 0), "Imported 2 connections");
}

#[test]
fn the_done_message_mentions_what_was_skipped() {
    assert_eq!(done_message(1, 3), "Imported 1 connection (3 skipped)");
}

#[test]
fn the_message_for_an_empty_source_names_the_program() {
    assert_eq!(nothing_found_message(ImportSource::WinScp), "No connections found in that WinSCP source");
}
