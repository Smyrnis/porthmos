use std::collections::BTreeMap;

use porthmos_core::connections_tree::TreeRow;
use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};

use super::*;

fn entry(name: &str, group: Option<&str>, tags: &[&str]) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: format!("{name}.example.com"),
        port: 22,
        username: "me".to_string(),
        password: None,
        options: BTreeMap::new(),
        group: group.map(str::to_string),
        tags: tags.iter().map(|tag| tag.to_string()).collect(),
        saved_password: false,
        in_keyring: Vec::new(),
        source: ConnectionSource::Profile,
    }
}

fn manager(entries: Vec<ConnectionEntry>) -> Manager {
    let mut manager = Manager::default();
    manager.replace(entries);
    manager
}

fn grouped() -> Manager {
    manager(vec![entry("a", Some("Work"), &["prod"]), entry("b", None, &[]), entry("c", Some("Work/Db"), &["dev"])])
}

#[test]
fn rows_group_connections_and_collapse_by_default() {
    let manager = manager(vec![entry("a", Some("Work"), &[]), entry("b", None, &[])]);
    assert_eq!(
        manager.rows(),
        vec![
            TreeRow::Group { path: "Work".to_string(), name: "Work".to_string(), depth: 0, count: 1, expanded: false },
            TreeRow::Connection { index: 1, depth: 0 },
        ]
    );
}

#[test]
fn toggle_group_expands_and_collapses() {
    let mut manager = manager(vec![entry("a", Some("Work"), &[])]);
    manager.toggle_group("Work");
    assert_eq!(manager.rows().len(), 2);
    manager.toggle_group("Work");
    assert_eq!(manager.rows().len(), 1);
}

#[test]
fn filter_opens_every_group_and_hides_non_matches() {
    let mut manager = grouped();
    manager.set_filter("c.example".to_string());
    let rows = manager.rows();
    assert_eq!(rows.len(), 3);
    assert!(matches!(rows[2], TreeRow::Connection { index: 2, depth: 2 }));
    assert_eq!(manager.match_count(), 1);
}

#[test]
fn hash_filter_matches_tags_only() {
    let mut manager = grouped();
    manager.set_filter("#prod".to_string());
    assert_eq!(manager.match_count(), 1);
    manager.set_filter("#".to_string());
    assert_eq!(manager.match_count(), 3);
}

#[test]
fn replace_keeps_selection_by_name() {
    let mut manager = manager(vec![entry("a", None, &[]), entry("b", None, &[])]);
    manager.select_row(1);
    assert_eq!(manager.selected_entry().unwrap().name, "b");
    manager.replace(vec![entry("0", None, &[]), entry("a", None, &[]), entry("b", None, &[])]);
    assert_eq!(manager.selected_entry().unwrap().name, "b");
}

#[test]
fn replace_clears_selection_when_the_entry_is_gone() {
    let mut manager = manager(vec![entry("a", None, &[]), entry("b", None, &[])]);
    manager.select_row(1);
    manager.replace(vec![entry("a", None, &[])]);
    assert!(manager.selected_entry().is_none());
}

#[test]
fn a_shadowed_label_does_not_steal_the_selection_from_its_profile() {
    let mut shadowed = entry("a", None, &[]);
    shadowed.source = ConnectionSource::ShadowedSshHost;
    let mut manager = manager(vec![entry("a", None, &[]), shadowed.clone()]);
    manager.select_row(1);
    assert_eq!(manager.selected_entry().unwrap().source, ConnectionSource::ShadowedSshHost);
    manager.replace(vec![shadowed, entry("a", None, &[])]);
    assert_eq!(manager.selected_entry().unwrap().source, ConnectionSource::ShadowedSshHost);
}

#[test]
fn select_row_on_a_group_selects_no_entry() {
    let mut manager = grouped();
    manager.select_row(1);
    assert!(manager.selected_entry().is_some());
    manager.select_row(0);
    assert!(manager.selected_entry().is_none());
    assert_eq!(manager.selected_row(), Some(0));
}

#[test]
fn move_selection_clamps_at_both_ends() {
    let mut manager = manager(vec![entry("a", None, &[]), entry("b", None, &[])]);
    manager.move_selection(false);
    assert_eq!(manager.selected_entry().unwrap().name, "b");
    manager.move_selection(true);
    manager.move_selection(true);
    assert_eq!(manager.selected_entry().unwrap().name, "b");
    manager.move_selection(false);
    manager.move_selection(false);
    assert_eq!(manager.selected_entry().unwrap().name, "a");
}

#[test]
fn move_selection_walks_through_group_rows() {
    let mut manager = grouped();
    manager.select_row(0);
    manager.move_selection(true);
    assert_eq!(manager.selected_row(), Some(1));
}

#[test]
fn reveal_expands_the_group_chain_and_selects() {
    let mut manager = grouped();
    manager.reveal("c");
    assert_eq!(manager.selected_entry().unwrap().name, "c");
    assert!(
        manager
            .rows()
            .iter()
            .any(|row| matches!(row, TreeRow::Group { path, expanded: true, .. } if path == "Work/Db"))
    );
}

#[test]
fn reveal_of_an_unknown_name_changes_nothing() {
    let mut manager = grouped();
    manager.reveal("zzz");
    assert!(manager.selected_entry().is_none());
}

#[test]
fn badge_and_subtitle_text() {
    let mut item = entry("a", None, &[]);
    item.protocol = "webdav".to_string();
    item.port = 8099;
    assert_eq!(badge(&item), "WEBDAV");
    assert_eq!(subtitle(&item), "me@a.example.com:8099");
}

#[test]
fn orphan_label_subtitle_says_labels_only() {
    let mut item = entry("a", None, &[]);
    item.source = ConnectionSource::MissingSshHost;
    assert_eq!(subtitle(&item), "labels only");
}

#[test]
fn unlabelled_hosts_are_ssh_config_entries_without_group_or_tags() {
    let mut labelled = entry("b", Some("G"), &[]);
    labelled.source = ConnectionSource::SshConfig;
    let mut plain = entry("a", None, &[]);
    plain.source = ConnectionSource::SshConfig;
    let saved = entry("c", None, &[]);
    let manager = manager(vec![plain, labelled, saved]);

    assert_eq!(manager.unlabelled_hosts(), ["a"]);
}

#[test]
fn orphans_are_announced_once_and_remembered() {
    let mut gone = entry("gone", None, &[]);
    gone.source = ConnectionSource::MissingSshHost;
    let mut hidden = entry("hidden", None, &[]);
    hidden.source = ConnectionSource::ShadowedSshHost;
    let entries = vec![gone, hidden, entry("fine", None, &[])];
    let mut announced = HashSet::new();

    let first = announcement(&entries, &mut announced).unwrap();
    assert_eq!(
        first,
        "These labelled hosts are no longer in ~/.ssh/config: gone. These labelled hosts are hidden by a saved \
         connection of the same name: hidden."
    );
    assert!(announcement(&entries, &mut announced).is_none());
}

#[test]
fn nothing_is_announced_without_orphans() {
    let entries = vec![entry("fine", None, &[])];

    assert!(announcement(&entries, &mut HashSet::new()).is_none());
}

#[test]
fn filtering_clears_a_selection_that_is_no_longer_shown() {
    let mut manager = manager(vec![entry("alpha", None, &[]), entry("beta", None, &[])]);
    manager.select_row(0);

    manager.set_filter("beta".to_string());

    assert!(manager.selected_entry().is_none());
}

#[test]
fn filtering_keeps_a_selection_that_still_matches() {
    let mut manager = manager(vec![entry("alpha", None, &[]), entry("beta", None, &[])]);
    manager.select_row(1);

    manager.set_filter("bet".to_string());

    assert_eq!(manager.selected_entry().unwrap().name, "beta");
}

#[test]
fn collapsing_a_group_clears_a_selection_inside_it() {
    let mut manager = manager(vec![entry("a", Some("Work"), &[])]);
    manager.reveal("a");
    assert_eq!(manager.selected_entry().unwrap().name, "a");

    manager.toggle_group("Work");

    assert!(manager.selected_entry().is_none());
}

#[test]
fn replace_drops_a_selection_the_filter_no_longer_matches() {
    let mut manager = manager(vec![entry("alpha", None, &[]), entry("beta", None, &[])]);
    manager.set_filter("alpha.example".to_string());
    manager.select_row(0);
    assert_eq!(manager.selected_entry().unwrap().name, "alpha");
    let mut moved = entry("alpha", None, &[]);
    moved.host = "elsewhere.test".to_string();

    manager.replace(vec![moved, entry("beta", None, &[])]);

    assert!(manager.selected_entry().is_none());
}

#[test]
fn reveal_clears_a_filter_that_hides_the_entry() {
    let mut manager = manager(vec![entry("alpha", None, &[]), entry("beta", None, &[])]);
    manager.set_filter("alp".to_string());

    manager.reveal("beta");

    assert_eq!(manager.filter(), "");
    assert_eq!(manager.selected_entry().unwrap().name, "beta");
}

#[test]
fn reveal_keeps_a_filter_that_matches_the_entry() {
    let mut manager = manager(vec![entry("alpha", None, &[]), entry("beta", None, &[])]);
    manager.set_filter("alp".to_string());

    manager.reveal("alpha");

    assert_eq!(manager.filter(), "alp");
}
