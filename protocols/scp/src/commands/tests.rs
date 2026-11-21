use porthmos_vfs::{ErrorKind, VfsPath};

use super::*;

#[test]
fn listing_commands_use_utc_and_the_c_locale() {
    let env = "TZ=UTC0 LC_ALL=C QUOTING_STYLE=literal TIME_STYLE=+%s";
    assert_eq!(list(&VfsPath::new("/srv/a b")), format!("{env} ls -lanL -- '/srv/a b/'"));
    assert_eq!(list(&VfsPath::new("/")), format!("{env} ls -lanL -- '/'"));
    assert_eq!(stat(&VfsPath::new("/it's")), format!(r"{env} ls -ldnL -- '/it'\''s'"));
}

#[test]
fn operations_quote_every_path_after_a_double_dash() {
    assert_eq!(mkdir(&VfsPath::new("/-x")), "mkdir -- '/-x'");
    assert_eq!(remove(&VfsPath::new("/$HOME")), "rm -- '/$HOME'");
    assert_eq!(remove_tree(&VfsPath::new("/`id`")), "rm -rf -- '/`id`'");
    assert_eq!(remove_dir(&VfsPath::new("/-d e")), "rmdir -- '/-d e'");
    assert_eq!(rename(&VfsPath::new("/a"), &VfsPath::new("/b c")), "mv -f -- '/a' '/b c'");
}

#[test]
fn transfer_commands() {
    assert_eq!(read_from(&VfsPath::new("/f"), 0), "cat -- '/f'");
    assert_eq!(read_from(&VfsPath::new("/f"), 10), "tail -c +11 -- '/f'");
    assert_eq!(scp_source(&VfsPath::new("/f")), "scp -f -- '/f'");
    assert_eq!(scp_sink(&VfsPath::new("/f")), ": > '/f' && scp -t -- '/f'");
    assert_eq!(append(&VfsPath::new("/f")), "cat >> '/f'");
    assert_eq!(create(&VfsPath::new("/f")), "cat > '/f'");
}

fn mapped(stderr: &str) -> (ErrorKind, String) {
    let error = failure(stderr.as_bytes(), Some(1), &VfsPath::new("/x"), "ls");
    (error.kind(), error.to_string())
}

#[test]
fn stderr_maps_to_error_kinds() {
    assert_eq!(
        mapped("ls: cannot access '/x': No such file or directory\n"),
        (ErrorKind::NotFound, "/x not found".to_string())
    );
    for text in
        ["rm: cannot remove '/x': Permission denied", "mv: Operation not permitted", "mkdir: Read-only file system"]
    {
        assert_eq!(mapped(text), (ErrorKind::PermissionDenied, "/x: permission denied".to_string()));
    }
    assert_eq!(
        mapped("mkdir: cannot create directory '/x': File exists"),
        (ErrorKind::Other, "/x already exists".to_string())
    );
    assert_eq!(mapped("something odd\nsecond line"), (ErrorKind::Other, "something odd".to_string()));
    assert_eq!(mapped(""), (ErrorKind::Other, "ls failed (exit 1)".to_string()));
}

#[test]
fn the_probe_marks_a_working_shell() {
    assert!(PROBE.starts_with("printf 'porthmos"));
}

#[test]
fn touch_tests_for_the_file_then_stamps_it_without_creating_one() {
    let quoted = shell_quote("/srv/it's a $dir/file name");

    let command = touch(&VfsPath::new("/srv/it's a $dir/file name"), 1_700_000_000);

    assert_eq!(command, format!("test -e {quoted} && touch -c -d @1700000000 -- {quoted}"));
}

#[test]
fn chmod_writes_the_mode_in_octal_and_quotes_the_path() {
    let quoted = shell_quote("/srv/it's a $dir/run.sh");

    let command = chmod(&VfsPath::new("/srv/it's a $dir/run.sh"), 0o755);

    assert_eq!(command, format!("chmod -- 00755 {quoted}"));
}

#[test]
fn chmod_keeps_leading_zeros_so_a_private_mode_is_not_misread() {
    assert_eq!(chmod(&VfsPath::new("/srv/f"), 0o600), "chmod -- 00600 '/srv/f'");
    assert_eq!(chmod(&VfsPath::new("/srv/f"), 0o007), "chmod -- 00007 '/srv/f'");
}

#[test]
fn chmod_sends_the_setuid_setgid_and_sticky_bits() {
    assert_eq!(chmod(&VfsPath::new("/srv/f"), 0o4755), "chmod -- 04755 '/srv/f'");
    assert_eq!(chmod(&VfsPath::new("/srv/f"), 0o2775), "chmod -- 02775 '/srv/f'");
    assert_eq!(chmod(&VfsPath::new("/srv/f"), 0o1777), "chmod -- 01777 '/srv/f'");
}

#[test]
fn chmod_has_five_digits_so_gnu_chmod_clears_the_special_bits_of_a_folder_too() {
    let command = chmod(&VfsPath::new("/srv/dir"), 0o755);

    let mode = command.split(' ').nth(2).unwrap();
    assert_eq!(mode.len(), 5, "{command}");
    assert!(mode.starts_with("00"), "{command}");
}

#[test]
fn chmod_ignores_bits_above_the_permission_bits() {
    assert_eq!(chmod(&VfsPath::new("/srv/f"), 0o100644), "chmod -- 00644 '/srv/f'");
}

#[test]
fn free_space_asks_df_for_posix_kilobytes_in_the_c_locale() {
    assert_eq!(free_space(&VfsPath::new("/srv/a b")), "LC_ALL=C df -Pk -- '/srv/a b'");
}

#[test]
fn chmod_ends_the_options_before_the_mode_because_bsd_reads_a_later_double_dash_as_a_file() {
    let command = chmod(&VfsPath::new("/srv/f"), 0o644);

    let mut words = command.split(' ');
    assert_eq!(words.next(), Some("chmod"));
    assert_eq!(words.next(), Some("--"));
}
