use std::{collections::HashSet, path::PathBuf, sync::Arc};

use super::super::{Engine, Event, Internal};
use crate::{
    Severity,
    error::failure_report,
    import::{ImportError, ImportPlan, ImportSource, Verdict, read_plan},
    profiles::{ConnectionProfile, store},
    tasks::Scope,
};

const READ_FAILED: &str = "Couldn't read the connections to import";
const NOT_IN_THE_LIST: &str = "that row is not in the list";

#[derive(Default)]
pub(crate) struct ImportState {
    requested: Option<u64>,
    plan: Option<Arc<ImportPlan>>,
}

impl Engine {
    fn taken_connection_names(&self) -> anyhow::Result<HashSet<String>> {
        Ok(self
            .all_profiles()?
            .into_iter()
            .filter(|entry| !entry.source.is_orphan_labels())
            .map(|entry| entry.name)
            .collect())
    }

    fn forget_import(&mut self) {
        self.import.requested = None;
        self.import.plan = None;
        self.tasks.cancel(Scope::Import);
        self.tasks.forget(Scope::Import);
    }

    pub(crate) fn read_import(&mut self, import_id: u64, source: ImportSource, path: PathBuf) {
        self.forget_import();
        let taken = match self.taken_connection_names() {
            Ok(taken) => taken,
            Err(err) => {
                return self.emit(Event::ImportReadFailed { import_id, message: failure_report(READ_FAILED, &err) });
            }
        };
        self.import.requested = Some(import_id);
        let protocols = self.protocols.clone();
        let internal = self.internal.clone();
        self.tasks.spawn("import-read", Scope::Import, move |_cancel| async move {
            let result = tokio::task::spawn_blocking(move || read_plan(import_id, source, &path, &protocols, &taken))
                .await
                .unwrap_or_else(|_| Err(ImportError::Unreadable("the reader stopped unexpectedly".to_string())));
            let _ = internal.send(Internal::ImportRead { import_id, result });
        });
    }

    pub(crate) fn import_read_finished(&mut self, import_id: u64, result: Result<ImportPlan, ImportError>) {
        if self.import.requested != Some(import_id) {
            return;
        }
        match result {
            Ok(plan) => {
                let plan = Arc::new(plan);
                self.import.plan = Some(plan.clone());
                self.emit(Event::ImportPlanReady(plan));
            }
            Err(err) => {
                self.import.requested = None;
                self.emit(Event::ImportReadFailed { import_id, message: failure_report(READ_FAILED, &err) });
            }
        }
    }

    pub(crate) fn cancel_import(&mut self, import_id: u64) {
        if self.import.requested == Some(import_id) {
            self.forget_import();
        }
    }

    pub(crate) fn run_import(&mut self, import_id: u64, rows: Vec<u32>) {
        let Some(plan) = self.import.plan.clone().filter(|plan| plan.import_id == import_id) else {
            return;
        };
        let mut taken = match self.taken_connection_names() {
            Ok(taken) => taken,
            Err(err) => return self.report(Severity::Error, failure_report(READ_FAILED, &err)),
        };
        self.forget_import();

        let mut profiles = Vec::new();
        let mut skipped = 0;
        let mut first_reason: Option<String> = None;
        for index in rows {
            match Self::import_row_profile(&plan, index, &mut taken) {
                Ok(profile) => profiles.push(profile),
                Err(reason) => {
                    skipped += 1;
                    first_reason.get_or_insert(reason);
                }
            }
        }
        let mut added = profiles.len();
        if !profiles.is_empty()
            && let Err(err) = store::save_many(&self.paths, &profiles)
        {
            skipped += added;
            added = 0;
            first_reason.get_or_insert(format!("couldn't save the connections: {err:#}"));
        }
        self.emit(Event::ImportDone { import_id, added, skipped });
        self.list_profiles();
        if let Some(reason) = first_reason {
            self.report(Severity::Warning, format!("Skipped {skipped}: {reason}"));
        }
    }

    fn import_row_profile(
        plan: &ImportPlan, index: u32, taken: &mut HashSet<String>,
    ) -> Result<ConnectionProfile, String> {
        let row = plan.rows.get(index as usize).ok_or_else(|| NOT_IN_THE_LIST.to_string())?;
        let candidate = match (&row.verdict, &row.candidate) {
            (Verdict::New, Some(candidate)) => candidate,
            _ => return Err(format!("\"{}\" can't be imported", row.name)),
        };
        if !taken.insert(candidate.name.clone()) {
            return Err(format!("\"{}\" already exists", candidate.name));
        }
        Ok(ConnectionProfile {
            name: candidate.name.clone(),
            protocol: candidate.protocol.clone(),
            host: candidate.host.clone(),
            port: candidate.port,
            username: candidate.username.clone(),
            group: candidate.group.clone(),
            tags: Vec::new(),
            in_keyring: Vec::new(),
            options: candidate.options.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
