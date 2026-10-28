use crate::VfsPath;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;
use crate::{
    Answer, ErrorKind, FileKind, FileSystem, Prompter, Protocol, Question, SearchEvent, SearchQuery, Target,
    walk_search,
};

#[tokio::test]
async fn stat_follows_symlinks_and_read_dir_does_not() {
    let fs = FakeFs::new();
    fs.file("/data/a.txt", b"hello", Some(10)).symlink("/data/link", "/data/a.txt");

    assert_eq!(fs.stat(&VfsPath::new("/data/link")).await.unwrap().size, 5);
    let items = fs.read_dir(&VfsPath::new("/data")).await.unwrap();
    let link = items.iter().find(|item| item.name == "link").unwrap();
    assert_eq!(link.metadata.kind, FileKind::Symlink);
}

#[tokio::test]
async fn open_write_at_an_offset_truncates_then_appends() {
    let fs = FakeFs::new();
    fs.file("/f.part", b"abcdef", None);
    let mut writer = fs.open_write(&VfsPath::new("/f.part"), 3).await.unwrap();
    assert_eq!(writer.offset, 3);
    writer.stream.write_all(b"XY").await.unwrap();
    writer.stream.shutdown().await.unwrap();
    assert_eq!(fs.contents("/f.part").unwrap(), b"abcXY");
}

#[tokio::test]
async fn open_write_on_a_missing_file_starts_at_zero() {
    let fs = FakeFs::new();
    fs.dir("/d");
    let writer = fs.open_write(&VfsPath::new("/d/new"), 5).await.unwrap();
    assert_eq!(writer.offset, 0);
}

#[tokio::test]
async fn open_read_starts_at_the_offset() {
    let fs = FakeFs::new();
    fs.file("/f", b"abcdef", None);
    let mut reader = fs.open_read(&VfsPath::new("/f"), 2).await.unwrap();
    let mut read = Vec::new();
    reader.read_to_end(&mut read).await.unwrap();
    assert_eq!(read, b"cdef");
}

#[tokio::test]
async fn a_failing_read_errors_while_copying() {
    let fs = FakeFs::new();
    fs.file("/f", b"abc", None).fail_reads("/f");
    let mut reader = fs.open_read(&VfsPath::new("/f"), 0).await.unwrap();
    let mut read = Vec::new();
    assert!(reader.read_to_end(&mut read).await.is_err());
}

#[tokio::test]
async fn rename_replaces_an_existing_target() {
    let fs = FakeFs::new();
    fs.file("/a", b"new", None).file("/b", b"old", None);
    fs.rename(&VfsPath::new("/a"), &VfsPath::new("/b")).await.unwrap();
    assert_eq!(fs.contents("/b").unwrap(), b"new");
    assert!(!fs.exists("/a"));
}

#[tokio::test]
async fn delete_removes_a_directory_tree_without_following_symlinks() {
    let fs = FakeFs::new();
    fs.file("/keep/file", b"x", None).file("/gone/inner/file", b"y", None).symlink("/gone/link", "/keep");
    fs.delete(&VfsPath::new("/gone")).await.unwrap();
    assert!(!fs.exists("/gone/inner/file"));
    assert!(fs.exists("/keep/file"));
}

#[tokio::test]
async fn a_failing_read_dir_reports_permission_denied() {
    let fs = FakeFs::new();
    fs.dir("/locked").fail_read_dir("/locked");
    let error = fs.read_dir(&VfsPath::new("/locked")).await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
}

#[tokio::test]
async fn list_resolves_symlinked_directories_as_directories() {
    let fs = FakeFs::new();
    fs.dir("/root/real").symlink("/root/alias", "/root/real");
    let entries = fs.list(&VfsPath::new("/root")).await.unwrap();
    assert!(entries.iter().all(|entry| entry.is_dir));
}

#[tokio::test]
async fn walk_does_not_descend_into_symlinked_directories() {
    let fs = FakeFs::new();
    fs.file("/root/real/match.txt", b"", None).symlink("/root/alias", "/root/real");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    walk_search(&fs, SearchQuery::new("/root".into(), "*match*".into()), tx, Default::default()).await;
    let mut found = Vec::new();
    while let Ok(SearchEvent::Found(entry)) = rx.try_recv() {
        found.push(entry.path);
    }
    assert_eq!(found, vec![VfsPath::new("/root/real/match.txt")]);
}

#[tokio::test]
async fn list_reports_the_modified_time_set_on_a_file_and_none_for_unknown() {
    let fs = FakeFs::new();
    fs.file("/root/dated.txt", b"x", Some(1_700_000_000)).file("/root/undated.txt", b"x", None).dir("/root/sub");

    let mut entries = fs.list(&VfsPath::new("/root")).await.unwrap();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let times: Vec<(&str, Option<u64>)> = entries.iter().map(|e| (e.name.as_str(), e.modified)).collect();
    assert_eq!(times, vec![("dated.txt", Some(1_700_000_000)), ("sub", None), ("undated.txt", None)]);
}

#[tokio::test]
async fn a_search_result_carries_the_modified_time_of_the_listing() {
    let fs = FakeFs::new();
    fs.file("/root/match.txt", b"", Some(1_700_000_000));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    walk_search(&fs, SearchQuery::new("/root".into(), "*match*".into()), tx, Default::default()).await;

    let Ok(SearchEvent::Found(entry)) = rx.try_recv() else { panic!("no match found") };

    assert_eq!(entry.modified, Some(1_700_000_000));
}

struct Scripted(Option<Answer>);

#[async_trait::async_trait]
impl Prompter for Scripted {
    async fn ask(&mut self, _question: Question) -> Option<Answer> {
        self.0.take()
    }
}

fn target(password: Option<&str>) -> Target {
    Target {
        name: "srv".into(),
        host: "h".into(),
        port: 2222,
        username: "u".into(),
        password: password.map(str::to_string),
        options: Default::default(),
    }
}

#[tokio::test]
async fn fake_protocol_asks_for_a_missing_password_and_reports_cancel() {
    let protocol = FakeProtocol::new(FakeFs::new()).requiring_password("s3cret");
    let error = protocol.connect(&target(None), &mut Scripted(None)).await.err().unwrap();
    assert_eq!(error.kind(), ErrorKind::Cancelled);
}

#[tokio::test]
async fn fake_protocol_rejects_a_wrong_password() {
    let protocol = FakeProtocol::new(FakeFs::new()).requiring_password("s3cret");
    let answer = Some(Answer::Password("nope".into()));
    let error = protocol.connect(&target(None), &mut Scripted(answer)).await.err().unwrap();
    assert_eq!(error.kind(), ErrorKind::AuthRejected);
}

#[tokio::test]
async fn fake_protocol_accepts_a_saved_password_without_asking() {
    let protocol = FakeProtocol::new(FakeFs::new()).requiring_password("s3cret");
    assert!(protocol.connect(&target(Some("s3cret")), &mut Scripted(None)).await.is_ok());
}

#[tokio::test]
async fn a_failing_home_reports_permission_denied() {
    let fs = FakeFs::new();
    fs.fail_home();
    let error = fs.home().await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
}

#[test]
fn a_fake_protocol_can_take_another_id_and_form() {
    let mut form = crate::ConnectionForm::standard(21);
    form.host.label = "Server";
    let protocol = FakeProtocol::new(FakeFs::new()).with_id("ftp").with_form(form.clone());

    assert_eq!(protocol.id(), "ftp");
    assert_eq!(protocol.connection_form(), form);
}

#[tokio::test]
async fn a_writer_on_a_failing_shutdown_path_errors_on_shutdown() {
    let fs = FakeFs::new();
    fs.fail_shutdown("/f");
    let mut writer = fs.open_write(&VfsPath::new("/f"), 0).await.unwrap();
    writer.stream.write_all(b"abc").await.unwrap();
    assert!(writer.stream.shutdown().await.is_err());
}

#[tokio::test]
async fn a_sized_write_writes_normally_and_records_the_size() {
    let fs = FakeFs::new();
    fs.dir("/d");

    let mut writer = fs.open_write_sized(&VfsPath::new("/d/f"), 0, 3).await.unwrap();
    writer.stream.write_all(b"abc").await.unwrap();
    writer.stream.shutdown().await.unwrap();

    assert_eq!(fs.contents("/d/f").unwrap(), b"abc");
    assert_eq!(fs.written_sizes(), vec![(VfsPath::new("/d/f"), 3)]);
}

struct OnlyWrites(FakeFs);

#[async_trait::async_trait]
impl FileSystem for OnlyWrites {
    async fn list(&self, dir: &VfsPath) -> Result<Vec<crate::Entry>, crate::ProtocolError> {
        self.0.list(dir).await
    }

    async fn read_dir(&self, dir: &VfsPath) -> Result<Vec<crate::DirItem>, crate::ProtocolError> {
        self.0.read_dir(dir).await
    }

    async fn stat(&self, path: &VfsPath) -> Result<crate::Metadata, crate::ProtocolError> {
        self.0.stat(path).await
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), crate::ProtocolError> {
        self.0.create_dir(path).await
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), crate::ProtocolError> {
        self.0.rename(from, to).await
    }

    async fn remove_file(&self, path: &VfsPath) -> Result<(), crate::ProtocolError> {
        self.0.remove_file(path).await
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), crate::ProtocolError> {
        self.0.delete(path).await
    }

    async fn home(&self) -> Result<VfsPath, crate::ProtocolError> {
        self.0.home().await
    }

    async fn open_read(&self, path: &VfsPath, offset: u64) -> Result<crate::Reader, crate::ProtocolError> {
        self.0.open_read(path, offset).await
    }

    async fn open_write(&self, path: &VfsPath, offset: u64) -> Result<crate::Writer, crate::ProtocolError> {
        self.0.open_write(path, offset).await
    }
}

#[tokio::test]
async fn by_default_a_sized_write_is_a_plain_write() {
    let fs = FakeFs::new();
    fs.dir("/d");
    let plain = OnlyWrites(fs.clone());

    let mut writer = plain.open_write_sized(&VfsPath::new("/d/f"), 0, 99).await.unwrap();
    writer.stream.write_all(b"xy").await.unwrap();
    writer.stream.shutdown().await.unwrap();

    assert_eq!(fs.contents("/d/f").unwrap(), b"xy");
    assert!(fs.written_sizes().is_empty());
}

#[test]
fn by_default_a_file_system_has_no_transfer_limit() {
    assert_eq!(OnlyWrites(FakeFs::new()).transfer_limit(), None);
}

#[tokio::test]
async fn the_fake_protocol_remembers_the_target_it_was_given() {
    let protocol = FakeProtocol::new(FakeFs::new());
    assert!(protocol.seen_target().is_none());

    protocol.connect(&target(Some("pw")), &mut Scripted(None)).await.unwrap();

    assert_eq!(protocol.seen_target().and_then(|seen| seen.password).as_deref(), Some("pw"));
}

#[test]
fn with_discovered_targets_are_returned_by_discover() {
    let protocol = FakeProtocol::new(FakeFs::new()).with_discovered(vec![target(None)]);

    let names: Vec<String> =
        protocol.discover(&crate::Environment::default()).unwrap().into_iter().map(|found| found.name).collect();

    assert_eq!(names, vec![target(None).name]);
}

#[tokio::test]
async fn set_modified_changes_the_time_stat_reports() {
    let fs = FakeFs::new();
    fs.file("/home/user/a", b"x", Some(10));

    fs.set_modified(&VfsPath::new("/home/user/a"), 99).await.unwrap();

    assert_eq!(fs.stat(&VfsPath::new("/home/user/a")).await.unwrap().modified, Some(99));
    assert_eq!(fs.modified_of("/home/user/a"), Some(99));
}

#[tokio::test]
async fn set_modified_on_a_missing_file_is_not_found() {
    let fs = FakeFs::new();

    let error = fs.set_modified(&VfsPath::new("/home/user/missing"), 5).await.unwrap_err();

    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn a_fake_without_time_support_refuses_and_says_so() {
    let fs = FakeFs::new().with_time_support(false);
    fs.file("/home/user/a", b"x", Some(10));

    let error = fs.set_modified(&VfsPath::new("/home/user/a"), 5).await.unwrap_err();

    assert!(!fs.can_set_modified());
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert_eq!(fs.modified_of("/home/user/a"), Some(10));
}

#[test]
fn a_fake_supports_times_by_default_with_a_two_second_resolution() {
    let fs = FakeFs::new();

    assert!(fs.can_set_modified());
    assert_eq!(fs.time_resolution(), 2);
    assert_eq!(FakeFs::new().with_time_resolution(60).time_resolution(), 60);
}

#[tokio::test]
async fn a_filesystem_has_no_server_info_by_default() {
    struct Plain;
    #[async_trait::async_trait]
    impl FileSystem for Plain {
        async fn list(&self, _: &VfsPath) -> Result<Vec<crate::Entry>, crate::ProtocolError> {
            Ok(Vec::new())
        }

        async fn read_dir(&self, _: &VfsPath) -> Result<Vec<crate::DirItem>, crate::ProtocolError> {
            Ok(Vec::new())
        }

        async fn stat(&self, _: &VfsPath) -> Result<crate::Metadata, crate::ProtocolError> {
            Err(crate::ProtocolError::new(ErrorKind::NotFound, anyhow::anyhow!("none")))
        }

        async fn create_dir(&self, _: &VfsPath) -> Result<(), crate::ProtocolError> {
            Ok(())
        }

        async fn rename(&self, _: &VfsPath, _: &VfsPath) -> Result<(), crate::ProtocolError> {
            Ok(())
        }

        async fn remove_file(&self, _: &VfsPath) -> Result<(), crate::ProtocolError> {
            Ok(())
        }

        async fn delete(&self, _: &VfsPath) -> Result<(), crate::ProtocolError> {
            Ok(())
        }

        async fn home(&self) -> Result<VfsPath, crate::ProtocolError> {
            Ok("/".into())
        }

        async fn open_read(&self, _: &VfsPath, _: u64) -> Result<crate::Reader, crate::ProtocolError> {
            Err(crate::ProtocolError::new(ErrorKind::Unsupported, anyhow::anyhow!("none")))
        }

        async fn open_write(&self, _: &VfsPath, _: u64) -> Result<crate::Writer, crate::ProtocolError> {
            Err(crate::ProtocolError::new(ErrorKind::Unsupported, anyhow::anyhow!("none")))
        }
    }

    assert_eq!(Plain.server_info(), None);
}

#[test]
fn the_fake_filesystem_returns_the_configured_server_info() {
    let fs = FakeFs::new();
    assert_eq!(fs.server_info(), None);

    fs.with_server_info("OpenSSH_9.6");

    assert_eq!(fs.server_info().as_deref(), Some("OpenSSH_9.6"));
    assert_eq!(fs.clone().server_info().as_deref(), Some("OpenSSH_9.6"));
}

#[test]
fn the_hook_is_object_safe_through_arc_dyn_filesystem() {
    let fs = FakeFs::new();
    fs.with_server_info("vsFTPd 3.0");
    let shared: std::sync::Arc<dyn FileSystem> = std::sync::Arc::new(fs);

    assert_eq!(shared.server_info().as_deref(), Some("vsFTPd 3.0"));
}

#[tokio::test]
async fn a_denied_stat_is_a_permission_error_and_a_hanging_one_never_returns() {
    let fs = FakeFs::new();
    fs.dir("/a").dir("/b").denying_stat("/a").hanging_stat("/b");

    assert_eq!(fs.stat(&VfsPath::new("/a")).await.unwrap_err().kind(), ErrorKind::PermissionDenied);
    let hung = tokio::time::timeout(std::time::Duration::from_millis(20), fs.stat(&VfsPath::new("/b"))).await;
    assert!(hung.is_err());
}

#[test]
fn handles_count_the_live_clones_of_the_filesystem() {
    let fs = FakeFs::new();
    let before = fs.handles();
    let clone = fs.clone();
    assert_eq!(fs.handles(), before + 1);
    drop(clone);
    assert_eq!(fs.handles(), before);
}

#[tokio::test]
async fn a_declined_trust_question_cancels_the_fake_connect() {
    struct Declining(Vec<Question>);
    #[async_trait::async_trait]
    impl Prompter for Declining {
        async fn ask(&mut self, question: Question) -> Option<Answer> {
            self.0.push(question);
            None
        }
    }
    let target = Target {
        name: "t".into(),
        host: "h".into(),
        port: 22,
        username: "u".into(),
        password: None,
        options: Default::default(),
    };
    let mut prompter = Declining(Vec::new());

    let host_key = FakeProtocol::new(FakeFs::new()).asking_to_trust_host_key("ssh-ed25519", "SHA256:abc");
    assert_eq!(host_key.connect(&target, &mut prompter).await.err().unwrap().kind(), ErrorKind::Cancelled);
    let certificate = FakeProtocol::new(FakeFs::new()).asking_to_trust_certificate("AA:BB", "CN=x");
    assert_eq!(certificate.connect(&target, &mut prompter).await.err().unwrap().kind(), ErrorKind::Cancelled);

    assert!(matches!(prompter.0[0], Question::TrustHostKey { .. }));
    assert!(matches!(prompter.0[1], Question::TrustCertificate { .. }));
}

#[tokio::test]
async fn failing_with_uses_the_given_kind() {
    let protocol = FakeProtocol::new(FakeFs::new()).failing_with(ErrorKind::AuthRejected, "no");
    let target = Target {
        name: "t".into(),
        host: "h".into(),
        port: 22,
        username: "u".into(),
        password: None,
        options: Default::default(),
    };
    struct Never;
    #[async_trait::async_trait]
    impl Prompter for Never {
        async fn ask(&mut self, _: Question) -> Option<Answer> {
            None
        }
    }

    assert_eq!(protocol.connect(&target, &mut Never).await.err().unwrap().kind(), ErrorKind::AuthRejected);
}

#[tokio::test]
async fn remove_dir_removes_an_empty_folder() {
    let fs = FakeFs::new();
    fs.dir("/data/empty");

    fs.remove_dir(&VfsPath::new("/data/empty")).await.unwrap();

    assert!(!fs.exists("/data/empty"));
    assert!(fs.exists("/data"));
}

#[tokio::test]
async fn remove_dir_refuses_a_folder_with_a_file_and_keeps_it() {
    let fs = FakeFs::new();
    fs.file("/data/keep.txt", b"x", None);

    assert!(fs.remove_dir(&VfsPath::new("/data")).await.is_err());

    assert!(fs.exists("/data/keep.txt"));
}

#[tokio::test]
async fn remove_dir_refuses_a_folder_that_holds_an_empty_folder() {
    let fs = FakeFs::new();
    fs.dir("/data/inner");

    assert!(fs.remove_dir(&VfsPath::new("/data")).await.is_err());

    assert!(fs.exists("/data/inner"));
}

#[tokio::test]
async fn remove_dir_refuses_a_folder_that_holds_a_symlink() {
    let fs = FakeFs::new();
    fs.file("/elsewhere/a.txt", b"x", None).symlink("/data/link", "/elsewhere/a.txt");

    assert!(fs.remove_dir(&VfsPath::new("/data")).await.is_err());

    assert!(fs.exists("/data/link") && fs.exists("/elsewhere/a.txt"));
}

#[tokio::test]
async fn remove_dir_of_a_missing_folder_is_not_found() {
    let fs = FakeFs::new();

    let err = fs.remove_dir(&VfsPath::new("/nope")).await.unwrap_err();

    assert_eq!(err.kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn remove_dir_never_removes_a_file() {
    let fs = FakeFs::new();
    fs.file("/data/a.txt", b"x", None);

    assert!(fs.remove_dir(&VfsPath::new("/data/a.txt")).await.is_err());

    assert!(fs.exists("/data/a.txt"));
}

struct Bare;

#[async_trait::async_trait]
impl FileSystem for Bare {
    async fn list(&self, _dir: &VfsPath) -> Result<Vec<crate::Entry>, crate::ProtocolError> {
        Ok(Vec::new())
    }
    async fn read_dir(&self, _dir: &VfsPath) -> Result<Vec<crate::DirItem>, crate::ProtocolError> {
        Ok(Vec::new())
    }
    async fn stat(&self, _path: &VfsPath) -> Result<crate::Metadata, crate::ProtocolError> {
        Ok(crate::Metadata { kind: FileKind::Dir, size: 0, modified: None, permissions: None })
    }
    async fn create_dir(&self, _path: &VfsPath) -> Result<(), crate::ProtocolError> {
        Ok(())
    }
    async fn rename(&self, _from: &VfsPath, _to: &VfsPath) -> Result<(), crate::ProtocolError> {
        Ok(())
    }
    async fn remove_file(&self, _path: &VfsPath) -> Result<(), crate::ProtocolError> {
        Ok(())
    }
    async fn delete(&self, _path: &VfsPath) -> Result<(), crate::ProtocolError> {
        panic!("a default remove_dir must never fall back to the recursive delete");
    }
    async fn home(&self) -> Result<VfsPath, crate::ProtocolError> {
        Ok(VfsPath::new("/"))
    }
    async fn open_read(&self, _path: &VfsPath, _offset: u64) -> Result<crate::Reader, crate::ProtocolError> {
        Err(crate::ProtocolError::new(ErrorKind::Unsupported, anyhow::anyhow!("no reads")))
    }
    async fn open_write(&self, _path: &VfsPath, _offset: u64) -> Result<crate::Writer, crate::ProtocolError> {
        Err(crate::ProtocolError::new(ErrorKind::Unsupported, anyhow::anyhow!("no writes")))
    }
}

#[tokio::test]
async fn a_filesystem_without_its_own_remove_dir_refuses_instead_of_deleting_recursively() {
    let err = Bare.remove_dir(&VfsPath::new("/empty")).await.unwrap_err();

    assert_eq!(err.kind(), ErrorKind::Unsupported);
}

#[tokio::test]
async fn a_hanging_read_gives_the_first_bytes_then_never_answers() {
    use tokio::io::AsyncReadExt;
    let fs = FakeFs::new();
    fs.file("/d/a.txt", b"abcdef", None).hanging_reads("/d/a.txt", 3);
    let mut reader = fs.open_read(&VfsPath::new("/d/a.txt"), 0).await.unwrap();
    let mut first = [0u8; 8];

    let got = reader.read(&mut first).await.unwrap();
    let stalled = tokio::time::timeout(std::time::Duration::from_millis(100), reader.read(&mut first)).await;

    assert_eq!(&first[..got], b"abc");
    assert!(stalled.is_err());
}

#[tokio::test]
async fn free_space_is_unknown_until_configured() {
    let fs = FakeFs::new();

    assert_eq!(fs.free_space(&VfsPath::new("/home/user")).await.unwrap(), None);
}

#[tokio::test]
async fn configured_free_space_is_reported() {
    let fs = FakeFs::new();
    fs.with_free_space(2_000);

    assert_eq!(fs.free_space(&VfsPath::new("/home/user")).await.unwrap(), Some(2_000));
}

#[tokio::test]
async fn free_space_can_fail() {
    let fs = FakeFs::new();
    fs.fail_free_space();

    assert!(fs.free_space(&VfsPath::new("/home/user")).await.is_err());
}

#[tokio::test]
async fn listing_names_what_a_symlink_points_to() {
    let fs = FakeFs::new();
    fs.file("/d/real.txt", b"x", None).symlink("/d/link", "/d/real.txt");

    let entries = fs.list(&VfsPath::new("/d")).await.unwrap();
    let link = entries.iter().find(|entry| entry.name == "link").unwrap();
    let real = entries.iter().find(|entry| entry.name == "real.txt").unwrap();

    assert_eq!(link.link_target.as_deref(), Some("/d/real.txt"));
    assert_eq!(real.link_target, None);
}

#[tokio::test]
async fn listing_carries_the_configured_owner_and_group() {
    let fs = FakeFs::new();
    fs.file("/d/a.txt", b"x", None).file("/d/b.txt", b"x", None).with_owner("/d/a.txt", "nikos", "staff");

    let entries = fs.list(&VfsPath::new("/d")).await.unwrap();
    let a = entries.iter().find(|entry| entry.name == "a.txt").unwrap();
    let b = entries.iter().find(|entry| entry.name == "b.txt").unwrap();

    assert_eq!((a.owner.as_deref(), a.group.as_deref()), (Some("nikos"), Some("staff")));
    assert_eq!((b.owner.as_deref(), b.group.as_deref()), (None, None));
}

#[tokio::test]
async fn a_hanging_free_space_query_never_answers() {
    let fs = FakeFs::new();
    fs.hanging_free_space();

    let answer =
        tokio::time::timeout(std::time::Duration::from_millis(100), fs.free_space(&VfsPath::new("/home/user"))).await;

    assert!(answer.is_err());
}

#[tokio::test]
async fn a_file_system_has_no_connection_to_watch_by_default() {
    assert_eq!(FakeFs::new().connection_alive().await, None);
}

#[tokio::test]
async fn a_fake_holding_a_connection_reports_it_alive() {
    assert_eq!(FakeFs::new().holding_connection().connection_alive().await, Some(true));
}

#[tokio::test]
async fn a_lost_connection_is_reported_until_it_is_restored() {
    let fs = FakeFs::new().holding_connection();

    fs.lose_connection();
    let lost = fs.connection_alive().await;
    fs.restore_connection();

    assert_eq!(lost, Some(false));
    assert_eq!(fs.connection_alive().await, Some(true));
}

#[tokio::test]
async fn losing_the_connection_leaves_ordinary_operations_alone() {
    let fs = FakeFs::new().holding_connection();
    fs.dir("/d");

    fs.lose_connection();

    assert!(fs.stat(&VfsPath::new("/d")).await.is_ok());
}

#[tokio::test]
async fn fake_fs_keeps_windows_looking_names_apart() {
    let fs = FakeFs::new();
    fs.dir("/srv").file("/srv/x:1.txt", b"1", None).file("/srv/a\\b", b"2", None).file("/srv/con", b"3", None);

    let mut entries = fs.list(&VfsPath::new("/srv")).await.unwrap();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let paths: Vec<String> = entries.iter().map(|entry| entry.path.to_string_lossy().into_owned()).collect();
    assert_eq!(paths, vec!["/srv/a\\b", "/srv/con", "/srv/x:1.txt"]);

    fs.delete(&VfsPath::new("/srv/a\\b")).await.unwrap();

    let mut names: Vec<String> = fs.list(&VfsPath::new("/srv")).await.unwrap().into_iter().map(|e| e.name).collect();
    names.sort();
    assert_eq!(names, vec!["con", "x:1.txt"]);
}

#[test]
fn a_fake_file_system_reports_the_rules_it_was_given() {
    assert_eq!(FakeFs::new().name_rules(), crate::NameRules::Permissive);
    assert_eq!(FakeFs::new().with_name_rules(crate::NameRules::Windows).name_rules(), crate::NameRules::Windows);
}
