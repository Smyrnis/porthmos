use std::fs;

use super::*;
use crate::testing::{archive_with, raw_archive, stub_archive};

const CAP: u64 = 1024 * 1024;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

struct Folder {
    root: tempfile::TempDir,
}

impl Folder {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("stage")).unwrap();
        Folder { root }
    }

    fn archive(&self, bytes: &[u8]) -> std::path::PathBuf {
        let path = self.root.path().join("archive.tar.gz");
        fs::write(&path, bytes).unwrap();
        path
    }

    fn destination(&self) -> std::path::PathBuf {
        self.root.path().join("stage").join("porthmos")
    }

    fn stage_listing(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.root.path().join("stage"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[cfg(unix)]
    fn script(&self, name: &str, body: &str, mode: u32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.path().join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }
}

#[cfg(unix)]
#[test]
fn the_binary_is_extracted_executable_and_the_license_is_ignored() {
    use std::os::unix::fs::PermissionsExt;

    use crate::testing::stub_script;
    let folder = Folder::new();
    let archive = folder.archive(&stub_archive("0.5.0", &[("LICENSE", b"MIT")]));

    extract_binary(&archive, &folder.destination(), CAP).unwrap();

    assert_eq!(fs::read(folder.destination()).unwrap(), stub_script("0.5.0"));
    assert_eq!(fs::metadata(folder.destination()).unwrap().permissions().mode() & 0o777, 0o755);
    assert_eq!(folder.stage_listing(), ["porthmos"]);
}

#[test]
fn a_member_with_a_path_traversal_name_is_never_written_outside() {
    let folder = Folder::new();
    let archive = folder.archive(&archive_with(&[
        ("../evil", b"bad"),
        ("/tmp/porthmos-evil-absolute", b"bad"),
        ("stage/../../evil2", b"bad"),
        ("porthmos", b"#!/bin/sh\n"),
    ]));

    extract_binary(&archive, &folder.destination(), CAP).unwrap();

    assert_eq!(folder.stage_listing(), ["porthmos"]);
    assert!(!folder.root.path().join("evil").exists());
    assert!(!folder.root.path().join("evil2").exists());
    assert!(!std::path::Path::new("/tmp/porthmos-evil-absolute").exists());
}

#[test]
fn a_linked_or_directory_member_named_porthmos_is_not_accepted() {
    let folder = Folder::new();
    let members: [(&str, &[u8], tar::EntryType, u32); 2] = [
        ("porthmos", b"/etc/passwd", tar::EntryType::Symlink, 0o777),
        ("porthmos/", b"", tar::EntryType::Directory, 0o755),
    ];
    let archive = folder.archive(&raw_archive(&members));

    let result = extract_binary(&archive, &folder.destination(), CAP);

    assert!(result.is_err());
    assert!(folder.stage_listing().is_empty());
}

#[test]
fn an_archive_without_the_binary_is_an_error_and_leaves_no_file() {
    let folder = Folder::new();
    let archive = folder.archive(&archive_with(&[("LICENSE", b"MIT"), ("other", b"x")]));

    let message = extract_binary(&archive, &folder.destination(), CAP).unwrap_err().to_string();

    assert!(message.contains("porthmos"), "{message}");
    assert!(folder.stage_listing().is_empty());
}

#[test]
fn a_member_over_the_size_cap_is_refused_and_removed() {
    let folder = Folder::new();
    let archive = folder.archive(&archive_with(&[("porthmos", &[b'x'; 5000])]));

    assert!(extract_binary(&archive, &folder.destination(), 10).is_err());
    assert!(folder.stage_listing().is_empty());
}

#[test]
fn a_decompression_bomb_before_the_binary_is_cut_off() {
    let folder = Folder::new();
    let bomb = vec![0u8; 20 * 1024 * 1024];
    let archive = folder.archive(&archive_with(&[("padding", &bomb), ("porthmos", b"#!/bin/sh\n")]));

    assert!(extract_binary(&archive, &folder.destination(), 1024).is_err());
    assert!(folder.stage_listing().is_empty());
}

#[test]
fn a_corrupt_archive_is_an_error_not_a_panic() {
    let folder = Folder::new();
    let good = stub_archive("0.5.0", &[]);

    for bytes in [vec![1u8, 2, 3, 4, 5], good[..good.len() / 2].to_vec(), Vec::new()] {
        let archive = folder.archive(&bytes);

        assert!(extract_binary(&archive, &folder.destination(), CAP).is_err());
        assert!(folder.stage_listing().is_empty());
    }
}

#[cfg(unix)]
#[test]
fn a_binary_that_prints_the_expected_version_passes() {
    let folder = Folder::new();
    let binary = folder.script("good", "#!/bin/sh\necho \"porthmos 0.5.0\"\n", 0o755);

    assert!(health_check(&binary, &version("0.5.0"), Duration::from_secs(5)).is_ok());
}

#[cfg(unix)]
#[test]
fn a_binary_that_prints_another_version_fails() {
    let folder = Folder::new();
    let binary = folder.script("other", "#!/bin/sh\necho \"porthmos 0.4.9\"\n", 0o755);

    assert!(health_check(&binary, &version("0.5.0"), Duration::from_secs(5)).is_err());
}

#[cfg(unix)]
#[test]
fn a_binary_that_exits_non_zero_fails() {
    let folder = Folder::new();
    let binary = folder.script("bad", "#!/bin/sh\necho \"porthmos 0.5.0\"\nexit 3\n", 0o755);

    assert!(health_check(&binary, &version("0.5.0"), Duration::from_secs(5)).is_err());
}

#[cfg(unix)]
#[test]
fn a_binary_that_hangs_is_killed_at_the_timeout() {
    use std::time::Instant;

    let folder = Folder::new();
    let binary = folder.script("hang", "#!/bin/sh\nexec sleep 30\n", 0o755);
    let started = Instant::now();

    let result = health_check(&binary, &version("0.5.0"), Duration::from_millis(300));

    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

#[cfg(unix)]
#[test]
fn a_file_that_is_not_executable_fails() {
    let folder = Folder::new();
    let binary = folder.script("plain", "#!/bin/sh\necho \"porthmos 0.5.0\"\n", 0o644);

    assert!(health_check(&binary, &version("0.5.0"), Duration::from_secs(5)).is_err());
}

#[cfg(unix)]
#[test]
fn a_binary_with_extra_output_fails() {
    let folder = Folder::new();
    let binary = folder.script("chatty", "#!/bin/sh\necho \"porthmos 0.5.0\"\necho more\n", 0o755);

    assert!(health_check(&binary, &version("0.5.0"), Duration::from_secs(5)).is_err());
}

#[test]
fn a_missing_binary_fails() {
    let folder = Folder::new();

    assert!(health_check(&folder.root.path().join("absent"), &version("0.5.0"), Duration::from_secs(1)).is_err());
}

#[test]
fn only_the_member_named_exactly_porthmos_is_the_program() {
    let folder = Folder::new();
    let archive = folder.archive(&archive_with(&[
        ("bin/porthmos", b"decoy in a subfolder"),
        ("../porthmos", b"decoy outside"),
        ("porthmos", b"the real one"),
    ]));

    extract_binary(&archive, &folder.destination(), CAP).unwrap();

    assert_eq!(fs::read(folder.destination()).unwrap(), b"the real one");
}

#[test]
fn an_archive_with_only_look_alike_members_has_no_program() {
    let folder = Folder::new();
    let archive = folder.archive(&archive_with(&[("bin/porthmos", b"decoy"), ("../porthmos", b"decoy")]));

    assert!(extract_binary(&archive, &folder.destination(), CAP).is_err());
    assert!(folder.stage_listing().is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn a_binary_that_is_still_being_closed_by_its_writer_is_retried() {
    use std::{io::Write, os::unix::fs::PermissionsExt, thread, time::Duration};
    let folder = Folder::new();
    let path = folder.root.path().join("fresh");
    let mut writer = fs::File::create(&path).unwrap();
    writer.write_all(b"#!/bin/sh\necho \"porthmos 0.5.0\"\n").unwrap();
    writer.set_permissions(fs::Permissions::from_mode(0o755)).unwrap();
    let closer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        drop(writer);
    });

    let result = health_check(&path, &version("0.5.0"), Duration::from_secs(5));

    closer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
}
