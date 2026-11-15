use porthmos_vfs::{Entry, VfsPath};

use super::*;
use crate::Location;

const REMOTE: Location = Location::Session(1);

fn entry(name: &str, is_dir: bool) -> Entry {
    Entry {
        name: name.to_string(),
        path: VfsPath::new(&format!("/{name}")),
        is_dir,
        size: 0,
        permissions: None,
        modified: None,
        owner: None,
        group: None,
        link_target: None,
        hidden: name.starts_with('.'),
    }
}

fn folders(names: &[&str]) -> Vec<Entry> {
    names.iter().map(|name| entry(name, true)).collect()
}

fn names(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("folder{index:02}")).collect()
}

#[test]
fn a_single_matching_folder_is_completed_with_a_trailing_slash() {
    let mut entries = folders(&["log", "lib"]);
    entries.push(entry("notes.txt", false));

    let completion = complete("/var/lo", REMOTE, &entries);

    assert_eq!(completion.candidates, ["log"]);
    assert_eq!(completion.completed, "/var/log/");
    assert_eq!(completion.prefix, "/var/");
}

#[test]
fn several_matches_complete_to_their_shared_prefix() {
    let entries = folders(&["lib", "local", "lock", "log"]);

    assert_eq!(complete("/var/l", REMOTE, &entries).completed, "/var/l");

    let narrowed = complete("/var/lo", REMOTE, &entries);
    assert_eq!(narrowed.completed, "/var/lo");
    assert_eq!(narrowed.candidates, ["local", "lock", "log"]);

    let narrower = complete("/var/loc", REMOTE, &entries);
    assert_eq!(narrower.completed, "/var/loc");
    assert_eq!(narrower.candidates, ["local", "lock"]);

    let shared = complete("/x/li", REMOTE, &folders(&["lib64", "lib32"]));
    assert_eq!(shared.completed, "/x/lib");
}

#[test]
fn files_are_never_offered() {
    let entries = vec![entry("notes.txt", false), entry("notebook", false)];

    let completion = complete("/h/not", REMOTE, &entries);

    assert!(completion.candidates.is_empty());
    assert_eq!(completion.completed, "/h/not");
}

#[test]
fn hidden_folders_need_a_typed_dot() {
    let entries = folders(&[".cache", "docs"]);

    assert_eq!(complete("/h/", REMOTE, &entries).candidates, ["docs"]);
    assert_eq!(complete("/h/.c", REMOTE, &entries).candidates, [".cache"]);
}

#[test]
fn a_folder_with_the_hidden_attribute_but_no_leading_dot_is_offered() {
    let mut attribute_hidden = entry("AppData", true);
    attribute_hidden.hidden = true;

    let completion = complete("C:/Users/me/App", REMOTE, &[attribute_hidden]);

    assert_eq!(completion.candidates, ["AppData"]);
}

#[test]
fn matching_is_case_sensitive() {
    let completion = complete("/h/d", REMOTE, &folders(&["Docs"]));

    assert!(completion.candidates.is_empty());
    assert_eq!(completion.completed, "/h/d");
}

#[test]
fn names_with_spaces_and_unicode_are_kept_as_is() {
    let entries = folders(&["My Docs", "ünï"]);

    assert_eq!(complete("/h/My", REMOTE, &entries).completed, "/h/My Docs/");
    assert_eq!(complete("/h/ü", REMOTE, &entries).completed, "/h/ünï/");
}

#[test]
fn a_trailing_separator_lists_every_folder_and_adds_nothing() {
    let completion = complete("/var/", REMOTE, &folders(&["src", "docs"]));

    assert_eq!(completion.completed, "/var/");
    assert_eq!(completion.candidates, ["docs", "src"]);
}

#[test]
fn a_relative_fragment_has_an_empty_prefix() {
    let completion = complete("su", REMOTE, &folders(&["sub"]));

    assert_eq!(completion.prefix, "");
    assert_eq!(completion.completed, "sub/");
}

#[test]
fn a_lone_tilde_completes_to_tilde_slash() {
    let completion = complete("~", REMOTE, &[]);

    assert_eq!(completion.completed, "~/");
    assert!(completion.candidates.is_empty());
}

#[test]
fn split_directory_uses_the_last_separator() {
    assert_eq!(split_directory("/a/b/c", REMOTE), ("/a/b/", "c"));
    assert_eq!(split_directory("c", REMOTE), ("", "c"));
    assert_eq!(split_directory("/a/", REMOTE), ("/a/", ""));
}

#[cfg(windows)]
#[test]
fn a_local_windows_path_splits_at_the_last_backslash() {
    assert_eq!(split_directory("C:\\Users\\na", Location::Local), ("C:\\Users\\", "na"));
    let completion = complete("C:\\Users\\na", Location::Local, &folders(&["nathan"]));
    assert_eq!(completion.completed, "C:\\Users\\nathan\\");
}

#[test]
fn step_replaces_when_text_grew_shows_when_stuck_and_does_nothing_otherwise() {
    let grew = complete("/var/lo", REMOTE, &folders(&["log"]));
    assert_eq!(grew.step("/var/lo"), CompletionStep::Replace("/var/log/".to_string()));

    let stuck = complete("/var/loc", REMOTE, &folders(&["local", "lock"]));
    assert_eq!(stuck.step("/var/loc"), CompletionStep::Show(vec!["local".to_string(), "lock".to_string()]));

    let none = complete("/var/zz", REMOTE, &folders(&["log"]));
    assert_eq!(none.step("/var/zz"), CompletionStep::Nothing);

    let done = complete("/var/log/", REMOTE, &[]);
    assert_eq!(done.step("/var/log/"), CompletionStep::Nothing);

    let lone = complete("/var/", REMOTE, &folders(&["log"]));
    assert_eq!(lone.step("/var/log/"), CompletionStep::Nothing);
}

#[test]
fn shown_caps_the_list_and_counts_the_rest() {
    let many = Completion { candidates: names(25), ..Completion::default() };
    let (shown, more) = many.shown();
    assert_eq!((shown.len(), more), (MAX_SHOWN, 5));
    assert_eq!(shown[0], "folder00");

    let few = Completion { candidates: names(3), ..Completion::default() };
    assert_eq!((few.shown().0.len(), few.shown().1), (3, 0));
}

#[test]
fn path_for_joins_prefix_name_and_separator() {
    let completion = complete("/var/l", REMOTE, &folders(&["lib", "log"]));

    assert_eq!(completion.path_for("log"), "/var/log/");

    let relative = complete("l", REMOTE, &folders(&["lib", "log"]));
    assert_eq!(relative.path_for("log"), "log/");
}
