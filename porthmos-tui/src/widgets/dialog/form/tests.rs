use crossterm::event::{KeyCode, KeyEventKind, KeyEventState, KeyModifiers};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell},
    theme::{ColorDepth, ThemeSettings},
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn sample_form() -> FormDialog {
    FormDialog::new(
        "Add connection",
        vec![
            FormField::text("name", "Name", ""),
            FormField::text("host", "Host", ""),
            FormField::masked("password", "Password", ""),
        ],
    )
}

fn choice_form() -> FormDialog {
    FormDialog::new(
        "Add connection",
        vec![
            FormField::choice(
                "protocol",
                "Protocol",
                vec![("sftp".into(), "SFTP".into()), ("ftp".into(), "FTP".into()), ("s3".into(), "S3".into())],
                "ftp",
            ),
            FormField::text("name", "Name", ""),
        ],
    )
}

fn rendered(form: &FormDialog) -> String {
    let backend = ratatui::backend::TestBackend::new(60, 10);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|frame| render_form(frame, frame.area(), form, &Theme::default())).unwrap();
    terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn new_form_starts_focused_on_the_first_field() {
    let form = sample_form();
    assert_eq!(form.focused, 0);
}

#[test]
fn typing_inserts_into_the_focused_field_only() {
    let mut form = sample_form();
    form.handle_key(key(KeyCode::Char('a')));
    assert_eq!(form.fields[0].value, "a");
    assert_eq!(form.fields[1].value, "");
}

#[test]
fn tab_moves_focus_to_the_next_field_and_wraps() {
    let mut form = sample_form();
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.focused, 1);
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.focused, 2);
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.focused, 0);
}

#[test]
fn backtab_moves_focus_to_the_previous_field_and_wraps() {
    let mut form = sample_form();
    form.handle_key(key(KeyCode::BackTab));
    assert_eq!(form.focused, 2);
}

#[test]
fn tab_then_typing_inserts_into_the_newly_focused_field() {
    let mut form = sample_form();
    form.handle_key(key(KeyCode::Tab));
    form.handle_key(key(KeyCode::Char('h')));
    assert_eq!(form.fields[1].value, "h");
}

#[test]
fn enter_submits_every_fields_key_and_value_in_order() {
    let mut form = sample_form();
    form.fields[0].value = "prod".to_string();
    form.fields[1].value = "server.example.com".to_string();
    form.fields[2].value = "secret".to_string();

    assert_eq!(
        form.handle_key(key(KeyCode::Enter)),
        FormOutcome::Submitted(vec![
            ("name", "prod".to_string()),
            ("host", "server.example.com".to_string()),
            ("password", "secret".to_string()),
        ])
    );
}

#[test]
fn a_choice_starts_on_the_given_value_and_falls_back_to_the_first() {
    assert_eq!(choice_form().value("protocol").as_deref(), Some("ftp"));
    let unknown = FormField::choice("p", "P", vec![("a".into(), "A".into()), ("b".into(), "B".into())], "zzz");
    assert_eq!(unknown.submitted_value(), "a");
}

#[test]
fn left_and_right_cycle_a_focused_choice_with_wrapping_and_report_the_change() {
    let mut form = choice_form();

    assert_eq!(form.handle_key(key(KeyCode::Right)), FormOutcome::ChoiceChanged { key: "protocol" });
    assert_eq!(form.value("protocol").as_deref(), Some("s3"));
    form.handle_key(key(KeyCode::Right));
    assert_eq!(form.value("protocol").as_deref(), Some("sftp"));
    form.handle_key(key(KeyCode::Left));
    assert_eq!(form.value("protocol").as_deref(), Some("s3"));
}

#[test]
fn typing_and_deleting_do_nothing_on_a_choice() {
    let mut form = choice_form();

    for code in [KeyCode::Char('x'), KeyCode::Backspace, KeyCode::Delete, KeyCode::Home, KeyCode::End] {
        assert_eq!(form.handle_key(key(code)), FormOutcome::Pending);
    }
    assert_eq!(form.value("protocol").as_deref(), Some("ftp"));
}

#[test]
fn a_choice_submits_its_value_not_its_label() {
    let mut form = choice_form();

    assert_eq!(
        form.handle_key(key(KeyCode::Enter)),
        FormOutcome::Submitted(vec![("protocol", "ftp".to_string()), ("name", String::new())])
    );
}

#[test]
fn a_choice_renders_its_label_between_arrows() {
    let content = rendered(&choice_form());

    assert!(content.contains("Protocol"), "{content}");
    assert!(content.contains("◀ FTP ▶"), "{content}");
}

#[test]
fn the_form_no_longer_shows_a_fixed_protocol_line() {
    assert!(!rendered(&sample_form()).contains("SFTP"));
}

#[test]
fn esc_cancels() {
    let mut form = sample_form();
    assert_eq!(form.handle_key(key(KeyCode::Esc)), FormOutcome::Cancelled);
}

#[test]
fn backspace_removes_from_the_focused_field_at_the_cursor() {
    let mut form = sample_form();
    form.fields[0].value = "ab".to_string();
    form.fields[0].cursor = 2;
    form.handle_key(key(KeyCode::Backspace));
    assert_eq!(form.fields[0].value, "a");
}

#[test]
fn masked_field_renders_bullets_not_the_value() {
    let mut form = sample_form();
    form.handle_key(key(KeyCode::Tab));
    form.handle_key(key(KeyCode::Tab));
    for c in "secret".chars() {
        form.handle_key(key(KeyCode::Char(c)));
    }

    let backend = ratatui::backend::TestBackend::new(60, 10);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|frame| render_form(frame, frame.area(), &form, &Theme::default())).unwrap();

    let content: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();

    assert!(content.contains("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"));
    assert!(!content.contains("secret"));
}

#[test]
fn error_message_renders_when_set() {
    let mut form = sample_form();
    form.error = Some("Host can't be empty".to_string());

    let backend = ratatui::backend::TestBackend::new(60, 10);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|frame| render_form(frame, frame.area(), &form, &Theme::default())).unwrap();

    let content: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();

    assert!(content.contains("Host can't be empty"));
}

fn press(form: &mut FormDialog, code: KeyCode) -> FormOutcome {
    form.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn screen_of(form: &FormDialog) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(70, 10)).unwrap();
    terminal.draw(|frame| render_form(frame, frame.area(), form, &Theme::default())).unwrap();
    terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn a_saved_secret_is_kept_until_edited() {
    let field = FormField::saved_secret("password", "Password");
    assert_eq!(field.submitted_value(), KEPT_SECRET);

    let mut form = FormDialog::new("t", vec![field]);
    press(&mut form, KeyCode::Char('x'));
    press(&mut form, KeyCode::Char('y'));
    assert_eq!(form.fields[0].submitted_value(), "xy");
}

#[test]
fn backspace_or_delete_on_a_saved_secret_clears_it() {
    for code in [KeyCode::Backspace, KeyCode::Delete] {
        let mut form = FormDialog::new("t", vec![FormField::saved_secret("password", "Password")]);
        press(&mut form, code);
        assert_eq!(form.fields[0].submitted_value(), "");
    }
}

#[test]
fn moving_around_or_submitting_keeps_a_saved_secret() {
    let mut form =
        FormDialog::new("t", vec![FormField::text("a", "A", ""), FormField::saved_secret("password", "Password")]);
    press(&mut form, KeyCode::Tab);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Right);
    press(&mut form, KeyCode::Home);
    press(&mut form, KeyCode::End);
    press(&mut form, KeyCode::BackTab);
    press(&mut form, KeyCode::Tab);

    match press(&mut form, KeyCode::Enter) {
        FormOutcome::Submitted(values) => assert_eq!(values[1], ("password", KEPT_SECRET.to_string())),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn typing_can_never_produce_the_kept_marker() {
    let mut form = FormDialog::new("t", vec![FormField::masked("password", "Password", "")]);
    for character in KEPT_SECRET.chars().filter(|character| !character.is_control()) {
        press(&mut form, KeyCode::Char(character));
    }
    assert_ne!(form.fields[0].submitted_value(), KEPT_SECRET);
    assert!(KEPT_SECRET.chars().any(char::is_control));
}

#[test]
fn a_saved_secret_shows_that_it_is_saved_without_a_value() {
    let form = FormDialog::new("t", vec![FormField::saved_secret("password", "Password")]);
    let screen = screen_of(&form);
    assert!(screen.contains("(saved)"), "{screen}");
    assert!(screen.contains("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"), "{screen}");
}

#[test]
fn an_edited_saved_secret_is_shown_masked() {
    let mut form = FormDialog::new("t", vec![FormField::saved_secret("password", "Password")]);
    press(&mut form, KeyCode::Char('s'));
    press(&mut form, KeyCode::Char('3'));
    let screen = screen_of(&form);
    assert!(!screen.contains("(saved)") && !screen.contains("s3"), "{screen}");
    assert!(screen.contains("\u{2022}\u{2022}"), "{screen}");
}

#[test]
fn a_hint_line_is_rendered_and_absent_by_default() {
    let mut form = FormDialog::new("t", vec![FormField::text("a", "A", "")]);
    assert!(form.hint.is_none());
    let plain = screen_of(&form);
    form.hint = Some("Passwords are not saved".into());
    let hinted = screen_of(&form);
    assert!(!plain.contains("Passwords are not saved"));
    assert!(hinted.contains("Passwords are not saved"), "{hinted}");
}

#[test]
fn a_submitted_form_never_prints_its_values() {
    let outcome = FormOutcome::Submitted(vec![("password", "hunter2".to_string()), ("host", "h".to_string())]);

    let printed = format!("{outcome:?}");

    assert!(!printed.contains("hunter2"), "{printed}");
    assert!(printed.contains("password") && printed.contains("host"), "{printed}");
}

#[test]
fn the_error_line_and_hint_use_the_theme_colours() {
    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default());
    let mut form = sample_form();
    form.hint = Some("A hint".to_string());
    form.error = Some("Host can't be empty".to_string());

    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 10)).unwrap();
    terminal.draw(|frame| render_form(frame, frame.area(), &form, &theme)).unwrap();

    let (hint_column, hint_row) = find_text(&terminal, "A hint").expect("hint is drawn");
    let (error_column, error_row) = find_text(&terminal, "Host can't").expect("error is drawn");
    assert_eq!(cell_fg(&terminal, hint_column, hint_row), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, error_column, error_row), theme.err);
}

fn ctrl(character: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(character),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn toggle(key: &'static str, label: &str, value: &str) -> FormField {
    FormField::choice(key, label, vec![("true".into(), "Yes".into()), ("false".into(), "No".into())], value)
}

fn screen_form() -> FormDialog {
    let mut form = FormDialog::new(
        "Add connection",
        vec![
            FormField::choice(
                "protocol",
                "Protocol",
                vec![("sftp".into(), "SFTP".into()), ("ftp".into(), "FTP".into()), ("s3".into(), "S3".into())],
                "ftp",
            ),
            FormField::text("name", "Name", ""),
            FormField::text("host", "Host", ""),
            toggle("tls", "TLS", "false"),
            FormField::masked("password", "Password", ""),
        ],
    );
    form.layout = FormLayout::Screen;
    form
}

fn typed(form: &mut FormDialog, text: &str) {
    for character in text.chars() {
        form.handle_key(key(KeyCode::Char(character)));
    }
}

#[test]
fn down_moves_to_the_next_field_and_up_to_the_previous() {
    let mut form = sample_form();

    form.handle_key(key(KeyCode::Down));
    assert_eq!(form.focused, 1);
    form.handle_key(key(KeyCode::Up));
    assert_eq!(form.focused, 0);
}

#[test]
fn up_and_down_wrap_around_in_popup_forms() {
    let mut form = sample_form();

    form.handle_key(key(KeyCode::Up));
    assert_eq!(form.focused, 2);
    form.handle_key(key(KeyCode::Down));
    assert_eq!(form.focused, 0);
}

#[test]
fn ctrl_s_submits_every_form() {
    let mut popup = sample_form();
    popup.handle_key(key(KeyCode::Char('a')));

    let outcome = popup.handle_key(ctrl('s'));

    assert_eq!(
        outcome,
        FormOutcome::Submitted(vec![("name", "a".into()), ("host", String::new()), ("password", String::new())])
    );
    let mut screen = screen_form();
    screen.focused = 2;
    assert!(matches!(screen.handle_key(ctrl('s')), FormOutcome::Submitted(_)));
    assert_eq!(screen.value("host").as_deref(), Some(""));
}

#[test]
fn typing_letters_digits_and_question_marks_only_edits_the_field() {
    let mut form = sample_form();

    typed(&mut form, "q1?n d");

    assert_eq!(form.fields[0].value, "q1?n d");
    assert_eq!(form.focused, 0);
}

#[test]
fn a_space_flips_a_toggle_in_popup_forms_too() {
    let mut form = FormDialog::new("Options", vec![toggle("tls", "TLS", "false")]);

    let outcome = form.handle_key(key(KeyCode::Char(' ')));

    assert_eq!(outcome, FormOutcome::ChoiceChanged { key: "tls" });
    assert_eq!(form.value("tls").as_deref(), Some("true"));
}

#[test]
fn up_and_down_on_the_protocol_list_change_the_protocol_and_report_it() {
    let mut form = screen_form();

    let down = form.handle_key(key(KeyCode::Down));
    assert_eq!(
        (down, form.value("protocol").as_deref(), form.focused),
        (FormOutcome::ChoiceChanged { key: "protocol" }, Some("s3"), 0)
    );
    let up = form.handle_key(key(KeyCode::Up));
    assert_eq!((up, form.value("protocol").as_deref()), (FormOutcome::ChoiceChanged { key: "protocol" }, Some("ftp")));
    form.handle_key(key(KeyCode::Up));
    assert_eq!(form.value("protocol").as_deref(), Some("sftp"));
}

#[test]
fn right_tab_and_enter_on_the_list_move_into_the_first_field() {
    for code in [KeyCode::Right, KeyCode::Tab, KeyCode::Enter] {
        let mut form = screen_form();

        let outcome = form.handle_key(key(code));

        assert_eq!((outcome, form.focused), (FormOutcome::Pending, 1), "{code:?}");
    }
}

#[test]
fn left_on_the_list_does_nothing() {
    let mut form = screen_form();

    let outcome = form.handle_key(key(KeyCode::Left));

    assert_eq!((outcome, form.focused, form.value("protocol").as_deref()), (FormOutcome::Pending, 0, Some("ftp")));
}

#[test]
fn shift_tab_on_the_list_goes_to_the_last_field() {
    let mut form = screen_form();

    form.handle_key(key(KeyCode::BackTab));

    assert_eq!(form.focused, 4);
}

#[test]
fn down_stops_at_the_last_field_and_tab_wraps_to_the_list() {
    let mut form = screen_form();
    form.focused = 4;

    form.handle_key(key(KeyCode::Down));
    assert_eq!(form.focused, 4);
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.focused, 0);
}

#[test]
fn up_and_shift_tab_from_the_first_field_return_to_the_list() {
    for code in [KeyCode::Up, KeyCode::BackTab] {
        let mut form = screen_form();
        form.focused = 1;

        form.handle_key(key(code));

        assert_eq!(form.focused, 0, "{code:?}");
    }
}

#[test]
fn down_and_up_move_between_form_fields() {
    let mut form = screen_form();
    form.focused = 1;

    form.handle_key(key(KeyCode::Down));
    assert_eq!(form.focused, 2);
    form.handle_key(key(KeyCode::Up));
    assert_eq!(form.focused, 1);
}

#[test]
fn left_at_caret_zero_returns_to_the_list_but_moves_the_caret_otherwise() {
    let mut form = screen_form();
    form.focused = 1;
    typed(&mut form, "ab");

    form.handle_key(key(KeyCode::Left));
    assert_eq!((form.focused, form.fields[1].cursor), (1, 1));
    form.handle_key(key(KeyCode::Left));
    assert_eq!((form.focused, form.fields[1].cursor), (1, 0));
    form.handle_key(key(KeyCode::Left));
    assert_eq!(form.focused, 0);
}

#[test]
fn enter_on_a_field_submits_and_ctrl_s_submits_from_the_list() {
    let mut form = screen_form();
    form.focused = 1;
    typed(&mut form, "prod");

    assert!(matches!(form.handle_key(key(KeyCode::Enter)), FormOutcome::Submitted(_)));
    form.focused = 0;
    assert!(matches!(form.handle_key(ctrl('s')), FormOutcome::Submitted(_)));
}

#[test]
fn space_left_and_right_flip_a_toggle_field() {
    let mut form = screen_form();
    form.focused = 3;

    assert_eq!(form.handle_key(key(KeyCode::Char(' '))), FormOutcome::ChoiceChanged { key: "tls" });
    assert_eq!(form.value("tls").as_deref(), Some("true"));
    form.handle_key(key(KeyCode::Left));
    assert_eq!(form.value("tls").as_deref(), Some("false"));
    form.handle_key(key(KeyCode::Right));
    assert_eq!(form.value("tls").as_deref(), Some("true"));
    assert_eq!(form.focused, 3);
}

#[test]
fn space_types_a_space_in_a_text_field() {
    let mut form = screen_form();
    form.focused = 1;

    typed(&mut form, "my host");

    assert_eq!(form.fields[1].value, "my host");
}

#[test]
fn typing_letters_in_a_screen_form_field_never_moves_focus_or_submits() {
    let mut form = screen_form();
    form.focused = 1;

    for character in ['q', 'n', 'e', 'd', '/', '?', '1', 's'] {
        assert_eq!(form.handle_key(key(KeyCode::Char(character))), FormOutcome::Pending);
    }

    assert_eq!((form.fields[1].value.as_str(), form.focused), ("qned/?1s", 1));
}

#[test]
fn letters_on_the_protocol_list_do_nothing() {
    let mut form = screen_form();

    assert_eq!(form.handle_key(key(KeyCode::Char('q'))), FormOutcome::Pending);
    assert_eq!((form.focused, form.value("protocol").as_deref()), (0, Some("ftp")));
}

#[test]
fn a_saved_secret_field_still_turns_into_a_masked_field_when_typed_in() {
    let mut form = screen_form();
    form.fields[4] = FormField::saved_secret("password", "Password");
    form.focused = 4;

    form.handle_key(key(KeyCode::Char('x')));

    assert_eq!((form.fields[4].kind.clone(), form.fields[4].value.as_str()), (FieldKind::Masked, "x"));
}

#[test]
fn a_kept_secret_is_submitted_as_kept_after_navigating_with_the_new_keys() {
    let mut form = screen_form();
    form.fields[4] = FormField::saved_secret("password", "Password");
    form.focused = 4;

    form.handle_key(key(KeyCode::Up));
    form.handle_key(key(KeyCode::Down));
    form.handle_key(key(KeyCode::Tab));
    form.handle_key(key(KeyCode::BackTab));
    form.handle_key(key(KeyCode::Left));
    let outcome = form.handle_key(ctrl('s'));

    let FormOutcome::Submitted(values) = outcome else { panic!("not submitted") };
    assert!(values.contains(&("password", KEPT_SECRET.to_string())));
}

#[test]
fn left_on_a_saved_secret_returns_to_the_list_without_clearing_it() {
    let mut form = screen_form();
    form.fields[4] = FormField::saved_secret("password", "Password");
    form.focused = 4;

    form.handle_key(key(KeyCode::Left));

    assert_eq!((form.focused, form.fields[4].kind.clone()), (0, FieldKind::SavedSecret));
}

#[test]
fn esc_cancels_a_screen_form_from_anywhere() {
    for focused in [0, 2, 3] {
        let mut form = screen_form();
        form.focused = focused;

        assert_eq!(form.handle_key(key(KeyCode::Esc)), FormOutcome::Cancelled);
    }
}

#[test]
fn a_popup_form_ignores_the_screen_focus_rules() {
    let mut form = choice_form();

    form.handle_key(key(KeyCode::Right));

    assert_eq!(form.value("protocol").as_deref(), Some("s3"));
    assert_eq!(form.focused, 0);
}

#[test]
fn is_toggle_recognises_only_true_false_choices() {
    assert!(toggle("tls", "TLS", "true").is_toggle());
    assert!(!FormField::text("a", "A", "").is_toggle());
    assert!(!choice_form().fields[0].is_toggle());
}

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(form: &FormDialog, width: u16, height: u16) -> ratatui::Terminal<ratatui::backend::TestBackend> {
    let theme = indexed();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_form(frame, frame.area(), form, &theme)).unwrap();
    terminal
}

#[test]
fn a_password_prompt_draws_bullets_and_never_the_typed_text() {
    let mut form = FormDialog::new("Password for me@nas", vec![FormField::masked("password", "Password", "")]);
    for character in "hunter2".chars() {
        press(&mut form, KeyCode::Char(character));
    }

    let terminal = draw(&form, 60, 12);

    assert!(find_text(&terminal, "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}").is_some());
    assert!(find_text(&terminal, "hunter2").is_none());
    assert!(!format!("{:?}", press(&mut form, KeyCode::Enter)).contains("hunter2"));
}

#[test]
fn the_focused_field_has_a_bright_label_and_an_accent_underline() {
    let theme = indexed();
    let terminal = draw(&sample_form(), 60, 12);

    let (x, y) = find_text(&terminal, "Name").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, x + 15, y + 1), theme.accent);
    let (x, y) = find_text(&terminal, "Host").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, x + 15, y + 1), theme.line_strong);
}

#[test]
fn the_caret_is_the_terminal_cursor_in_the_focused_text_field() {
    let mut form = sample_form();
    press(&mut form, KeyCode::Char('a'));
    press(&mut form, KeyCode::Char('b'));

    let mut terminal = draw(&form, 60, 12);

    let (x, y) = find_text(&terminal, "ab").unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (x + 2, y));
}

#[test]
fn choice_fields_show_arrows_and_the_selected_label() {
    let form = FormDialog::new(
        "Synchronize with nas",
        vec![FormField::choice(
            "direction",
            "Direction",
            vec![("a".into(), "Local \u{2192} Remote".into()), ("b".into(), "Remote \u{2192} Local".into())],
            "a",
        )],
    );

    let terminal = draw(&form, 70, 12);

    assert!(find_text(&terminal, "\u{25c0} Local \u{2192} Remote \u{25b6}").is_some());
}

#[test]
fn toggle_fields_are_checkboxes() {
    let theme = indexed();
    let form = FormDialog::new("t", vec![FormField::text("a", "A", ""), toggle("tls", "use tls", "true")]);

    let terminal = draw(&form, 60, 12);

    let (x, y) = find_text(&terminal, "[x] use tls").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.accent);
}

#[test]
fn the_error_is_a_bullet_in_err_above_the_footer() {
    let theme = indexed();
    let mut form = sample_form();
    form.error = Some("Host can't be empty".to_string());

    let terminal = draw(&form, 60, 14);

    let (x, y) = find_text(&terminal, "\u{2022} Host can't be empty").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.err);
    let (_, footer_row) = find_text(&terminal, "enter ok").unwrap();
    assert!(footer_row > y);
}

#[test]
fn the_footer_shows_tab_next_only_with_several_fields_and_always_enter_ok_and_esc_cancel() {
    let several = draw(&sample_form(), 60, 14);
    let single = draw(&FormDialog::new("t", vec![FormField::masked("password", "Password", "")]), 60, 14);

    assert!(find_text(&several, "tab next").is_some());
    assert!(find_text(&single, "tab next").is_none());
    for terminal in [&several, &single] {
        assert!(find_text(terminal, "enter ok").is_some());
        assert!(find_text(terminal, "esc cancel").is_some());
    }
}

#[test]
fn the_popup_grows_with_the_fields() {
    let one = draw(&FormDialog::new("t", vec![FormField::text("a", "A", "")]), 60, 20);
    let three = draw(&sample_form(), 60, 20);

    let (_, one_footer) = find_text(&one, "enter ok").unwrap();
    let (_, three_footer) = find_text(&three, "enter ok").unwrap();
    assert_eq!(three_footer - one_footer, 2);
}

#[test]
fn a_short_screen_scrolls_the_focused_field_into_view() {
    let mut form = sample_form();
    form.focused = 2;

    let terminal = draw(&form, 60, 9);

    assert!(find_text(&terminal, "Password").is_some());
    assert!(find_text(&terminal, "enter ok").is_some());
}

#[test]
fn hostile_labels_values_hints_and_errors_are_sanitised_and_cut() {
    let mut form = FormDialog::new(
        "T\u{1b}[2J",
        vec![
            FormField::text("a", "La\u{1b}bel", "va\u{202e}lue".repeat(40)),
            FormField::choice("c", "Ch", vec![("x".into(), "ch\u{1b}oice".into())], "x"),
        ],
    );
    form.hint = Some("hi\u{1b}nt".repeat(50));
    form.error = Some("er\u{7}ror\n".repeat(50));

    let terminal = draw(&form, 80, 20);

    assert!(!has_unprintable_cell(&terminal));
    let buffer = terminal.backend().buffer();
    assert!((0..20).all(|y| buffer[(0, y)].symbol() == " " && buffer[(79, y)].symbol() == " "));
}

#[test]
fn tiny_screens_do_not_panic() {
    let mut form = sample_form();
    form.hint = Some("hint".to_string());
    form.error = Some("error".to_string());
    for width in (0..=12).chain([90]) {
        for height in 0..=14 {
            draw(&form, width, height);
            draw(&FormDialog::new("t", Vec::new()), width, height);
        }
    }
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier() {
    assert!(!include_str!("mod.rs").contains("REVERSED"));
}
