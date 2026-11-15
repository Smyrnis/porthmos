use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use porthmos_core::import::{Candidate, ImportPlan, ImportRow, ImportSource, Verdict, default_location_here};
use porthmos_core::{Command, Event, Severity};

use crate::connections::Screen;
use crate::testing::Harness;

use super::super::*;

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

type SourceModal = (ImportSource, String, Option<String>, Option<u64>);

fn source_modal(harness: &Harness) -> SourceModal {
    match harness.app.screen() {
        Some(Screen::ImportSource(state)) => (state.source, state.path.clone(), state.error.clone(), state.waiting),
        other => panic!("expected the import source screen, found {other:?}"),
    }
}

fn fresh() -> Harness {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);
    harness.sent();
    harness
}

fn open(harness: &mut Harness) {
    harness.press(Message::ImportPressed);
}

fn type_path(harness: &mut Harness, path: &str) {
    harness.press(Message::ImportPathTyped(path.to_string()));
}

fn read(harness: &mut Harness, path: &str) -> u64 {
    open(harness);
    type_path(harness, path);
    harness.press(Message::ImportReadPressed);
    let sent = harness.sent();
    let Some(Command::ReadImport { import_id, .. }) = sent.first() else { panic!("{sent:?}") };
    *import_id
}

fn last_notice(harness: &Harness) -> (Severity, String) {
    let notice = harness.app.notices().last().unwrap();
    (notice.severity, notice.message.to_string())
}

fn review_ticks(harness: &Harness) -> Vec<u32> {
    match harness.app.screen() {
        Some(Screen::ImportReview(view)) => view.ticked_rows(),
        other => panic!("expected the import review, found {other:?}"),
    }
}

#[test]
fn import_button_opens_the_source_screen_with_the_default_path() {
    let mut harness = fresh();

    open(&mut harness);

    let expected = default_location_here(ImportSource::FileZilla)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert_eq!(source_modal(&harness), (ImportSource::FileZilla, expected, None, None));
    assert!(harness.sent().is_empty());
}

#[test]
fn changing_source_reprefills_an_untouched_path_only() {
    let mut harness = fresh();
    open(&mut harness);
    harness.press(Message::ImportSourceChosen(ImportSource::Putty));
    let putty_default =
        default_location_here(ImportSource::Putty).map(|path| path.to_string_lossy().into_owned()).unwrap_or_default();
    assert_eq!(source_modal(&harness).1, putty_default);
    assert_eq!(source_modal(&harness).0, ImportSource::Putty);

    type_path(&mut harness, "/my/own/path");
    harness.press(Message::ImportSourceChosen(ImportSource::WinScp));

    assert_eq!(source_modal(&harness), (ImportSource::WinScp, "/my/own/path".to_string(), None, None));
}

#[test]
fn empty_path_shows_an_inline_error_and_sends_nothing() {
    let mut harness = fresh();
    open(&mut harness);
    type_path(&mut harness, "   ");

    harness.press(Message::ImportReadPressed);

    assert_eq!(source_modal(&harness).2.as_deref(), Some("Enter the path of the file"));
    assert!(harness.sent().is_empty());
}

#[test]
fn typing_clears_the_inline_error() {
    let mut harness = fresh();
    open(&mut harness);
    type_path(&mut harness, "");
    harness.press(Message::ImportReadPressed);

    type_path(&mut harness, "/x");

    assert_eq!(source_modal(&harness).2, None);
}

#[test]
fn read_sends_read_import_and_disables_the_buttons() {
    let mut harness = fresh();
    open(&mut harness);
    type_path(&mut harness, "/tmp/sitemanager.xml");

    harness.press(Message::ImportReadPressed);

    assert_eq!(
        harness.sent(),
        vec![Command::ReadImport {
            import_id: 1,
            source: ImportSource::FileZilla,
            path: PathBuf::from("/tmp/sitemanager.xml")
        }]
    );
    assert_eq!(source_modal(&harness).3, Some(1));
    harness.press(Message::ImportReadPressed);
    harness.press(Message::ImportSourceChosen(ImportSource::Putty));
    assert!(harness.sent().is_empty());
    assert_eq!(source_modal(&harness).0, ImportSource::FileZilla);
}

#[test]
fn plan_arrival_shows_the_review_modal() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");

    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a"), new_row("b")])));

    assert_eq!(review_ticks(&harness), vec![0, 1]);
    assert!(harness.sent().is_empty());
}

#[test]
fn a_plan_for_another_id_is_cancelled() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");

    harness.feed(Event::ImportPlanReady(plan(import_id + 5, vec![new_row("a")])));

    assert!(matches!(harness.app.screen(), Some(Screen::ImportSource(_))));
    assert_eq!(harness.sent(), vec![Command::CancelImport { import_id: import_id + 5 }]);
}

#[test]
fn a_plan_after_the_source_screen_was_left_is_cancelled() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");
    harness.press(Message::ScreenCancelled);

    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));

    assert!(matches!(harness.app.screen(), Some(Screen::List)));
    assert_eq!(harness.sent(), vec![Command::CancelImport { import_id }]);
}

#[test]
fn a_plan_with_no_rows_is_cancelled_and_explained() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");

    harness.feed(Event::ImportPlanReady(plan(import_id, Vec::new())));

    assert_eq!(harness.sent(), vec![Command::CancelImport { import_id }]);
    assert_eq!(source_modal(&harness).2.as_deref(), Some("No connections found in that FileZilla source"));
    assert_eq!(source_modal(&harness).3, None);
}

#[test]
fn rows_toggle_and_bulk_buttons_change_the_ticks() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");
    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a"), new_row("b")])));

    harness.press(Message::ImportRowToggled(0));
    assert_eq!(review_ticks(&harness), vec![1]);
    harness.press(Message::ImportUntickAll);
    assert!(review_ticks(&harness).is_empty());
    harness.press(Message::ImportTickNew);
    assert_eq!(review_ticks(&harness), vec![0, 1]);
}

#[test]
fn run_sends_ticked_rows_and_closes() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");
    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a"), new_row("b")])));
    harness.press(Message::ImportRowToggled(0));

    harness.press(Message::ImportRunPressed);

    assert_eq!(harness.sent(), vec![Command::RunImport { import_id, rows: vec![1] }]);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
}

#[test]
fn run_with_nothing_ticked_sends_nothing_and_stays_open() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");
    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));
    harness.press(Message::ImportUntickAll);

    harness.press(Message::ImportRunPressed);

    assert!(harness.sent().is_empty());
    assert!(matches!(harness.app.screen(), Some(Screen::ImportReview(_))));
}

#[test]
fn cancel_sends_cancel_import() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");
    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));

    harness.press(Message::ImportCancelled);

    assert_eq!(harness.sent(), vec![Command::CancelImport { import_id }]);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
}

#[test]
fn leaving_the_review_by_any_route_cancels_the_import() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");
    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));

    harness.press(Message::ScreenCancelled);

    assert_eq!(harness.sent(), vec![Command::CancelImport { import_id }]);
}

#[test]
fn import_done_pushes_a_notice_with_counts() {
    let mut harness = fresh();

    harness.feed(Event::ImportDone { import_id: 1, added: 2, skipped: 0 });
    assert_eq!(last_notice(&harness), (Severity::Info, "Imported 2 connections".to_string()));

    harness.feed(Event::ImportDone { import_id: 1, added: 1, skipped: 3 });
    assert_eq!(last_notice(&harness), (Severity::Info, "Imported 1 connection (3 skipped)".to_string()));
}

#[test]
fn a_read_error_returns_the_modal_to_editable() {
    let mut harness = fresh();
    read(&mut harness, "/tmp/a.xml");

    harness.feed(Event::Notice {
        severity: Severity::Error,
        message: porthmos_core::Message::from(
            "Couldn't read the connections to import:\nCan't read the file: no such file",
        ),
    });

    let (_, _, error, waiting) = source_modal(&harness);
    assert_eq!(waiting, None);
    assert!(error.is_some_and(|text| text.contains("Can't read the file")));
}

#[test]
fn a_warning_while_waiting_does_not_end_the_wait() {
    let mut harness = fresh();
    read(&mut harness, "/tmp/a.xml");

    harness.feed(Event::Notice {
        severity: Severity::Warning,
        message: porthmos_core::Message::from("something unrelated"),
    });

    assert_eq!(source_modal(&harness).3, Some(1));
}

#[test]
fn closing_the_connections_window_on_the_review_cancels_the_import() {
    let mut harness = fresh();
    let import_id = read(&mut harness, "/tmp/a.xml");
    harness.feed(Event::ImportPlanReady(plan(import_id, vec![new_row("a")])));

    harness.press(Message::ManagerClosed);

    assert!(harness.app.screen().is_none());
    assert_eq!(harness.sent(), vec![Command::CancelImport { import_id }]);
}

#[test]
fn the_import_button_does_nothing_when_the_connections_window_is_not_on_its_list() {
    let mut harness = Harness::with_builtin_protocols();
    harness.press(Message::ManagerToggled);
    harness.sent();
    harness.press(Message::ManagerAddPressed);

    harness.press(Message::ImportPressed);

    assert!(!matches!(harness.app.screen(), Some(Screen::ImportSource(_))));
}

#[test]
fn the_main_window_gets_no_import_dialog() {
    let mut harness = fresh();

    open(&mut harness);

    assert!(harness.app.modal().is_none());
}

#[test]
fn a_picked_path_fills_the_field_and_clears_the_error() {
    let mut harness = fresh();
    open(&mut harness);
    type_path(&mut harness, "");
    harness.press(Message::ImportReadPressed);

    harness.press(Message::ImportPathPicked(Some(PathBuf::from("/home/me/sitemanager.xml"))));

    let state = source_modal(&harness);
    assert_eq!((state.1.as_str(), state.2), ("/home/me/sitemanager.xml", None));
}

#[test]
fn cancelling_the_chooser_changes_nothing() {
    let mut harness = fresh();
    open(&mut harness);
    type_path(&mut harness, "/keep/me");

    harness.press(Message::ImportPathPicked(None));

    assert_eq!(source_modal(&harness).1, "/keep/me");
}

#[test]
fn a_path_picked_while_reading_or_after_leaving_is_ignored() {
    let mut harness = fresh();
    read(&mut harness, "/tmp/a.xml");
    harness.press(Message::ImportPathPicked(Some(PathBuf::from("/other"))));
    assert_eq!(source_modal(&harness).1, "/tmp/a.xml");

    harness.press(Message::ScreenCancelled);
    harness.press(Message::ImportPathPicked(Some(PathBuf::from("/other"))));
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
}

#[test]
fn browsing_is_ignored_while_reading() {
    let mut harness = fresh();
    read(&mut harness, "/tmp/a.xml");

    harness.press(Message::ImportBrowsePressed(false));

    assert!(harness.sent().is_empty());
}
