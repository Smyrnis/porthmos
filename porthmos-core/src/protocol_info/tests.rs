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

fn info(id: &'static str, description: &'static str, limits_note: Option<&'static str>) -> ProtocolInfo {
    ProtocolInfo { id, display_name: "Name", description, limits_note, form: porthmos_vfs::ConnectionForm::standard(1) }
}

#[test]
fn the_shown_description_is_the_one_the_protocol_gave_when_the_file_has_none() {
    assert_eq!(info("unlisted", "plain words", None).description_text(), "plain words");
}

#[test]
fn the_shown_note_is_the_one_the_protocol_gave_when_the_file_has_none() {
    assert_eq!(info("unlisted", "", Some("a limit")).limits_note_text(), Some("a limit".to_string()));
    assert_eq!(info("unlisted", "", None).limits_note_text(), None);
}

#[test]
fn every_built_in_protocol_has_its_description_in_the_language_file() {
    for (id, description) in [
        ("sftp", "ssh file transfer"),
        ("scp", "when sftp is disabled"),
        ("ftp", "legacy hosting, tls"),
        ("webdav", "nextcloud, nas shares"),
        ("s3", "aws, r2, minio, b2"),
    ] {
        assert_eq!(info(id, "", None).description_text(), description, "{id}");
    }
}

#[test]
fn the_object_store_note_is_in_the_language_file() {
    let note = "no permissions or symlinks \u{b7} rename is copy + delete";

    assert_eq!(info("s3", "", Some("other")).limits_note_text(), Some(note.to_string()));
}
