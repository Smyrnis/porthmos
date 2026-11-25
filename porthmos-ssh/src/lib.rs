pub mod askpass;
pub mod client;
mod connect;
mod discovery;
mod exec;
mod jump;
mod proxy;
mod quote;
pub mod shell;
mod shell_channel;
pub mod ssh_config;
#[cfg(feature = "testing")]
pub mod testing;
mod version;

pub use connect::{ConnectOptions, Session, connect};
pub use discovery::{DEFAULT_PORT, discover, identity_path, shell_command};
pub use exec::{ExecChannel, ExecInput, Output, exec, open_exec};
pub use quote::shell_quote;
pub use shell_channel::{ShellChannel, ShellInput, ShellSize, open_shell, open_shell_stream};

pub const PROXY_FIELD: porthmos_vfs::OptionField = porthmos_vfs::OptionField {
    key: "proxy",
    label: "Proxy",
    required: false,
    kind: porthmos_vfs::OptionKind::Text { default: "" },
};

pub const PROXY_USER_FIELD: porthmos_vfs::OptionField = porthmos_vfs::OptionField {
    key: "proxy_user",
    label: "Proxy user",
    required: false,
    kind: porthmos_vfs::OptionKind::Text { default: "" },
};

pub const PROXY_PASSWORD_FIELD: porthmos_vfs::OptionField = porthmos_vfs::OptionField {
    key: "proxy_password",
    label: "Proxy password",
    required: false,
    kind: porthmos_vfs::OptionKind::Secret,
};

pub const PROXY_COMMAND_FIELD: porthmos_vfs::OptionField = porthmos_vfs::OptionField {
    key: "proxy_command",
    label: "Proxy command",
    required: false,
    kind: porthmos_vfs::OptionKind::Text { default: "" },
};

pub const PROXY_JUMP_FIELD: porthmos_vfs::OptionField = porthmos_vfs::OptionField {
    key: "proxy_jump",
    label: "Jump host",
    required: false,
    kind: porthmos_vfs::OptionKind::Text { default: "" },
};
