use iced::{Point, Size, window};

use super::*;

fn position_of(settings: &window::Settings) -> Option<Point> {
    match settings.position {
        window::Position::Specific(point) => Some(point),
        _ => None,
    }
}

fn registry() -> (Windows, window::Id) {
    let main = window::Id::unique();
    (Windows::new(main, Size::new(1100.0, 700.0)), main)
}

fn open_new(windows: &mut Windows, kind: WindowKind) -> (window::Id, window::Settings) {
    let mut given = None;
    let opened = windows.open(kind, |settings| {
        given = Some(settings);
        window::Id::unique()
    });
    match opened {
        Opened::Created => {
            let id = windows.id_of(kind).expect("a created window is registered");
            (id, given.expect("the settings were handed to the creator"))
        }
        Opened::Existing => panic!("expected a new window, got an existing one"),
    }
}

fn open_again(windows: &mut Windows, kind: WindowKind) -> Opened {
    windows.open(kind, |_| panic!("no second window of a kind may be created"))
}

#[test]
fn opening_a_kind_creates_one_window_and_registers_it() {
    let (mut windows, _) = registry();

    let (id, settings) = open_new(&mut windows, WindowKind::Settings);

    assert_eq!(windows.kind_of(id), Some(WindowKind::Settings));
    assert_eq!(windows.id_of(WindowKind::Settings), Some(id));
    assert_eq!(windows.size_of(id), Some(settings.size));
}

#[test]
fn opening_the_same_kind_again_returns_the_existing_id() {
    let (mut windows, _) = registry();
    let (id, _) = open_new(&mut windows, WindowKind::Connections);

    assert!(matches!(open_again(&mut windows, WindowKind::Connections), Opened::Existing));
    assert_eq!(windows.id_of(WindowKind::Connections), Some(id));
}

#[test]
fn settings_and_connections_get_different_ids() {
    let (mut windows, main) = registry();

    let (settings, _) = open_new(&mut windows, WindowKind::Settings);
    let (connections, _) = open_new(&mut windows, WindowKind::Connections);

    assert_ne!(settings, connections);
    assert_ne!(settings, main);
    assert_ne!(connections, main);
}

#[test]
fn main_is_never_created_or_closed() {
    let (mut windows, main) = registry();

    assert!(matches!(open_again(&mut windows, WindowKind::Main), Opened::Existing));
    assert_eq!(windows.id_of(WindowKind::Main), Some(main));
    assert_eq!(windows.close(main), None);
    assert_eq!(windows.kind_of(main), Some(WindowKind::Main));
    assert_eq!(windows.main(), main);
}

#[test]
fn closing_forgets_the_window_and_a_later_open_creates_a_new_id() {
    let (mut windows, _) = registry();
    let (first, _) = open_new(&mut windows, WindowKind::Settings);

    assert_eq!(windows.close(first), Some(WindowKind::Settings));
    assert_eq!(windows.kind_of(first), None);
    assert_eq!(windows.id_of(WindowKind::Settings), None);
    assert_eq!(windows.size_of(first), None);
    let (second, _) = open_new(&mut windows, WindowKind::Settings);
    assert_ne!(first, second);
}

#[test]
fn closing_an_unknown_id_returns_none() {
    let (mut windows, _) = registry();

    assert_eq!(windows.close(window::Id::unique()), None);
}

#[test]
fn sizes_are_kept_per_window() {
    let (mut windows, main) = registry();
    let (settings, _) = open_new(&mut windows, WindowKind::Settings);

    windows.set_size(settings, Size::new(900.0, 640.0));

    assert_eq!(windows.size_of(settings), Some(Size::new(900.0, 640.0)));
    assert_eq!(windows.size_of(main), Some(Size::new(1100.0, 700.0)));
}

#[test]
fn set_size_for_an_unknown_id_is_ignored() {
    let (mut windows, main) = registry();

    windows.set_size(window::Id::unique(), Size::new(10.0, 10.0));

    assert_eq!(windows.size_of(main), Some(Size::new(1100.0, 700.0)));
}

#[test]
fn secondary_windows_do_not_exit_on_close_request() {
    for kind in [WindowKind::Settings, WindowKind::Connections] {
        assert!(!settings_for(kind).exit_on_close_request, "{kind:?}");
    }
}

#[test]
fn the_secondary_windows_have_their_own_sizes_and_a_minimum() {
    let settings = settings_for(WindowKind::Settings);
    let connections = settings_for(WindowKind::Connections);

    assert_eq!(settings.size, SETTINGS_WINDOW_SIZE);
    assert_eq!(connections.size, CONNECTIONS_WINDOW_SIZE);
    assert!(settings.min_size.is_some() && connections.min_size.is_some());
}

#[cfg(target_os = "linux")]
#[test]
fn every_window_carries_the_application_id() {
    for kind in [WindowKind::Settings, WindowKind::Connections] {
        assert_eq!(settings_for(kind).platform_specific.application_id, "porthmos-gui");
    }
}

#[test]
fn the_search_window_is_a_third_kind_with_its_own_size_and_title() {
    let (mut windows, _) = registry();

    let (id, settings) = open_new(&mut windows, WindowKind::Search);

    assert_eq!(windows.kind_of(id), Some(WindowKind::Search));
    assert!(matches!(open_again(&mut windows, WindowKind::Search), Opened::Existing));
    assert_eq!(settings.size, SEARCH_WINDOW_SIZE);
    assert!(settings.min_size.is_some());
    assert!(!settings.exit_on_close_request);
    assert_eq!(WindowKind::Search.title(), "Porthmos Search");
}

#[test]
fn a_closed_window_reopens_with_the_size_and_position_it_last_had() {
    let (mut windows, _) = registry();
    let (id, _) = open_new(&mut windows, WindowKind::Search);
    windows.set_size(id, Size::new(900.0, 650.0));
    windows.set_position(id, Point::new(30.0, 70.0));
    windows.close(id);

    let (_, settings) = open_new(&mut windows, WindowKind::Search);

    assert_eq!(settings.size, Size::new(900.0, 650.0));
    assert_eq!(position_of(&settings), Some(Point::new(30.0, 70.0)));
}

#[test]
fn a_window_that_was_never_moved_opens_at_the_default_position() {
    let (mut windows, _) = registry();
    let (id, _) = open_new(&mut windows, WindowKind::Settings);
    windows.set_size(id, Size::new(900.0, 650.0));
    windows.close(id);

    let (_, settings) = open_new(&mut windows, WindowKind::Settings);

    assert_eq!(position_of(&settings), None);
}

#[test]
fn the_main_window_geometry_is_remembered_without_being_reopened() {
    let (mut windows, main) = registry();
    windows.set_size(main, Size::new(1000.0, 640.0));
    windows.set_position(main, Point::new(5.0, 6.0));

    let settings = windows.memory().applied_to(WindowKind::Main, main_window::settings());

    assert_eq!(settings.size, Size::new(1000.0, 640.0));
    assert_eq!(position_of(&settings), Some(Point::new(5.0, 6.0)));
}

#[test]
fn moving_an_unknown_window_is_ignored() {
    let (mut windows, _) = registry();

    windows.set_position(window::Id::unique(), Point::new(1.0, 2.0));

    assert_eq!(windows.memory(), &WindowMemory::default());
}

#[test]
fn remembered_geometry_given_to_the_registry_shapes_the_windows_it_opens() {
    let (mut windows, _) = registry();
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Connections, Size::new(950.0, 600.0));
    memory.moved(WindowKind::Connections, Point::new(11.0, 12.0));
    windows.set_memory(memory);

    let (_, settings) = open_new(&mut windows, WindowKind::Connections);

    assert_eq!(settings.size, Size::new(950.0, 600.0));
    assert_eq!(position_of(&settings), Some(Point::new(11.0, 12.0)));
}
