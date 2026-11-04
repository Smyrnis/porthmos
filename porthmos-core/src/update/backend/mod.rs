use std::{path::PathBuf, sync::atomic::AtomicBool};

use porthmos_update::{CheckError, Checker, Outcome, StageError, Staged, Stager, UpdateInfo, Version};

use super::AppIdentity;
use porthmos_vfs::async_trait;

#[async_trait]
pub trait Updater: Send + Sync {
    async fn check(&self, current: &Version) -> Result<Outcome, CheckError>;

    async fn stage(
        &self, update: &UpdateInfo, cancel: &AtomicBool, progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<Staged, StageError>;
}

pub struct ReleaseUpdater {
    checker: Checker,
    stager: Stager,
}

impl ReleaseUpdater {
    pub fn new(identity: &AppIdentity, update_root: PathBuf) -> anyhow::Result<ReleaseUpdater> {
        let user_agent = identity.user_agent();
        Ok(ReleaseUpdater {
            checker: Checker::release(identity.product, &identity.version, &user_agent)?,
            stager: Stager::new(identity.product, update_root, &user_agent)?,
        })
    }
}

#[async_trait]
impl Updater for ReleaseUpdater {
    async fn check(&self, current: &Version) -> Result<Outcome, CheckError> {
        self.checker.check(current).await
    }

    async fn stage(
        &self, update: &UpdateInfo, cancel: &AtomicBool, progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<Staged, StageError> {
        self.stager.stage(update, cancel, progress).await
    }
}

#[cfg(test)]
mod tests;
