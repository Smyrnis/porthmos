use std::time::{Duration, Instant};

use porthmos_core::Location;

use super::*;

fn action(label: &str) -> Entry {
    Entry { group: Group::Actions, label: label.to_string(), hint: None, target: Target::Action(Action::Quit) }
}

fn host(label: &str) -> Entry {
    Entry {
        group: Group::Hosts,
        label: label.to_string(),
        hint: Some("sftp".to_string()),
        target: Target::Connect(label.to_string()),
    }
}

fn file(label: &str) -> Entry {
    Entry {
        group: Group::Files,
        label: label.to_string(),
        hint: Some("local".to_string()),
        target: Target::File { location: Location::Local, name: label.to_string() },
    }
}

fn mixed() -> Palette {
    Palette::new(vec![
        action("Copy to the other panel"),
        host("Connect nas"),
        file("copy.txt"),
        action("New connection"),
        host("Connect staging"),
        file("notes.md"),
    ])
}

fn labels(palette: &Palette) -> Vec<&str> {
    palette.hits().iter().map(|hit| palette.entry(hit).label.as_str()).collect()
}

fn type_all(palette: &mut Palette, text: &str) {
    for character in text.chars() {
        palette.type_char(character);
    }
}

#[test]
fn group_titles_are_lowercase() {
    assert_eq!((Group::Actions.title(), Group::Hosts.title(), Group::Files.title()), ("actions", "hosts", "files"));
}

#[test]
fn a_new_palette_lists_actions_and_hosts_but_not_files() {
    assert_eq!(labels(&mixed()), ["Copy to the other panel", "New connection", "Connect nas", "Connect staging"]);
}

#[test]
fn an_empty_query_keeps_the_entry_order_inside_each_group() {
    let palette = Palette::new(vec![action("b"), host("x"), action("a"), action("c"), host("a")]);

    assert_eq!(labels(&palette), ["b", "a", "c", "x", "a"]);
}

#[test]
fn typing_filters_and_ranks_by_score_within_a_group() {
    let mut palette = Palette::new(vec![action("Copy to the other panel"), action("Connect"), action("Open terminal")]);

    type_all(&mut palette, "co");

    assert_eq!(labels(&palette), ["Connect", "Copy to the other panel"]);
}

#[test]
fn hits_are_grouped_actions_then_hosts_then_files_whatever_their_score() {
    let mut palette = Palette::new(vec![file("connect"), host("Connect"), action("Reconnect to the server")]);

    palette.type_char('#');
    assert_eq!(labels(&palette), ["connect"]);
    palette.backspace();
    type_all(&mut palette, "connect");

    assert_eq!(labels(&palette), ["Reconnect to the server", "Connect"]);
    let groups: Vec<Group> = palette.hits().iter().map(|hit| palette.entry(hit).group).collect();
    assert_eq!(groups, [Group::Actions, Group::Hosts]);
}

#[test]
fn the_prefixes_pick_one_group() {
    let mut palette = mixed();

    type_all(&mut palette, ">");
    assert_eq!(labels(&palette), ["Copy to the other panel", "New connection"]);
    palette.backspace();
    type_all(&mut palette, "@");
    assert_eq!(labels(&palette), ["Connect nas", "Connect staging"]);
    palette.backspace();
    type_all(&mut palette, "#");
    assert_eq!(labels(&palette), ["copy.txt", "notes.md"]);
}

#[test]
fn a_bare_prefix_lists_everything_in_its_scope() {
    let mut palette = Palette::new((0..30).map(|index| file(&format!("f{index}"))).collect());

    palette.type_char('#');

    assert_eq!(palette.hits().len(), 30);
}

#[test]
fn the_text_after_a_prefix_is_matched_without_it_and_leading_spaces_are_ignored() {
    let mut palette = mixed();

    type_all(&mut palette, ">   copy");

    assert_eq!(labels(&palette), ["Copy to the other panel"]);
    assert_eq!(palette.hits()[0].matched, [0, 1, 2, 3]);
    let mut hosts = mixed();
    type_all(&mut hosts, "@ staging");
    assert_eq!(labels(&hosts), ["Connect staging"]);
}

#[test]
fn a_prefix_in_the_middle_of_the_query_is_ordinary_text() {
    let mut palette = Palette::new(vec![action("a>b"), action("ab")]);

    type_all(&mut palette, "a>b");

    assert_eq!(labels(&palette), ["a>b"]);
}

#[test]
fn a_doubled_prefix_matches_the_rest_as_text() {
    let mut palette = Palette::new(vec![action(">x thing"), action("x thing")]);

    type_all(&mut palette, ">>x");

    assert_eq!(labels(&palette), [">x thing"]);
}

#[test]
fn the_matched_positions_refer_to_the_label() {
    let mut palette = Palette::new(vec![action("New connection")]);

    type_all(&mut palette, "nc");

    assert_eq!(palette.hits()[0].matched, [0, 4]);
}

#[test]
fn editing_the_query_selects_the_first_hit_again() {
    let mut palette = mixed();
    palette.move_selection(2);
    assert_eq!(palette.selected(), 2);

    palette.type_char('c');
    assert_eq!(palette.selected(), 0);
    palette.move_selection(1);
    palette.backspace();
    assert_eq!(palette.selected(), 0);
}

#[test]
fn the_selection_clamps_at_both_ends_without_wrapping() {
    let mut palette = mixed();

    palette.move_selection(-3);
    assert_eq!(palette.selected(), 0);
    palette.move_selection(1);
    assert_eq!(palette.selected(), 1);
    palette.move_selection(100);
    assert_eq!(palette.selected(), 3);
    palette.move_selection(1);
    assert_eq!(palette.selected(), 3);
}

#[test]
fn moving_the_selection_with_no_hits_does_nothing() {
    let mut palette = mixed();
    type_all(&mut palette, "zzzz");

    palette.move_selection(1);
    palette.move_selection(-1);

    assert!(palette.hits().is_empty());
    assert_eq!(palette.selected(), 0);
}

#[test]
fn selected_entry_follows_the_selection_and_is_none_without_hits() {
    let mut palette = mixed();
    assert_eq!(palette.selected_entry().map(|entry| entry.label.as_str()), Some("Copy to the other panel"));

    palette.move_selection(2);
    assert_eq!(palette.selected_entry().map(|entry| entry.label.as_str()), Some("Connect nas"));

    type_all(&mut palette, "zzzz");
    assert!(palette.selected_entry().is_none());
}

#[test]
fn caret_editing_inserts_at_the_caret_and_backspace_and_delete_remove_around_it() {
    let mut palette = mixed();
    type_all(&mut palette, "abc");

    palette.move_caret(-1);
    assert_eq!(palette.caret(), 2);
    palette.type_char('X');
    assert_eq!((palette.query(), palette.caret()), ("abXc", 3));
    palette.backspace();
    assert_eq!((palette.query(), palette.caret()), ("abc", 2));
    palette.delete();
    assert_eq!((palette.query(), palette.caret()), ("ab", 2));
    palette.delete();
    assert_eq!(palette.query(), "ab");
    palette.caret_home();
    palette.backspace();
    assert_eq!((palette.query(), palette.caret()), ("ab", 0));
    palette.delete();
    assert_eq!(palette.query(), "b");
    palette.caret_end();
    assert_eq!(palette.caret(), 1);
}

#[test]
fn the_caret_counts_characters_and_stays_in_bounds() {
    let mut palette = mixed();
    palette.move_caret(-5);
    palette.move_caret(5);
    palette.backspace();
    palette.delete();
    assert_eq!((palette.query(), palette.caret()), ("", 0));

    type_all(&mut palette, "\u{65e5}\u{672c}\u{1f600}");
    assert_eq!(palette.caret(), 3);
    palette.move_caret(-1);
    palette.type_char('x');
    assert_eq!(palette.query(), "\u{65e5}\u{672c}x\u{1f600}");
    palette.move_caret(-100);
    assert_eq!(palette.caret(), 0);
    palette.move_caret(100);
    assert_eq!(palette.caret(), 4);
}

#[test]
fn control_characters_are_never_typed() {
    let mut palette = mixed();

    for character in ['\n', '\u{1b}', '\t', '\u{0}', '\u{202e}', '\u{200f}'] {
        palette.type_char(character);
    }

    assert_eq!(palette.query(), "");
    assert_eq!(palette.hits().len(), 4);
}

#[test]
fn a_space_is_typed() {
    let mut palette = mixed();

    type_all(&mut palette, "new c");

    assert_eq!(palette.query(), "new c");
    assert_eq!(labels(&palette), ["New connection"]);
}

#[test]
fn ten_thousand_files_rank_within_the_budget() {
    let mut palette = Palette::new((0..10_000).map(|index| file(&format!("dir{index}/file_{index}.txt"))).collect());

    let start = Instant::now();
    type_all(&mut palette, "#file_99");

    assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());
    assert!(!palette.hits().is_empty());
    assert!(palette.hits().len() < 10_000);
}

#[test]
fn an_entry_with_an_empty_label_never_panics_and_matches_only_an_empty_query() {
    let mut palette = Palette::new(vec![action(""), action("a")]);
    assert_eq!(palette.hits().len(), 2);

    palette.type_char('a');

    assert_eq!(labels(&palette), ["a"]);
}

#[test]
fn hints_and_targets_travel_with_the_entries() {
    let palette = mixed();

    let nas = palette.hits().iter().map(|hit| palette.entry(hit)).find(|entry| entry.label == "Connect nas").unwrap();
    assert_eq!(nas.hint.as_deref(), Some("sftp"));
    assert_eq!(nas.target, Target::Connect("Connect nas".to_string()));
}
