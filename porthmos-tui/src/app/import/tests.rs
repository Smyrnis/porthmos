use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::{
    Command, Completion, Event, Location, VfsPath,
    import::{Candidate, ImportPlan, ImportRow, ImportSource, Verdict},
};

use crate::{
    app::testing::{TestApp, find_text, render_app, test_app},
    input::Action,
    widgets::dialog::Dialog,
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app() -> TestApp {
    test_app(&VfsPath::new("/d"))
}

fn candidate(name: &str) -> Candidate {
    Candidate {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: format!("{name}.example"),
        port: Some(22),
        username: "alice".to_string(),
        group: None,
        options: BTreeMap::new(),
        notes: Vec::new(),
    }
}

fn new_row(name: &str) -> ImportRow {
    ImportRow { name: name.to_string(), candidate: Some(candidate(name)), verdict: Verdict::New }
}

fn plan(import_id: u64, rows: Vec<ImportRow>) -> Arc<ImportPlan> {
    Arc::new(ImportPlan { import_id, source: ImportSource::FileZilla, rows })
}

fn open_form(test: &mut TestApp) {
    test.app.apply_action(Action::ImportConnections);
}

fn form_value(test: &TestApp, field: &str) -> Option<String> {
    match test.app.dialog.as_ref() {
        Some(Dialog::Form(form)) => form.value(field),
        _ => None,
    }
}

fn type_path(test: &mut TestApp, path: &str) {
    let Some(Dialog::Form(form)) = test.app.dialog.as_mut() else { panic!("no form") };
    let field = form.fields.iter_mut().find(|field| field.key == "path").unwrap();
    field.value = path.to_string();
    field.cursor = path.chars().count();
}

fn submit(test: &mut TestApp) {
    test.app.apply_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
}

fn read_requested(test: &mut TestApp, path: &str) -> u64 {
    open_form(test);
    type_path(test, path);
    submit(test);
    let sent = test.sent();
    let Some(Command::ReadImport { import_id, .. }) = sent.first() else { panic!("{sent:?}") };
    *import_id
}

fn review_is_open(test: &TestApp) -> bool {
    matches!(test.app.dialog, Some(Dialog::Import(_)))
}

#[test]
fn the_action_opens_a_form_asking_for_the_source_and_the_path() {
    let mut test = app();

    open_form(&mut test);

    assert_eq!(form_value(&test, "source").as_deref(), Some("filezilla"));
    assert!(form_value(&test, "path").is_some());
    assert!(test.sent().is_empty());
}

#[test]
fn an_empty_path_is_refused_on_the_path_field_and_nothing_is_read() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "   ");

    submit(&mut test);

    let Some(Dialog::Form(form)) = test.app.dialog.as_ref() else { panic!("form closed") };
    assert_eq!(form.rejected_field(), Some("path"));
    assert!(test.sent().is_empty());
}

#[test]
fn submitting_asks_the_core_to_read_the_file_and_keeps_the_form_open_without_a_notice() {
    let mut test = app();

    let import_id = read_requested(&mut test, "/tmp/sitemanager.xml");

    assert_eq!(import_id, 1);
    assert!(matches!(test.app.dialog, Some(Dialog::Form(_))));
    assert_eq!(test.notification(), None);
}

#[test]
fn submitting_again_while_the_read_is_running_sends_nothing() {
    let mut test = app();
    read_requested(&mut test, "/tmp/sitemanager.xml");

    submit(&mut test);

    assert!(test.sent().is_empty());
}

#[test]
fn escape_on_the_waiting_form_cancels_the_read() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/sitemanager.xml");

    test.app.apply_key(key(KeyCode::Esc));

    assert!(test.app.dialog.is_none());
    assert_eq!(test.sent(), vec![Command::CancelImport { import_id }]);
    test.app.apply_core_event(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));
    assert!(!review_is_open(&test));
}

#[test]
fn a_failed_read_shows_its_reason_on_the_form_and_allows_another_try() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/missing.xml");

    test.app.apply_core_event(Event::ImportReadFailed {
        import_id,
        message: porthmos_core::Message::from("Couldn't read the connections to import:\nno such file"),
    });

    let Some(Dialog::Form(form)) = test.app.dialog.as_ref() else { panic!("form closed") };
    assert!(form.error.as_deref().is_some_and(|text| text.contains("no such file")));
    assert_eq!(test.notification(), None);
    submit(&mut test);
    assert_eq!(test.sent().len(), 1);
}

#[test]
fn a_failure_for_an_older_read_is_ignored() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/a.xml");

    test.app.apply_core_event(Event::ImportReadFailed {
        import_id: import_id + 4,
        message: porthmos_core::Message::from("stale"),
    });

    let Some(Dialog::Form(form)) = test.app.dialog.as_ref() else { panic!("form closed") };
    assert_eq!(form.error, None);
}

#[test]
fn the_source_and_path_reach_the_core_unchanged() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/tmp/putty-sessions");
    let Some(Dialog::Form(form)) = test.app.dialog.as_mut() else { panic!() };
    form.fields.iter_mut().find(|field| field.key == "source").unwrap().kind =
        crate::widgets::dialog::FieldKind::Choice {
            choices: vec![("putty".to_string(), "PuTTY".to_string())],
            selected: 0,
        };
    submit(&mut test);

    assert_eq!(
        test.sent(),
        vec![Command::ReadImport {
            import_id: 1,
            source: ImportSource::Putty,
            path: PathBuf::from("/tmp/putty-sessions")
        }]
    );
}

#[test]
fn each_read_gets_a_new_import_id() {
    let mut test = app();
    let first = read_requested(&mut test, "/tmp/a.xml");
    let second = read_requested(&mut test, "/tmp/b.xml");

    assert_eq!((first, second), (1, 2));
}

#[test]
fn changing_the_source_keeps_a_path_the_user_typed() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/my/own/path");

    test.app.form_choice_changed("source");

    assert_eq!(form_value(&test, "path").as_deref(), Some("/my/own/path"));
}

#[test]
fn changing_the_source_changes_the_hint() {
    let mut test = app();
    open_form(&mut test);
    let hint_for = |test: &TestApp| match test.app.dialog.as_ref() {
        Some(Dialog::Form(form)) => form.hint.clone(),
        _ => None,
    };
    let before = hint_for(&test);
    let Some(Dialog::Form(form)) = test.app.dialog.as_mut() else { panic!() };
    let field = form.fields.iter_mut().find(|field| field.key == "source").unwrap();
    if let crate::widgets::dialog::FieldKind::Choice { selected, .. } = &mut field.kind {
        *selected = 2;
    }

    test.app.form_choice_changed("source");

    assert_ne!(before, hint_for(&test));
    assert!(hint_for(&test).is_some_and(|hint| hint.contains("PuTTY")));
}

#[test]
fn the_plan_for_the_open_read_opens_the_review_dialog() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/a.xml");

    test.app.apply_core_event(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));

    assert!(review_is_open(&test));
    assert!(test.sent().is_empty());
}

#[test]
fn a_plan_nobody_is_waiting_for_is_cancelled_in_the_core() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/a.xml");

    test.app.apply_core_event(Event::ImportPlanReady(plan(import_id + 7, vec![new_row("a")])));

    assert!(!review_is_open(&test));
    assert_eq!(test.sent(), vec![Command::CancelImport { import_id: import_id + 7 }]);
}

#[test]
fn a_plan_that_arrives_while_another_dialog_is_open_is_cancelled() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/a.xml");
    test.app.apply_action(Action::Mkdir);
    assert!(test.app.dialog.is_some());

    test.app.apply_core_event(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));

    assert!(!review_is_open(&test));
    assert_eq!(test.sent(), vec![Command::CancelImport { import_id }]);
}

#[test]
fn a_plan_with_nothing_to_import_is_cancelled_and_explained() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/a.xml");

    test.app.apply_core_event(Event::ImportPlanReady(plan(import_id, Vec::new())));

    assert!(!review_is_open(&test));
    assert_eq!(test.sent(), vec![Command::CancelImport { import_id }]);
    let Some(Dialog::Form(form)) = test.app.dialog.as_ref() else { panic!("form closed") };
    assert!(form.error.as_deref().is_some_and(|text| text.contains("No connections")), "{:?}", form.error);
    assert_eq!(test.notification(), None);
}

#[test]
fn enter_in_the_review_runs_the_ticked_rows() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/a.xml");
    test.app.apply_core_event(Event::ImportPlanReady(plan(import_id, vec![new_row("a"), new_row("b")])));

    test.app.apply_key(key(KeyCode::Char(' ')));
    test.app.apply_key(key(KeyCode::Enter));

    assert_eq!(test.sent(), vec![Command::RunImport { import_id, rows: vec![1] }]);
    assert!(test.app.dialog.is_none());
}

#[test]
fn escape_in_the_review_cancels_the_import() {
    let mut test = app();
    let import_id = read_requested(&mut test, "/tmp/a.xml");
    test.app.apply_core_event(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));

    test.app.apply_key(key(KeyCode::Esc));

    assert_eq!(test.sent(), vec![Command::CancelImport { import_id }]);
    assert!(test.app.dialog.is_none());
}

#[test]
fn the_done_event_reports_how_many_were_added() {
    let mut test = app();

    test.app.apply_core_event(Event::ImportDone { import_id: 1, added: 2, skipped: 0 });
    assert_eq!(test.notification().as_deref(), Some("Imported 2 connections"));
    test.app.notifications.dismiss_current();

    test.app.apply_core_event(Event::ImportDone { import_id: 1, added: 1, skipped: 3 });
    assert_eq!(test.notification().as_deref(), Some("Imported 1 connection (3 skipped)"));
}

#[test]
fn the_palette_offers_the_import() {
    let mut test = app();

    test.app.apply_action(Action::OpenPalette);
    test.type_text("import");

    let screen = render_app(&test.app, 100, 30);
    assert!(find_text(&screen, "Import connections").is_some());
}

fn complete(test: &mut TestApp) {
    test.app.apply_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
}

fn completion_reply(test: &mut TestApp, text: &str, candidates: &[&str], completed: &str) {
    let completion = Completion {
        prefix: String::new(),
        candidates: candidates.iter().map(|name| name.to_string()).collect(),
        completed: completed.to_string(),
    };
    test.app.apply_core_event(Event::PathCompleted { location: Location::Local, text: text.to_string(), completion });
}

fn focus_field(test: &mut TestApp, key: &str) {
    let Some(Dialog::Form(form)) = test.app.dialog.as_mut() else { panic!("no form") };
    form.focused = form.fields.iter().position(|field| field.key == key).unwrap();
}

fn form_hint(test: &TestApp) -> Option<String> {
    match test.app.dialog.as_ref() {
        Some(Dialog::Form(form)) => form.hint.clone(),
        _ => None,
    }
}

#[test]
fn ctrl_t_on_the_path_field_asks_core_to_complete_what_was_typed() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/home/me/.config/File");
    focus_field(&mut test, "path");
    test.sent();

    complete(&mut test);

    assert_eq!(
        test.sent(),
        vec![Command::CompletePath {
            location: Location::Local,
            from: VfsPath::new("/d"),
            text: "/home/me/.config/File".to_string()
        }]
    );
}

#[test]
fn ctrl_t_on_the_source_field_does_nothing() {
    let mut test = app();
    open_form(&mut test);
    focus_field(&mut test, "source");
    test.sent();

    complete(&mut test);

    assert!(test.sent().is_empty());
}

#[test]
fn a_completion_replaces_the_path_and_moves_the_cursor_to_its_end() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/home/me/.config/File");
    focus_field(&mut test, "path");
    complete(&mut test);

    completion_reply(&mut test, "/home/me/.config/File", &["filezilla/"], "/home/me/.config/filezilla/");

    assert_eq!(form_value(&test, "path").as_deref(), Some("/home/me/.config/filezilla/"));
    let Some(Dialog::Form(form)) = test.app.dialog.as_ref() else { panic!("no form") };
    let field = form.fields.iter().find(|field| field.key == "path").unwrap();
    assert_eq!(field.cursor, "/home/me/.config/filezilla/".chars().count());
}

#[test]
fn several_candidates_are_listed_in_the_hint_and_the_path_is_kept() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/home/me/.s");
    focus_field(&mut test, "path");
    complete(&mut test);

    completion_reply(&mut test, "/home/me/.s", &["ssh/", "sessions/"], "/home/me/.s");

    assert_eq!(form_value(&test, "path").as_deref(), Some("/home/me/.s"));
    assert_eq!(form_hint(&test).as_deref(), Some("ssh/  sessions/"));
}

#[test]
fn an_answer_for_text_that_has_since_changed_is_ignored() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/home/me/.config/File");
    focus_field(&mut test, "path");
    complete(&mut test);
    type_path(&mut test, "/elsewhere");

    completion_reply(&mut test, "/home/me/.config/File", &["filezilla/"], "/home/me/.config/filezilla/");

    assert_eq!(form_value(&test, "path").as_deref(), Some("/elsewhere"));
}

#[test]
fn an_answer_nobody_asked_for_does_not_touch_the_form() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/home/me/.s");

    completion_reply(&mut test, "/home/me/.s", &["ssh/"], "/home/me/.ssh/");

    assert_eq!(form_value(&test, "path").as_deref(), Some("/home/me/.s"));
}

#[test]
fn a_second_request_waits_for_the_first_answer() {
    let mut test = app();
    open_form(&mut test);
    type_path(&mut test, "/ho");
    focus_field(&mut test, "path");
    test.sent();

    complete(&mut test);
    complete(&mut test);

    assert_eq!(test.sent().len(), 1);
}

#[test]
fn the_hint_tells_where_the_completion_key_is() {
    let mut test = app();

    open_form(&mut test);

    assert!(form_hint(&test).is_some_and(|hint| hint.contains("Ctrl+T completes the path")));
}
