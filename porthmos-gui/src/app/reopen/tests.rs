use porthmos_core::{Command, Event};

use crate::palette::Target;
use crate::testing::Harness;

use super::super::*;
use super::offer_label;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| name.to_string()).collect()
}

fn offered(harness: &mut Harness, list: &[&str]) {
    harness.feed(Event::ReopenOffer { names: names(list) });
    harness.sent();
}

fn reopen_commands(sent: &[Command]) -> usize {
    sent.iter().filter(|command| matches!(command, Command::ReopenConnections)).count()
}

#[test]
fn there_is_no_offer_until_core_makes_one() {
    let harness = Harness::new();

    assert!(harness.app.reopen_offer().is_empty());
}

#[test]
fn an_offer_from_core_is_kept_in_order() {
    let mut harness = Harness::new();

    harness.feed(Event::ReopenOffer { names: names(&["prod", "nas"]) });

    assert_eq!(harness.app.reopen_offer(), names(&["prod", "nas"]).as_slice());
}

#[test]
fn pressing_reopen_asks_core_once_and_withdraws_the_offer() {
    let mut harness = Harness::new();
    offered(&mut harness, &["prod", "nas"]);

    harness.press(Message::ReopenPressed);
    let first = harness.sent();
    harness.press(Message::ReopenPressed);
    let second = harness.sent();

    assert_eq!(reopen_commands(&first), 1);
    assert_eq!(reopen_commands(&second), 0);
    assert!(harness.app.reopen_offer().is_empty());
}

#[test]
fn pressing_reopen_without_an_offer_sends_nothing() {
    let mut harness = Harness::new();

    harness.press(Message::ReopenPressed);

    assert_eq!(reopen_commands(&harness.sent()), 0);
}

#[test]
fn the_palette_offers_to_reopen_only_while_there_is_something_to_reopen() {
    let mut harness = Harness::new();
    let without = harness.app.palette_entries();
    offered(&mut harness, &["prod"]);

    let with = harness.app.palette_entries();

    assert!(!without.iter().any(|entry| entry.target == Target::Reopen));
    let entry = with.iter().find(|entry| entry.target == Target::Reopen).expect("a reopen entry");
    assert_eq!(entry.label, "Reopen last connections");
}

#[test]
fn choosing_the_palette_entry_reopens_like_the_button() {
    let mut harness = Harness::new();
    offered(&mut harness, &["prod"]);

    let _ = harness.app.run_palette_target(Target::Reopen);

    assert_eq!(reopen_commands(&harness.sent()), 1);
    assert!(harness.app.reopen_offer().is_empty());
}

#[test]
fn the_label_names_the_connections_that_would_be_reopened() {
    assert_eq!(offer_label(&names(&["prod"])), "Reopen prod");
    assert_eq!(offer_label(&names(&["prod", "nas"])), "Reopen prod, nas");
    assert_eq!(offer_label(&names(&["prod", "nas", "db"])), "Reopen prod, nas, db");
}

#[test]
fn a_long_list_is_shortened_in_the_label() {
    assert_eq!(offer_label(&names(&["a", "b", "c", "d", "e"])), "Reopen a, b, c and 2 more");
}
