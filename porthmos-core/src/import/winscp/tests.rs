use super::*;
use crate::import::Verdict;

fn session(header: &str, lines: &[&str]) -> String {
    format!("[Sessions\\{header}]\n{}\n", lines.join("\n"))
}

fn only(text: &str) -> ImportRow {
    let mut rows = parse(text).unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    rows.remove(0)
}

fn candidate(row: &ImportRow) -> &Candidate {
    assert_eq!(row.verdict, Verdict::New, "{row:?}");
    row.candidate.as_ref().unwrap()
}

#[test]
fn reads_a_site_with_a_folder() {
    let text =
        session("Work/Prod/My%20Site", &["HostName=h.example", "UserName=alice", "PortNumber=2222", "FSProtocol=2"]);
    let row = only(&text);
    let found = candidate(&row);
    assert_eq!(found.name, "My Site");
    assert_eq!(found.group.as_deref(), Some("Work/Prod"));
    assert_eq!(
        (found.protocol.as_str(), found.host.as_str(), found.username.as_str(), found.port),
        ("sftp", "h.example", "alice", Some(2222))
    );
}

#[test]
fn a_site_without_a_folder_has_no_group() {
    let row = only(&session("Plain", &["HostName=h", "UserName=u"]));
    assert_eq!(candidate(&row).group, None);
}

#[test]
fn decodes_utf8_marker_names() {
    let row = only(&session("%EF%BB%BFcaf%C3%A9", &["HostName=h", "UserName=u"]));
    assert_eq!(row.name, "café");
}

#[test]
fn values_are_decoded_too() {
    let row = only(&session("S", &["HostName=h", "UserName=a%20b", "RemoteDirectory=/srv/my%20docs"]));
    let found = candidate(&row);
    assert_eq!(found.username, "a b");
    assert_eq!(found.options.get("remote_path").map(String::as_str), Some("/srv/my docs"));
}

#[test]
fn skips_default_settings_workspaces_and_sections_without_a_host() {
    let text = [
        session("Default%20Settings", &["HostName=ignored", "UserName=u"]),
        session("0000", &["IsWorkspace=1", "HostName=h"]),
        session("NoHost", &["UserName=u"]),
        session("Real", &["HostName=h", "UserName=u"]),
    ]
    .concat();
    let rows = parse(&text).unwrap();
    assert_eq!(rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(), ["Real"]);
}

#[test]
fn ignores_non_session_sections() {
    let text = format!(
        "[Configuration\\Interface]\nHostName=nope\n{}[SshHostKeys]\nx=y\n",
        session("Real", &["HostName=h", "UserName=u"])
    );
    assert_eq!(parse(&text).unwrap().len(), 1);
}

#[test]
fn maps_protocols_and_ftps_modes() {
    let table = [
        (vec!["FSProtocol=0"], "scp", None),
        (vec!["FSProtocol=1"], "sftp", None),
        (vec!["FSProtocol=2"], "sftp", None),
        (vec![], "sftp", None),
        (vec!["FSProtocol=5"], "ftp", Some("plain")),
        (vec!["FSProtocol=5", "Ftps=0"], "ftp", Some("plain")),
        (vec!["FSProtocol=5", "Ftps=1"], "ftp", Some("implicit")),
        (vec!["FSProtocol=5", "Ftps=2"], "ftp", Some("explicit")),
        (vec!["FSProtocol=5", "Ftps=3"], "ftp", Some("explicit")),
        (vec!["FSProtocol=6"], "webdav", Some("http")),
        (vec!["FSProtocol=6", "Ftps=0"], "webdav", Some("http")),
        (vec!["FSProtocol=6", "Ftps=3"], "webdav", Some("https")),
        (vec!["FSProtocol=7"], "s3", None),
    ];
    for (extra, protocol, security) in table {
        let mut lines = vec!["HostName=h", "UserName=u"];
        lines.extend(extra.iter().copied());
        let row = only(&session("S", &lines));
        let found = candidate(&row);
        assert_eq!(found.protocol, protocol, "{extra:?}");
        assert_eq!(found.options.get("security").map(String::as_str), security, "{extra:?}");
    }
}

#[test]
fn obsolete_and_unknown_protocols_are_unsupported() {
    for value in ["3", "4", "9"] {
        let row = only(&session("S", &["HostName=h", "UserName=u", &format!("FSProtocol={value}")]));
        assert!(matches!(row.verdict, Verdict::Unsupported(_)), "{value}");
        assert!(row.candidate.is_none());
    }
}

#[test]
fn a_garbled_protocol_or_ftps_value_is_invalid() {
    let row = only(&session("S", &["HostName=h", "FSProtocol=abc"]));
    assert_eq!(row.verdict, Verdict::Invalid("unknown protocol".to_string()));
    let row = only(&session("S", &["HostName=h", "FSProtocol=5", "Ftps=9"]));
    assert_eq!(row.verdict, Verdict::Invalid("unknown FTPS mode".to_string()));
}

#[test]
fn user_at_host_is_split_only_when_username_is_empty() {
    let split = only(&session("S", &["HostName=bob@h.example"]));
    assert_eq!((candidate(&split).username.as_str(), candidate(&split).host.as_str()), ("bob", "h.example"));
    let kept = only(&session("S", &["HostName=bob@h.example", "UserName=alice"]));
    assert_eq!((candidate(&kept).username.as_str(), candidate(&kept).host.as_str()), ("alice", "bob@h.example"));
    let last_at = only(&session("S", &["HostName=a@b@h"]));
    assert_eq!((candidate(&last_at).username.as_str(), candidate(&last_at).host.as_str()), ("a@b", "h"));
}

#[test]
fn missing_port_is_none_and_bad_port_is_invalid() {
    assert_eq!(candidate(&only(&session("S", &["HostName=h", "UserName=u"]))).port, None);
    assert_eq!(candidate(&only(&session("S", &["HostName=h", "UserName=u", "PortNumber=0"]))).port, None);
    for port in ["70000", "abc", "-1"] {
        let row = only(&session("S", &["HostName=h", "UserName=u", &format!("PortNumber={port}")]));
        assert_eq!(row.verdict, Verdict::Invalid("port".to_string()), "{port}");
    }
}

#[test]
fn an_empty_host_is_invalid_and_a_host_with_controls_is_invalid() {
    assert_eq!(only(&session("S", &["HostName="])).verdict, Verdict::Invalid("no host".to_string()));
    assert_eq!(only(&session("S", &["HostName=h%1Bx"])).verdict, Verdict::Invalid("invalid host".to_string()));
}

#[test]
fn names_with_controls_are_cleaned_and_an_empty_name_is_invalid() {
    let row = only(&session("a%1B%5B31mb", &["HostName=h", "UserName=u"]));
    assert_eq!(row.name, "a[31mb");
    let row = only(&session("Folder/", &["HostName=h", "UserName=u"]));
    assert_eq!(row.verdict, Verdict::Invalid("no name".to_string()));
}

#[test]
fn ppk_key_is_noted_and_other_keys_are_kept() {
    let ppk = only(&session("S", &["HostName=h", "UserName=u", "PublicKeyFile=C%3A%5Ckeys%5Cid.ppk"]));
    assert!(!candidate(&ppk).options.contains_key("identity_file"));
    assert_eq!(candidate(&ppk).notes, ["key is in PuTTY format, convert it first"]);
    let other = only(&session("S", &["HostName=h", "UserName=u", "PublicKeyFile=%2Fhome%2Fa%2Fid_ed25519"]));
    assert_eq!(candidate(&other).options.get("identity_file").map(String::as_str), Some("/home/a/id_ed25519"));
}

#[test]
fn the_key_is_ignored_for_ftp() {
    let row = only(&session("S", &["HostName=h", "UserName=u", "FSProtocol=5", "PublicKeyFile=/k"]));
    assert!(!candidate(&row).options.contains_key("identity_file"));
}

#[test]
fn text_without_sites_is_wrong_format() {
    for text in ["", "<html><body>x</body></html>", "[Configuration\\Interface]\nFoo=1\n", "just words"] {
        assert!(matches!(parse(text), Err(ImportError::WrongFormat(_))), "{text:?}");
    }
}

#[test]
fn a_file_with_only_default_settings_has_no_rows() {
    assert_eq!(parse(&session("Default%20Settings", &["HostName=h"])).unwrap(), Vec::new());
}

#[test]
fn secrets_are_never_read() {
    let text = session(
        "S",
        &[
            "HostName=h",
            "UserName=u",
            "Password=SENTINEL-PASSWORD-1234",
            "PasswordPlain=SENTINEL-PLAIN",
            "ProxyPassword=SENTINEL-PROXY",
            "ProxyPasswordEnc=SENTINEL-PROXYENC",
            "TunnelPassword=SENTINEL-TUNNEL",
            "Passphrase=SENTINEL-PASSPHRASE",
            "EncryptKeyPlain=SENTINEL-ENCRYPT",
        ],
    );
    let shown = format!("{:?}", parse(&text).unwrap());
    for sentinel in [
        "SENTINEL-PASSWORD",
        "SENTINEL-PLAIN",
        "SENTINEL-PROXY",
        "SENTINEL-TUNNEL",
        "SENTINEL-PASSPHRASE",
        "SENTINEL-ENCRYPT",
    ] {
        assert!(!shown.contains(sentinel), "{sentinel} leaked into {shown}");
    }
}

#[test]
fn crlf_files_and_a_bom_parse_the_same() {
    let plain = session("My%20Site", &["HostName=h", "UserName=u"]);
    let windows = format!("\u{feff}{}", plain.replace('\n', "\r\n"));
    assert_eq!(parse(&windows).unwrap(), parse(&plain).unwrap());
}

#[test]
fn comment_lines_and_blank_lines_are_skipped() {
    let text = format!("; exported\n\n{}", session("S", &["; note", "", "HostName=h", "UserName=u"]));
    assert_eq!(candidate(&only(&text)).host, "h");
}

#[test]
fn sites_keep_file_order() {
    let text = [session("B", &["HostName=h"]), session("A", &["HostName=h"])].concat();
    let names: Vec<String> = parse(&text).unwrap().into_iter().map(|row| row.name).collect();
    assert_eq!(names, ["B", "A"]);
}

#[test]
fn a_password_in_the_host_field_makes_the_row_invalid_and_is_not_kept_anywhere() {
    let row = only(&session("S", &["HostName=u:SECRETPW@h.example"]));

    assert_eq!(row.verdict, Verdict::Invalid("invalid host".to_string()));
    assert!(!format!("{row:?}").contains("SECRETPW"));
}
