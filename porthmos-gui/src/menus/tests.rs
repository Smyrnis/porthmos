use super::{MoreItem, MoreState, more_items};

fn state(hidden_shown: bool, file: bool) -> MoreState {
    MoreState { hidden_shown, file, terminal: true }
}

#[test]
fn more_items_toggle_the_hidden_label() {
    let hidden = more_items(state(false, true));
    let shown = more_items(state(true, true));

    assert_eq!(
        hidden,
        vec![
            (MoreItem::SelectAll, "Select all".to_string(), true),
            (MoreItem::ToggleHidden, "Show hidden files".to_string(), true),
            (MoreItem::Preview, "Preview".to_string(), true),
            (MoreItem::Edit, "Edit".to_string(), true),
            (MoreItem::Search, "Search…".to_string(), true),
            (MoreItem::Bookmarks, "Bookmarks…".to_string(), true),
            (MoreItem::BookmarkHere, "Bookmark this folder".to_string(), true),
            (MoreItem::History, "Transfer history…".to_string(), true),
            (MoreItem::Terminal, "Open terminal here".to_string(), true)
        ]
    );
    assert_eq!(shown[1], (MoreItem::ToggleHidden, "Hide hidden files".to_string(), true));
}

fn enabled(items: &[(super::MenuItem, bool)]) -> Vec<super::MenuItem> {
    items.iter().filter(|(_, on)| *on).map(|(item, _)| *item).collect()
}

fn titles(items: &[(super::MenuItem, bool)]) -> Vec<super::MenuItem> {
    items.iter().map(|(item, _)| *item).collect()
}

#[test]
fn entries_menu_lists_all_actions_when_a_connection_exists() {
    use super::{MenuItem, MenuTarget, context_items};

    let items = context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true }, true);

    assert_eq!(
        titles(&items),
        [
            MenuItem::Open,
            MenuItem::Preview,
            MenuItem::Edit,
            MenuItem::CopyToOther,
            MenuItem::MoveToOther,
            MenuItem::Rename,
            MenuItem::Delete,
            MenuItem::NewFolder,
            MenuItem::Refresh
        ]
    );
    assert_eq!(enabled(&items).len(), 9);
}

#[test]
fn copy_and_move_are_disabled_without_the_other_panel() {
    use super::{MenuItem, MenuTarget, context_items};

    let items = context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true }, false);

    assert!(!enabled(&items).contains(&MenuItem::CopyToOther));
    assert!(!enabled(&items).contains(&MenuItem::MoveToOther));
    assert!(enabled(&items).contains(&MenuItem::Delete));
}

#[test]
fn the_parent_row_offers_only_open_new_folder_and_refresh() {
    use super::{MenuItem, MenuTarget, context_items};

    let items = context_items(&MenuTarget::Entries { count: 1, only_parent: true, file: false }, true);

    assert_eq!(enabled(&items), [MenuItem::Open, MenuItem::NewFolder, MenuItem::Refresh]);
}

#[test]
fn rename_and_open_need_exactly_one_entry() {
    use super::{MenuItem, MenuTarget, context_items};

    let items = context_items(&MenuTarget::Entries { count: 3, only_parent: false, file: false }, true);

    assert!(!enabled(&items).contains(&MenuItem::Rename));
    assert!(!enabled(&items).contains(&MenuItem::Open));
    assert!(enabled(&items).contains(&MenuItem::Delete));
}

#[test]
fn background_menu_has_new_folder_and_refresh_only() {
    use super::{MenuItem, MenuTarget, context_items};

    let items = context_items(&MenuTarget::Background, true);

    assert_eq!(titles(&items), [MenuItem::NewFolder, MenuItem::Refresh]);
    assert_eq!(enabled(&items).len(), 2);
}

#[test]
fn every_item_maps_to_its_keyboard_action() {
    use super::MenuItem;
    use crate::shortcuts::Action;

    assert_eq!(MenuItem::CopyToOther.action(), Action::Copy);
    assert_eq!(MenuItem::MoveToOther.action(), Action::Move);
    assert_eq!(MenuItem::Delete.action(), Action::Delete);
    assert_eq!(MenuItem::Open.action(), Action::Open);
}

#[test]
fn preview_in_the_more_menu_follows_the_file_flag() {
    let enabled_for = |file: bool| {
        more_items(state(false, file)).into_iter().find(|(item, _, _)| *item == MoreItem::Preview).unwrap().2
    };

    assert!(enabled_for(true));
    assert!(!enabled_for(false));
}

#[test]
fn preview_needs_exactly_one_file() {
    use super::{MenuItem, MenuTarget, context_items};

    let folder = context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: false }, true);
    let file = context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true }, true);

    assert!(!enabled(&folder).contains(&MenuItem::Preview));
    assert!(enabled(&file).contains(&MenuItem::Preview));
    assert_eq!(MenuItem::Preview.action(), crate::shortcuts::Action::Preview);
}

#[test]
fn edit_needs_exactly_one_file() {
    use super::{MenuItem, MenuTarget, context_items};

    let folder = context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: false }, true);
    let file = context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true }, true);
    let parent = context_items(&MenuTarget::Entries { count: 1, only_parent: true, file: true }, true);

    assert!(!enabled(&folder).contains(&MenuItem::Edit));
    assert!(!enabled(&parent).contains(&MenuItem::Edit));
    assert!(enabled(&file).contains(&MenuItem::Edit));
    assert_eq!(MenuItem::Edit.action(), crate::shortcuts::Action::Edit);
}

#[test]
fn edit_in_the_more_menu_follows_the_file_flag() {
    let enabled_for =
        |file: bool| more_items(state(false, file)).into_iter().find(|(item, _, _)| *item == MoreItem::Edit).unwrap().2;

    assert!(enabled_for(true));
    assert!(!enabled_for(false));
}
