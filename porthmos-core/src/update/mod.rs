#[cfg(feature = "update")]
pub mod backend;
#[cfg_attr(not(feature = "update"), allow(dead_code))]
pub(crate) mod record;

mod apply;

pub use apply::{ApplyOutcome, apply_staged_update};

#[cfg(feature = "update")]
pub use backend::{ReleaseUpdater, Updater};
#[cfg(feature = "update")]
pub use porthmos_update::{CheckError, Outcome, StageError, Staged, UpdateInfo, Version};

use std::time::Duration;

use crate::config::UpdateSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Available { version: String },
    Downloading { version: String, downloaded: u64, total: u64 },
    Ready { version: String },
    Managed { version: String, reason: String },
}

pub(crate) const STARTUP_DELAY: Duration = Duration::from_secs(5);
pub(crate) const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UpdateTiming {
    pub(crate) startup_delay: Duration,
    pub(crate) retry_after_failure: Duration,
    pub(crate) interval: Duration,
}

impl UpdateTiming {
    pub(crate) fn for_settings(settings: &UpdateSettings) -> Self {
        Self {
            startup_delay: STARTUP_DELAY,
            retry_after_failure: RETRY_AFTER_FAILURE,
            interval: Duration::from_secs(settings.interval_hours * 60 * 60),
        }
    }
}

#[cfg(feature = "update")]
pub(crate) type SharedUpdater = std::sync::Arc<dyn Updater>;
#[cfg(not(feature = "update"))]
pub(crate) type SharedUpdater = std::convert::Infallible;

#[cfg(feature = "update")]
pub(crate) fn current_version() -> Version {
    env!("CARGO_PKG_VERSION").parse().expect("the crate version is valid semver")
}
