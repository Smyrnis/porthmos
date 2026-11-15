use iced::{Point, Size, window};

use super::*;

fn defaults() -> window::Settings {
    window::Settings { size: Size::new(1100.0, 700.0), ..window::Settings::default() }
}

fn specific(settings: &window::Settings) -> Option<Point> {
    match settings.position {
        window::Position::Specific(point) => Some(point),
        _ => None,
    }
}

#[test]
fn nothing_remembered_leaves_the_settings_alone() {
    let memory = WindowMemory::default();

    let settings = memory.applied_to(WindowKind::Main, defaults());

    assert_eq!(settings.size, Size::new(1100.0, 700.0));
    assert!(matches!(settings.position, window::Position::Default));
}

#[test]
fn a_remembered_size_and_position_replace_the_defaults() {
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Main, Size::new(900.0, 640.0));
    memory.moved(WindowKind::Main, Point::new(120.0, 80.0));

    let settings = memory.applied_to(WindowKind::Main, defaults());

    assert_eq!(settings.size, Size::new(900.0, 640.0));
    assert_eq!(specific(&settings), Some(Point::new(120.0, 80.0)));
}

#[test]
fn a_size_without_a_position_keeps_the_default_position() {
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Search, Size::new(800.0, 500.0));

    let settings = memory.applied_to(WindowKind::Search, defaults());

    assert_eq!(settings.size, Size::new(800.0, 500.0));
    assert!(matches!(settings.position, window::Position::Default));
}

#[test]
fn a_position_without_a_size_keeps_the_default_size() {
    let mut memory = WindowMemory::default();
    memory.moved(WindowKind::Settings, Point::new(40.0, 60.0));

    let settings = memory.applied_to(WindowKind::Settings, defaults());

    assert_eq!(settings.size, Size::new(1100.0, 700.0));
    assert_eq!(specific(&settings), Some(Point::new(40.0, 60.0)));
}

#[test]
fn each_window_kind_is_remembered_on_its_own() {
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Main, Size::new(900.0, 600.0));
    memory.resized(WindowKind::Settings, Size::new(500.0, 400.0));
    memory.resized(WindowKind::Connections, Size::new(700.0, 450.0));
    memory.resized(WindowKind::Search, Size::new(640.0, 480.0));

    for (kind, expected) in [
        (WindowKind::Main, Size::new(900.0, 600.0)),
        (WindowKind::Settings, Size::new(500.0, 400.0)),
        (WindowKind::Connections, Size::new(700.0, 450.0)),
        (WindowKind::Search, Size::new(640.0, 480.0)),
    ] {
        assert_eq!(memory.applied_to(kind, defaults()).size, expected);
    }
}

#[test]
fn a_minimized_window_does_not_overwrite_what_was_remembered() {
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Main, Size::new(900.0, 600.0));
    memory.moved(WindowKind::Main, Point::new(10.0, 20.0));

    memory.resized(WindowKind::Main, Size::new(0.0, 0.0));
    memory.moved(WindowKind::Main, Point::new(-32000.0, -32000.0));

    let settings = memory.applied_to(WindowKind::Main, defaults());
    assert_eq!(settings.size, Size::new(900.0, 600.0));
    assert_eq!(specific(&settings), Some(Point::new(10.0, 20.0)));
}

#[test]
fn sizes_and_positions_that_are_not_plausible_are_never_recorded() {
    let mut memory = WindowMemory::default();
    for size in [
        Size::new(f32::NAN, 600.0),
        Size::new(900.0, f32::INFINITY),
        Size::new(99.0, 600.0),
        Size::new(900.0, 99.0),
        Size::new(20000.0, 600.0),
        Size::new(900.0, 20000.0),
    ] {
        memory.resized(WindowKind::Main, size);
    }
    for point in [
        Point::new(f32::NAN, 0.0),
        Point::new(0.0, f32::NEG_INFINITY),
        Point::new(-20000.0, 0.0),
        Point::new(0.0, 20000.0),
    ] {
        memory.moved(WindowKind::Main, point);
    }

    assert_eq!(memory, WindowMemory::default());
}

#[test]
fn the_edge_values_of_the_plausible_range_are_accepted() {
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Main, Size::new(100.0, 16384.0));
    memory.moved(WindowKind::Main, Point::new(-16384.0, 16384.0));

    let settings = memory.applied_to(WindowKind::Main, defaults());

    assert_eq!(settings.size, Size::new(100.0, 16384.0));
    assert_eq!(specific(&settings), Some(Point::new(-16384.0, 16384.0)));
}

#[test]
fn a_remembered_position_may_be_negative_for_a_monitor_left_of_the_first() {
    let mut memory = WindowMemory::default();
    memory.moved(WindowKind::Main, Point::new(-1800.0, 40.0));

    assert_eq!(specific(&memory.applied_to(WindowKind::Main, defaults())), Some(Point::new(-1800.0, 40.0)));
}

#[test]
fn a_remembered_size_never_goes_below_the_windows_own_minimum() {
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Main, Size::new(200.0, 150.0));
    let base = window::Settings { min_size: Some(Size::new(480.0, 320.0)), ..defaults() };

    let settings = memory.applied_to(WindowKind::Main, base);

    assert_eq!(settings.size, Size::new(480.0, 320.0));
}

#[test]
fn saving_then_loading_gives_back_the_same_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut memory = WindowMemory::default();
    memory.resized(WindowKind::Main, Size::new(900.5, 640.0));
    memory.moved(WindowKind::Main, Point::new(-120.0, 80.0));
    memory.resized(WindowKind::Search, Size::new(640.0, 480.0));

    memory.save(&file_in(dir.path())).unwrap();
    let loaded = WindowMemory::load(&file_in(dir.path()));

    assert_eq!(loaded.memory, memory);
    assert_eq!(loaded.warning, None);
}

#[test]
fn loading_a_missing_file_gives_an_empty_memory_without_a_warning() {
    let dir = tempfile::tempdir().unwrap();

    let loaded = WindowMemory::load(&file_in(dir.path()));

    assert_eq!(loaded.memory, WindowMemory::default());
    assert_eq!(loaded.warning, None);
}

#[test]
fn a_broken_file_is_set_aside_with_a_warning_and_gives_an_empty_memory() {
    let dir = tempfile::tempdir().unwrap();
    let file = file_in(dir.path());
    std::fs::write(&file, "main = [ not toml").unwrap();

    let loaded = WindowMemory::load(&file);

    assert_eq!(loaded.memory, WindowMemory::default());
    assert!(loaded.warning.is_some());
    assert!(!file.exists());
    assert!(dir.path().join("gui-windows.toml.broken").exists());
}

#[test]
fn values_that_are_not_plausible_in_the_file_are_dropped_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let file = file_in(dir.path());
    std::fs::write(
        &file,
        "[main]\nwidth = 0.0\nheight = 0.0\nx = -32000.0\ny = -32000.0\n\n[search]\nwidth = 640.0\nheight = 480.0\n",
    )
    .unwrap();

    let loaded = WindowMemory::load(&file);

    assert_eq!(loaded.memory.applied_to(WindowKind::Main, defaults()).size, Size::new(1100.0, 700.0));
    assert!(matches!(loaded.memory.applied_to(WindowKind::Main, defaults()).position, window::Position::Default));
    assert_eq!(loaded.memory.applied_to(WindowKind::Search, defaults()).size, Size::new(640.0, 480.0));
}

#[test]
fn unknown_fields_in_the_file_do_not_break_loading() {
    let dir = tempfile::tempdir().unwrap();
    let file = file_in(dir.path());
    std::fs::write(&file, "future = 1\n\n[main]\nwidth = 900.0\nheight = 600.0\nmaximized = true\n").unwrap();

    let loaded = WindowMemory::load(&file);

    assert_eq!(loaded.warning, None);
    assert_eq!(loaded.memory.applied_to(WindowKind::Main, defaults()).size, Size::new(900.0, 600.0));
}

#[test]
fn the_file_lives_in_the_state_directory_under_its_own_name() {
    assert_eq!(file_in(std::path::Path::new("/state")), std::path::PathBuf::from("/state/gui-windows.toml"));
}

#[cfg(unix)]
#[test]
fn the_saved_file_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = file_in(dir.path());

    WindowMemory::default().save(&file).unwrap();

    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}
