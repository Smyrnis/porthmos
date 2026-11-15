use std::{collections::HashSet, fs, path::Path, sync::Arc};

use porthmos_vfs::Protocol;

use super::*;
use crate::import::{
    MAX_INPUT_BYTES, Verdict,
    testing::{ftp_like, sftp_like},
};

fn protocols() -> Vec<Arc<dyn Protocol>> {
    vec![sftp_like(), ftp_like()]
}

const FILEZILLA: &str = "<FileZilla3><Servers>\
    <Server><Host>a.example</Host><Port>22</Port><Protocol>1</Protocol><User>u</User><Logontype>1</Logontype><Name>Alpha</Name></Server>\
    <Server><Host>b.example</Host><Protocol>0</Protocol><User>u</User><Logontype>1</Logontype><Name>Beta</Name></Server>\
    </Servers></FileZilla3>";

fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    path
}

fn plan(source: ImportSource, path: &Path, taken: &HashSet<String>) -> Result<ImportPlan, ImportError> {
    read_plan(7, source, path, &protocols(), taken)
}

fn names(plan: &ImportPlan) -> Vec<&str> {
    plan.rows.iter().map(|row| row.name.as_str()).collect()
}

#[test]
fn a_filezilla_file_becomes_a_plan() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "sitemanager.xml", FILEZILLA);
    let plan = plan(ImportSource::FileZilla, &path, &HashSet::new()).unwrap();
    assert_eq!((plan.import_id, plan.source), (7, ImportSource::FileZilla));
    assert_eq!(names(&plan), ["Alpha", "Beta"]);
    assert_eq!(plan.new_count(), 2);
}

#[test]
fn a_utf8_bom_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "sitemanager.xml", &format!("\u{feff}{FILEZILLA}"));
    assert_eq!(plan(ImportSource::FileZilla, &path, &HashSet::new()).unwrap().rows.len(), 2);
}

#[test]
fn importing_the_same_file_twice_marks_everything_exists() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "sitemanager.xml", FILEZILLA);
    let first = plan(ImportSource::FileZilla, &path, &HashSet::new()).unwrap();
    let taken: HashSet<String> = first.rows.iter().map(|row| row.name.clone()).collect();
    let second = plan(ImportSource::FileZilla, &path, &taken).unwrap();
    assert!(second.rows.iter().all(|row| row.verdict == Verdict::Exists));
    assert_eq!(second.new_count(), 0);
}

#[test]
fn a_winscp_file_becomes_a_plan() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "WinSCP.ini", "[Sessions\\Work/Box]\nHostName=h\nUserName=u\n");
    let plan = plan(ImportSource::WinScp, &path, &HashSet::new()).unwrap();
    assert_eq!(names(&plan), ["Box"]);
}

#[test]
fn an_oversized_file_is_too_large_without_being_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.xml");
    fs::File::create(&path).unwrap().set_len(MAX_INPUT_BYTES as u64 + 1).unwrap();
    assert_eq!(plan(ImportSource::FileZilla, &path, &HashSet::new()), Err(ImportError::TooLarge));
}

#[test]
fn a_missing_file_is_unreadable_with_a_short_message() {
    let dir = tempfile::tempdir().unwrap();
    let error = plan(ImportSource::FileZilla, &dir.path().join("nope.xml"), &HashSet::new()).unwrap_err();
    assert_eq!(error, ImportError::Unreadable("no such file or folder".to_string()));
}

#[test]
fn a_folder_given_for_a_single_file_source_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let error = plan(ImportSource::WinScp, dir.path(), &HashSet::new()).unwrap_err();
    assert!(matches!(error, ImportError::Unreadable(_)), "{error:?}");
}

#[test]
fn a_utf16_file_is_not_a_utf8_text_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sitemanager.xml");
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(FILEZILLA.encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        plan(ImportSource::FileZilla, &path, &HashSet::new()),
        Err(ImportError::WrongFormat("This is not a UTF-8 text file"))
    );
}

#[test]
fn the_wrong_kind_of_file_is_a_wrong_format_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "x.ini", "[Sessions\\A]\nHostName=h\n");
    assert!(matches!(plan(ImportSource::FileZilla, &path, &HashSet::new()), Err(ImportError::WrongFormat(_))));
}

#[test]
fn a_putty_folder_reads_every_session_file() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "My%20Box", "HostName=h1\nUserName=u\n");
    write(dir.path(), "Another", "HostName=h2\nUserName=u\nProtocol=telnet\n");
    write(dir.path(), "Default%20Settings", "HostName=ignored\n");
    fs::create_dir(dir.path().join("subfolder")).unwrap();
    let plan = plan(ImportSource::Putty, dir.path(), &HashSet::new()).unwrap();
    assert_eq!(names(&plan), ["Another", "My Box"]);
    assert!(matches!(plan.rows[0].verdict, Verdict::Unsupported(_)));
    assert_eq!(plan.rows[1].verdict, Verdict::New);
}

#[test]
fn a_putty_session_file_that_is_too_big_is_listed_as_invalid() {
    let dir = tempfile::tempdir().unwrap();
    fs::File::create(dir.path().join("Huge")).unwrap().set_len(2 * 1024 * 1024).unwrap();
    write(dir.path(), "Fine", "HostName=h\nUserName=u\n");
    let plan = plan(ImportSource::Putty, dir.path(), &HashSet::new()).unwrap();
    assert_eq!(names(&plan), ["Fine", "Huge"]);
    assert_eq!(plan.rows[1].verdict, Verdict::Invalid("could not be read".to_string()));
}

#[test]
fn a_putty_folder_with_too_many_files_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    for number in 0..=10_000 {
        fs::File::create(dir.path().join(format!("s{number}"))).unwrap();
    }
    assert!(matches!(plan(ImportSource::Putty, dir.path(), &HashSet::new()), Err(ImportError::WrongFormat(_))));
}

#[test]
fn an_empty_putty_folder_says_there_are_no_sessions() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        plan(ImportSource::Putty, dir.path(), &HashSet::new()),
        Err(ImportError::WrongFormat("There are no PuTTY sessions in this folder"))
    );
}

#[test]
fn a_putty_reg_export_in_utf16_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let text = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Box]\r\n\"HostName\"=\"h\"\r\n\"UserName\"=\"u\"\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let path = dir.path().join("putty.reg");
    fs::write(&path, bytes).unwrap();
    let plan = plan(ImportSource::Putty, &path, &HashSet::new()).unwrap();
    assert_eq!(names(&plan), ["Box"]);
}
