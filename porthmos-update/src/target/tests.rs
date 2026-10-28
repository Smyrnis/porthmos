use super::*;

#[test]
fn the_target_names_this_machines_architecture() {
    assert!(running_target().starts_with(std::env::consts::ARCH), "{}", running_target());
}

#[test]
fn the_target_names_this_machines_operating_system() {
    let expected = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else {
        "windows"
    };

    assert!(running_target().contains(expected), "{}", running_target());
}
