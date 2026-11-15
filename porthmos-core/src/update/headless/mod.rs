use crate::Paths;

#[cfg(feature = "update")]
use std::{path::Path, sync::atomic::AtomicBool, time::Duration};

#[cfg(feature = "update")]
use porthmos_update::{InstallPlace, UpdateDir, find_staged, install_place};

#[cfg(feature = "update")]
use crate::{
    config::UpdateMode,
    update::{AppIdentity, ApplyOutcome, Outcome, ReleaseUpdater, UpdateInfo, Updater, apply::apply_with},
};

#[cfg(feature = "update")]
const INSTALL_BUDGET: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadlessUpdate {
    UpToDate { current: String },
    Available { version: String },
    Managed { version: String, reason: String },
    Installed { version: String },
}

#[cfg(feature = "update")]
pub async fn run_update(paths: &Paths, check_only: bool, identity: &AppIdentity) -> Result<HeadlessUpdate, String> {
    let updater = ReleaseUpdater::new(identity, paths.update_dir()).map_err(|err| format!("{err:#}"))?;
    let executable = std::env::current_exe().map_err(|err| format!("couldn't find the running program: {err}"))?;
    run_update_with(&updater, paths, &executable, std::env::home_dir().as_deref(), identity, check_only, INSTALL_BUDGET)
        .await
}

#[cfg(not(feature = "update"))]
pub async fn run_update(_paths: &Paths, _check_only: bool) -> Result<HeadlessUpdate, String> {
    Err("this build can't check for updates".to_string())
}

#[cfg(feature = "update")]
async fn run_update_with(
    updater: &dyn Updater, paths: &Paths, executable: &Path, home: Option<&Path>, identity: &AppIdentity,
    check_only: bool, budget: Duration,
) -> Result<HeadlessUpdate, String> {
    super::legacy::remove(paths);
    let current = &identity.version;
    let info = match updater.check(current).await.map_err(|error| error.to_string())? {
        Outcome::UpToDate => return Ok(HeadlessUpdate::UpToDate { current: current.to_string() }),
        Outcome::Available(info) => info,
    };
    let version = info.version.to_string();
    if check_only {
        return Ok(HeadlessUpdate::Available { version });
    }
    if let InstallPlace::Managed { reason } = install_place(executable, home) {
        return Ok(HeadlessUpdate::Managed { version, reason });
    }
    stage_unless_ready(updater, paths, &info, identity).await?;
    match apply_with(paths, UpdateMode::Auto, executable, home, identity, budget) {
        ApplyOutcome::Installed { version } => Ok(HeadlessUpdate::Installed { version }),
        ApplyOutcome::Failed { message, .. } => Err(message),
        ApplyOutcome::Nothing => Err("the update was downloaded but could not be installed".to_string()),
    }
}

#[cfg(feature = "update")]
async fn stage_unless_ready(
    updater: &dyn Updater, paths: &Paths, info: &UpdateInfo, identity: &AppIdentity,
) -> Result<(), String> {
    let dir = UpdateDir::for_product(paths.update_dir(), identity.product);
    if find_staged(&dir, &identity.version).is_some_and(|staged| staged.version == info.version) {
        return Ok(());
    }
    let cancel = AtomicBool::new(false);
    updater.stage(info, &cancel, &mut |_, _| {}).await.map(|_| ()).map_err(|error| error.to_string())
}

#[cfg(all(test, feature = "update", unix))]
mod tests;
