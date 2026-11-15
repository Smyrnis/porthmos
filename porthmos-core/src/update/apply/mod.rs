use crate::{Paths, config::UpdateMode};

#[cfg(feature = "update")]
use std::{path::Path, time::Duration};

#[cfg(feature = "update")]
use porthmos_update::{InstallError, InstallPlace, UpdateDir, find_staged, install, install_place};

#[cfg(feature = "update")]
use crate::update::{AppIdentity, record::UpdateRecord};

#[cfg(feature = "update")]
const INSTALL_BUDGET: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    Nothing,
    Installed { version: String },
    Failed { version: String, message: String },
}

#[cfg(feature = "update")]
pub fn apply_staged_update(paths: &Paths, mode: UpdateMode, identity: &AppIdentity) -> ApplyOutcome {
    let Ok(executable) = std::env::current_exe() else { return ApplyOutcome::Nothing };
    apply_with(paths, mode, &executable, std::env::home_dir().as_deref(), identity, INSTALL_BUDGET)
}

#[cfg(not(feature = "update"))]
pub fn apply_staged_update(_paths: &Paths, _mode: UpdateMode) -> ApplyOutcome {
    ApplyOutcome::Nothing
}

#[cfg(feature = "update")]
pub(super) fn apply_with(
    paths: &Paths, mode: UpdateMode, executable: &Path, home: Option<&Path>, identity: &AppIdentity, budget: Duration,
) -> ApplyOutcome {
    if mode != UpdateMode::Auto {
        return ApplyOutcome::Nothing;
    }
    let InstallPlace::Updatable { executable } = install_place(executable, home) else {
        return ApplyOutcome::Nothing;
    };
    let dir = UpdateDir::for_product(paths.update_dir(), identity.product);
    let Some(staged) = find_staged(&dir, &identity.version) else { return ApplyOutcome::Nothing };
    let version = staged.version.to_string();
    match install(&dir, &staged, &executable, budget) {
        Ok(_) => {
            remember_failure(paths, identity, None);
            ApplyOutcome::Installed { version }
        }
        Err(InstallError::NothingStaged | InstallError::AlreadyCurrent | InstallError::Managed(_)) => {
            ApplyOutcome::Nothing
        }
        Err(error) => {
            let message = error.to_string();
            remember_failure(paths, identity, Some(format!("{version}: {message}")));
            ApplyOutcome::Failed { version, message }
        }
    }
}

#[cfg(feature = "update")]
fn remember_failure(paths: &Paths, identity: &AppIdentity, failure: Option<String>) {
    let mut record = UpdateRecord::load(paths, identity.product.id()).0;
    if !record.is_writable() || (failure.is_none() && record.last_failure().is_none()) {
        return;
    }
    record.set_last_failure(failure);
    let _ = record.save();
}

#[cfg(all(test, feature = "update", unix))]
mod tests;
