use std::collections::BTreeMap;

use porthmos_core::{
    CommonField, ConnectionForm, OptionField, OptionKind, PortField, ProtocolInfo,
    profiles::{ConnectionEntry, ConnectionSource},
};
use ratatui::{Terminal, backend::TestBackend};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_fg, find_text},
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn s3_form() -> ConnectionForm {
    ConnectionForm {
        host: CommonField { label: "Endpoint", required: true },
        port: PortField { label: "Port", default: 443 },
        username: CommonField { label: "Access key", required: true },
        password: CommonField { label: "Secret", required: false },
        options: vec![
            OptionField { key: "bucket", label: "Bucket", required: false, kind: OptionKind::Text { default: "" } },
            OptionField { key: "region", label: "Region", required: false, kind: OptionKind::Text { default: "" } },
            OptionField { key: "session_token", label: "Session token", required: false, kind: OptionKind::Secret },
            OptionField {
                key: "path_style",
                label: "Path style",
                required: false,
                kind: OptionKind::Toggle { default: false },
            },
        ],
    }
}

fn protocols() -> Vec<ProtocolInfo> {
    vec![
        ProtocolInfo {
            id: "sftp",
            display_name: "sftp",
            description: "",
            limits_note: None,
            form: ConnectionForm::standard(22),
        },
        ProtocolInfo {
            id: "s3",
            display_name: "s3",
            description: "",
            limits_note: Some("no permissions or symlinks \u{b7} rename is copy + delete"),
            form: s3_form(),
        },
    ]
}

fn sftp_entry() -> ConnectionEntry {
    ConnectionEntry {
        name: "prod-web".to_string(),
        protocol: "sftp".to_string(),
        host: "10.0.4.12".to_string(),
        port: 22,
        username: "deploy".to_string(),
        password: None,
        options: BTreeMap::new(),
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
        source: ConnectionSource::Profile,
    }
}

fn s3_entry() -> ConnectionEntry {
    ConnectionEntry {
        name: "backups".to_string(),
        protocol: "s3".to_string(),
        host: "s3.eu-central-1.amazonaws.com".to_string(),
        port: 443,
        username: "AKIA1234".to_string(),
        options: BTreeMap::from([
            ("bucket".to_string(), "tc-backups".to_string()),
            ("region".to_string(), "eu-central-1".to_string()),
            ("path_style".to_string(), "true".to_string()),
            ("remote_path".to_string(), "/nightly/".to_string()),
        ]),
        group: Some("storage".to_string()),
        tags: vec!["storage".to_string(), "nightly".to_string()],
        ..sftp_entry()
    }
}

fn pairs(entry: &ConnectionEntry) -> Vec<(String, String)> {
    details_lines(entry, &protocols())
}

fn kv(key: &str, value: &str) -> (String, String) {
    (key.to_string(), value.to_string())
}

#[test]
fn a_sftp_entry_lists_host_user_and_nothing_else() {
    assert_eq!(pairs(&sftp_entry()), vec![kv("host", "10.0.4.12"), kv("username", "deploy")]);
}

#[test]
fn the_start_folder_group_and_tags_are_listed_when_present() {
    let mut entry = sftp_entry();
    entry.options.insert("remote_path".to_string(), "/var/www".to_string());
    entry.group = Some("production/web".to_string());
    entry.tags = vec!["blue".to_string(), "db".to_string()];

    let lines = pairs(&entry);

    assert!(lines.contains(&kv("start in", "/var/www")));
    assert!(lines.contains(&kv("group", "production/web")));
    assert!(lines.contains(&kv("tags", "blue, db")));
}

#[test]
fn the_port_appears_only_when_it_is_not_the_default() {
    let mut entry = sftp_entry();
    assert!(!pairs(&entry).iter().any(|(key, _)| key == "port"));

    entry.port = 2222;
    assert!(pairs(&entry).contains(&kv("port", "2222")));
}

#[test]
fn an_s3_entry_uses_the_protocols_own_labels_in_form_order() {
    let lines = pairs(&s3_entry());

    assert_eq!(
        lines,
        vec![
            kv("endpoint", "s3.eu-central-1.amazonaws.com"),
            kv("access key", "AKIA1234"),
            kv("bucket", "tc-backups"),
            kv("region", "eu-central-1"),
            kv("path style", "yes"),
            kv("start in", "/nightly/"),
            kv("group", "storage"),
            kv("tags", "storage, nightly"),
        ]
    );
}

#[test]
fn a_false_toggle_reads_no() {
    let mut entry = s3_entry();
    entry.options.insert("path_style".to_string(), "false".to_string());

    assert!(pairs(&entry).contains(&kv("path style", "no")));
}

#[test]
fn secrets_are_never_listed_only_as_saved() {
    let mut entry = s3_entry();
    entry.password = Some("hunter2".to_string());
    entry.saved_password = true;
    entry.in_keyring = vec!["session_token".to_string()];
    entry.options.insert("session_token".to_string(), "TOPSECRET".to_string());

    let lines = pairs(&entry);

    assert!(lines.contains(&kv("password", "saved")));
    assert!(lines.contains(&kv("session token", "saved")));
    let everything = format!("{lines:?}");
    assert!(!everything.contains("hunter2") && !everything.contains("TOPSECRET"), "{everything}");
}

#[test]
fn an_unsaved_secret_is_not_listed_at_all() {
    let mut entry = s3_entry();
    entry.options.insert("session_token".to_string(), "TOPSECRET".to_string());

    let everything = format!("{:?}", pairs(&entry));

    assert!(!everything.contains("TOPSECRET") && !everything.contains("session token"), "{everything}");
}

#[test]
fn an_ssh_config_entry_names_its_source() {
    let entry = ConnectionEntry { source: ConnectionSource::SshConfig, ..sftp_entry() };

    assert!(pairs(&entry).contains(&kv("source", "~/.ssh/config")));
}

#[test]
fn an_unknown_protocol_falls_back_to_the_raw_option_keys_and_always_shows_the_port() {
    let mut entry = sftp_entry();
    entry.protocol = "mystery".to_string();
    entry.options.insert("flavour".to_string(), "mint".to_string());

    let lines = pairs(&entry);

    assert!(lines.contains(&kv("host", "10.0.4.12")));
    assert!(lines.contains(&kv("port", "22")));
    assert!(lines.contains(&kv("flavour", "mint")));
}

#[test]
fn values_are_sanitised() {
    let mut entry = sftp_entry();
    entry.host = "h\u{202e}ost\nname".to_string();

    assert!(pairs(&entry).contains(&kv("host", "h?ost?name")));
}

struct Draw<'a> {
    protocols: Vec<ProtocolInfo>,
    connected: bool,
    failure: Option<&'a str>,
}

impl<'a> Draw<'a> {
    fn new() -> Self {
        Self { protocols: protocols(), connected: false, failure: None }
    }

    fn selection(&self, selection: Option<Selection<'_>>, width: u16, height: u16) -> Terminal<TestBackend> {
        let theme = indexed();
        let context = DetailsContext { protocols: &self.protocols, connected: self.connected, failure: self.failure };
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render_connection_details(frame, frame.area(), selection, 4, &context, &theme)).unwrap();
        terminal
    }

    fn entry(&self, entry: &ConnectionEntry, width: u16, height: u16) -> Terminal<TestBackend> {
        self.selection(Some(Selection::Connection(entry)), width, height)
    }
}

fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    let mut x = 0;
    while x < buffer.area.width {
        let symbol = buffer[(x, y)].symbol();
        text.push_str(symbol);
        x += symbol.width().max(1) as u16;
    }
    text
}

fn screen(terminal: &Terminal<TestBackend>) -> String {
    (0..terminal.backend().buffer().area.height).map(|y| row(terminal, y)).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_header_shows_name_protocol_and_connected_status() {
    let theme = indexed();
    let mut draw = Draw::new();
    draw.connected = true;

    let terminal = draw.entry(&s3_entry(), 42, 14);

    assert!(row(&terminal, 0).starts_with("backups s3"), "{:?}", row(&terminal, 0));
    assert!(row(&terminal, 0).trim_end().ends_with("\u{2022} connected"), "{:?}", row(&terminal, 0));
    assert_eq!(cell_fg(&terminal, 0, 0), theme.fg_bright);
    let (x, y) = find_text(&terminal, "s3").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    let (ok_x, ok_y) = find_text(&terminal, "connected").unwrap();
    assert_eq!(cell_fg(&terminal, ok_x, ok_y), theme.ok);
    assert_eq!(row(&terminal, 1), "\u{2500}".repeat(42));
    assert_eq!(cell_fg(&terminal, 3, 1), theme.line);
}

#[test]
fn a_failed_entry_shows_its_first_error_line_in_err() {
    let theme = indexed();
    let mut draw = Draw::new();
    draw.failure = Some("Authentication failed\nsecond line");

    let terminal = draw.entry(&sftp_entry(), 50, 8);

    let (x, y) = find_text(&terminal, "Authentication failed").unwrap();
    assert_eq!((y, cell_fg(&terminal, x, y)), (0, theme.err));
    assert!(!screen(&terminal).contains("second line"));
}

#[test]
fn keys_are_dim_and_values_are_in_the_text_colour() {
    let theme = indexed();

    let terminal = Draw::new().entry(&s3_entry(), 42, 14);

    let (key_x, key_y) = find_text(&terminal, "endpoint").unwrap();
    assert_eq!(cell_fg(&terminal, key_x, key_y), theme.fg_dim);
    let (value_x, value_y) = find_text(&terminal, "s3.eu-central-1").unwrap();
    assert_eq!((value_y, cell_fg(&terminal, value_x, value_y)), (key_y, theme.fg));
    assert_eq!(value_x - key_x, 12);
}

#[test]
fn the_limits_note_is_shown_dim_when_the_protocol_has_one() {
    let theme = indexed();

    let terminal = Draw::new().entry(&s3_entry(), 42, 16);

    let (x, y) = find_text(&terminal, "no permissions or symlinks").expect("limits note");
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    let sftp = Draw::new().entry(&sftp_entry(), 42, 16);
    assert!(!screen(&sftp).contains("no permissions"));
}

#[test]
fn a_group_row_shows_its_path_and_count() {
    let terminal = Draw::new().selection(Some(Selection::Group("production/web")), 42, 8);

    assert!(row(&terminal, 0).starts_with("production/web"), "{:?}", row(&terminal, 0));
    assert!(row(&terminal, 0).trim_end().ends_with("4 connections"), "{:?}", row(&terminal, 0));
}

#[test]
fn no_selection_draws_nothing() {
    let terminal = Draw::new().selection(None, 42, 8);

    assert_eq!(screen(&terminal).trim(), "");
}

#[test]
fn long_values_are_cut_by_display_width() {
    let mut entry = sftp_entry();
    entry.host = "\u{65e5}\u{672c}".repeat(100);
    entry.options.insert("remote_path".to_string(), "p".repeat(500));
    entry.name = "n".repeat(300);

    for width in [42, 20, 10, 5] {
        let terminal = Draw::new().entry(&entry, width, 10);
        for y in 0..10 {
            assert!(row(&terminal, y).width() <= usize::from(width), "width {width} row {y}");
        }
    }
}

#[test]
fn the_pane_never_prints_a_stored_secret() {
    let mut entry = s3_entry();
    entry.password = Some("hunter2".to_string());
    entry.saved_password = true;
    entry.options.insert("session_token".to_string(), "TOPSECRET".to_string());
    entry.in_keyring = vec!["session_token".to_string()];

    let shown = screen(&Draw::new().entry(&entry, 60, 20));

    assert!(!shown.contains("hunter2") && !shown.contains("TOPSECRET"));
    assert!(shown.contains("saved"));
}

#[test]
fn tiny_areas_do_not_panic() {
    let draw = Draw::new();
    for width in 0..=12 {
        for height in 0..=6 {
            let _ = draw.entry(&s3_entry(), width, height);
            let _ = draw.selection(Some(Selection::Group("g")), width, height);
            let _ = draw.selection(None, width, height);
        }
    }
}
