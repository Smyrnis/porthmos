use super::*;

fn version(major: u32, minor: u32) -> Option<SshVersion> {
    Some(SshVersion { major, minor })
}

#[test]
fn linux_output_is_read() {
    assert_eq!(parse_ssh_version("OpenSSH_9.6p1 Ubuntu-3ubuntu13, OpenSSL 3.0.13 30 Jan 2024"), version(9, 6));
}

#[test]
fn macos_output_is_read() {
    assert_eq!(parse_ssh_version("OpenSSH_9.7p1, LibreSSL 3.3.6"), version(9, 7));
    assert_eq!(parse_ssh_version("OpenSSH_8.1p1, LibreSSL 2.7.3"), version(8, 1));
}

#[test]
fn windows_output_is_read() {
    assert_eq!(parse_ssh_version("OpenSSH_for_Windows_9.5p1, LibreSSL 3.8.2"), version(9, 5));
}

#[test]
fn text_around_the_version_is_tolerated() {
    assert_eq!(parse_ssh_version("warning: something\nOpenSSH_8.4p1 Debian-5, OpenSSL 1.1.1k\n"), version(8, 4));
    assert_eq!(parse_ssh_version("OpenSSH_10.0p2, OpenSSL 3.5.0\r\n"), version(10, 0));
}

#[test]
fn a_version_without_a_minor_part_is_read_as_minor_zero() {
    assert_eq!(parse_ssh_version("OpenSSH_9p1"), version(9, 0));
}

#[test]
fn garbage_and_empty_output_give_nothing() {
    for text in ["", "ssh: command not found", "OpenSSH_", "OpenSSH_x.yp1", "dropbear v2022.83", "OpenSSH_for_Windows_"]
    {
        assert_eq!(parse_ssh_version(text), None, "{text:?}");
    }
}

#[test]
fn versions_compare_numerically_not_as_text() {
    assert!(SshVersion { major: 8, minor: 4 }.supports_askpass_password());
    assert!(SshVersion { major: 9, minor: 0 }.supports_askpass_password());
    assert!(SshVersion { major: 10, minor: 0 }.supports_askpass_password());
    assert!(SshVersion { major: 8, minor: 10 }.supports_askpass_password());
    assert!(!SshVersion { major: 8, minor: 3 }.supports_askpass_password());
    assert!(!SshVersion { major: 7, minor: 9 }.supports_askpass_password());
}
