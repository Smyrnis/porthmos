use iced::Size;

use super::*;

#[test]
fn the_window_has_the_size_and_close_behaviour_the_app_relies_on() {
    let settings = settings();

    assert_eq!(settings.size, INITIAL_SIZE);
    assert_eq!(INITIAL_SIZE, Size::new(1100.0, 700.0));
    assert_eq!(settings.min_size, Some(Size::new(480.0, 320.0)));
    assert!(!settings.exit_on_close_request);
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_is_named_after_the_launcher_so_the_dock_can_match_them() {
    assert_eq!(settings().platform_specific.application_id, "porthmos-gui");
}
