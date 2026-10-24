#[cfg(feature = "update")]
pub mod backend;
#[allow(dead_code)]
pub(crate) mod record;

#[cfg(feature = "update")]
pub use backend::{ReleaseUpdater, Updater};
#[cfg(feature = "update")]
pub use porthmos_update::{CheckError, Outcome, StageError, Staged, UpdateInfo, Version};
