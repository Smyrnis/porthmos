use std::{collections::HashSet, sync::Arc};

use porthmos_vfs::Protocol;

use super::*;
use crate::import::{
    Candidate, ImportRow, Verdict,
    testing::{candidate, ftp_like, sftp_like},
};

fn protocols() -> Vec<Arc<dyn Protocol>> {
    vec![sftp_like(), ftp_like()]
}

fn new_row(name: &str, protocol: &str) -> ImportRow {
    ImportRow { name: name.to_string(), candidate: Some(candidate(name, protocol)), verdict: Verdict::New }
}

fn skipped(name: &str, verdict: Verdict) -> ImportRow {
    ImportRow { name: name.to_string(), candidate: None, verdict }
}

fn names(rows: &[ImportRow]) -> Vec<&str> {
    rows.iter().map(|row| row.name.as_str()).collect()
}

fn candidate_of(row: &ImportRow) -> &Candidate {
    row.candidate.as_ref().unwrap()
}

#[test]
fn unavailable_protocol_becomes_unsupported() {
    let rows = finish(vec![new_row("a", "s3")], &protocols(), &HashSet::new());
    assert_eq!(rows[0].verdict, Verdict::Unsupported("S3 is not available in this build".to_string()));
    assert!(rows[0].candidate.is_none());
}

#[test]
fn rows_that_are_not_new_pass_through_untouched() {
    let input = vec![skipped("u", Verdict::Unsupported("Storj".into())), skipped("i", Verdict::Invalid("port".into()))];
    assert_eq!(finish(input.clone(), &protocols(), &HashSet::from(["u".to_string()])), input);
}

#[test]
fn options_outside_the_form_are_dropped() {
    let mut row = new_row("a", "sftp");
    row.candidate.as_mut().unwrap().options.extend([
        ("identity_file".to_string(), "/k".to_string()),
        ("remote_path".to_string(), "/srv".to_string()),
        ("bogus".to_string(), "x".to_string()),
        ("security".to_string(), "explicit".to_string()),
    ]);
    let rows = finish(vec![row], &protocols(), &HashSet::new());
    let keys: Vec<&str> = candidate_of(&rows[0]).options.keys().map(String::as_str).collect();
    assert_eq!(keys, ["identity_file", "remote_path"]);
}

#[test]
fn a_bad_choice_or_toggle_is_invalid() {
    let mut choice = new_row("a", "ftp");
    choice.candidate.as_mut().unwrap().options.insert("security".to_string(), "sslv3".to_string());
    let mut toggle = new_row("b", "ftp");
    toggle.candidate.as_mut().unwrap().options.insert("passive".to_string(), "maybe".to_string());
    let rows = finish(vec![choice, toggle], &protocols(), &HashSet::new());
    assert_eq!(rows[0].verdict, Verdict::Invalid("Security must be one of the offered choices".to_string()));
    assert_eq!(rows[1].verdict, Verdict::Invalid("Passive mode must be on or off".to_string()));
    assert!(rows.iter().all(|row| row.candidate.is_none()));
}

#[test]
fn a_good_choice_is_kept() {
    let mut row = new_row("a", "ftp");
    row.candidate.as_mut().unwrap().options.insert("security".to_string(), "implicit".to_string());
    let rows = finish(vec![row], &protocols(), &HashSet::new());
    assert_eq!(rows[0].verdict, Verdict::New);
}

#[test]
fn a_username_the_form_requires_is_checked() {
    let mut sftp = new_row("a", "sftp");
    sftp.candidate.as_mut().unwrap().username.clear();
    let mut ftp = new_row("b", "ftp");
    ftp.candidate.as_mut().unwrap().username.clear();
    let rows = finish(vec![sftp, ftp], &protocols(), &HashSet::new());
    assert_eq!(rows[0].verdict, Verdict::Invalid("no username".to_string()));
    assert_eq!(rows[1].verdict, Verdict::New);
}

#[test]
fn same_name_twice_gets_numbered_suffixes() {
    let rows = finish(
        vec![new_row("Site", "sftp"), new_row("Site", "sftp"), new_row("Site", "sftp")],
        &protocols(),
        &HashSet::new(),
    );
    assert_eq!(names(&rows), ["Site", "Site (2)", "Site (3)"]);
    assert_eq!(candidate_of(&rows[2]).name, "Site (3)");
}

#[test]
fn suffix_does_not_collide_with_a_literal_second_site() {
    let rows = finish(
        vec![new_row("A", "sftp"), new_row("A", "sftp"), new_row("A (2)", "sftp")],
        &protocols(),
        &HashSet::new(),
    );
    let unique: HashSet<&str> = names(&rows).into_iter().collect();
    assert_eq!(unique.len(), 3);
    assert_eq!(names(&rows)[0], "A");
    assert_eq!(names(&rows)[2], "A (2)");
}

#[test]
fn names_that_are_not_new_do_not_take_part_in_numbering() {
    let rows = finish(
        vec![skipped("Site", Verdict::Unsupported("x".into())), new_row("Site", "sftp")],
        &protocols(),
        &HashSet::new(),
    );
    assert_eq!(names(&rows), ["Site", "Site"]);
}

#[test]
fn taken_names_become_exists_but_stay_listed_with_their_candidate() {
    let taken = HashSet::from(["Site".to_string()]);
    let rows = finish(vec![new_row("Site", "sftp"), new_row("Other", "sftp")], &protocols(), &taken);
    assert_eq!(rows[0].verdict, Verdict::Exists);
    assert!(rows[0].candidate.is_some());
    assert_eq!(rows[1].verdict, Verdict::New);
}

#[test]
fn names_are_compared_exactly() {
    let taken = HashSet::from(["site".to_string()]);
    let rows = finish(vec![new_row("Site", "sftp")], &protocols(), &taken);
    assert_eq!(rows[0].verdict, Verdict::New);
}

#[test]
fn a_duplicate_of_a_taken_name_gets_a_free_second_name() {
    let taken = HashSet::from(["Site".to_string()]);
    let rows = finish(vec![new_row("Site", "sftp"), new_row("Site", "sftp")], &protocols(), &taken);
    assert_eq!((rows[0].verdict.clone(), rows[1].verdict.clone()), (Verdict::Exists, Verdict::New));
    assert_eq!(rows[1].name, "Site (2)");
}

fn with_field(edit: impl FnOnce(&mut Candidate, String), length: usize) -> ImportRow {
    let mut row = new_row("a", "sftp");
    edit(row.candidate.as_mut().unwrap(), "x".repeat(length));
    let rows = finish(vec![row], &protocols(), &HashSet::new());
    rows.into_iter().next().unwrap()
}

#[test]
fn a_field_at_its_limit_is_accepted_and_one_past_it_is_invalid() {
    type FieldCase = (&'static str, usize, fn(&mut Candidate, String));
    let cases: [FieldCase; 4] = [
        ("host", MAX_HOST, |candidate, text| candidate.host = text),
        ("username", MAX_USERNAME, |candidate, text| candidate.username = text),
        ("group", MAX_GROUP, |candidate, text| candidate.group = Some(text)),
        ("remote_path", MAX_PATH, |candidate, text| {
            candidate.options.insert("remote_path".to_string(), text);
        }),
    ];
    for (field, limit, edit) in cases {
        assert_eq!(with_field(edit, limit).verdict, Verdict::New, "{field} at the limit");
        let over = with_field(edit, limit + 1);
        assert!(matches!(&over.verdict, Verdict::Invalid(reason) if reason.contains(field)), "{field}: {over:?}");
        assert!(over.candidate.is_none());
    }
}

#[test]
fn a_name_past_its_limit_is_invalid() {
    let long = "n".repeat(MAX_NAME + 1);
    let mut row = new_row(&long, "sftp");
    row.candidate.as_mut().unwrap().name = long.clone();

    let rows = finish(vec![row], &protocols(), &HashSet::new());

    assert!(matches!(&rows[0].verdict, Verdict::Invalid(reason) if reason.contains("name")), "{:?}", rows[0]);
}

#[test]
fn the_limits_count_characters_not_bytes() {
    let mut row = new_row("a", "sftp");
    row.candidate.as_mut().unwrap().username = "é".repeat(MAX_USERNAME);

    let rows = finish(vec![row], &protocols(), &HashSet::new());

    assert_eq!(rows[0].verdict, Verdict::New);
}

fn with_security(protocol: &str, security: &str, port: Option<u16>) -> ImportRow {
    let mut row = new_row("a", protocol);
    let candidate = row.candidate.as_mut().unwrap();
    candidate.options.insert("security".to_string(), security.to_string());
    candidate.port = port;
    row
}

#[test]
fn natural_ports_follow_the_security_choice() {
    assert_eq!(natural_port("ftp", Some("implicit")), Some(990));
    assert_eq!(natural_port("ftp", Some("explicit")), None);
    assert_eq!(natural_port("ftp", Some("plain")), None);
    assert_eq!(natural_port("webdav", Some("http")), Some(80));
    assert_eq!(natural_port("webdav", Some("https")), Some(443));
    assert_eq!(natural_port("sftp", None), None);
}

#[test]
fn an_implicit_ftps_site_without_a_port_gets_the_implicit_port() {
    let rows = finish(vec![with_security("ftp", "implicit", None)], &protocols(), &HashSet::new());

    assert_eq!(candidate_of(&rows[0]).port, Some(990));
}

#[test]
fn a_port_the_source_gave_is_never_replaced() {
    let rows = finish(vec![with_security("ftp", "implicit", Some(2121))], &protocols(), &HashSet::new());

    assert_eq!(candidate_of(&rows[0]).port, Some(2121));
}

#[test]
fn a_plain_site_without_a_port_keeps_the_protocols_own_default() {
    let rows = finish(vec![with_security("ftp", "plain", None)], &protocols(), &HashSet::new());

    assert_eq!(candidate_of(&rows[0]).port, None);
}
