use std::collections::BTreeMap;
use std::sync::Arc;

use porthmos_core::Event;
use porthmos_core::import::{Candidate, ImportPlan, ImportRow, ImportSource, Verdict};

use super::{name_hint, rendered_range, row_texts, run_enabled, run_label};
use crate::app::Message;
use crate::import_view::ImportView;
use crate::testing::Harness;
use crate::view::connection_form::overlay;

fn new_row(index: usize) -> ImportRow {
    let name = format!("site {index}");
    ImportRow {
        name: name.clone(),
        candidate: Some(Candidate {
            name,
            protocol: "sftp".to_string(),
            host: format!("h{index}.example"),
            port: Some(22),
            username: "alice".to_string(),
            group: Some("Work".to_string()),
            options: BTreeMap::new(),
            notes: vec!["a note".to_string()],
        }),
        verdict: Verdict::New,
    }
}

fn plan(rows: Vec<ImportRow>) -> ImportPlan {
    ImportPlan { import_id: 1, source: ImportSource::FileZilla, rows }
}

fn harness_with(rows: usize) -> Harness {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);
    harness.press(Message::ImportPressed);
    harness.press(Message::ImportPathTyped("/tmp/a.xml".to_string()));
    harness.press(Message::ImportReadPressed);
    harness.sent();
    harness.feed(Event::ImportPlanReady(Arc::new(plan((0..rows).map(new_row).collect()))));
    harness
}

#[test]
fn the_run_button_counts_the_ticked_rows_and_is_off_at_zero() {
    let mut view = ImportView::new(&plan(vec![new_row(0), new_row(1)]));
    assert_eq!((run_label(&view), run_enabled(&view)), ("Import 2".to_string(), true));

    view.untick_all();

    assert_eq!((run_label(&view), run_enabled(&view)), ("Import 0".to_string(), false));
}

#[test]
fn a_row_shows_its_summary_group_and_notes_in_fixed_columns() {
    let view = ImportView::new(&plan(vec![new_row(3)]));

    let texts = row_texts(&view.rows()[0]);

    assert_eq!(texts.name, "site 3");
    assert_eq!(texts.summary, "sftp alice@h3.example:22");
    assert_eq!(texts.group, "Work");
    assert_eq!(texts.detail, "a note");
}

#[test]
fn only_the_rows_near_the_viewport_are_built() {
    let (first, count) = rendered_range(0.0, 5000);
    assert_eq!(first, 0);
    assert!(count < 40, "{count}");

    let (first, count) = rendered_range(100_000.0, 5000);
    assert!(first > 4000 && first + count <= 5000, "{first} {count}");
}

#[test]
fn the_review_renders_for_a_few_rows_and_for_five_hundred() {
    for rows in [1, 500] {
        let harness = harness_with(rows);
        assert!(overlay(&harness.app).is_some());
    }
}

#[test]
fn scrolling_the_review_moves_the_rendered_window() {
    let mut harness = harness_with(500);

    harness.press(Message::ImportScrolled(2200.0));

    let Some(crate::connections::Screen::ImportReview(view)) = harness.app.screen() else { panic!() };
    assert_eq!(view.scroll(), 2200.0);
    assert!(overlay(&harness.app).is_some());
}

#[test]
fn a_cut_name_is_offered_in_full_as_a_hint() {
    let long = "a-very-long-connection-name-that-goes-on";
    let mut plan_rows = vec![new_row(0)];
    plan_rows[0].name = long.to_string();
    let view = ImportView::new(&plan(plan_rows));

    assert_ne!(row_texts(&view.rows()[0]).name, long);
    assert_eq!(name_hint(&view.rows()[0]).as_deref(), Some(long));
}

#[test]
fn a_name_that_fits_needs_no_hint() {
    let view = ImportView::new(&plan(vec![new_row(0)]));

    assert_eq!(name_hint(&view.rows()[0]), None);
}
