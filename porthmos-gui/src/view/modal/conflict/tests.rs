use porthmos_core::transfer::plan::ExistingFile;

use super::{apply_text, existing_text, new_text, partial_text};
use crate::conflicts::ConflictView;

fn view(existing: Option<ExistingFile>, partial: Option<ExistingFile>, new_modified: Option<u64>) -> ConflictView {
    ConflictView { file_name: "a.bin".to_string(), existing, partial, new_size: 2048, new_modified, index: 0, total: 1 }
}

fn file(size: u64, modified: Option<u64>) -> Option<ExistingFile> {
    Some(ExistingFile { size, modified, is_dir: false })
}

#[test]
fn a_missing_file_shows_dashes() {
    assert_eq!(existing_text(None), ("—".to_string(), "—".to_string()));
}

#[test]
fn an_existing_file_shows_its_size() {
    assert_eq!(existing_text(file(1536, None)).0, "1.5K");
    assert_eq!(existing_text(file(5, None)).1, "—");
}

#[test]
fn the_new_file_carries_the_newer_or_older_word() {
    let newer = view(file(5, Some(1_000)), None, Some(2_000));
    let older = view(file(5, Some(2_000)), None, Some(1_000));
    let same = view(file(5, Some(1_000)), None, Some(1_000));

    assert!(new_text(&newer).1.ends_with("(newer)"), "{:?}", new_text(&newer));
    assert!(new_text(&older).1.ends_with("(older)"), "{:?}", new_text(&older));
    assert!(!new_text(&same).1.contains('('));
    assert_eq!(new_text(&same).0, "2.0K");
}

#[test]
fn a_partial_copy_is_described_only_when_there_is_one() {
    assert_eq!(partial_text(&view(file(5, None), None, None)), None);
    assert_eq!(
        partial_text(&view(None, file(1024, None), None)),
        Some("A partial copy of 1.0K is on the destination; Resume continues it.".to_string())
    );
    assert_eq!(
        partial_text(&view(None, file(800, None), None)),
        Some("A partial copy of 800 bytes is on the destination; Resume continues it.".to_string())
    );
}

#[test]
fn the_apply_line_names_what_it_does() {
    assert_eq!(apply_text(), "Apply to the rest of this batch");
}
