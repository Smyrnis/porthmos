use std::{collections::BTreeMap, sync::Arc};

use porthmos_vfs::{
    Choice, ConnectionForm, FileSystem, OptionField, OptionKind, Prompter, Protocol, ProtocolError, Target,
};

use super::Candidate;

const SECURITY_CHOICES: &[Choice] = &[
    Choice { value: "plain", label: "Plain" },
    Choice { value: "explicit", label: "Explicit" },
    Choice { value: "implicit", label: "Implicit" },
];

pub(crate) struct TestProtocol {
    id: &'static str,
    form: ConnectionForm,
}

#[porthmos_vfs::async_trait]
impl Protocol for TestProtocol {
    fn id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        "TEST"
    }

    fn default_port(&self) -> u16 {
        self.form.port.default
    }

    fn connection_form(&self) -> ConnectionForm {
        self.form.clone()
    }

    async fn connect(
        &self, _target: &Target, _prompter: &mut dyn Prompter,
    ) -> Result<Arc<dyn FileSystem>, ProtocolError> {
        Err(ProtocolError::new(porthmos_vfs::ErrorKind::Unsupported, anyhow::anyhow!("test protocol")))
    }
}

pub(crate) fn sftp_like() -> Arc<dyn Protocol> {
    let mut form = ConnectionForm::standard(22);
    form.options = vec![OptionField {
        key: "identity_file",
        label: "Identity file",
        required: false,
        kind: OptionKind::Text { default: "" },
    }];
    Arc::new(TestProtocol { id: "sftp", form })
}

pub(crate) fn ftp_like() -> Arc<dyn Protocol> {
    let mut form = ConnectionForm::standard(21);
    form.username.required = false;
    form.options = vec![
        OptionField {
            key: "security",
            label: "Security",
            required: false,
            kind: OptionKind::Choice { choices: SECURITY_CHOICES, default: "explicit" },
        },
        OptionField {
            key: "passive",
            label: "Passive mode",
            required: false,
            kind: OptionKind::Toggle { default: true },
        },
    ];
    Arc::new(TestProtocol { id: "ftp", form })
}

pub(crate) fn candidate(name: &str, protocol: &str) -> Candidate {
    Candidate {
        name: name.to_string(),
        protocol: protocol.to_string(),
        host: "example.com".to_string(),
        port: None,
        username: "user".to_string(),
        group: None,
        options: BTreeMap::new(),
        notes: Vec::new(),
    }
}
