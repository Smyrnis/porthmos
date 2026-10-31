use super::{NO_DISPLAY_MESSAGE, missing_in};

fn lookup<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| set.iter().find(|(key, _)| *key == name).map(|(_, value)| value.to_string())
}

#[test]
fn a_display_is_missing_when_neither_variable_is_set() {
    assert!(missing_in(lookup(&[])));
}

#[test]
fn empty_variables_count_as_unset() {
    assert!(missing_in(lookup(&[("DISPLAY", ""), ("WAYLAND_DISPLAY", "")])));
}

#[test]
fn either_x11_or_wayland_is_enough() {
    assert!(!missing_in(lookup(&[("DISPLAY", ":0")])));
    assert!(!missing_in(lookup(&[("WAYLAND_DISPLAY", "wayland-0")])));
}

#[test]
fn the_message_names_both_variables_and_the_terminal_version() {
    assert!(NO_DISPLAY_MESSAGE.contains("DISPLAY"));
    assert!(NO_DISPLAY_MESSAGE.contains("WAYLAND_DISPLAY"));
    assert!(NO_DISPLAY_MESSAGE.contains("porthmos for the terminal"));
}
