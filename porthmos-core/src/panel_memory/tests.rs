use super::*;
use crate::listing::{SortKey, SortOrder, SortSpec};

fn settings(show_hidden: bool, key: SortKey, order: SortOrder) -> PanelSettings {
    PanelSettings { show_hidden, sort: SortSpec { key, order } }
}

#[test]
fn what_was_remembered_wins_over_the_configured_defaults() {
    let remembered = PanelMemory::of(&settings(true, SortKey::Size, SortOrder::Descending));

    let applied = remembered.applied_to(&PanelSettings::default());

    assert_eq!(applied, settings(true, SortKey::Size, SortOrder::Descending));
}

#[test]
fn nothing_remembered_leaves_the_configured_defaults() {
    let configured = settings(true, SortKey::Size, SortOrder::Ascending);

    assert_eq!(PanelMemory::default().applied_to(&configured), configured);
}

#[test]
fn a_value_that_is_not_understood_falls_back_to_the_configured_one() {
    let memory = PanelMemory {
        show_hidden: Some(true),
        sort_key: Some("date".to_string()),
        sort_order: Some("sideways".to_string()),
    };
    let configured = settings(false, SortKey::Size, SortOrder::Descending);

    assert_eq!(memory.applied_to(&configured), settings(true, SortKey::Size, SortOrder::Descending));
}

#[test]
fn every_choice_survives_being_written_and_read_back() {
    for show_hidden in [false, true] {
        for key in [SortKey::Name, SortKey::Size] {
            for order in [SortOrder::Ascending, SortOrder::Descending] {
                let original = settings(show_hidden, key, order);
                let text = toml::to_string(&PanelMemory::of(&original)).unwrap();

                let read: PanelMemory = toml::from_str(&text).unwrap();

                assert_eq!(read.applied_to(&settings(!show_hidden, SortKey::Name, SortOrder::Ascending)), original);
            }
        }
    }
}

#[test]
fn the_choice_is_written_with_the_words_the_config_file_uses() {
    let text = toml::to_string(&PanelMemory::of(&settings(true, SortKey::Size, SortOrder::Descending))).unwrap();

    assert_eq!(text.trim(), "show_hidden = true\nsort_key = \"size\"\nsort_order = \"descending\"");
}
