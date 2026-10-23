use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Duration, TimeZone, Utc};
use porthmos_core::{
    ConnectionForm, ProtocolInfo,
    profiles::{ConnectionEntry, ConnectionSource},
};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text},
    theme::{ColorDepth, ThemeSettings},
    widgets::filter_line::FilterLine,
};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, 12, 0, 0).unwrap()
}

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn entry(name: &str) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        host: format!("{name}.example.com"),
        protocol: "sftp".to_string(),
        port: 22,
        username: "deploy".to_string(),
        options: BTreeMap::new(),
        password: None,
        source: ConnectionSource::Profile,
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
    }
}

fn grouped(name: &str, group: &str) -> ConnectionEntry {
    ConnectionEntry { group: Some(group.to_string()), ..entry(name) }
}

fn view_of(entries: Vec<ConnectionEntry>) -> ConnectionsView {
    let mut view = ConnectionsView::new();
    view.replace(entries);
    view
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
            id: "ftp",
            display_name: "ftps",
            description: "",
            limits_note: None,
            form: ConnectionForm::standard(21),
        },
    ]
}

struct Setup<'a> {
    connected: HashSet<&'a str>,
    lost: HashSet<&'a str>,
    failures: HashMap<String, String>,
    last_used: BTreeMap<String, DateTime<Utc>>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        Self { connected: HashSet::new(), lost: HashSet::new(), failures: HashMap::new(), last_used: BTreeMap::new() }
    }

    fn draw(&self, view: &ConnectionsView, width: u16, height: u16) -> Terminal<TestBackend> {
        let theme = indexed();
        let protocols = protocols();
        let context = ConnectionsContext {
            connected: &self.connected,
            lost: &self.lost,
            failures: &self.failures,
            protocols: &protocols,
            last_used: &self.last_used,
            now: now(),
        };
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render_connections_list(frame, frame.area(), view, &context, &theme)).unwrap();
        terminal
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

fn column_of(terminal: &Terminal<TestBackend>, y: u16, needle: &str) -> Option<u16> {
    let text = row(terminal, y);
    text.find(needle).map(|byte| text[..byte].width() as u16)
}

fn modifier_at(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> Modifier {
    terminal.backend().buffer()[(x, y)].modifier
}

#[test]
fn the_filter_line_shows_a_placeholder_then_the_typed_text_and_a_caret() {
    let theme = indexed();
    let mut view = view_of(vec![entry("web")]);

    let idle = Setup::new().draw(&view, 60, 6);
    assert!(row(&idle, 0).starts_with("/ filter"), "{:?}", row(&idle, 0));
    assert_eq!(cell_fg(&idle, 0, 0), theme.accent);
    assert_eq!(cell_fg(&idle, 2, 0), theme.fg_dim);

    view.start_filter();
    view.type_filter('w');
    view.type_filter('e');
    let typing = Setup::new().draw(&view, 60, 6);
    assert!(row(&typing, 0).starts_with("/ we\u{2588}"), "{:?}", row(&typing, 0));
    assert_eq!(cell_fg(&typing, 2, 0), theme.fg_bright);
}

#[test]
fn a_kept_filter_is_shown_without_a_caret() {
    let mut view = view_of(vec![entry("web"), entry("db")]);
    view.start_filter();
    view.type_filter('w');
    view.finish_filter();

    let terminal = Setup::new().draw(&view, 60, 6);

    assert!(row(&terminal, 0).starts_with("/ w "), "{:?}", row(&terminal, 0));
    assert!(!row(&terminal, 0).contains('\u{2588}'));
}

#[test]
fn the_count_is_right_aligned_and_shows_matches_while_filtering() {
    let mut view = view_of(vec![entry("web"), entry("db"), entry("mail")]);

    let total = Setup::new().draw(&view, 40, 6);
    assert!(row(&total, 0).trim_end().ends_with(" 3"), "{:?}", row(&total, 0));

    view.start_filter();
    view.type_filter('w');
    let filtered = Setup::new().draw(&view, 40, 6);
    assert!(row(&filtered, 0).trim_end().ends_with("1 of 3"), "{:?}", row(&filtered, 0));
    assert_eq!(cell_fg(&filtered, 39, 0), indexed().fg_dim);
}

#[test]
fn the_rule_under_the_filter_line_is_accent() {
    let theme = indexed();
    let terminal = Setup::new().draw(&view_of(vec![entry("web")]), 40, 6);

    assert_eq!(row(&terminal, 1), "\u{2500}".repeat(40));
    assert_eq!(cell_fg(&terminal, 5, 1), theme.accent);
}

#[test]
fn group_titles_are_dim_with_a_collapse_glyph_and_a_blank_row_above_all_but_the_first() {
    let theme = indexed();
    let mut view = view_of(vec![grouped("a", "production"), grouped("b", "storage")]);
    view.toggle();
    view.cursor = 2;
    view.toggle();

    let terminal = Setup::new().draw(&view, 60, 12);

    assert!(row(&terminal, 2).starts_with("\u{25be} production"), "{:?}", row(&terminal, 2));
    assert_eq!(cell_fg(&terminal, 0, 2), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, 4, 2), theme.fg_dim);
    assert!(row(&terminal, 3).contains(" a "), "{:?}", row(&terminal, 3));
    assert_eq!(row(&terminal, 4).trim(), "");
    assert!(row(&terminal, 5).starts_with("\u{25be} storage"), "{:?}", row(&terminal, 5));
}

#[test]
fn a_collapsed_group_shows_the_right_pointing_glyph_and_its_count() {
    let view = view_of(vec![grouped("a", "work"), grouped("b", "work")]);

    let terminal = Setup::new().draw(&view, 60, 6);

    assert!(row(&terminal, 2).starts_with("\u{25b8} work (2)"), "{:?}", row(&terminal, 2));
}

#[test]
fn nested_groups_indent_two_cells_per_depth() {
    let mut view = view_of(vec![grouped("deep", "a/b")]);
    view.toggle();
    view.cursor = 1;
    view.toggle();

    let terminal = Setup::new().draw(&view, 60, 8);

    assert!(row(&terminal, 2).starts_with("\u{25be} a"));
    assert!(row(&terminal, 3).starts_with("  \u{25be} b"), "{:?}", row(&terminal, 3));
    assert_eq!(column_of(&terminal, 4, "\u{2022}"), Some(4), "{:?}", row(&terminal, 4));
}

#[test]
fn a_row_shows_dot_name_protocol_and_user_at_host() {
    let terminal = Setup::new().draw(&view_of(vec![entry("web")]), 80, 6);

    let line = row(&terminal, 2);
    assert!(line.contains("\u{2022} web"), "{line:?}");
    assert!(line.contains("sftp"), "{line:?}");
    assert!(line.contains("deploy@web.example.com"), "{line:?}");
    assert!(!line.contains(":22"));
}

#[test]
fn the_port_is_shown_only_when_it_differs_from_the_default() {
    let odd = ConnectionEntry { port: 2222, ..entry("web") };

    let terminal = Setup::new().draw(&view_of(vec![odd]), 80, 6);

    assert!(row(&terminal, 2).contains("deploy@web.example.com:2222"), "{:?}", row(&terminal, 2));
}

#[test]
fn an_empty_username_leaves_out_the_at_sign() {
    let anonymous = ConnectionEntry { username: String::new(), ..entry("web") };

    let terminal = Setup::new().draw(&view_of(vec![anonymous]), 80, 6);

    assert!(row(&terminal, 2).contains(" web.example.com"), "{:?}", row(&terminal, 2));
    assert!(!row(&terminal, 2).contains('@'));
}

#[test]
fn the_protocol_column_uses_the_display_name_and_is_as_wide_as_the_longest() {
    let mut other = entry("db");
    other.protocol = "ftp".to_string();
    let terminal = Setup::new().draw(&view_of(vec![entry("web"), other]), 80, 8);

    let web = column_of(&terminal, 2, "deploy").unwrap();
    let db = column_of(&terminal, 3, "deploy").unwrap();
    assert_eq!(web, db);
    assert!(row(&terminal, 3).contains("ftps"), "{:?}", row(&terminal, 3));
}

#[test]
fn an_unknown_protocol_shows_its_raw_id() {
    let mut odd = entry("web");
    odd.protocol = "mystery".to_string();

    let terminal = Setup::new().draw(&view_of(vec![odd]), 80, 6);

    assert!(row(&terminal, 2).contains("mystery"));
}

#[test]
fn a_connected_session_has_an_ok_dot_and_a_bright_name() {
    let theme = indexed();
    let mut setup = Setup::new();
    setup.connected.insert("web");
    let mut view = view_of(vec![entry("web"), entry("db")]);
    view.cursor = 1;

    let terminal = setup.draw(&view, 80, 8);

    let (dot_x, dot_y) = find_text(&terminal, "\u{2022} web").unwrap();
    assert_eq!(cell_fg(&terminal, dot_x, dot_y), theme.ok);
    assert_eq!(cell_fg(&terminal, dot_x + 2, dot_y), theme.fg_bright);
}

#[test]
fn several_connected_sessions_all_show_ok_dots() {
    let theme = indexed();
    let mut setup = Setup::new();
    setup.connected.extend(["a", "b", "c"]);
    let mut view = view_of(vec![entry("a"), entry("b"), entry("c")]);
    view.cursor = 0;

    let terminal = setup.draw(&view, 80, 8);

    for y in 2..5 {
        assert_eq!(cell_fg(&terminal, 2, y), theme.ok, "row {y}");
    }
}

#[test]
fn a_failed_connection_has_an_err_dot_and_its_first_message_line_replaces_the_host() {
    let theme = indexed();
    let mut setup = Setup::new();
    setup.failures.insert("web".to_string(), "Authentication failed for web\ntried 3 keys".to_string());

    let terminal = setup.draw(&view_of(vec![entry("web")]), 80, 6);

    let line = row(&terminal, 2);
    assert!(line.contains("Authentication failed for web"), "{line:?}");
    assert!(!line.contains("tried 3 keys") && !line.contains("deploy@"), "{line:?}");
    assert_eq!(cell_fg(&terminal, 2, 2), theme.err);
    let (x, y) = find_text(&terminal, "Authentication").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.err);
}

#[test]
fn an_idle_connection_has_an_idle_dot_and_dim_host() {
    let theme = indexed();

    let terminal = Setup::new().draw(&view_of(vec![entry("web"), entry("db")]), 80, 8);

    assert_eq!(cell_fg(&terminal, 2, 3), theme.idle);
    let (x, y) = find_text(&terminal, "deploy@db").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn the_focused_row_has_a_chevron_focus_background_and_bold_name() {
    let theme = indexed();
    let mut view = view_of(vec![entry("web"), entry("db")]);
    view.cursor = 1;

    let terminal = Setup::new().draw(&view, 80, 8);

    assert_eq!(terminal.backend().buffer()[(0, 3)].symbol(), "\u{203a}");
    assert_eq!(cell_fg(&terminal, 0, 3), theme.accent);
    let (x, y) = find_text(&terminal, "db").unwrap();
    assert_eq!((y, cell_fg(&terminal, x, y)), (3, theme.fg_focus));
    assert!(modifier_at(&terminal, x, y).contains(Modifier::BOLD));
    assert_eq!(cell_bg(&terminal, 79, 3), theme.row_focus);
    assert_eq!(terminal.backend().buffer()[(0, 2)].symbol(), " ");
}

#[test]
fn the_focused_row_stays_visible_when_the_list_scrolls_past_spacer_rows() {
    let entries: Vec<ConnectionEntry> =
        (0..12).map(|index| grouped(&format!("c{index}"), &format!("g{index:02}"))).collect();
    let mut view = view_of(entries);
    view.cursor = 11;

    let terminal = Setup::new().draw(&view, 40, 8);

    assert!(screen(&terminal).contains("g11"), "{}", screen(&terminal));
    assert_eq!(terminal.backend().buffer()[(0, 7)].symbol(), "\u{25b8}");
}

#[test]
fn missing_and_shadowed_entries_keep_their_warning_text() {
    let missing = ConnectionEntry::orphan_labels("old".into(), Default::default(), ConnectionSource::MissingSshHost);
    let shadowed = ConnectionEntry::orphan_labels("web1".into(), Default::default(), ConnectionSource::ShadowedSshHost);

    let text = screen(&Setup::new().draw(&view_of(vec![missing, shadowed]), 80, 8));

    assert!(text.contains("! old  not in ~/.ssh/config"), "{text}");
    assert!(text.contains("! web1  hidden by a saved connection"), "{text}");
}

#[test]
fn columns_are_aligned_for_wide_names() {
    let wide = entry("\u{65e5}\u{672c}\u{8a9e}");
    let terminal = Setup::new().draw(&view_of(vec![wide, entry("ascii")]), 80, 8);

    assert_eq!(column_of(&terminal, 2, "sftp"), column_of(&terminal, 3, "sftp"));
}

#[test]
fn a_five_hundred_character_name_host_and_group_are_cut_and_sanitised() {
    let long = "n".repeat(500);
    let hostile = ConnectionEntry {
        host: format!("h\u{202e}{}", "x".repeat(500)),
        group: Some(format!("g\n{}", "y".repeat(500))),
        ..entry(&long)
    };
    let mut view = view_of(vec![hostile]);
    view.toggle();

    for width in [80, 40, 20, 8] {
        let terminal = Setup::new().draw(&view, width, 8);
        for y in 0..8 {
            assert!(row(&terminal, y).width() <= usize::from(width), "width {width} row {y}");
        }
        let text = screen(&terminal);
        assert!(!text.contains('\u{202e}') && !text.contains("g\n"));
    }
}

#[test]
fn an_empty_list_draws_only_the_filter_line() {
    let terminal = Setup::new().draw(&view_of(Vec::new()), 40, 6);

    assert!(row(&terminal, 0).starts_with("/ filter"));
    assert_eq!(row(&terminal, 3).trim(), "");
}

#[test]
fn tiny_areas_do_not_panic() {
    let mut view = view_of(vec![grouped("a", "g"), entry("b")]);
    view.toggle();
    let mut setup = Setup::new();
    setup.failures.insert("b".to_string(), "boom".to_string());

    for width in 0..=12 {
        for height in 0..=4 {
            let _ = setup.draw(&view, width, height);
            let _ = Setup::new().draw(&view_of(Vec::new()), width, height);
        }
    }
}

#[test]
fn an_indexed_theme_keeps_the_three_dot_states_distinct() {
    let theme = indexed();

    assert_ne!(theme.ok, theme.err);
    assert_ne!(theme.ok, theme.idle);
    assert_ne!(theme.err, theme.idle);
    assert_ne!(theme.row_focus, theme.bg);
}

#[test]
fn the_list_no_longer_uses_the_reversed_modifier_or_a_box() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
    assert!(!source.contains("Borders"));
}

#[test]
fn an_unknown_protocol_id_is_sanitised_in_the_row() {
    let mut odd = entry("web");
    odd.protocol = "my\u{202e}stery\n".to_string();

    let text = screen(&Setup::new().draw(&view_of(vec![odd]), 80, 6));

    assert!(!text.contains('\u{202e}') && text.contains("my?stery?"), "{text}");
}

fn used(setup: &mut Setup<'_>, name: &str, age: Duration) {
    setup.last_used.insert(name.to_string(), now() - age);
}

fn right_edge(terminal: &Terminal<TestBackend>, y: u16, needle: &str) -> Option<usize> {
    column_of(terminal, y, needle).map(|column| usize::from(column) + needle.width())
}

#[test]
fn a_used_connection_shows_its_age_at_the_right_edge() {
    let mut setup = Setup::new();
    used(&mut setup, "web", Duration::hours(3));

    let terminal = setup.draw(&view_of(vec![entry("web")]), 80, 6);

    assert_eq!(right_edge(&terminal, 2, "3 h ago"), Some(80), "{:?}", row(&terminal, 2));
}

#[test]
fn a_connection_never_used_says_never_at_the_right_edge() {
    let terminal = Setup::new().draw(&view_of(vec![entry("web")]), 80, 6);

    assert_eq!(right_edge(&terminal, 2, "never"), Some(80), "{:?}", row(&terminal, 2));
}

#[test]
fn every_row_ends_at_the_same_column_whatever_the_text() {
    let mut setup = Setup::new();
    used(&mut setup, "a", Duration::minutes(3));
    used(&mut setup, "b", Duration::days(12));

    let terminal = setup.draw(&view_of(vec![entry("a"), entry("b"), entry("c")]), 80, 8);

    assert_eq!(right_edge(&terminal, 2, "3 min ago"), Some(80));
    assert_eq!(right_edge(&terminal, 3, "12 d ago"), Some(80));
    assert_eq!(right_edge(&terminal, 4, "never"), Some(80));
}

#[test]
fn the_age_is_dim() {
    let theme = indexed();
    let mut setup = Setup::new();
    used(&mut setup, "web", Duration::hours(3));

    let terminal = setup.draw(&view_of(vec![entry("web")]), 80, 6);

    let (x, y) = find_text(&terminal, "3 h ago").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn the_host_stays_on_the_row_beside_the_age() {
    let mut setup = Setup::new();
    used(&mut setup, "web", Duration::hours(3));

    let terminal = setup.draw(&view_of(vec![entry("web")]), 80, 6);

    let line = row(&terminal, 2);
    assert!(line.contains("deploy@web.example.com") && line.contains("3 h ago"), "{line:?}");
}

#[test]
fn a_failed_connection_keeps_its_message_and_still_shows_the_age() {
    let mut setup = Setup::new();
    setup.failures.insert("web".to_string(), "Authentication failed for web".to_string());
    used(&mut setup, "web", Duration::days(2));

    let terminal = setup.draw(&view_of(vec![entry("web")]), 80, 6);

    let line = row(&terminal, 2);
    assert!(line.contains("Authentication failed for web") && line.contains("2 d ago"), "{line:?}");
}

#[test]
fn a_narrow_list_drops_the_age_before_it_squeezes_the_host() {
    let mut setup = Setup::new();
    used(&mut setup, "web", Duration::hours(3));
    let view = view_of(vec![entry("web")]);

    let wide = setup.draw(&view, 60, 6);
    let narrow = setup.draw(&view, 40, 6);

    assert!(row(&wide, 2).contains("3 h ago"), "{:?}", row(&wide, 2));
    assert!(!row(&narrow, 2).contains("3 h ago"), "{:?}", row(&narrow, 2));
    assert!(row(&narrow, 2).contains("deploy@web.exa"), "{:?}", row(&narrow, 2));
}

#[test]
fn the_age_never_runs_over_the_width_at_any_size() {
    let mut setup = Setup::new();
    used(&mut setup, "web", Duration::days(400));
    let view = view_of(vec![entry("web"), grouped("db", "prod")]);

    for width in 0..=100 {
        let terminal = setup.draw(&view, width, 8);
        for y in 0..8 {
            assert!(row(&terminal, y).width() <= usize::from(width), "{width}x{y}");
        }
    }
}

#[test]
fn a_group_row_has_no_age() {
    let mut setup = Setup::new();
    used(&mut setup, "web", Duration::hours(3));

    let terminal = setup.draw(&view_of(vec![grouped("web", "prod")]), 80, 8);

    let group = (2..8).map(|y| row(&terminal, y)).find(|line| line.contains("prod")).unwrap();
    assert!(!group.contains("ago") && !group.contains("never"), "{group:?}");
}

#[test]
fn a_hidden_ssh_label_row_has_no_age() {
    let mut setup = Setup::new();
    used(&mut setup, "ghost", Duration::hours(3));
    let ghost = ConnectionEntry { source: ConnectionSource::MissingSshHost, ..entry("ghost") };

    let terminal = setup.draw(&view_of(vec![ghost]), 80, 6);

    let line = row(&terminal, 2);
    assert!(line.contains("not in ~/.ssh/config") && !line.contains("ago") && !line.contains("never"), "{line:?}");
}

#[test]
fn the_age_column_is_shown_on_every_row_or_on_none_whatever_the_nesting() {
    let mut setup = Setup::new();
    used(&mut setup, "web", Duration::hours(3));
    let mut view = view_of(vec![entry("web"), grouped("db", "a"), grouped("cache", "a/b/c")]);
    let mut index = 0;
    while index < view.rows().len() {
        view.cursor = index;
        view.expand();
        index += 1;
    }

    for width in 20..=80 {
        let terminal = setup.draw(&view, width, 14);
        let rows: Vec<String> = (2..14).map(|y| row(&terminal, y)).filter(|line| line.contains('\u{2022}')).collect();
        let with_age = rows.iter().filter(|line| line.contains("ago") || line.contains("never")).count();
        assert_eq!(rows.len(), 3, "{width}");
        assert!(with_age == 0 || with_age == rows.len(), "{width}: {rows:?}");
    }
}

#[test]
fn a_lost_session_has_an_err_dot_and_says_so() {
    let theme = indexed();
    let mut setup = Setup::new();
    setup.lost.insert("web");
    let view = view_of(vec![entry("web")]);

    let terminal = setup.draw(&view, 80, 6);

    let (dot_x, dot_y) = find_text(&terminal, "\u{2022} web").unwrap();
    assert_eq!(cell_fg(&terminal, dot_x, dot_y), theme.err);
    let (x, y) = find_text(&terminal, "connection lost").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.err);
}
