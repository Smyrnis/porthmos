use super::*;
use crate::import::Verdict;

fn document(servers: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><FileZilla3 version=\"3.71.0\" platform=\"*nix\"><Servers>{servers}</Servers></FileZilla3>"
    )
}

fn server(host: &str, port: &str, protocol: &str, extra: &str) -> String {
    format!(
        "<Server><Host>{host}</Host><Port>{port}</Port><Protocol>{protocol}</Protocol><Type>0</Type><User>alice</User><Logontype>1</Logontype><Name>{host}</Name>{extra}</Server>"
    )
}

fn only(servers: &str) -> ImportRow {
    let mut rows = parse(&document(servers)).unwrap();
    assert_eq!(rows.len(), 1);
    rows.remove(0)
}

fn candidate(row: &ImportRow) -> &Candidate {
    assert_eq!(row.verdict, Verdict::New, "{row:?}");
    row.candidate.as_ref().unwrap()
}

#[test]
fn reads_a_site_inside_nested_folders_as_a_group() {
    let xml = document(&format!(
        "<Folder expanded=\"1\">Work<Folder>Prod{}</Folder></Folder>",
        server("h.example", "22", "1", "")
    ));
    let rows = parse(&xml).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(candidate(&rows[0]).group.as_deref(), Some("Work/Prod"));
    assert_eq!(candidate(&rows[0]).name, "h.example");
}

#[test]
fn sites_keep_document_order() {
    let xml = document(&format!(
        "{}<Folder>F{}</Folder>{}",
        server("a", "22", "1", ""),
        server("b", "22", "1", ""),
        server("c", "22", "1", "")
    ));
    let names: Vec<String> = parse(&xml).unwrap().into_iter().map(|row| row.name).collect();
    assert_eq!(names, ["a", "b", "c"]);
}

#[test]
fn maps_every_supported_protocol() {
    let table = [
        ("1", "sftp", None),
        ("0", "ftp", Some("explicit")),
        ("4", "ftp", Some("explicit")),
        ("3", "ftp", Some("implicit")),
        ("6", "ftp", Some("plain")),
        ("7", "s3", None),
        ("9", "webdav", Some("https")),
        ("19", "webdav", Some("http")),
    ];
    for (number, protocol, security) in table {
        let row = only(&server("h.example", "", number, ""));
        let found = candidate(&row);
        assert_eq!(found.protocol, protocol, "protocol {number}");
        assert_eq!(found.options.get("security").map(String::as_str), security, "protocol {number}");
    }
}

#[test]
fn http_https_and_storj_are_unsupported_with_a_reason() {
    for (number, reason) in [("2", "HTTP"), ("5", "HTTPS"), ("8", "Storj"), ("14", "storage service")] {
        let row = only(&server("h.example", "", number, ""));
        match &row.verdict {
            Verdict::Unsupported(text) => assert!(text.contains(reason), "{number}: {text}"),
            other => panic!("{number}: {other:?}"),
        }
        assert!(row.candidate.is_none());
        assert_eq!(row.name, "h.example");
    }
}

#[test]
fn unknown_protocol_number_is_invalid() {
    for number in ["25", "-1", "abc", ""] {
        let row = only(&server("h.example", "", number, ""));
        assert_eq!(row.verdict, Verdict::Invalid("unknown protocol".to_string()), "{number:?}");
    }
}

#[test]
fn anonymous_ftp_uses_the_anonymous_username() {
    let anonymous =
        "<Server><Host>ftp.example</Host><Port>21</Port><Protocol>0</Protocol><Logontype>0</Logontype></Server>";
    let row = only(anonymous);
    assert_eq!(candidate(&row).username, "anonymous");
    assert_eq!(row.name, "ftp.example");
}

#[test]
fn anonymous_sftp_is_invalid() {
    let anonymous = "<Server><Host>h</Host><Port>22</Port><Protocol>1</Protocol><Logontype>0</Logontype></Server>";
    assert_eq!(only(anonymous).verdict, Verdict::Invalid("anonymous login".to_string()));
}

#[test]
fn profile_and_adc_logons_are_unsupported() {
    for logon in ["6", "7"] {
        let xml = format!(
            "<Server><Host>s3.example</Host><Protocol>7</Protocol><Logontype>{logon}</Logontype><User>k</User></Server>"
        );
        assert!(matches!(only(&xml).verdict, Verdict::Unsupported(_)), "logon {logon}");
    }
}

#[test]
fn remote_dir_decodes_unix_safe_paths() {
    for (safe, expected) in [
        ("1 0 4 home 4 user 3 www", "/home/user/www"),
        ("1 0", "/"),
        ("1 0 4 home 3 usr", "/home/usr"),
        ("0 0 3 srv", "/srv"),
        ("1 0 7 my docs", "/my docs"),
    ] {
        let row = only(&server("h", "22", "1", &format!("<RemoteDir>{safe}</RemoteDir>")));
        let found = candidate(&row);
        assert_eq!(found.options.get("remote_path").map(String::as_str), Some(expected), "{safe}");
        assert!(found.notes.is_empty(), "{safe}");
    }
}

#[test]
fn bad_remote_dir_is_dropped_with_a_note() {
    for safe in ["1 0 9 home", "3 0 2 C: 4 data", "1 5", "garbage", "1 0 0 ", "1 0 4 home extra", "1 x"] {
        let row = only(&server("h", "22", "1", &format!("<RemoteDir>{safe}</RemoteDir>")));
        let found = candidate(&row);
        assert!(!found.options.contains_key("remote_path"), "{safe}");
        assert_eq!(found.notes, ["remote folder not imported"], "{safe}");
    }
}

#[test]
fn an_empty_or_absent_remote_dir_adds_no_note() {
    for extra in ["", "<RemoteDir></RemoteDir>"] {
        let row = only(&server("h", "22", "1", extra));
        assert!(candidate(&row).notes.is_empty());
    }
}

#[test]
fn ppk_key_is_not_imported_and_is_noted() {
    let row = only(&server("h", "22", "1", "<Keyfile>/home/a/key.PPK</Keyfile>"));
    let found = candidate(&row);
    assert!(!found.options.contains_key("identity_file"));
    assert_eq!(found.notes, ["key is in PuTTY format, convert it first"]);
}

#[test]
fn other_key_path_becomes_identity_file() {
    let row = only(&server("h", "22", "1", "<Keyfile>/home/a/id_ed25519</Keyfile>"));
    assert_eq!(candidate(&row).options.get("identity_file").map(String::as_str), Some("/home/a/id_ed25519"));
}

#[test]
fn name_falls_back_to_element_text_then_host() {
    let own_text = "<Server>Own name<Host>h1</Host><Protocol>1</Protocol><User>u</User></Server>";
    assert_eq!(only(own_text).name, "Own name");
    let host_only = "<Server><Host>h2</Host><Protocol>1</Protocol><User>u</User></Server>";
    assert_eq!(only(host_only).name, "h2");
    let named = "<Server><Host>h3</Host><Protocol>1</Protocol><User>u</User><Name>  Shown  </Name></Server>";
    assert_eq!(only(named).name, "Shown");
}

#[test]
fn ports_are_checked() {
    assert_eq!(candidate(&only(&server("h", "", "1", ""))).port, None);
    assert_eq!(candidate(&only(&server("h", "2222", "1", ""))).port, Some(2222));
    for port in ["0", "70000", "abc", "-5"] {
        assert_eq!(only(&server("h", port, "1", "")).verdict, Verdict::Invalid("port".to_string()), "{port}");
    }
}

#[test]
fn a_site_without_a_host_is_invalid() {
    let xml = "<Server><Host>  </Host><Protocol>1</Protocol><User>u</User><Name>Nameless host</Name></Server>";
    let row = only(xml);
    assert_eq!(row.verdict, Verdict::Invalid("no host".to_string()));
    assert_eq!(row.name, "Nameless host");
}

#[test]
fn control_characters_never_reach_names_hosts_or_groups() {
    let xml = document(
        "<Folder>G\u{9b}[31mrp<Server><Host>h\u{9b}x</Host><Protocol>1</Protocol><User>u</User><Name>Na\u{9b}[0mme</Name></Server></Folder>",
    );
    let rows = parse(&xml).unwrap();
    assert_eq!(rows[0].name, "Na[0mme");
    assert_eq!(rows[0].verdict, Verdict::Invalid("invalid host".to_string()));
    let ok = document(
        "<Folder>G\u{9b}[31mrp<Server><Host>h</Host><Protocol>1</Protocol><User>u</User><Name>Na\u{9b}[0mme</Name></Server></Folder>",
    );
    let rows = parse(&ok).unwrap();
    assert_eq!(candidate(&rows[0]).group.as_deref(), Some("G[31mrp"));
}

#[test]
fn a_slash_in_a_folder_name_does_not_split_the_group() {
    let xml = document(&format!("<Folder>A/B{}</Folder>", server("h", "22", "1", "")));
    assert_eq!(candidate(&parse(&xml).unwrap()[0]).group.as_deref(), Some("A-B"));
}

#[test]
fn wrong_documents_are_wrong_format() {
    for text in ["", "plain text", "<html><body>hi</body></html>", "<Foo/>", "<FileZilla3><Settings/></FileZilla3>"] {
        assert!(matches!(parse(text), Err(ImportError::WrongFormat(_))), "{text:?}");
    }
}

#[test]
fn an_empty_servers_element_gives_no_rows() {
    assert_eq!(parse(&document("")).unwrap(), Vec::new());
}

#[test]
fn secrets_are_never_read() {
    let extra = "<Pass encoding=\"base64\">SENTINEL-PASSWORD-1234</Pass>\
        <Pass encoding=\"crypt\" pubkey=\"SENTINEL-KEY\">SENTINEL-CRYPT</Pass>\
        <Account>SENTINEL-ACCOUNT</Account>\
        <Parameter Name=\"ssecustomerkey\">SENTINEL-PARAM</Parameter>\
        <Comments>SENTINEL-COMMENT</Comments>\
        <PostLoginCommands><Command>SENTINEL-COMMAND</Command></PostLoginCommands>";
    let rows = parse(&document(&server("h.example", "22", "1", extra))).unwrap();
    let shown = format!("{rows:?}");
    for sentinel in [
        "SENTINEL-PASSWORD",
        "SENTINEL-KEY",
        "SENTINEL-CRYPT",
        "SENTINEL-ACCOUNT",
        "SENTINEL-PARAM",
        "SENTINEL-COMMENT",
        "SENTINEL-COMMAND",
    ] {
        assert!(!shown.contains(sentinel), "{sentinel} leaked into {shown}");
    }
}

#[test]
fn an_entity_bomb_is_refused() {
    let bomb = "<?xml version=\"1.0\"?><!DOCTYPE FileZilla3 [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;\"><!ENTITY c \"&b;&b;&b;&b;&b;&b;&b;&b;\"><!ENTITY d \"&c;&c;&c;&c;&c;&c;&c;&c;\">]><FileZilla3><Servers><Server><Host>&d;</Host></Server></Servers></FileZilla3>";
    assert!(parse(bomb).is_err());
}

#[test]
fn very_deep_nesting_is_refused_or_handled_without_a_crash() {
    let depth = 10_000;
    let xml =
        document(&format!("{}{}{}", "<Folder>f".repeat(depth), server("h", "22", "1", ""), "</Folder>".repeat(depth)));
    let outcome = std::thread::Builder::new().stack_size(512 * 1024).spawn(move || parse(&xml).is_ok()).unwrap().join();
    assert!(outcome.is_ok());
}

#[test]
fn a_file_holding_an_escape_byte_is_not_a_valid_xml_file() {
    let xml = document("<Server><Host>h</Host><Protocol>1</Protocol><Name>a\u{1b}b</Name></Server>");
    assert!(matches!(parse(&xml), Err(ImportError::WrongFormat(_))));
}

#[test]
fn many_servers_in_one_nameless_folder_parse_in_linear_time() {
    let servers = (0..20_000).map(|index| server(&format!("h{index}"), "22", "1", "")).collect::<String>();
    let xml = document(&format!("<Folder>{servers}</Folder>"));
    let started = std::time::Instant::now();

    let rows = parse(&xml).unwrap();

    assert_eq!(rows.len(), 20_000);
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
}

#[test]
fn many_servers_under_deep_non_folder_nesting_parse_in_linear_time() {
    let depth = 10_000;
    let servers = (0..depth).map(|index| server(&format!("h{index}"), "22", "1", "")).collect::<String>();
    let xml = document(&format!("{}{servers}{}", "<Group>".repeat(depth), "</Group>".repeat(depth)));
    let started = std::time::Instant::now();

    let rows = std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(move || parse(&xml).map(|rows| rows.len()))
        .unwrap()
        .join()
        .unwrap();

    assert_eq!(rows, Ok(depth));
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
}

#[test]
fn only_the_nearest_thirty_two_folders_name_the_group() {
    let nested = (0..40).fold(server("h", "22", "1", ""), |inner, level| format!("<Folder>f{level}{inner}</Folder>"));
    let rows = parse(&document(&nested)).unwrap();
    let group = rows[0].candidate.as_ref().unwrap().group.clone().unwrap();

    assert_eq!(group.split('/').count(), 32);
    assert!(group.ends_with("f0"), "{group}");
}
