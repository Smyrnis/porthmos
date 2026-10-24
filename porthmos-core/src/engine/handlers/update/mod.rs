use std::path::PathBuf;

use super::super::Engine;
use crate::{
    config::UpdateSettings,
    update::{SharedUpdater, UpdateTiming, record::UpdateRecord},
};

#[cfg(feature = "update")]
use std::time::Duration;

#[cfg(feature = "update")]
use super::super::{Event, Internal};
#[cfg(feature = "update")]
use crate::{
    Severity,
    config::UpdateMode,
    health::wait_unless_cancelled,
    tasks::Scope,
    update::{CheckError, Outcome, StageError, Staged, UpdateInfo, UpdateState, current_version},
};

#[cfg_attr(not(feature = "update"), allow(dead_code))]
pub(crate) struct UpdateEngine {
    pub(crate) settings: UpdateSettings,
    pub(crate) record: UpdateRecord,
    pub(crate) updater: Option<SharedUpdater>,
    pub(crate) timing: UpdateTiming,
    pub(crate) executable: Option<PathBuf>,
    pub(crate) busy: bool,
    #[cfg(feature = "update")]
    timer: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    #[cfg(feature = "update")]
    write_warned: bool,
    #[cfg(feature = "update")]
    pub(crate) staged: Option<Staged>,
}

impl UpdateEngine {
    pub(crate) fn new(
        settings: UpdateSettings, record: UpdateRecord, updater: Option<SharedUpdater>, timing: UpdateTiming,
        executable: Option<PathBuf>,
    ) -> Self {
        Self {
            settings,
            record,
            updater,
            timing,
            executable,
            busy: false,
            #[cfg(feature = "update")]
            timer: None,
            #[cfg(feature = "update")]
            write_warned: false,
            #[cfg(feature = "update")]
            staged: None,
        }
    }
}

#[cfg(not(feature = "update"))]
impl Engine {
    pub(crate) fn start_updates(&mut self) {}

    pub(crate) fn check_for_update(&mut self, manual: bool) {
        if manual {
            self.report(crate::Severity::Warning, "This build can't check for updates");
        }
    }
}

#[cfg(feature = "update")]
impl Engine {
    pub(crate) fn start_updates(&mut self) {
        let mode = self.update.settings.mode;
        if mode == UpdateMode::Off {
            return;
        }
        self.report_earlier_install_failure();
        if mode == UpdateMode::Auto {
            self.announce_earlier_update();
        }
        if self.update.updater.is_none() {
            return;
        }
        let delay = self.first_check_delay();
        self.schedule_update_timer(delay);
    }

    fn report_earlier_install_failure(&mut self) {
        let Some(failure) = self.update.record.last_failure().map(str::to_string) else { return };
        self.report(Severity::Warning, format!("Couldn't install the update: {failure}"));
        self.update.record.set_last_failure(None);
        self.persist_update_record();
    }

    fn announce_earlier_update(&mut self) {
        if let Some(executable) = self.update.executable.as_deref()
            && matches!(
                porthmos_update::install_place(executable, self.env.home.as_deref()),
                porthmos_update::InstallPlace::Updatable { .. }
            )
        {
            porthmos_update::forget_previous_version(executable);
        }
        let dir = porthmos_update::UpdateDir::new(self.paths.update_dir());
        if let Some(staged) = porthmos_update::find_staged(&dir, &current_version()) {
            let version = staged.version.to_string();
            self.update.staged = Some(staged);
            self.emit(Event::Update(UpdateState::Ready { version }));
        }
    }

    fn first_check_delay(&self) -> Duration {
        let interval = chrono::Duration::from_std(self.update.timing.interval).unwrap_or(chrono::Duration::MAX);
        let remaining = self
            .update
            .record
            .last_checked()
            .and_then(|checked| (checked + interval).signed_duration_since((self.clock)()).to_std().ok())
            .unwrap_or(Duration::ZERO);
        remaining.max(self.update.timing.startup_delay)
    }

    fn schedule_update_timer(&mut self, delay: Duration) {
        if let Some(previous) = self.update.timer.take() {
            previous.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let internal = self.internal.clone();
        let cancel = self.tasks.spawn("update-timer", Scope::Background, move |cancel| async move {
            if wait_unless_cancelled(delay, &cancel).await {
                let _ = internal.send(Internal::UpdateCheckDue);
            }
        });
        self.update.timer = Some(cancel);
    }

    pub(crate) fn update_check_due(&mut self) {
        self.update.timer = None;
        self.begin_update_check(false);
    }

    pub(crate) fn check_for_update(&mut self, manual: bool) {
        self.begin_update_check(manual);
    }

    fn begin_update_check(&mut self, manual: bool) {
        if self.update.settings.mode == UpdateMode::Off {
            if manual {
                self.report(Severity::Warning, "Update checks are turned off in config.toml");
            }
            return;
        }
        let Some(updater) = self.update.updater.clone() else {
            if manual {
                self.report(Severity::Warning, "Update checks aren't available here");
            }
            return;
        };
        if self.update.busy {
            if manual {
                self.info("An update check is already running");
            } else {
                let delay = self.update.timing.startup_delay;
                self.schedule_update_timer(delay);
            }
            return;
        }
        self.update.busy = true;
        let internal = self.internal.clone();
        let current = current_version();
        self.tasks.spawn("update-check", Scope::Background, move |_| async move {
            let result = updater.check(&current).await;
            let _ = internal.send(Internal::UpdateChecked { manual, result });
        });
    }

    pub(crate) fn update_checked(&mut self, manual: bool, result: Result<Outcome, CheckError>) {
        match result {
            Ok(Outcome::UpToDate) => {
                self.remember_update_check();
                self.update.busy = false;
                if manual {
                    self.info(format!("Porthmos is up to date ({})", current_version()));
                }
                self.schedule_after_check(manual, true);
            }
            Ok(Outcome::Available(info)) => {
                self.remember_update_check();
                self.found_update(info, manual);
            }
            Err(CheckError::NoBuildForTarget { version, target }) => {
                self.remember_update_check();
                self.update.busy = false;
                if manual {
                    self.report(Severity::Warning, format!("Version {version} has no build for {target}"));
                }
                self.schedule_after_check(manual, true);
            }
            Err(error) => {
                self.update.busy = false;
                tracing::debug!("update check failed: {error}");
                if manual {
                    self.report(Severity::Error, error.to_string());
                }
                self.schedule_after_check(manual, false);
            }
        }
    }

    fn schedule_after_check(&mut self, manual: bool, succeeded: bool) {
        if manual {
            return;
        }
        let delay = if succeeded { self.update.timing.interval } else { self.update.timing.retry_after_failure };
        self.schedule_update_timer(delay);
    }

    fn found_update(&mut self, info: UpdateInfo, manual: bool) {
        let version = info.version.to_string();
        if self.update.settings.mode == UpdateMode::Notify {
            self.emit(Event::Update(UpdateState::Available { version }));
            self.update.busy = false;
            self.schedule_after_check(manual, true);
            return;
        }
        let place = self
            .update
            .executable
            .as_deref()
            .map(|executable| porthmos_update::install_place(executable, self.env.home.as_deref()));
        match place {
            Some(porthmos_update::InstallPlace::Updatable { .. }) => {
                if self.update.staged.as_ref().is_some_and(|staged| staged.version == info.version) {
                    self.update.busy = false;
                    self.schedule_after_check(manual, true);
                    return;
                }
                self.emit(Event::Update(UpdateState::Available { version }));
                self.begin_staging(info, manual);
            }
            Some(porthmos_update::InstallPlace::Managed { reason }) => self.update_is_managed(version, reason, manual),
            None => self.update_is_managed(version, "its own file could not be found".to_string(), manual),
        }
    }

    fn update_is_managed(&mut self, version: String, reason: String, manual: bool) {
        self.emit(Event::Update(UpdateState::Managed { version, reason }));
        self.update.busy = false;
        self.schedule_after_check(manual, true);
    }

    fn begin_staging(&mut self, info: UpdateInfo, manual: bool) {
        let Some(updater) = self.update.updater.clone() else {
            self.update.busy = false;
            return;
        };
        let internal = self.internal.clone();
        let version = info.version.to_string();
        self.tasks.spawn("update-stage", Scope::Background, move |cancel| async move {
            let mut last_percent = None;
            let reporter = internal.clone();
            let reported = version.clone();
            let result = updater
                .stage(&info, &cancel, &mut |downloaded, total| {
                    let percent = (downloaded * 100).checked_div(total).unwrap_or(0);
                    if last_percent != Some(percent) {
                        last_percent = Some(percent);
                        let _ =
                            reporter.send(Internal::UpdateProgress { version: reported.clone(), downloaded, total });
                    }
                })
                .await;
            let _ = internal.send(Internal::UpdateStaged { manual, version, result });
        });
    }

    pub(crate) fn update_staged(&mut self, manual: bool, version: String, result: Result<Staged, StageError>) {
        self.update.busy = false;
        match result {
            Ok(staged) => {
                self.update.staged = Some(staged);
                self.emit(Event::Update(UpdateState::Ready { version }));
                self.schedule_after_check(manual, true);
            }
            Err(StageError::Cancelled) => {}
            Err(error) => {
                tracing::debug!("staging the update failed: {error}");
                if manual {
                    self.report(Severity::Error, error.to_string());
                }
                self.schedule_after_check(manual, false);
            }
        }
    }

    fn remember_update_check(&mut self) {
        let now = (self.clock)();
        self.update.record.set_last_checked(now);
        self.persist_update_record();
    }

    fn persist_update_record(&mut self) {
        if !self.update.record.is_writable() {
            return self.update_write_failed(
                "the existing file could not be read or set aside, so it was left alone".to_string(),
            );
        }
        let path = self.update.record.path().to_path_buf();
        let outcome = self
            .update
            .record
            .render()
            .map_err(|err| format!("{err:#}"))
            .and_then(|text| self.writer.write(path, text.into_bytes(), 0o600));
        if let Err(message) = outcome {
            self.update_write_failed(message);
        }
    }

    pub(crate) fn update_write_failed(&mut self, message: String) {
        if self.update.write_warned {
            tracing::debug!("{message}");
            return;
        }
        self.update.write_warned = true;
        self.report(Severity::Warning, format!("Couldn't save the update state: {message}"));
    }
}

#[cfg(all(test, feature = "update"))]
mod tests;
