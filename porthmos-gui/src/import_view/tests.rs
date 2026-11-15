use std::collections::BTreeMap;

use porthmos_core::import::{Candidate, ImportPlan, ImportRow, ImportSource, Verdict};

use super::*;

fn candidate(name: &str) -> Candidate {
    Candidate {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: format!("{name}.example"),
        port: Some(22),
        username: "alice".to_string(),
        group: Some("Work".to_string()),
        options: BTreeMap::new(),
        notes: vec!["key is in PuTTY format".to_string()],
    }
}

fn new_row(name: &str) -> ImportRow {
    ImportRow { name: name.to_string(), candidate: Some(candidate(name)), verdict: Verdict::New }
}

fn other_row(name: &str, verdict: Verdict) -> ImportRow {
    let candidate = (verdict == Verdict::Exists).then(|| candidate(name));
    ImportRow { name: name.to_string(), candidate, verdict }
}

fn plan(rows: Vec<ImportRow>) -> ImportPlan {
    ImportPlan { import_id: 4, source: ImportSource::WinScp, rows }
}

fn mixed() -> ImportView {
    ImportView::new(&plan(vec![
        new_row("alpha"),
        other_row("beta", Verdict::Exists),
        new_row("gamma"),
        other_row("delta", Verdict::Unsupported("Storj is not supported".to_string())),
        other_row("epsilon", Verdict::Invalid("port".to_string())),
    ]))
}

#[test]
fn new_rows_start_ticked_and_other_rows_cannot_be_ticked() {
    let view = mixed();
    let state: Vec<(bool, bool)> = view.rows().iter().map(|row| (row.tickable, row.ticked)).collect();

    assert_eq!(state, vec![(true, true), (false, false), (true, true), (false, false), (false, false)]);
}

#[test]
fn toggle_ignores_untickable_rows() {
    let mut view = mixed();

    view.toggle(1);
    view.toggle(99);
    view.toggle(0);

    assert_eq!(view.ticked_rows(), vec![2]);
}

#[test]
fn tick_new_and_untick_all() {
    let mut view = mixed();

    view.untick_all();
    assert!(view.ticked_rows().is_empty());
    view.tick_new();

    assert_eq!(view.ticked_rows(), vec![0, 2]);
}

#[test]
fn ticked_rows_are_plan_indices_in_order() {
    let mut view = mixed();
    view.toggle(0);
    view.toggle(0);

    assert_eq!(view.ticked_rows(), vec![0, 2]);
}

#[test]
fn summary_counts_new_exists_unsupported_invalid() {
    assert_eq!(mixed().summary(), "2 new, 1 existing, 1 unsupported, 1 invalid");
    assert_eq!(ImportView::new(&plan(vec![new_row("a")])).summary(), "1 new");
}

#[test]
fn rows_carry_the_texts_the_table_shows() {
    let view = mixed();
    let rows = view.rows();

    assert_eq!(rows[0].summary, "sftp alice@alpha.example:22");
    assert_eq!(rows[0].group, "Work");
    assert_eq!(rows[0].verdict_text, "");
    assert_eq!(rows[0].notes, vec!["key is in PuTTY format".to_string()]);
    assert_eq!(rows[1].verdict_text, "already exists");
    assert_eq!(rows[3].verdict_text, "Storj is not supported");
    assert_eq!(rows[4].verdict_text, "can't import: port");
}

#[test]
fn names_with_escape_characters_are_cleaned() {
    let mut row = new_row("a\u{1b}[31mred");
    row.name = "a\u{1b}[31mred".to_string();
    if let Some(candidate) = row.candidate.as_mut() {
        candidate.host = "h\u{7}ost".to_string();
        candidate.group = Some("g\u{1b}roup".to_string());
        candidate.notes = vec!["n\u{1b}ote".to_string()];
    }
    let view = ImportView::new(&plan(vec![row]));
    let shown = &view.rows()[0];

    for text in [&shown.name, &shown.summary, &shown.group, &shown.notes[0]] {
        assert!(!text.chars().any(char::is_control), "{text:?}");
    }
}

#[test]
fn the_view_remembers_which_import_and_source_it_shows() {
    let view = mixed();

    assert_eq!(view.import_id(), 4);
    assert_eq!(view.source(), ImportSource::WinScp);
}

#[test]
fn each_source_filters_the_chooser_by_its_own_extension() {
    let filters: Vec<(String, &[&str])> = ImportSource::ALL
        .into_iter()
        .map(|source| pick_request(source, ""))
        .map(|r| (r.filter_name, r.extensions))
        .collect();

    assert_eq!(
        filters,
        vec![
            ("FileZilla site manager".to_string(), &["xml"][..]),
            ("WinSCP settings".to_string(), &["ini"][..]),
            ("PuTTY registry export".to_string(), &["reg"][..])
        ]
    );
}

#[test]
fn the_chooser_starts_in_the_folder_of_the_typed_path() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sitemanager.xml");
    std::fs::write(&file, "x").unwrap();

    let from_file = pick_request(ImportSource::FileZilla, file.to_str().unwrap());
    let from_folder = pick_request(ImportSource::Putty, dir.path().to_str().unwrap());
    let from_missing_file = pick_request(ImportSource::FileZilla, dir.path().join("gone.xml").to_str().unwrap());

    assert_eq!(from_file.start_dir.as_deref(), Some(dir.path()));
    assert_eq!(from_folder.start_dir.as_deref(), Some(dir.path()));
    assert_eq!(from_missing_file.start_dir.as_deref(), Some(dir.path()));
}

#[test]
fn a_typed_path_in_a_missing_folder_falls_back_without_panicking() {
    let request = pick_request(ImportSource::WinScp, "/definitely/not/here/WinSCP.ini");

    assert!(request.start_dir.as_deref().is_none_or(|dir| dir.is_dir()));
}

#[test]
fn only_a_row_the_core_judged_invalid_is_marked_invalid() {
    let view = mixed();

    let flags: Vec<bool> = view.rows().iter().map(|row| row.invalid).collect();

    assert_eq!(flags, [false, false, false, false, true]);
}

#[test]
fn a_status_text_that_starts_like_the_invalid_one_does_not_make_a_row_invalid() {
    let view = ImportView::new(&plan(vec![other_row(
        "zeta",
        Verdict::Unsupported("can't import: it only looks invalid".to_string()),
    )]));

    assert!(!view.rows()[0].invalid);
}
