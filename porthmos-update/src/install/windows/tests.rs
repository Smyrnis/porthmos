use std::{cell::Cell, fs, io, time::Instant};

use super::*;

const HOME: Option<&str> = Some(r"C:\Users\me");

fn managed(path: &str) -> bool {
    managed_reason(path, HOME).is_some()
}

#[test]
fn a_verbatim_cargo_bin_path_is_managed() {
    let reason = managed_reason(r"\\?\C:\Users\me\.cargo\bin\porthmos.exe", HOME).unwrap();

    assert!(reason.contains("cargo"), "{reason}");
}

#[test]
fn program_files_is_managed_in_any_letter_case() {
    assert!(managed(r"C:\Program Files\Porthmos\porthmos.exe"));
    assert!(managed(r"c:\PROGRAM FILES\Porthmos\porthmos.exe"));
    assert!(managed(r"C:\Program Files (x86)\Porthmos\porthmos.exe"));
    assert!(managed(r"\\?\C:\Program Files\Porthmos\porthmos.exe"));
}

#[test]
fn store_scoop_winget_and_chocolatey_copies_are_managed() {
    assert!(managed(r"C:\Program Files\WindowsApps\Porthmos_1.0\porthmos.exe"));
    assert!(managed(r"C:\Users\me\scoop\apps\porthmos\current\porthmos.exe"));
    assert!(managed(r"C:\Users\me\AppData\Local\Microsoft\WinGet\Packages\Porthmos\porthmos.exe"));
    assert!(managed(r"C:\ProgramData\chocolatey\lib\porthmos\tools\porthmos.exe"));
}

#[test]
fn a_per_user_folder_is_not_managed() {
    assert!(!managed(r"C:\Users\me\AppData\Local\Porthmos\porthmos.exe"));
    assert!(!managed(r"D:\tools\porthmos.exe"));
    assert!(!managed(r"C:\Users\me\Program Files Backup\porthmos.exe"));
}

#[test]
fn forward_slashes_are_treated_like_backslashes() {
    assert!(managed("C:/Program Files/Porthmos/porthmos.exe"));
    assert!(managed("C:/Users/me/.cargo/bin/porthmos.exe"));
}

#[test]
fn a_home_with_a_trailing_separator_still_finds_the_cargo_bin() {
    assert!(managed_reason(r"C:\Users\me\.cargo\bin\porthmos.exe", Some(r"C:\Users\me\")).is_some());
}

#[test]
fn the_cargo_bin_of_another_user_is_not_this_users() {
    assert!(!managed(r"C:\Users\other\.cargo\bin\porthmos.exe"));
}

fn in_a_minute() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

struct Folder {
    root: tempfile::TempDir,
}

impl Folder {
    fn new() -> Self {
        Folder { root: tempfile::tempdir().unwrap() }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
}

#[test]
fn renaming_puts_the_new_file_in_place_and_keeps_the_old_one() {
    let folder = Folder::new();
    let (real, pending) = (folder.write("porthmos.exe", b"old"), folder.write("pending", b"new"));
    let old = folder.path("porthmos.exe.old");

    replace_by_renaming(&pending, &real, &old, in_a_minute()).unwrap();

    assert_eq!(fs::read(&real).unwrap(), b"new");
    assert_eq!(fs::read(&old).unwrap(), b"old");
    assert!(!pending.exists());
}

#[test]
fn a_stale_old_file_is_replaced() {
    let folder = Folder::new();
    let (real, pending) = (folder.write("porthmos.exe", b"current"), folder.write("pending", b"new"));
    let old = folder.write("porthmos.exe.old", b"stale");

    replace_by_renaming(&pending, &real, &old, in_a_minute()).unwrap();

    assert_eq!(fs::read(&old).unwrap(), b"current");
}

#[test]
fn a_failed_second_rename_puts_the_original_back() {
    let folder = Folder::new();
    let real = folder.write("porthmos.exe", b"original");
    let (pending, old) = (folder.path("missing"), folder.path("porthmos.exe.old"));

    assert!(replace_by_renaming(&pending, &real, &old, in_a_minute()).is_err());

    assert_eq!(fs::read(&real).unwrap(), b"original");
    assert!(!old.exists());
}

#[test]
fn a_busy_file_is_retried_until_it_frees_up() {
    let folder = Folder::new();
    let (real, pending) = (folder.write("porthmos.exe", b"old"), folder.write("pending", b"new"));
    let old = folder.path("porthmos.exe.old");
    let refusals = Cell::new(0);
    let rename = |from: &Path, to: &Path| {
        if refusals.get() < 2 {
            refusals.set(refusals.get() + 1);
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        fs::rename(from, to)
    };

    replace_by_renaming_with(&pending, &real, &old, &rename, Duration::ZERO, in_a_minute()).unwrap();

    assert_eq!(refusals.get(), 2);
    assert_eq!(fs::read(&real).unwrap(), b"new");
}

#[test]
fn a_sharing_violation_is_retried_like_a_permission_denied() {
    let folder = Folder::new();
    let (real, pending) = (folder.write("porthmos.exe", b"old"), folder.write("pending", b"new"));
    let old = folder.path("porthmos.exe.old");
    let refusals = Cell::new(0);
    let rename = |from: &Path, to: &Path| {
        if refusals.get() < 2 {
            refusals.set(refusals.get() + 1);
            return Err(io::Error::from_raw_os_error([32, 33][refusals.get() - 1]));
        }
        fs::rename(from, to)
    };

    replace_by_renaming_with(&pending, &real, &old, &rename, Duration::ZERO, in_a_minute()).unwrap();

    assert_eq!(refusals.get(), 2);
    assert_eq!(fs::read(&real).unwrap(), b"new");
}

#[test]
fn a_file_that_stays_busy_gives_up_at_the_deadline_and_the_original_stays() {
    let folder = Folder::new();
    let (real, pending) = (folder.write("porthmos.exe", b"old"), folder.write("pending", b"new"));
    let old = folder.path("porthmos.exe.old");
    let rename = |_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::PermissionDenied));
    let deadline = Instant::now() + Duration::from_millis(30);

    let result = replace_by_renaming_with(&pending, &real, &old, &rename, Duration::from_millis(1), deadline);

    assert_eq!(result, Err(InstallError::TimedOut));
    assert_eq!(fs::read(&real).unwrap(), b"old");
}

#[test]
fn a_stale_old_file_that_cannot_be_removed_is_reported_by_name() {
    let folder = Folder::new();
    let (real, pending) = (folder.write("porthmos.exe", b"current"), folder.write("pending", b"new"));
    let old = folder.path("porthmos.exe.old");
    fs::create_dir(&old).unwrap();

    let result = replace_by_renaming(&pending, &real, &old, in_a_minute());

    let Err(InstallError::Io(message)) = result else { panic!("{result:?}") };
    assert!(message.contains("porthmos.exe.old") && message.contains("other Porthmos"), "{message}");
    assert_eq!(fs::read(&real).unwrap(), b"current");
}

#[test]
fn rolling_back_still_runs_after_the_deadline_has_passed() {
    let folder = Folder::new();
    let real = folder.write("porthmos.exe", b"original");
    let (pending, old) = (folder.path("missing"), folder.path("porthmos.exe.old"));

    let result = replace_by_renaming(&pending, &real, &old, Instant::now());

    assert!(result.is_err());
    assert_eq!(fs::read(&real).unwrap(), b"original");
}

#[test]
fn a_double_failure_says_where_the_previous_version_is() {
    let folder = Folder::new();
    let (real, pending) = (folder.write("porthmos.exe", b"original"), folder.write("pending", b"new"));
    let old = folder.path("porthmos.exe.old");
    let calls = Cell::new(0);
    let rename = |from: &Path, to: &Path| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 { fs::rename(from, to) } else { Err(io::Error::other("disk on fire")) }
    };

    let result = replace_by_renaming_with(&pending, &real, &old, &rename, Duration::ZERO, in_a_minute());

    let Err(InstallError::Io(message)) = result else { panic!("{result:?}") };
    assert!(message.contains("porthmos.exe.old") && message.contains("put"), "{message}");
    assert_eq!(fs::read(&old).unwrap(), b"original");
}
