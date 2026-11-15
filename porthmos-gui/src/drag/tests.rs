use iced::Point;
use porthmos_core::listing::SortSpec;
use porthmos_core::{Command, Entry, Location, VfsPath};

use super::{Drag, DragEvent, DragSource, Phase, Refusal, Release, Target, decide, decide_drop_in, dragged_entries};
use crate::app::PanelSide;
use crate::native::NativeEvent;
use crate::panel::{Modifiers, Panel};

fn at(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn entry(folder: &str, name: &str, is_dir: bool) -> Entry {
    Entry { name: name.to_string(), path: VfsPath::new(folder).join(name), is_dir, ..Entry::default() }
}

fn source(side: PanelSide, entries: Vec<Entry>) -> DragSource {
    DragSource { side, entries }
}

fn panel(names: &[&str]) -> Panel {
    let folder = VfsPath::new("/data");
    let entries = names.iter().map(|name| entry("/data", name, false)).collect();
    let mut panel = Panel::new(folder.clone(), SortSpec::default(), false);
    panel.replace_listing(folder, entries);
    panel
}

fn local_target(folder: &str) -> Target {
    Target { side: PanelSide::Local, location: Location::Local, folder: VfsPath::new(folder) }
}

fn remote_target(folder: &str) -> Target {
    Target { side: PanelSide::Remote, location: Location::Session(1), folder: VfsPath::new(folder) }
}

fn pressed() -> Drag {
    let mut drag = Drag::default();
    drag.press(Some(source(PanelSide::Local, vec![entry("/data", "a", false)])));
    drag
}

#[test]
fn a_press_and_a_small_move_is_still_a_click() {
    let mut drag = pressed();

    assert_eq!(drag.moved(at(10.0, 10.0)), DragEvent::None);
    assert_eq!(drag.moved(at(13.0, 12.0)), DragEvent::None);

    assert!(!drag.is_active());
    assert_eq!(drag.release(), Release::Click);
}

#[test]
fn moving_past_the_threshold_starts_a_drag_once() {
    let mut drag = pressed();
    drag.moved(at(10.0, 10.0));

    assert_eq!(drag.moved(at(10.0, 17.0)), DragEvent::Started);
    assert!(drag.is_active());
    assert_eq!(drag.moved(at(40.0, 40.0)), DragEvent::Updated);
    assert_eq!(drag.pointer(), Some(at(40.0, 40.0)));
}

#[test]
fn releasing_before_the_threshold_is_not_a_drop() {
    let mut drag = pressed();
    drag.moved(at(10.0, 10.0));
    drag.moved(at(12.0, 10.0));

    assert!(matches!(drag.release(), Release::Click));
    assert_eq!(drag.release(), Release::Nothing);
}

#[test]
fn releasing_after_a_drag_returns_the_source_and_the_pointer() {
    let mut drag = pressed();
    drag.moved(at(10.0, 10.0));
    drag.moved(at(60.0, 10.0));

    match drag.release() {
        Release::Drop { source, at: pointer } => {
            assert_eq!(source.entries.len(), 1);
            assert_eq!(pointer, at(60.0, 10.0));
        }
        other => panic!("{other:?}"),
    }
    assert!(!drag.is_tracking());
}

#[test]
fn escape_cancels_and_a_later_release_drops_nothing() {
    let mut drag = pressed();
    drag.moved(at(10.0, 10.0));
    drag.moved(at(60.0, 10.0));

    drag.cancel();

    assert!(!drag.is_active());
    assert_eq!(drag.moved(at(90.0, 90.0)), DragEvent::None);
    assert_eq!(drag.release(), Release::Nothing);
    assert!(!drag.is_tracking());
}

#[test]
fn a_press_without_a_source_never_starts_a_drag() {
    let mut drag = Drag::default();
    drag.press(None);

    drag.moved(at(10.0, 10.0));
    assert_eq!(drag.moved(at(200.0, 200.0)), DragEvent::None);
    assert_eq!(drag.release(), Release::Click);
}

#[test]
fn moves_and_releases_while_idle_do_nothing() {
    let mut drag = Drag::default();

    assert_eq!(drag.moved(at(5.0, 5.0)), DragEvent::None);
    assert_eq!(drag.release(), Release::Nothing);
    assert!(!drag.is_tracking());
}

#[test]
fn dragging_a_selected_row_drags_the_whole_selection() {
    let mut panel = panel(&["a", "b", "c"]);
    panel.click(1, Modifiers::default());
    panel.click(3, Modifiers { ctrl: true, shift: false });

    let names: Vec<String> = dragged_entries(&panel, 1).into_iter().map(|entry| entry.name).collect();

    assert_eq!(names, ["a", "c"]);
}

#[test]
fn dragging_an_unselected_row_drags_only_that_row() {
    let mut panel = panel(&["a", "b", "c"]);
    panel.click(1, Modifiers::default());

    let names: Vec<String> = dragged_entries(&panel, 2).into_iter().map(|entry| entry.name).collect();

    assert_eq!(names, ["b"]);
}

#[test]
fn the_parent_row_cannot_be_dragged() {
    let mut panel = panel(&["a"]);
    panel.click(1, Modifiers::default());

    assert!(dragged_entries(&panel, 0).is_empty());
    assert!(dragged_entries(&panel, 99).is_empty());
}

#[test]
fn a_local_selection_dropped_on_the_remote_panel_copies_into_its_folder() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "a", false), entry("/data", "b", false)]);

    let command = decide(&dragged, Location::Local, Some(&remote_target("/srv/in")), false);

    assert_eq!(
        command,
        Ok(Command::Copy {
            from: Location::Local,
            entries: dragged.entries.clone(),
            to: Location::Session(1),
            dest_dir: VfsPath::new("/srv/in")
        })
    );
}

#[test]
fn dropped_on_a_folder_row_it_goes_into_that_folder() {
    let dragged = source(PanelSide::Remote, vec![entry("/srv", "x", false)]);

    let command = decide(&dragged, Location::Session(1), Some(&local_target("/home/me/photos")), false);

    assert!(matches!(
        command,
        Ok(Command::Copy { to: Location::Local, dest_dir, .. }) if dest_dir == VfsPath::new("/home/me/photos")
    ));
}

#[test]
fn shift_turns_the_copy_into_a_move() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "a", false)]);

    let command = decide(&dragged, Location::Local, Some(&remote_target("/srv")), true);

    assert!(matches!(command, Ok(Command::Move { from: Location::Local, to: Location::Session(1), .. })));
}

#[test]
fn a_folder_dropped_on_itself_is_refused() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "photos", true)]);

    let result = decide(&dragged, Location::Local, Some(&local_target("/data/photos")), false);

    assert_eq!(result, Err(Refusal::ToItself));
}

#[test]
fn a_folder_dropped_on_its_own_descendant_is_refused() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "photos", true)]);

    let result = decide(&dragged, Location::Local, Some(&local_target("/data/photos/2024/summer")), false);

    assert_eq!(result, Err(Refusal::ToOwnChild));
}

#[test]
fn a_file_with_a_folder_like_prefix_is_not_a_child() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "photos", true)]);

    let result = decide(&dragged, Location::Local, Some(&local_target("/data/photos-old")), false);

    assert_eq!(result, Err(Refusal::SamePanel));
}

#[test]
fn files_dropped_on_the_folder_they_live_in_are_refused() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "a", false)]);

    let result = decide(&dragged, Location::Local, Some(&local_target("/data")), false);

    assert_eq!(result, Err(Refusal::SameFolder));
}

#[test]
fn a_drop_inside_the_same_panel_is_refused_because_core_only_moves_between_panels() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "a", false)]);

    let result = decide(&dragged, Location::Local, Some(&local_target("/data/sub")), false);

    assert_eq!(result, Err(Refusal::SamePanel));
}

#[test]
fn dropping_on_the_right_panel_without_a_connection_is_refused_with_connect_first() {
    let dragged = source(PanelSide::Local, vec![entry("/data", "a", false)]);

    let result = decide(&dragged, Location::Local, None, false);

    assert_eq!(result, Err(Refusal::NoConnection));
    assert!(Refusal::NoConnection.message().contains("Connect"));
}

#[test]
fn nothing_dragged_is_refused() {
    let dragged = source(PanelSide::Local, Vec::new());

    assert_eq!(decide(&dragged, Location::Local, Some(&remote_target("/srv")), false), Err(Refusal::NothingDragged));
}

#[test]
fn every_refusal_has_a_sentence() {
    for refusal in [
        Refusal::ToItself,
        Refusal::ToOwnChild,
        Refusal::SameFolder,
        Refusal::SamePanel,
        Refusal::NoConnection,
        Refusal::NothingDragged,
        Refusal::DesktopToLocal,
    ] {
        assert!(refusal.message().len() > 10, "{refusal:?}");
    }
}

#[test]
fn desktop_files_dropped_on_the_remote_panel_are_copied_into_its_folder() {
    let entries = vec![entry("/home/me", "a", false), entry("/home/me", "b", true)];

    let command = decide_drop_in(&entries, Some(&remote_target("/srv/in")));

    assert_eq!(
        command,
        Ok(Command::Copy {
            from: Location::Local,
            entries,
            to: Location::Session(1),
            dest_dir: VfsPath::new("/srv/in")
        })
    );
}

#[test]
fn desktop_files_dropped_on_the_local_panel_are_refused() {
    let entries = vec![entry("/home/me", "a", false)];

    assert_eq!(decide_drop_in(&entries, Some(&local_target("/data"))), Err(Refusal::DesktopToLocal));
    assert!(Refusal::DesktopToLocal.message().contains("right panel"));
}

#[test]
fn desktop_files_without_a_connection_ask_to_connect_first() {
    let entries = vec![entry("/home/me", "a", false)];

    assert_eq!(decide_drop_in(&entries, None), Err(Refusal::NoConnection));
}

#[test]
fn an_empty_desktop_drop_is_refused() {
    assert_eq!(decide_drop_in(&[], Some(&remote_target("/srv"))), Err(Refusal::NothingDragged));
}

fn dragging(side: PanelSide) -> Drag {
    let mut drag = Drag::default();
    drag.press(Some(source(side, vec![entry("/data", "a", false), entry("/data", "b", false)])));
    drag.moved(at(10.0, 10.0));
    drag.moved(at(60.0, 10.0));
    drag
}

#[test]
fn leaving_the_window_during_an_internal_drag_of_local_files_hands_off_to_the_native_drag() {
    let mut drag = dragging(PanelSide::Local);

    let handed = drag.leave().expect("a local drag hands off");

    assert_eq!(handed.entries.len(), 2);
    assert_eq!(drag.phase(), Phase::Native);
    assert!(drag.source().is_none());
    assert!(drag.pointer().is_none());
    assert!(drag.is_tracking());
}

#[test]
fn leaving_with_the_parent_row_or_nothing_dragged_does_nothing() {
    let mut idle = Drag::default();
    assert!(idle.leave().is_none());
    assert_eq!(idle.phase(), Phase::Idle);

    let mut parent_row = Drag::default();
    parent_row.press(None);
    parent_row.moved(at(10.0, 10.0));
    parent_row.moved(at(60.0, 10.0));
    assert!(parent_row.leave().is_none());
    assert_eq!(parent_row.phase(), Phase::Pressed);

    let mut below_threshold = pressed();
    below_threshold.moved(at(10.0, 10.0));
    assert!(below_threshold.leave().is_none());
    assert_eq!(below_threshold.phase(), Phase::Pressed);
}

#[test]
fn leaving_with_remote_entries_hands_off_to_staging() {
    let mut drag = dragging(PanelSide::Remote);

    let handed = drag.leave().expect("a remote drag hands off to staging");

    assert_eq!(handed.side, PanelSide::Remote);
    assert_eq!(drag.phase(), Phase::Staging);
    assert!(drag.source().is_none());
    assert!(drag.is_tracking());
}

#[test]
fn staging_finishing_moves_to_the_native_phase_once() {
    let mut drag = dragging(PanelSide::Remote);
    drag.leave();

    assert!(drag.staged());
    assert_eq!(drag.phase(), Phase::Native);
    assert!(!drag.staged());
}

#[test]
fn staged_outside_staging_does_nothing() {
    let mut idle = Drag::default();
    assert!(!idle.staged());
    let mut internal = dragging(PanelSide::Remote);
    assert!(!internal.staged());
    assert_eq!(internal.phase(), Phase::Internal);
}

#[test]
fn releasing_during_staging_reports_it_was_abandoned() {
    let mut drag = dragging(PanelSide::Remote);
    drag.leave();

    assert_eq!(drag.release(), Release::Abandoned);
    assert_eq!(drag.phase(), Phase::Idle);
    assert!(!drag.is_tracking());
}

#[test]
fn cancelling_during_staging_swallows_the_later_release() {
    let mut drag = dragging(PanelSide::Remote);
    drag.leave();

    drag.cancel();

    assert!(drag.is_tracking());
    assert_eq!(drag.phase(), Phase::Idle);
    assert_eq!(drag.release(), Release::Nothing);
    assert!(!drag.is_tracking());
}

#[test]
fn abandon_returns_a_staging_or_native_drag_to_idle() {
    let mut staging = dragging(PanelSide::Remote);
    staging.leave();
    staging.abandon();
    assert!(!staging.is_tracking());

    let mut native = dragging(PanelSide::Local);
    native.leave();
    native.abandon();
    assert!(!native.is_tracking());

    let mut internal = dragging(PanelSide::Local);
    internal.abandon();
    assert_eq!(internal.phase(), Phase::Internal);
}

#[test]
fn native_events_never_end_a_staging_drag() {
    let mut drag = dragging(PanelSide::Remote);
    drag.leave();

    assert!(!drag.native_event(NativeEvent::Cancelled));
    assert_eq!(drag.phase(), Phase::Staging);
}

#[test]
fn leaving_twice_hands_off_once() {
    let mut drag = dragging(PanelSide::Local);

    assert!(drag.leave().is_some());
    assert!(drag.leave().is_none());
}

#[test]
fn native_finished_then_cancelled_ends_the_drag_once() {
    let mut drag = dragging(PanelSide::Local);
    drag.leave();

    assert!(!drag.native_event(NativeEvent::Started));
    assert_eq!(drag.phase(), Phase::Native);
    assert!(drag.native_event(NativeEvent::Finished));
    assert_eq!(drag.phase(), Phase::Idle);
    assert!(!drag.native_event(NativeEvent::Cancelled));
}

#[test]
fn a_native_cancel_ends_the_drag() {
    let mut drag = dragging(PanelSide::Local);
    drag.leave();

    assert!(drag.native_event(NativeEvent::Cancelled));
    assert_eq!(drag.phase(), Phase::Idle);
}

#[test]
fn native_events_after_idle_are_ignored() {
    let mut drag = Drag::default();

    for event in [NativeEvent::Started, NativeEvent::Finished, NativeEvent::Cancelled] {
        assert!(!drag.native_event(event));
        assert_eq!(drag.phase(), Phase::Idle);
    }
}

#[test]
fn native_events_never_end_an_internal_drag() {
    let mut drag = dragging(PanelSide::Local);

    assert!(!drag.native_event(NativeEvent::Finished));
    assert_eq!(drag.phase(), Phase::Internal);
}

#[test]
fn the_button_release_does_not_end_the_native_drag() {
    let mut drag = dragging(PanelSide::Local);
    drag.leave();

    assert_eq!(drag.release(), Release::Nothing);
    assert_eq!(drag.phase(), Phase::Native);
}

#[test]
fn cancelling_during_the_native_drag_waits_for_the_native_answer() {
    let mut drag = dragging(PanelSide::Local);
    drag.leave();

    drag.cancel();

    assert_eq!(drag.phase(), Phase::Native);
}

#[test]
fn phases_follow_press_threshold_and_release() {
    let mut drag = Drag::default();
    assert_eq!(drag.phase(), Phase::Idle);
    drag.press(Some(source(PanelSide::Local, vec![entry("/data", "a", false)])));
    assert_eq!(drag.phase(), Phase::Pressed);
    drag.moved(at(10.0, 10.0));
    drag.moved(at(60.0, 10.0));
    assert_eq!(drag.phase(), Phase::Internal);
    drag.release();
    assert_eq!(drag.phase(), Phase::Idle);
}
