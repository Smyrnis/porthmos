use super::*;

#[test]
fn new_parses_the_version_and_names_the_product() {
    let identity = AppIdentity::new(Product::Gui, "2026.5.0-dev.1");

    assert_eq!(identity.product, Product::Gui);
    assert_eq!(identity.version, "2026.5.0-dev.1".parse::<Version>().unwrap());
}

#[test]
fn the_user_agent_is_the_program_slash_the_version() {
    assert_eq!(AppIdentity::new(Product::Gui, "2026.5.0-dev.1").user_agent(), "porthmos-gui/2026.5.0-dev.1");
    assert_eq!(AppIdentity::new(Product::Tui, "2026.5.0").user_agent(), "porthmos/2026.5.0");
}

#[test]
#[should_panic(expected = "the crate version is valid semver")]
fn a_bad_version_string_panics() {
    let _ = AppIdentity::new(Product::Tui, "not a version");
}
