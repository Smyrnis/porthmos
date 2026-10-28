use std::{path::PathBuf, sync::atomic::AtomicBool};

use porthmos_update::{CheckError, Checker, Outcome, StageError, Staged, Stager, UpdateDir, UpdateInfo, Version};
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
    pub fn new(update_dir: PathBuf, user_agent: &str) -> anyhow::Result<ReleaseUpdater> {
        Ok(ReleaseUpdater {
            checker: Checker::release(user_agent)?,
            stager: Stager::new(UpdateDir::new(update_dir), user_agent)?,
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
