use porthmos_core::folder_compare::Mark;
use porthmos_core::{Entry, Event, Location, Severity, VfsPath};

use crate::shortcuts::Action;
use crate::testing::Harness;

fn dated(dir: &VfsPath, name: &str, modified: u64) -> Entry {
    Entry {
        name: name.to_string(),
        path: dir.join(name),
        is_dir: false,
        size: 1,
        modified: Some(modified),
        ..Default::default()
    }
}

fn both_sides() -> Harness {
    let mut harness = Harness::new();
    let local = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: local.clone(),
        entries: vec![dated(&local, "same.txt", 100), dated(&local, "mine.txt", 100), dated(&local, "news.txt", 5000)],
    });
    harness.feed(Event::Connected { session: 1, name: "alpha".to_string(), shell_available: false });
    let remote = VfsPath::new("/srv");
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: remote.clone(),
        entries: vec![
            dated(&remote, "same.txt", 100),
            dated(&remote, "theirs.txt", 100),
            dated(&remote, "news.txt", 10),
        ],
    });
    harness.sent();
    harness
}

fn last_notice(harness: &Harness) -> (Severity, String) {
    let notice = harness.app.notices().last().expect("a notice");
    (notice.severity, notice.message.to_string())
}

fn perform(harness: &mut Harness) {
    let _ = harness.app.perform(Action::CompareFolders);
}

#[test]
fn it_starts_off_and_marks_nothing() {
    let harness = both_sides();

    assert!(!harness.app.comparing_folders());
    assert_eq!(harness.app.folder_marks(), None);
}

#[test]
fn turning_it_on_reports_how_many_entries_differ() {
    let mut harness = both_sides();

    perform(&mut harness);

    assert!(harness.app.comparing_folders());
    assert_eq!(last_notice(&harness), (Severity::Info, "Comparing folders: 3 differences".to_string()));
}

#[test]
fn the_marks_follow_the_two_panels() {
    let mut harness = both_sides();
    perform(&mut harness);

    let marks = harness.app.folder_marks().expect("marks while comparing");

    assert_eq!(marks.left("mine.txt"), Some(Mark::OnlyHere));
    assert_eq!(marks.left("news.txt"), Some(Mark::Newer));
    assert_eq!(marks.right("news.txt"), Some(Mark::Older));
    assert_eq!(marks.right("theirs.txt"), Some(Mark::OnlyHere));
    assert_eq!(marks.left("same.txt"), None);
}

#[test]
fn a_new_listing_updates_the_marks_without_asking_again() {
    let mut harness = both_sides();
    perform(&mut harness);
    let remote = VfsPath::new("/srv");

    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: remote.clone(),
        entries: vec![dated(&remote, "mine.txt", 100)],
    });

    let marks = harness.app.folder_marks().unwrap();
    assert_eq!(marks.left("mine.txt"), None);
    assert_eq!(marks.left("news.txt"), Some(Mark::OnlyHere));
}

#[test]
fn identical_folders_are_reported_as_identical() {
    let mut harness = Harness::new();
    let local = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: local.clone(),
        entries: vec![dated(&local, "a", 1)],
    });
    harness.feed(Event::Connected { session: 1, name: "alpha".to_string(), shell_available: false });
    let remote = VfsPath::new("/srv");
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: remote.clone(),
        entries: vec![dated(&remote, "a", 1)],
    });

    perform(&mut harness);

    assert_eq!(last_notice(&harness), (Severity::Info, "Comparing folders: they are identical".to_string()));
}

#[test]
fn a_single_difference_is_not_pluralised() {
    let mut harness = Harness::new();
    let local = harness.start_folder();
    harness.feed(Event::Listed {
        location: Location::Local,
        path: local.clone(),
        entries: vec![dated(&local, "a", 1)],
    });
    harness.feed(Event::Connected { session: 1, name: "alpha".to_string(), shell_available: false });

    perform(&mut harness);

    assert_eq!(last_notice(&harness), (Severity::Info, "Comparing folders: 1 difference".to_string()));
}

#[test]
fn without_a_connected_server_it_stays_off_and_says_why() {
    let mut harness = Harness::new();

    perform(&mut harness);

    assert!(!harness.app.comparing_folders());
    assert_eq!(last_notice(&harness), (Severity::Warning, "Connect to a server to compare folders".to_string()));
}

#[test]
fn the_second_press_turns_it_off_and_clears_the_marks() {
    let mut harness = both_sides();
    perform(&mut harness);

    perform(&mut harness);

    assert!(!harness.app.comparing_folders());
    assert_eq!(harness.app.folder_marks(), None);
}

#[test]
fn closing_the_last_connection_leaves_no_marks_behind() {
    let mut harness = both_sides();
    perform(&mut harness);

    harness.feed(Event::Disconnected { session: 1, name: "alpha".to_string() });

    assert_eq!(harness.app.folder_marks(), None);
}

#[test]
fn the_palette_offers_it_only_when_a_server_is_connected() {
    let connected = both_sides();
    let alone = Harness::new();

    let offered =
        |harness: &Harness| harness.app.palette_entries().iter().any(|entry| entry.label == "Compare folders");
    assert!(offered(&connected));
    assert!(!offered(&alone));
}
