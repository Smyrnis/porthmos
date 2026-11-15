use std::path::PathBuf;

use porthmos_core::{ProtocolInfo, i18n};
use porthmos_vfs::{OptionKind, Protocol};

fn all_labels(info: &ProtocolInfo) -> Vec<String> {
    let form = &info.form;
    let mut labels: Vec<String> = [form.host.label, form.port.label, form.username.label, form.password.label]
        .iter()
        .map(|label| label.to_string())
        .collect();
    for field in &form.options {
        labels.push(field.label.to_string());
        if let OptionKind::Choice { choices, .. } = &field.kind {
            labels.extend(choices.iter().map(|choice| choice.label.to_string()));
        }
    }
    labels
}

#[test]
fn every_label_a_protocol_form_shows_is_in_the_language_file() {
    let certificates = PathBuf::new();
    let protocols: Vec<Box<dyn Protocol>> = vec![
        Box::new(porthmos_sftp::Sftp),
        Box::new(porthmos_scp::Scp::default()),
        Box::new(porthmos_ftp::Ftp::new(certificates.clone())),
        Box::new(porthmos_webdav::WebDav::new(certificates.clone())),
        Box::new(porthmos_s3::S3::new(certificates)),
    ];
    let mut missing = Vec::new();
    for protocol in &protocols {
        let info = ProtocolInfo::from_protocol(protocol.as_ref());
        for label in all_labels(&info) {
            let key = i18n::label_key(&label);
            if i18n::text(&key) == key {
                missing.push(format!("{}: \"{label}\" needs {key}", info.id));
            }
        }
        assert_ne!(info.description_text(), "", "{}", info.id);
    }
    assert!(missing.is_empty(), "{missing:#?}");
}
