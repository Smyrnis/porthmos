use porthmos_vfs::Entry;

use super::{MAX_LISTED_NAMES, THIS_COMPUTER, delete_summary};

fn summary(entries: &[Entry], place: &str) -> Vec<String> {
    delete_summary(entries, place, str::to_string)
}

fn file(name: &str) -> Entry {
    Entry { name: name.to_string(), is_dir: false, ..Default::default() }
}

fn folder(name: &str) -> Entry {
    Entry { name: name.to_string(), is_dir: true, ..Default::default() }
}

#[test]
fn one_file_is_named_with_the_place_it_goes_from() {
    assert_eq!(summary(&[file("big.bin")], "prod"), ["Delete \"big.bin\" from prod?"]);
}

#[test]
fn the_same_name_on_this_computer_says_so() {
    assert_eq!(summary(&[file("big.bin")], THIS_COMPUTER), ["Delete \"big.bin\" from this computer?"]);
}

#[test]
fn one_folder_says_that_everything_inside_it_goes_too() {
    assert_eq!(
        summary(&[folder("photos")], "prod"),
        ["Delete the folder \"photos\" and everything inside it from prod?"]
    );
}

#[test]
fn several_files_give_a_count_the_names_and_the_place() {
    assert_eq!(summary(&[file("a"), file("b")], "prod"), ["Delete 2 items from prod?", "a", "b"]);
}

#[test]
fn folders_among_several_items_are_marked_and_explained() {
    assert_eq!(
        summary(&[folder("docs"), file("a")], THIS_COMPUTER),
        ["Delete 2 items from this computer?", "docs/", "a", "Folders are deleted with everything inside them."]
    );
}

#[test]
fn at_most_five_names_are_listed_and_the_rest_are_counted() {
    let many: Vec<Entry> = (0..50_000).map(|n| file(&format!("f{n}"))).collect();

    let lines = summary(&many, "prod");

    assert_eq!(lines.len(), 1 + MAX_LISTED_NAMES + 1);
    assert_eq!(lines[0], "Delete 50000 items from prod?");
    assert_eq!(lines.last().unwrap(), "and 49995 more");
}

#[test]
fn the_folder_note_counts_folders_that_are_not_among_the_listed_names() {
    let mut entries: Vec<Entry> = (0..MAX_LISTED_NAMES + 3).map(|n| file(&format!("f{n}"))).collect();
    entries.push(folder("hidden-at-the-end"));

    let lines = summary(&entries, "prod");

    assert_eq!(lines.last().unwrap(), "Folders are deleted with everything inside them.");
    assert!(lines.contains(&"and 4 more".to_string()));
}

#[test]
fn nothing_to_delete_gives_no_lines() {
    assert!(summary(&[], "prod").is_empty());
}

#[test]
fn only_the_names_that_are_shown_go_through_the_display_function() {
    let many: Vec<Entry> = (0..100).map(|n| file(&format!("f{n}"))).collect();
    let shown = std::cell::Cell::new(0);

    let lines = delete_summary(&many, "prod", |name| {
        shown.set(shown.get() + 1);
        name.to_uppercase()
    });

    assert_eq!(shown.get(), MAX_LISTED_NAMES);
    assert_eq!(lines[1], "F0");
}

#[test]
fn a_single_name_goes_through_the_display_function_too() {
    let lines = delete_summary(&[file("a\nb")], "prod", |name| name.replace('\n', "\u{b7}"));

    assert_eq!(lines, ["Delete \"a\u{b7}b\" from prod?"]);
}
