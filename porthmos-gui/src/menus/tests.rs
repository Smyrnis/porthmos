use super::{MoreItem, MoreState, more_items};

fn state(hidden_shown: bool, file: bool) -> MoreState {
    MoreState { hidden_shown, file, sync: true, reopen: false }
}

#[test]
fn more_items_toggle_the_hidden_label() {
    let hidden = more_items(state(false, true));
    let shown = more_items(state(true, true));

    assert_eq!(
        hidden,
        vec![
            (MoreItem::Palette, "Command palette…".to_string(), true),
            (MoreItem::SelectAll, "Select all".to_string(), true),
            (MoreItem::ToggleHidden, "Show hidden files".to_string(), true),
            (MoreItem::Preview, "Preview".to_string(), true),
            (MoreItem::Edit, "Edit".to_string(), true),
            (MoreItem::Search, "Search…".to_string(), true),
            (MoreItem::Bookmarks, "Bookmarks…".to_string(), true),
            (MoreItem::BookmarkHere, "Bookmark this folder".to_string(), true),
            (MoreItem::History, "Transfer history…".to_string(), true),
            (MoreItem::Sync, "Synchronize…".to_string(), true),
            (MoreItem::Reopen, "Reopen last connections".to_string(), false),
            (MoreItem::CheckForUpdates, "Check for updates".to_string(), true)
        ]
    );
    assert_eq!(shown[2], (MoreItem::ToggleHidden, "Hide hidden files".to_string(), true));
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

    let items =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true, follow_log: false }, true);

    assert_eq!(
        titles(&items),
        [
            MenuItem::Open,
            MenuItem::Preview,
            MenuItem::Edit,
            MenuItem::CopyToOther,
            MenuItem::MoveToOther,
            MenuItem::Rename,
            MenuItem::Permissions,
            MenuItem::Properties,
            MenuItem::Delete,
            MenuItem::NewFolder,
            MenuItem::Refresh
        ]
    );
    assert_eq!(enabled(&items).len(), 11);
}

#[test]
fn copy_and_move_are_disabled_without_the_other_panel() {
    use super::{MenuItem, MenuTarget, context_items};

    let items =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true, follow_log: false }, false);

    assert!(!enabled(&items).contains(&MenuItem::CopyToOther));
    assert!(!enabled(&items).contains(&MenuItem::MoveToOther));
    assert!(enabled(&items).contains(&MenuItem::Delete));
}

#[test]
fn the_parent_row_offers_only_open_new_folder_and_refresh() {
    use super::{MenuItem, MenuTarget, context_items};

    let items =
        context_items(&MenuTarget::Entries { count: 1, only_parent: true, file: false, follow_log: false }, true);

    assert_eq!(enabled(&items), [MenuItem::Open, MenuItem::NewFolder, MenuItem::Refresh]);
}

#[test]
fn rename_and_open_need_exactly_one_entry() {
    use super::{MenuItem, MenuTarget, context_items};

    let items =
        context_items(&MenuTarget::Entries { count: 3, only_parent: false, file: false, follow_log: false }, true);

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

    assert_eq!(MenuItem::CopyToOther.action(), Some(Action::Copy));
    assert_eq!(MenuItem::MoveToOther.action(), Some(Action::Move));
    assert_eq!(MenuItem::Delete.action(), Some(Action::Delete));
    assert_eq!(MenuItem::Open.action(), Some(Action::Open));
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

    let folder =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: false, follow_log: false }, true);
    let file =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true, follow_log: false }, true);

    assert!(!enabled(&folder).contains(&MenuItem::Preview));
    assert!(enabled(&file).contains(&MenuItem::Preview));
    assert_eq!(MenuItem::Preview.action(), Some(crate::shortcuts::Action::Preview));
}

#[test]
fn edit_needs_exactly_one_file() {
    use super::{MenuItem, MenuTarget, context_items};

    let folder =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: false, follow_log: false }, true);
    let file =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true, follow_log: false }, true);
    let parent =
        context_items(&MenuTarget::Entries { count: 1, only_parent: true, file: true, follow_log: false }, true);

    assert!(!enabled(&folder).contains(&MenuItem::Edit));
    assert!(!enabled(&parent).contains(&MenuItem::Edit));
    assert!(enabled(&file).contains(&MenuItem::Edit));
    assert_eq!(MenuItem::Edit.action(), Some(crate::shortcuts::Action::Edit));
}

#[test]
fn edit_in_the_more_menu_follows_the_file_flag() {
    let enabled_for =
        |file: bool| more_items(state(false, file)).into_iter().find(|(item, _, _)| *item == MoreItem::Edit).unwrap().2;

    assert!(enabled_for(true));
    assert!(!enabled_for(false));
}

#[test]
fn the_more_item_is_disabled_without_a_listed_connection() {
    let sync_for = |sync: bool| {
        more_items(MoreState { hidden_shown: false, file: false, sync, reopen: false })
            .into_iter()
            .find(|(item, _, _)| *item == MoreItem::Sync)
            .map(|(_, title, enabled)| (title, enabled))
            .unwrap()
    };

    assert_eq!(sync_for(true), ("Synchronize…".to_string(), true));
    assert_eq!(sync_for(false), ("Synchronize…".to_string(), false));
}

#[test]
fn permissions_work_on_any_number_of_real_entries() {
    use super::{MenuItem, MenuTarget, context_items};

    let several =
        context_items(&MenuTarget::Entries { count: 3, only_parent: false, file: false, follow_log: false }, true);
    let parent =
        context_items(&MenuTarget::Entries { count: 1, only_parent: true, file: false, follow_log: false }, true);

    assert!(enabled(&several).contains(&MenuItem::Permissions));
    assert!(!enabled(&parent).contains(&MenuItem::Permissions));
}

#[test]
fn the_permissions_item_maps_to_the_permissions_action() {
    use super::MenuItem;
    use crate::shortcuts::Action;

    assert_eq!(MenuItem::Permissions.action(), Some(Action::Permissions));
    assert_eq!(MenuItem::Permissions.title(), "Permissions…");
}

#[test]
fn properties_work_on_any_number_of_real_entries_and_map_to_their_action() {
    use super::{MenuItem, MenuTarget, context_items};
    use crate::shortcuts::Action;

    let several =
        context_items(&MenuTarget::Entries { count: 3, only_parent: false, file: false, follow_log: false }, true);
    let parent =
        context_items(&MenuTarget::Entries { count: 1, only_parent: true, file: false, follow_log: false }, true);

    assert!(enabled(&several).contains(&MenuItem::Properties));
    assert!(!enabled(&parent).contains(&MenuItem::Properties));
    assert_eq!(MenuItem::Properties.action(), Some(Action::Properties));
    assert_eq!(MenuItem::Properties.title(), "Properties…");
}

#[test]
fn the_reopen_item_is_enabled_only_while_core_offers_connections() {
    let reopen_for = |reopen: bool| {
        more_items(MoreState { hidden_shown: false, file: false, sync: false, reopen })
            .into_iter()
            .find(|(item, _, _)| *item == MoreItem::Reopen)
            .map(|(_, title, enabled)| (title, enabled))
            .unwrap()
    };

    assert_eq!(reopen_for(true), ("Reopen last connections".to_string(), true));
    assert_eq!(reopen_for(false), ("Reopen last connections".to_string(), false));
}

#[test]
fn follow_as_log_is_in_the_menu_only_when_the_target_allows_it() {
    use super::{MenuItem, MenuTarget, context_items};
    let allowed =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true, follow_log: true }, true);
    let refused =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true, follow_log: false }, true);

    assert!(allowed.contains(&(MenuItem::FollowAsLog, true)));
    assert!(!refused.iter().any(|(item, _)| *item == MenuItem::FollowAsLog));
    assert!(!context_items(&MenuTarget::Background, true).iter().any(|(item, _)| *item == MenuItem::FollowAsLog));
}

#[test]
fn follow_as_log_has_no_keyboard_action_and_a_title() {
    use super::MenuItem;
    assert_eq!(MenuItem::FollowAsLog.action(), None);
    assert!(!MenuItem::FollowAsLog.title().is_empty());
}

#[test]
fn follow_as_log_sits_right_after_edit() {
    use super::{MenuItem, MenuTarget, context_items};
    let items =
        context_items(&MenuTarget::Entries { count: 1, only_parent: false, file: true, follow_log: true }, true);
    let order: Vec<MenuItem> = items.iter().map(|(item, _)| *item).collect();

    let edit = order.iter().position(|item| *item == MenuItem::Edit).unwrap();

    assert_eq!(order[edit + 1], MenuItem::FollowAsLog);
}
