use porthmos_vfs::testing::{FakeFs, FakeProtocol};

use super::*;

#[test]
fn protocol_info_copies_the_protocols_id_name_and_form() {
    let mut form = ConnectionForm::standard(21);
    form.host.label = "Server";
    let protocol = FakeProtocol::new(FakeFs::new()).with_id("ftp").with_form(form.clone());

    let info = ProtocolInfo::from_protocol(&protocol);

    assert_eq!((info.id, info.form), ("ftp", form));
    assert_eq!(info.display_name, protocol.display_name());
}

#[test]
fn protocol_info_carries_the_description_and_limits_note() {
    let protocol =
        FakeProtocol::new(FakeFs::new()).with_description("legacy hosting, tls").with_limits_note("no symlinks");

    let info = ProtocolInfo::from_protocol(&protocol);

    assert_eq!((info.description, info.limits_note), ("legacy hosting, tls", Some("no symlinks")));
}

#[test]
fn the_defaults_are_an_empty_description_and_no_limits_note() {
    let info = ProtocolInfo::from_protocol(&FakeProtocol::new(FakeFs::new()));

    assert_eq!((info.description, info.limits_note), ("", None));
}
