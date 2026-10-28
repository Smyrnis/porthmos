use std::io;

use porthmos_vfs::ProtocolError;

#[cfg(any(windows, test))]
const SHARING_VIOLATION: i32 = 32;
#[cfg(any(windows, test))]
const LOCK_VIOLATION: i32 = 33;
#[cfg(any(windows, test))]
const IN_USE: &str = "in use by another program; close it there and try again";

#[cfg(any(windows, test))]
pub(crate) fn in_use_message(os_error: Option<i32>) -> Option<&'static str> {
    matches!(os_error, Some(SHARING_VIOLATION | LOCK_VIOLATION)).then_some(IN_USE)
}

#[cfg(unix)]
pub(crate) fn explain(error: io::Error) -> ProtocolError {
    ProtocolError::from(error)
}

#[cfg(windows)]
pub(crate) fn explain(error: io::Error) -> ProtocolError {
    match in_use_message(error.raw_os_error()) {
        Some(message) => {
            tracing::debug!(%error, "the file is held open by another program");
            ProtocolError::new(porthmos_vfs::ErrorKind::Other, anyhow::anyhow!(message))
        }
        None => ProtocolError::from(error),
    }
}

#[cfg(test)]
mod tests;
