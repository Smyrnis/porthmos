use super::*;
use crate::import::Verdict;

fn candidate(row: &ImportRow) -> &Candidate {
    assert_eq!(row.verdict, Verdict::New, "{row:?}");
    row.candidate.as_ref().unwrap()
}

fn session(name: &str, text: &str) -> ImportRow {
    parse_session(name, text).unwrap()
}

const REG_HEADER: &str = "Windows Registry Editor Version 5.00\r\n\r\n";
const SESSIONS: &str = "HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions";

fn reg(body: &str) -> String {
    format!("{REG_HEADER}{body}")
}

fn reg_session(name: &str, lines: &[&str]) -> String {
    format!("[{SESSIONS}\\{name}]\r\n{}\r\n\r\n", lines.join("\r\n"))
}

fn utf16le(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

#[test]
fn session_file_maps_ssh_to_sftp() {
    let row = session("My%20Box", "HostName=h.example\nPortNumber=2222\nUserName=alice\nProtocol=ssh\n");
    let found = candidate(&row);
    assert_eq!(row.name, "My Box");
    assert_eq!(
        (found.protocol.as_str(), found.host.as_str(), found.port, found.username.as_str()),
        ("sftp", "h.example", Some(2222), "alice")
    );
    assert_eq!(found.group, None);
}

#[test]
fn absent_protocol_and_port_mean_ssh_and_the_default() {
    let row = session("S", "HostName=h\nUserName=u\n");
    assert_eq!((candidate(&row).protocol.as_str(), candidate(&row).port), ("sftp", None));
}

#[test]
fn non_ssh_protocols_are_unsupported() {
    for protocol in ["telnet", "raw", "serial", "rlogin", "Telnet"] {
        let row = session("S", &format!("HostName=h\nProtocol={protocol}\n"));
        assert!(matches!(row.verdict, Verdict::Unsupported(_)), "{protocol}");
        assert!(row.candidate.is_none());
    }
}

#[test]
fn missing_host_is_invalid_and_host_with_controls_is_invalid() {
    assert_eq!(session("S", "UserName=u\n").verdict, Verdict::Invalid("no host".to_string()));
    assert_eq!(session("S", "HostName=h\u{9b}x\n").verdict, Verdict::Invalid("invalid host".to_string()));
}

#[test]
fn bad_ports_are_invalid() {
    for port in ["70000", "abc", "-1"] {
        assert_eq!(
            session("S", &format!("HostName=h\nPortNumber={port}\n")).verdict,
            Verdict::Invalid("port".to_string()),
            "{port}"
        );
    }
    assert_eq!(candidate(&session("S", "HostName=h\nPortNumber=0\n")).port, None);
}

#[test]
fn the_default_settings_file_is_not_a_site() {
    assert!(parse_session("Default%20Settings", "HostName=h\n").is_none());
}

#[test]
fn file_names_are_decoded_and_cleaned() {
    assert_eq!(session("caf%C3%A9", "HostName=h\n").name, "café");
    assert_eq!(session("a%1B%5B31mb", "HostName=h\n").name, "a[31mb");
    assert_eq!(session("%20%20", "HostName=h\n").verdict, Verdict::Invalid("no name".to_string()));
}

#[test]
fn user_at_host_is_split_when_the_username_is_empty() {
    let row = session("S", "HostName=bob@h.example\n");
    assert_eq!((candidate(&row).username.as_str(), candidate(&row).host.as_str()), ("bob", "h.example"));
}

#[test]
fn ppk_key_is_noted_and_other_keys_are_kept() {
    let ppk = session("S", "HostName=h\nPublicKeyFile=C:\\keys\\id.PPK\n");
    assert!(!candidate(&ppk).options.contains_key("identity_file"));
    assert_eq!(candidate(&ppk).notes, ["key is in PuTTY format, convert it first"]);
    let other = session("S", "HostName=h\nPublicKeyFile=/home/a/id=ed25519\n");
    assert_eq!(candidate(&other).options.get("identity_file").map(String::as_str), Some("/home/a/id=ed25519"));
}

#[test]
fn lines_without_an_equals_sign_and_crlf_endings_are_tolerated() {
    let row = session("S", "garbage\r\nHostName=h\r\n\r\nUserName=u\r\n");
    assert_eq!((candidate(&row).host.as_str(), candidate(&row).username.as_str()), ("h", "u"));
}

#[test]
fn session_secrets_are_never_read() {
    let text = "HostName=h\nUserName=u\nProxyPassword=SENTINEL-PROXY\nProxyUsername=SENTINEL-PROXYUSER\nPassphrase=SENTINEL-PASSPHRASE\nPassword=SENTINEL-PASSWORD-1234\nProxyTelnetCommand=SENTINEL-COMMAND\n";
    let shown = format!("{:?}", session("S", text));
    for sentinel in ["SENTINEL-PROXY", "SENTINEL-PASSPHRASE", "SENTINEL-PASSWORD", "SENTINEL-COMMAND"] {
        assert!(!shown.contains(sentinel), "{sentinel} leaked into {shown}");
    }
}

#[test]
fn reg_export_parses_strings_and_dwords() {
    let text = reg(&reg_session(
        "My%20Box",
        &[
            "\"HostName\"=\"h.example\"",
            "\"PortNumber\"=dword:000008ae",
            "\"Protocol\"=\"ssh\"",
            "\"UserName\"=\"alice\"",
            "\"PublicKeyFile\"=\"C:\\\\keys\\\\id_ed25519\"",
        ],
    ));
    let rows = parse_reg(&text).unwrap();
    assert_eq!(rows.len(), 1);
    let found = candidate(&rows[0]);
    assert_eq!(rows[0].name, "My Box");
    assert_eq!((found.host.as_str(), found.port, found.username.as_str()), ("h.example", Some(2222), "alice"));
    assert_eq!(found.options.get("identity_file").map(String::as_str), Some("C:\\keys\\id_ed25519"));
}

#[test]
fn reg_dword_port_is_decoded() {
    let text = reg(&reg_session("S", &["\"HostName\"=\"h\"", "\"PortNumber\"=dword:00000016"]));
    assert_eq!(candidate(&parse_reg(&text).unwrap()[0]).port, Some(22));
}

#[test]
fn reg_escaped_quote_and_backslash_in_values() {
    let text = reg(&reg_session("S", &["\"HostName\"=\"h\"", "\"UserName\"=\"a\\\"b\\\\c\""]));
    assert_eq!(candidate(&parse_reg(&text).unwrap()[0]).username, "a\"b\\c");
}

#[test]
fn reg_ignores_keys_outside_the_sessions_branch_and_default_settings() {
    let text = reg(&format!(
        "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\SshHostKeys]\r\n\"ssh-ed25519@22:h\"=\"0x1\"\r\n\r\n{}{}",
        reg_session("Default%20Settings", &["\"HostName\"=\"ignored\""]),
        reg_session("Real", &["\"HostName\"=\"h\""]),
    ));
    let names: Vec<String> = parse_reg(&text).unwrap().into_iter().map(|row| row.name).collect();
    assert_eq!(names, ["Real"]);
}

#[test]
fn reg_non_ssh_session_is_unsupported() {
    let text = reg(&reg_session("T", &["\"HostName\"=\"h\"", "\"Protocol\"=\"telnet\""]));
    assert!(matches!(parse_reg(&text).unwrap()[0].verdict, Verdict::Unsupported(_)));
}

#[test]
fn reg_without_putty_sessions_is_wrong_format() {
    for text in [
        "",
        "plain text",
        "<html></html>",
        &reg(""),
        &reg("[HKEY_CURRENT_USER\\Software\\Other]\r\n\"HostName\"=\"h\"\r\n"),
    ] {
        assert!(matches!(parse_reg(text), Err(ImportError::WrongFormat(_))), "{text:?}");
    }
}

#[test]
fn reg_with_an_empty_sessions_key_has_no_rows() {
    assert_eq!(parse_reg(&reg(&format!("[{SESSIONS}]\r\n\r\n"))).unwrap(), Vec::new());
}

#[test]
fn reg_secrets_are_never_read() {
    let text = reg(&reg_session(
        "S",
        &[
            "\"HostName\"=\"h\"",
            "\"ProxyPassword\"=\"SENTINEL-PROXY\"",
            "\"Passphrase\"=\"SENTINEL-PASSPHRASE\"",
            "\"ProxyUsername\"=\"SENTINEL-PROXYUSER\"",
        ],
    ));
    let shown = format!("{:?}", parse_reg(&text).unwrap());
    for sentinel in ["SENTINEL-PROXY", "SENTINEL-PASSPHRASE"] {
        assert!(!shown.contains(sentinel), "{sentinel} leaked into {shown}");
    }
}

#[test]
fn reg_text_in_utf16_and_utf8_both_decode_to_the_same_rows() {
    let text = reg(&reg_session("caf%C3%A9", &["\"HostName\"=\"h\u{e9}.example\""]));
    let from_utf16 = decode_reg_text(&utf16le(&text)).unwrap();
    let mut utf8_with_bom = vec![0xEF, 0xBB, 0xBF];
    utf8_with_bom.extend(text.as_bytes());
    let from_utf8 = decode_reg_text(&utf8_with_bom).unwrap();
    assert_eq!(from_utf16, text);
    assert_eq!(from_utf8, text);
    assert_eq!(parse_reg(&from_utf16).unwrap()[0].candidate.as_ref().unwrap().host, "h\u{e9}.example");
}

#[test]
fn utf16_big_endian_is_decoded_too() {
    let mut bytes = vec![0xFE, 0xFF];
    bytes.extend("abc".encode_utf16().flat_map(u16::to_be_bytes));
    assert_eq!(decode_reg_text(&bytes).unwrap(), "abc");
}

#[test]
fn bad_utf16_and_bad_utf8_are_refused_without_a_panic() {
    assert!(decode_reg_text(&[0xFF, 0xFE, 0x00, 0xD8, 0x41, 0x00]).is_err());
    assert!(decode_reg_text(&[0xFF, 0xFE, 0x41]).is_err());
    assert!(decode_reg_text(&[0x41, 0xFF, 0xFE, 0x42]).is_err());
}

#[test]
fn a_password_in_the_host_field_makes_the_row_invalid_and_is_not_kept_anywhere() {
    let row = session("S", "HostName=ftp://user:SECRETPW@h.example/path\n");

    assert_eq!(row.verdict, Verdict::Invalid("invalid host".to_string()));
    assert!(!format!("{row:?}").contains("SECRETPW"));
}
