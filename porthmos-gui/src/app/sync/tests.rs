use std::sync::Arc;

use iced::Size;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::sync::{SyncAction, SyncBy, SyncDirection, SyncFacts, SyncItem, SyncOptions, SyncPlan, SyncReason};
use porthmos_core::{Command, Entry, Event, Location, Severity, VfsPath};

use crate::menus::MoreItem;
use crate::modal::Modal;
use crate::shortcuts::KeyPress;
use crate::testing::Harness;
use crate::tools::Tool;

use super::super::*;
use super::SyncChoice;

fn key(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    let text = match &key {
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    };
    harness.press(Message::KeyPressed(KeyPress { key: key.clone(), modified_key: key, modifiers, text }));
}

fn ctrl_u(harness: &mut Harness) {
    key(harness, Key::Character("u".into()), Modifiers::CTRL);
}

fn connect(harness: &mut Harness, session: u64, name: &str, listed: bool, preserves_times: bool) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    if preserves_times {
        harness.feed(Event::SessionCapabilities { session, preserves_times });
    }
    if listed {
        harness.feed(Event::Listed {
            location: Location::Session(session),
            path: VfsPath::new("/srv/site"),
            entries: vec![Entry {
                name: "index.html".to_string(),
                path: VfsPath::new("/srv/site/index.html"),
                ..Entry::default()
            }],
        });
    }
    harness.sent();
}

fn last_notice(harness: &Harness) -> (Severity, String) {
    let notice = harness.app.notices().last().unwrap();
    (notice.severity, notice.message.clone())
}

fn options_modal(harness: &Harness) -> (u64, String, bool, SyncDirection, SyncBy, bool, Option<String>) {
    match harness.app.modal() {
        Some(Modal::SyncOptions { session, name, offer_both, direction, by, subfolders, error }) => {
            (*session, name.clone(), *offer_both, *direction, *by, *subfolders, error.clone())
        }
        other => panic!("expected the sync options, found {other:?}"),
    }
}

#[test]
fn ctrl_u_without_a_connection_warns() {
    let mut harness = Harness::new();
    harness.sent();

    ctrl_u(&mut harness);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
    assert_eq!(last_notice(&harness), (Severity::Warning, "Connect to a remote server first".to_string()));
}

#[test]
fn ctrl_u_before_the_first_listing_warns() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", false, false);

    ctrl_u(&mut harness);

    assert!(harness.app.modal().is_none());
    assert_eq!(last_notice(&harness), (Severity::Warning, "Wait for the remote folder to finish loading".to_string()));
}

#[test]
fn ctrl_u_opens_the_options_with_the_defaults() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, false);

    ctrl_u(&mut harness);

    assert_eq!(
        options_modal(&harness),
        (3, "alpha".to_string(), false, SyncDirection::LocalToRemote, SyncBy::Time, true, None)
    );
}

#[test]
fn both_is_offered_only_when_the_connection_keeps_times() {
    let mut keeps = Harness::new();
    connect(&mut keeps, 3, "alpha", true, true);
    ctrl_u(&mut keeps);
    assert!(options_modal(&keeps).2);

    let mut loses = Harness::new();
    connect(&mut loses, 4, "beta", true, false);
    ctrl_u(&mut loses);
    assert!(!options_modal(&loses).2);
}

#[test]
fn choosing_a_direction_or_comparison_or_subfolders_updates_the_dialog() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, true);
    ctrl_u(&mut harness);

    harness.press(Message::SyncOptionChosen(SyncChoice::Direction(SyncDirection::RemoteToLocal)));
    harness.press(Message::SyncOptionChosen(SyncChoice::By(SyncBy::Size)));
    harness.press(Message::SyncOptionChosen(SyncChoice::Subfolders(false)));
    let (_, _, _, direction, by, subfolders, error) = options_modal(&harness);
    assert_eq!((direction, by, subfolders, error), (SyncDirection::RemoteToLocal, SyncBy::Size, false, None));

    harness.press(Message::SyncOptionChosen(SyncChoice::Direction(SyncDirection::Both)));
    assert_eq!(options_modal(&harness).3, SyncDirection::Both);
}

#[test]
fn choosing_both_without_support_is_ignored() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, false);
    ctrl_u(&mut harness);

    harness.press(Message::SyncOptionChosen(SyncChoice::Direction(SyncDirection::Both)));

    assert_eq!(options_modal(&harness).3, SyncDirection::LocalToRemote);
}

#[test]
fn a_choice_without_the_dialog_is_ignored() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, true);

    harness.press(Message::SyncOptionChosen(SyncChoice::Subfolders(false)));

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn start_sends_start_sync_with_the_local_and_remote_folders_and_options() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, true);
    ctrl_u(&mut harness);
    harness.press(Message::SyncOptionChosen(SyncChoice::Direction(SyncDirection::Both)));
    harness.press(Message::SyncOptionChosen(SyncChoice::Subfolders(false)));

    harness.press(Message::ModalAccepted);

    assert_eq!(
        harness.sent(),
        vec![Command::StartSync {
            session: 3,
            local_dir: harness.start_folder(),
            remote_dir: VfsPath::new("/srv/site"),
            options: SyncOptions { direction: SyncDirection::Both, by: SyncBy::Time, subfolders: false },
        }]
    );
    assert!(harness.app.modal().is_none());
}

#[test]
fn enter_starts_the_sync() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, false);
    ctrl_u(&mut harness);

    key(&mut harness, Key::Named(Named::Enter), Modifiers::empty());

    assert!(matches!(harness.sent().as_slice(), [Command::StartSync { session: 3, .. }]));
    assert!(harness.app.modal().is_none());
}

#[test]
fn size_with_both_shows_the_core_message_and_keeps_the_dialog() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, true);
    harness.app.modals.push(Modal::SyncOptions {
        session: 3,
        name: "alpha".to_string(),
        offer_both: true,
        direction: SyncDirection::Both,
        by: SyncBy::Size,
        subfolders: true,
        error: None,
    });

    harness.press(Message::ModalAccepted);

    let expected = SyncOptions { direction: SyncDirection::Both, by: SyncBy::Size, subfolders: true }
        .validate()
        .unwrap_err()
        .to_string();
    assert_eq!(options_modal(&harness).6, Some(expected));
    assert!(harness.sent().is_empty());
}

#[test]
fn changing_a_choice_clears_the_error() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, true);
    harness.app.modals.push(Modal::SyncOptions {
        session: 3,
        name: "alpha".to_string(),
        offer_both: true,
        direction: SyncDirection::Both,
        by: SyncBy::Size,
        subfolders: true,
        error: Some("old".to_string()),
    });

    harness.press(Message::SyncOptionChosen(SyncChoice::By(SyncBy::Time)));

    assert_eq!(options_modal(&harness).6, None);
}

#[test]
fn cancel_and_escape_send_nothing() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, false);

    ctrl_u(&mut harness);
    harness.press(Message::ModalDeclined);
    assert!(harness.app.modal().is_none());

    ctrl_u(&mut harness);
    key(&mut harness, Key::Named(Named::Escape), Modifiers::empty());
    assert!(harness.app.modal().is_none());

    assert!(harness.sent().is_empty());
}

#[test]
fn a_vanished_session_warns_instead_of_sending() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, false);
    ctrl_u(&mut harness);
    harness.feed(Event::Disconnected { session: 3, name: "alpha".to_string() });
    harness.sent();

    harness.press(Message::ModalAccepted);

    assert!(harness.sent().is_empty());
    assert_eq!(last_notice(&harness), (Severity::Warning, "That connection is no longer available".to_string()));
    assert!(harness.app.modal().is_none());
}

#[test]
fn the_more_item_opens_the_dialog() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, false);

    harness.press(Message::MoreItemChosen(MoreItem::Sync));

    assert_eq!(options_modal(&harness).0, 3);
}

#[test]
fn sync_is_possible_only_for_a_listed_active_connection() {
    let mut harness = Harness::new();
    assert!(!harness.app.sync_possible());

    connect(&mut harness, 3, "alpha", false, false);
    assert!(!harness.app.sync_possible());

    harness.feed(Event::Listed {
        location: Location::Session(3),
        path: VfsPath::new("/srv/site"),
        entries: Vec::new(),
    });
    assert!(harness.app.sync_possible());
}

fn facts(size: u64, modified: u64) -> Option<SyncFacts> {
    Some(SyncFacts { size, modified: Some(modified) })
}

fn copy_item(id: u32, path: &str) -> SyncItem {
    SyncItem {
        id,
        path: VfsPath::from(path),
        local: facts(10, 5),
        remote: None,
        action: SyncAction::Upload,
        reason: SyncReason::OnlyLocal,
        ticked: true,
        flippable: false,
    }
}

fn odd_item(id: u32, path: &str) -> SyncItem {
    SyncItem {
        id,
        path: VfsPath::from(path),
        local: facts(1, 5),
        remote: facts(2, 5),
        action: SyncAction::Skip,
        reason: SyncReason::SameTimeDifferentSize,
        ticked: false,
        flippable: true,
    }
}

fn mismatch_item(id: u32, path: &str) -> SyncItem {
    SyncItem {
        id,
        path: VfsPath::from(path),
        local: facts(1, 1),
        remote: None,
        action: SyncAction::Skip,
        reason: SyncReason::KindMismatch,
        ticked: false,
        flippable: false,
    }
}

fn plan_with(sync_id: u64, items: Vec<SyncItem>) -> Arc<SyncPlan> {
    plan_in(SyncDirection::LocalToRemote, sync_id, items)
}

fn plan_in(direction: SyncDirection, sync_id: u64, items: Vec<SyncItem>) -> Arc<SyncPlan> {
    Arc::new(SyncPlan {
        sync_id,
        session: 3,
        local_root: VfsPath::from("/home/me/site"),
        remote_root: VfsPath::from("/srv/site"),
        options: SyncOptions { direction, by: SyncBy::Time, subfolders: true },
        items,
        skipped_symlinks: 0,
        unreadable_folders: Vec::new(),
        case_clashes: 0,
        unstorable: 0,
    })
}

fn small_plan(sync_id: u64) -> Arc<SyncPlan> {
    plan_with(
        sync_id,
        vec![copy_item(0, "a.txt"), copy_item(1, "b.txt"), odd_item(2, "c.txt"), mismatch_item(3, "dir")],
    )
}

fn big_plan(rows: u32) -> Arc<SyncPlan> {
    plan_with(11, (0..rows).map(|id| copy_item(id, &format!("file-{id:05}.txt"))).collect())
}

fn review(harness: &mut Harness, plan: Arc<SyncPlan>) {
    harness.feed(Event::SyncPlanReady(plan));
    harness.sent();
}

fn sync_open(harness: &Harness) -> bool {
    matches!(harness.app.tool(), Some(Tool::Sync))
}

fn named(harness: &mut Harness, which: Named) {
    key(harness, Key::Named(which), Modifiers::empty());
}

fn sent_cancels(harness: &mut Harness) -> Vec<u64> {
    harness
        .sent()
        .into_iter()
        .filter_map(|command| match command {
            Command::CancelSync { sync_id } => Some(sync_id),
            _ => None,
        })
        .collect()
}

#[test]
fn a_plan_opens_the_overlay_when_nothing_else_is_open() {
    let mut harness = Harness::new();

    harness.feed(Event::SyncPlanReady(small_plan(11)));

    assert!(sync_open(&harness));
    assert_eq!(harness.app.sync_view.sync_id(), Some(11));
}

#[test]
fn a_plan_is_held_and_announced_behind_a_modal() {
    let mut harness = Harness::new();
    harness.app.modals.push(Modal::Info { title: "t".to_string(), body: "b".to_string() });

    harness.feed(Event::SyncPlanReady(small_plan(11)));

    assert!(!sync_open(&harness));
    assert!(harness.app.sync_view.has_plan());
    assert!(matches!(harness.app.modal(), Some(Modal::Info { .. })));
    assert_eq!(
        last_notice(&harness),
        (Severity::Info, "Sync preview ready; open it from More \u{2192} Synchronize\u{2026}".to_string())
    );
}

#[test]
fn a_plan_is_held_behind_another_tool_and_ctrl_u_reopens_it_with_its_ticks() {
    let mut harness = Harness::new();
    harness.app.open_bookmarks();

    harness.feed(Event::SyncPlanReady(small_plan(11)));

    assert!(matches!(harness.app.tool(), Some(Tool::Bookmarks(_))));
    assert!(harness.app.sync_view.has_plan());
    harness.app.sync_view.toggle();
    harness.press(Message::ToolClosed);
    assert!(harness.app.tool().is_none());
    harness.sent();

    ctrl_u(&mut harness);

    assert!(sync_open(&harness));
    assert_eq!(harness.app.sync_view.visible_choices(), vec![(1, SyncAction::Upload)]);
    assert!(harness.sent().is_empty());
}

#[test]
fn a_plan_is_held_behind_the_connection_manager() {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);
    assert!(harness.app.connections_screen.is_some());

    harness.feed(Event::SyncPlanReady(small_plan(11)));

    assert!(!sync_open(&harness));
    assert!(harness.app.sync_view.has_plan());
}

#[test]
fn a_second_plan_is_cancelled_with_a_warning() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    harness.feed(Event::SyncPlanReady(small_plan(12)));

    assert_eq!(harness.sent(), vec![Command::CancelSync { sync_id: 12 }]);
    assert_eq!(harness.app.sync_view.sync_id(), Some(11));
    assert_eq!(
        last_notice(&harness),
        (Severity::Warning, "A sync preview is already open; the new one was dropped".to_string())
    );
}

#[test]
fn the_same_plan_twice_is_ignored() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));
    harness.app.sync_view.toggle();

    harness.feed(Event::SyncPlanReady(small_plan(11)));

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.sync_view.visible_choices(), vec![(1, SyncAction::Upload)]);
}

#[test]
fn escape_and_cancel_send_cancel_sync_once_and_forget_the_plan() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    named(&mut harness, Named::Escape);

    assert_eq!(harness.sent(), vec![Command::CancelSync { sync_id: 11 }]);
    assert!(!harness.app.sync_view.has_plan());
    assert!(harness.app.tool().is_none());

    named(&mut harness, Named::Escape);
    harness.press(Message::ToolClosed);
    assert!(harness.sent().is_empty());

    review(&mut harness, small_plan(12));
    harness.press(Message::ToolClosed);
    assert_eq!(harness.sent(), vec![Command::CancelSync { sync_id: 12 }]);
    assert!(!harness.app.sync_view.has_plan());
}

#[test]
fn opening_another_tool_cancels_the_open_review() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    harness.app.open_bookmarks();

    assert_eq!(sent_cancels(&mut harness), vec![11]);
    assert!(!harness.app.sync_view.has_plan());
    assert!(matches!(harness.app.tool(), Some(Tool::Bookmarks(_))));
}

#[test]
fn withdrawn_closes_the_overlay_with_a_notice() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    harness.feed(Event::SyncWithdrawn { sync_ids: vec![4, 11] });

    assert!(harness.app.tool().is_none());
    assert!(!harness.app.sync_view.has_plan());
    assert!(harness.sent().is_empty());
    assert_eq!(last_notice(&harness), (Severity::Info, "The sync plan was cancelled".to_string()));
}

#[test]
fn withdrawn_clears_a_held_plan_too() {
    let mut harness = Harness::new();
    harness.app.modals.push(Modal::Info { title: "t".to_string(), body: "b".to_string() });
    harness.feed(Event::SyncPlanReady(small_plan(11)));

    harness.feed(Event::SyncWithdrawn { sync_ids: vec![11] });

    assert!(!harness.app.sync_view.has_plan());
    assert!(harness.app.modal().is_some());
}

#[test]
fn withdrawn_for_another_id_changes_nothing() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));
    let notices = harness.app.notices().len();

    harness.feed(Event::SyncWithdrawn { sync_ids: vec![5, 6] });

    assert!(sync_open(&harness));
    assert!(harness.app.sync_view.has_plan());
    assert_eq!(harness.app.notices().len(), notices);
}

#[test]
fn clicking_a_row_selects_and_toggles_it() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    harness.press(Message::SyncRowPressed(1));

    assert_eq!(harness.app.sync_view.cursor(), 1);
    assert_eq!(harness.app.sync_view.visible_choices(), vec![(0, SyncAction::Upload)]);

    harness.press(Message::SyncRowPressed(2));
    assert_eq!(harness.app.sync_view.visible_choices(), vec![(0, SyncAction::Upload), (2, SyncAction::Upload)]);

    harness.press(Message::SyncRowPressed(3));
    assert_eq!(harness.app.sync_view.cursor(), 3);
    assert_eq!(harness.app.sync_view.visible_choices(), vec![(0, SyncAction::Upload), (2, SyncAction::Upload)]);
}

#[test]
fn space_toggles_the_selected_row() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    named(&mut harness, Named::Space);

    assert_eq!(harness.app.sync_view.visible_choices(), vec![(1, SyncAction::Upload)]);

    named(&mut harness, Named::Space);
    assert_eq!(harness.app.sync_view.visible_choices().len(), 2);
}

#[test]
fn arrow_page_home_end_move_the_cursor_and_scroll() {
    let mut harness = Harness::new();
    harness.press(Message::WindowResized(Size::new(1200.0, 800.0)));
    review(&mut harness, big_plan(200));

    named(&mut harness, Named::ArrowDown);
    assert_eq!(harness.app.sync_view.cursor(), 1);
    named(&mut harness, Named::ArrowUp);
    assert_eq!(harness.app.sync_view.cursor(), 0);

    named(&mut harness, Named::PageDown);
    let page = harness.app.sync_view.cursor();
    assert!(page > 5, "a page is more than a few rows, got {page}");

    named(&mut harness, Named::End);
    assert_eq!(harness.app.sync_view.cursor(), 199);
    assert!(harness.app.tool_scroll() > 0.0);

    named(&mut harness, Named::PageUp);
    assert_eq!(harness.app.sync_view.cursor(), 199 - page);

    named(&mut harness, Named::Home);
    assert_eq!(harness.app.sync_view.cursor(), 0);
    assert_eq!(harness.app.tool_scroll(), 0.0);
}

#[test]
fn typing_in_the_filter_narrows_the_rows_and_keeps_the_cursor_item() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));
    harness.press(Message::SyncRowPressed(2));
    harness.press(Message::SyncRowPressed(2));

    harness.press(Message::SyncFilterTyped("txt".to_string()));
    assert_eq!(harness.app.sync_view.matched(), 3);
    assert_eq!(harness.app.sync_view.selected_item(), Some(2));

    harness.press(Message::SyncFilterTyped("c.t".to_string()));
    assert_eq!(harness.app.sync_view.matched(), 1);
    assert_eq!(harness.app.sync_view.cursor(), 0);
    assert_eq!(harness.app.sync_view.selected_item(), Some(2));
}

#[test]
fn tick_all_untick_all_and_flip_buttons_act_on_visible_rows() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    harness.press(Message::SyncUntickAllPressed);
    assert!(harness.app.sync_view.visible_choices().is_empty());

    harness.press(Message::SyncTickAllPressed);
    assert_eq!(harness.app.sync_view.visible_choices(), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);

    harness.press(Message::SyncRowPressed(2));
    harness.press(Message::SyncUntickAllPressed);
    harness.press(Message::SyncFlipPressed);
    assert!(harness.app.sync_view.visible_choices().is_empty());
    harness.press(Message::SyncFlipPressed);
    assert_eq!(harness.app.sync_view.visible_choices(), vec![(2, SyncAction::Upload)]);
}

#[test]
fn switch_direction_is_a_no_op_on_an_unflippable_row() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));
    harness.press(Message::SyncRowPressed(3));

    harness.press(Message::SyncFlipPressed);

    assert_eq!(harness.app.sync_view.visible_choices(), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn run_sends_only_visible_ticked_rows_with_their_current_actions() {
    let mut harness = Harness::new();
    review(
        &mut harness,
        plan_in(SyncDirection::Both, 11, vec![copy_item(0, "a.txt"), copy_item(1, "b.txt"), odd_item(2, "c.txt")]),
    );
    harness.press(Message::SyncRowPressed(2));
    harness.press(Message::SyncFlipPressed);
    harness.press(Message::SyncFilterTyped("c.txt".to_string()));

    harness.press(Message::SyncRunPressed);

    assert_eq!(harness.sent(), vec![Command::RunSync { sync_id: 11, choices: vec![(2, SyncAction::Download)] }]);
}

#[test]
fn run_with_nothing_ticked_warns_and_sends_nothing() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));
    harness.press(Message::SyncUntickAllPressed);

    harness.press(Message::SyncRunPressed);

    assert!(harness.sent().is_empty());
    assert!(sync_open(&harness));
    assert_eq!(last_notice(&harness), (Severity::Warning, "Nothing is ticked".to_string()));
}

#[test]
fn run_with_a_filter_hiding_every_ticked_row_warns_with_the_rows_shown_text() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));
    harness.press(Message::SyncFilterTyped("dir".to_string()));

    harness.press(Message::SyncRunPressed);

    assert!(harness.sent().is_empty());
    assert_eq!(last_notice(&harness), (Severity::Warning, "Nothing is ticked in the rows shown".to_string()));
}

#[test]
fn run_clears_the_plan_and_closes_the_overlay() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    harness.press(Message::SyncRunPressed);

    assert!(matches!(harness.sent().as_slice(), [Command::RunSync { sync_id: 11, .. }]));
    assert!(harness.app.tool().is_none());
    assert!(!harness.app.sync_view.has_plan());
    named(&mut harness, Named::Escape);
    assert!(harness.sent().is_empty());
}

#[test]
fn enter_does_not_run() {
    let mut harness = Harness::new();
    review(&mut harness, small_plan(11));

    named(&mut harness, Named::Enter);

    assert!(harness.sent().is_empty());
    assert!(sync_open(&harness));
}

#[test]
fn pointer_scrolling_is_tracked() {
    let mut harness = Harness::new();
    review(&mut harness, big_plan(200));

    harness.press(Message::SyncScrolled(120.0));

    assert_eq!(harness.app.tool_scroll(), 120.0);
}

#[test]
fn typing_in_the_filter_does_not_fire_panel_shortcuts() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", true, false);
    review(&mut harness, small_plan(11));
    let hidden_before = harness.app.hidden_shown();

    key(&mut harness, Key::Character("j".into()), Modifiers::empty());
    key(&mut harness, Key::Character("h".into()), Modifiers::CTRL);
    harness.press(Message::SyncFilterTyped("q".to_string()));

    assert_eq!(harness.app.hidden_shown(), hidden_before);
    assert_eq!(harness.app.sync_view.filter(), Some("q"));
    assert!(harness.sent().is_empty());
    assert!(sync_open(&harness));
}
