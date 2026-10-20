use porthmos_core::{ConnectionForm, ProtocolInfo};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text},
    theme::{ColorDepth, Theme, ThemeSettings},
    widgets::dialog::{FieldKind, FormField, FormLayout},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn protocols() -> Vec<ProtocolInfo> {
    let info = |id: &'static str, name: &'static str, description: &'static str| ProtocolInfo {
        id,
        display_name: name,
        description,
        limits_note: None,
        form: ConnectionForm::standard(22),
    };
    vec![
        info("sftp", "sftp", "ssh file transfer"),
        info("ftp", "ftp / ftps", "legacy hosting, tls"),
        info("s3", "s3-compatible", "aws, r2, minio, b2"),
    ]
}

fn protocol_choices() -> Vec<(String, String)> {
    vec![("sftp".into(), "sftp".into()), ("ftp".into(), "ftp / ftps".into()), ("s3".into(), "s3-compatible".into())]
}

fn toggle(key: &'static str, label: &str, value: &str) -> FormField {
    FormField::choice(key, label, vec![("true".into(), "Yes".into()), ("false".into(), "No".into())], value)
}

fn sample() -> FormDialog {
    let mut form = FormDialog::new(
        "Add connection",
        vec![
            FormField::choice("protocol", "Protocol", protocol_choices(), "s3"),
            FormField::text("name", "Name", "backups"),
            FormField::text("host", "Endpoint", "s3.eu-central-1.amazonaws.com"),
            FormField::text("bucket", "Bucket", "tc-backups"),
            FormField::masked("password", "Secret", ""),
            toggle("tls", "credentials from ~/.aws/credentials", "true"),
            toggle("path_style", "path-style addressing", "false"),
        ],
    );
    form.layout = FormLayout::Screen;
    form.focused = 3;
    form
}

fn draw(form: &FormDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let protocols = protocols();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_connection_form(frame, frame.area(), form, &protocols, &theme)).unwrap();
    terminal
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

fn left_part(terminal: &Terminal<TestBackend>, y: u16, width: usize) -> String {
    row(terminal, y).chars().take(width).collect()
}

fn right_part(terminal: &Terminal<TestBackend>, y: u16, from: usize) -> String {
    row(terminal, y).chars().skip(from).collect()
}

#[test]
fn the_protocol_list_shows_names_and_descriptions_two_rows_each() {
    let terminal = draw(&sample(), 120, 20);

    assert_eq!(left_part(&terminal, 0, 8), "protocol");
    assert!(left_part(&terminal, 2, 42).contains("sftp"), "{:?}", row(&terminal, 2));
    assert!(left_part(&terminal, 3, 42).contains("ssh file transfer"));
    assert!(left_part(&terminal, 4, 42).contains("ftp / ftps"));
    assert!(left_part(&terminal, 5, 42).contains("legacy hosting, tls"));
    assert!(left_part(&terminal, 6, 42).contains("s3-compatible"));
    assert!(left_part(&terminal, 7, 42).contains("aws, r2, minio, b2"));
}

#[test]
fn the_selected_protocol_has_a_chevron_focus_background_and_bold_name() {
    let theme = indexed();
    let mut form = sample();
    form.focused = 0;

    let terminal = draw(&form, 120, 20);

    assert_eq!(terminal.backend().buffer()[(0, 6)].symbol(), "\u{203a}");
    assert_eq!(cell_fg(&terminal, 0, 6), theme.accent);
    let x = column_of(&terminal, 6, "s3-compatible").unwrap();
    assert_eq!(cell_fg(&terminal, x, 6), theme.fg_focus);
    assert!(modifier_at(&terminal, x, 6).contains(Modifier::BOLD));
    assert_eq!(cell_bg(&terminal, x, 6), theme.row_focus);
    assert_eq!(cell_bg(&terminal, x, 7), theme.row_focus);
    assert_eq!(terminal.backend().buffer()[(0, 2)].symbol(), " ");
    let description_x = column_of(&terminal, 7, "aws").unwrap();
    assert_eq!(cell_fg(&terminal, description_x, 7), theme.fg_dim);
}

#[test]
fn the_selection_background_dims_when_the_form_has_the_focus() {
    let theme = indexed();
    let terminal = draw(&sample(), 120, 20);

    let x = column_of(&terminal, 6, "s3-compatible").unwrap();
    assert_eq!(cell_bg(&terminal, x, 6), theme.row_focus_inactive);
    assert_ne!(theme.row_focus_inactive, theme.row_focus);
}

#[test]
fn the_list_rule_is_accent_only_while_the_list_has_the_focus() {
    let theme = indexed();
    let mut form = sample();

    let in_form = draw(&form, 120, 20);
    form.focused = 0;
    let in_list = draw(&form, 120, 20);

    assert_eq!(cell_fg(&in_form, 3, 1), theme.line);
    assert_eq!(cell_fg(&in_list, 3, 1), theme.accent);
}

#[test]
fn an_unregistered_protocol_is_listed_as_not_available_without_a_description() {
    let mut form = sample();
    form.fields[0] = FormField::choice(
        "protocol",
        "Protocol",
        vec![("sftp".into(), "sftp".into()), ("old".into(), "old (not available)".into())],
        "old",
    );

    let terminal = draw(&form, 120, 20);

    assert!(left_part(&terminal, 4, 42).contains("old (not available)"));
    assert_eq!(left_part(&terminal, 5, 42).trim(), "");
}

#[test]
fn between_60_and_79_columns_descriptions_are_hidden_and_below_60_the_list_is_hidden() {
    let medium = draw(&sample(), 70, 20);
    assert!(!screen(&medium).contains("ssh file transfer"));
    assert!(screen(&medium).contains("ftp / ftps"));

    let narrow = draw(&sample(), 50, 20);
    assert!(!screen(&narrow).contains("ssh file transfer") && !screen(&narrow).contains("legacy"));
    assert!(row(&narrow, 0).contains("\u{25c0} s3-compatible \u{25b6}"), "{:?}", row(&narrow, 0));
}

#[test]
fn the_right_column_title_is_the_selected_protocol_with_an_accent_rule() {
    let theme = indexed();
    let terminal = draw(&sample(), 120, 20);

    assert!(right_part(&terminal, 0, 46).starts_with("s3-compatible"), "{:?}", right_part(&terminal, 0, 46));
    assert_eq!(cell_fg(&terminal, 46, 0), theme.fg_dim);
    assert_eq!(terminal.backend().buffer()[(50, 1)].symbol(), "\u{2500}");
    assert_eq!(cell_fg(&terminal, 50, 1), theme.accent);
}

#[test]
fn fields_are_a_label_a_value_and_an_underline_row() {
    let terminal = draw(&sample(), 120, 20);

    assert!(right_part(&terminal, 2, 46).starts_with("Name"), "{:?}", right_part(&terminal, 2, 46));
    assert_eq!(column_of(&terminal, 2, "backups"), Some(61));
    assert_eq!(terminal.backend().buffer()[(61, 3)].symbol(), "\u{2500}");
    assert_eq!(terminal.backend().buffer()[(119, 3)].symbol(), "\u{2500}");
    assert_eq!(terminal.backend().buffer()[(60, 3)].symbol(), " ");
    assert!(right_part(&terminal, 4, 46).starts_with("Endpoint"));
}

#[test]
fn the_focused_field_has_a_bright_label_and_an_accent_underline() {
    let theme = indexed();
    let terminal = draw(&sample(), 120, 20);

    let bucket_row = 6;
    assert!(right_part(&terminal, bucket_row, 46).starts_with("Bucket"));
    assert_eq!(cell_fg(&terminal, 46, bucket_row), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, 62, bucket_row + 1), theme.accent);
    assert_eq!(cell_fg(&terminal, 46, 2), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, 62, 3), theme.line_strong);
    assert_eq!(cell_fg(&terminal, 62, bucket_row), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, 62, 2), theme.fg);
}

#[test]
fn masked_values_show_bullets_and_never_the_text() {
    let mut form = sample();
    form.fields[4].value = "hunter2".to_string();
    form.fields[4].cursor = 7;

    let terminal = draw(&form, 120, 20);

    assert!(!screen(&terminal).contains("hunter2"));
    assert!(screen(&terminal).contains("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"));
}

#[test]
fn a_saved_secret_shows_bullets_and_saved() {
    let mut form = sample();
    form.fields[4] = FormField::saved_secret("password", "Secret");

    let terminal = draw(&form, 120, 20);

    assert!(screen(&terminal).contains("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022} (saved)"));
}

#[test]
fn the_caret_is_the_terminal_cursor_at_the_caret_column_in_display_width() {
    let mut form = sample();
    form.focused = 1;
    form.fields[1].value = "\u{65e5}\u{672c}ab".to_string();
    form.fields[1].cursor = 3;

    let mut terminal = draw(&form, 120, 20);

    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (61 + 4 + 1, 2));
}

#[test]
fn the_cursor_is_only_shown_in_text_fields() {
    let mut on_toggle = sample();
    on_toggle.focused = 5;
    let mut on_list = sample();
    on_list.focused = 0;
    let mut on_text = sample();
    on_text.focused = 1;

    let mut hidden_toggle = draw(&on_toggle, 120, 20);
    let mut hidden_list = draw(&on_list, 120, 20);
    let mut shown = draw(&on_text, 120, 20);

    assert_eq!(hidden_toggle.get_cursor_position().unwrap().y, 0);
    assert_eq!(hidden_list.get_cursor_position().unwrap().y, 0);
    assert_eq!(shown.get_cursor_position().unwrap().y, 2);
}

#[test]
fn toggles_are_drawn_as_checkboxes_after_the_text_fields() {
    let theme = indexed();
    let terminal = draw(&sample(), 120, 20);

    let on =
        (0..20).find(|y| right_part(&terminal, *y, 46).contains("[x] credentials from ~/.aws/credentials")).unwrap();
    let off = (0..20).find(|y| right_part(&terminal, *y, 46).contains("[ ] path-style addressing")).unwrap();
    assert_eq!(off, on + 1);
    assert!(on > 8);
    let on_x = column_of(&terminal, on, "[x]").unwrap();
    let off_x = column_of(&terminal, off, "[ ]").unwrap();
    assert_eq!((on_x, off_x), (61, 61));
    assert_eq!(cell_fg(&terminal, on_x + 1, on), theme.accent);
    assert_eq!(cell_fg(&terminal, off_x + 1, off), theme.fg_dim);
}

#[test]
fn a_focused_toggle_has_a_bright_label() {
    let theme = indexed();
    let mut form = sample();
    form.focused = 6;

    let terminal = draw(&form, 120, 20);

    let (x, y) = find_text(&terminal, "path-style addressing").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
}

#[test]
fn choice_fields_show_arrows_and_the_selected_label() {
    let mut form = sample();
    form.fields.insert(
        4,
        FormField::choice(
            "mode",
            "TLS mode",
            vec![("explicit".into(), "Explicit".into()), ("implicit".into(), "Implicit".into())],
            "implicit",
        ),
    );

    let terminal = draw(&form, 120, 24);

    assert!(screen(&terminal).contains("\u{25c0} Implicit \u{25b6}"));
}

#[test]
fn the_error_and_the_keyring_hint_are_drawn_at_the_bottom() {
    let theme = indexed();
    let mut form = sample();
    form.error = Some("Name is required".to_string());
    form.hint = Some("Passwords are not saved: no system keyring.".to_string());

    let terminal = draw(&form, 120, 20);

    assert!(
        right_part(&terminal, 19, 46).starts_with("\u{2022} Name is required"),
        "{:?}",
        right_part(&terminal, 19, 46)
    );
    assert_eq!(cell_fg(&terminal, 46, 19), theme.err);
    assert!(right_part(&terminal, 18, 46).starts_with("Passwords are not saved"));
    assert_eq!(cell_fg(&terminal, 46, 18), theme.fg_dim);
}

fn many_fields(count: usize) -> FormDialog {
    let mut fields = vec![FormField::choice("protocol", "Protocol", protocol_choices(), "sftp")];
    for index in 0..count {
        let key: &'static str = Box::leak(format!("field{index}").into_boxed_str());
        fields.push(FormField::text(key, format!("Label{index:02}"), format!("value{index}")));
    }
    let mut form = FormDialog::new("Add connection", fields);
    form.layout = FormLayout::Screen;
    form
}

#[test]
fn scrolling_keeps_the_focused_field_visible() {
    let mut form = many_fields(20);

    form.focused = 20;
    let last = draw(&form, 120, 12);
    assert!(screen(&last).contains("Label19"), "{}", screen(&last));
    assert!(!screen(&last).contains("Label00"));

    form.focused = 1;
    let first = draw(&form, 120, 12);
    assert!(screen(&first).contains("Label00"));

    form.focused = 10;
    let middle = draw(&form, 120, 12);
    assert!(screen(&middle).contains("Label09"), "{}", screen(&middle));
}

#[test]
fn hostile_values_and_errors_are_sanitised_and_cut() {
    let mut form = sample();
    form.fields[1].value = format!("a\u{202e}b\nc{}", "x".repeat(500));
    form.fields[1].cursor = 5;
    form.error = Some(format!("bad\u{1b}[31m\nline{}", "y".repeat(800)));
    form.fields[3].value = "\u{65e5}\u{672c}".repeat(200);

    for width in [120, 80, 60, 40, 20] {
        let terminal = draw(&form, width, 20);
        let shown = screen(&terminal);
        for y in 0..20 {
            assert!(row(&terminal, y).width() <= usize::from(width), "width {width} row {y}");
        }
        assert!(!shown.contains('\u{202e}') && !shown.contains('\u{1b}'));
    }
}

#[test]
fn tiny_areas_do_not_panic() {
    let mut with_error = sample();
    with_error.error = Some("boom".to_string());
    with_error.hint = Some("hint".to_string());
    let lonely = {
        let mut form =
            FormDialog::new("x", vec![FormField::choice("protocol", "Protocol", protocol_choices(), "sftp")]);
        form.layout = FormLayout::Screen;
        form
    };

    for form in [&sample(), &with_error, &lonely] {
        for width in (0..=12).chain([90]) {
            for height in 0..=14 {
                let _ = draw(form, width, height);
            }
        }
    }
}

#[test]
fn an_indexed_theme_keeps_focus_and_selection_distinct() {
    let theme = indexed();

    assert_ne!(theme.accent, theme.line_strong);
    assert_ne!(theme.row_focus, theme.bg);
    assert_ne!(theme.row_focus_inactive, theme.bg);
    assert_ne!(theme.fg_bright, theme.fg_dim);
}

#[test]
fn the_form_no_longer_uses_the_reversed_modifier_or_a_box() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
    assert!(!source.contains("Borders"));
}

#[test]
fn a_saved_secret_kind_is_not_confused_with_a_masked_one() {
    let field = FormField::saved_secret("password", "Secret");

    assert_eq!(field.kind, FieldKind::SavedSecret);
}

fn long_value_form(cursor: usize) -> FormDialog {
    let mut form = sample();
    form.focused = 1;
    form.fields[1].value = "0123456789".repeat(10);
    form.fields[1].cursor = cursor;
    form
}

#[test]
fn the_caret_and_the_visible_text_follow_the_cursor_in_a_long_value() {
    for cursor in [0, 1, 27, 50, 99, 100] {
        let form = long_value_form(cursor);
        let mut terminal = draw(&form, 80, 20);

        let position = terminal.get_cursor_position().unwrap();
        assert_eq!(position.y, 2, "cursor {cursor}");
        assert!(position.x >= 36 + 15 && position.x < 80, "cursor {cursor}: {position:?}");
        if cursor < 100 {
            let expected = ((cursor % 10) as u8 + b'0') as char;
            assert_eq!(
                terminal.backend().buffer()[(position.x, position.y)].symbol(),
                expected.to_string(),
                "cursor {cursor}"
            );
        }
    }
}

#[test]
fn home_shows_the_start_of_a_long_value_and_end_shows_its_tail() {
    let at_start = draw(&long_value_form(0), 80, 20);
    let at_end = draw(&long_value_form(100), 80, 20);

    assert!(row(&at_start, 2).contains("0123456789012"), "{:?}", row(&at_start, 2));
    assert!(row(&at_end, 2).trim_end().ends_with("789"), "{:?}", row(&at_end, 2));
}
