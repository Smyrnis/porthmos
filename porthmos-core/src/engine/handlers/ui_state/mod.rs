use super::super::{Engine, Event};
use porthmos_vfs::VfsPath;

use crate::{
    Severity,
    config::settings::PanelSettings,
    panel_memory::{PanelKind, PanelMemory},
    state,
};

impl Engine {
    pub(crate) fn load_ui_state(&mut self) {
        let loaded = state::load(&self.paths);
        if let Some(warning) = loaded.warning {
            self.report(Severity::Warning, warning);
        }
        self.state_protected = loaded.protected;
        let save = loaded.state.save_passwords_in_keyring;
        self.ui_state = loaded.state;
        self.ui_state.speed_limit = self.ui_state.speed_limit.filter(|rate| *rate > 0);
        self.emit(Event::SaveChoice { save });
        if self.ui_state.speed_limit.is_some() {
            self.speed_limit.set(self.ui_state.speed_limit);
            self.publish_transfers();
        }
    }

    pub(crate) fn remember_save_choice(&mut self, save: bool) {
        if !self.state_protected {
            self.ui_state.save_passwords_in_keyring = save;
        }
        self.remember_ui_state("keyring choice");
    }

    pub(crate) fn remember_local_folder(&mut self, folder: &VfsPath) {
        let text = folder.to_string_lossy();
        if self.ui_state.local_folder.as_deref() == Some(&*text) {
            return;
        }
        self.ui_state.local_folder = Some(text.into_owned());
        self.save_quietly("local folder");
    }

    pub(crate) fn remember_panel(&mut self, kind: PanelKind, settings: &PanelSettings) {
        let memory = Some(PanelMemory::of(settings));
        let slot = match kind {
            PanelKind::Local => &mut self.ui_state.local_panel,
            PanelKind::Remote => &mut self.ui_state.remote_panel,
        };
        if *slot == memory {
            return;
        }
        *slot = memory;
        self.save_quietly("panel settings");
    }

    pub(crate) fn remember_open_groups(&mut self, mut groups: Vec<String>) {
        groups.sort();
        groups.dedup();
        if self.ui_state.open_groups == groups {
            return;
        }
        self.ui_state.open_groups = groups;
        self.save_quietly("open connection groups");
    }

    pub(crate) fn save_quietly(&mut self, what: &str) {
        if self.state_protected {
            return;
        }
        let Err(err) = state::save(&self.paths, &self.ui_state) else {
            return;
        };
        if self.state_write_warned {
            tracing::debug!("couldn't remember the {what}: {err:#}");
            return;
        }
        self.state_write_warned = true;
        self.report(Severity::Warning, format!("Couldn't remember the {what}: {err}"));
    }

    pub(crate) fn remember_ui_state(&mut self, what: &str) {
        if self.state_protected {
            self.report(
                Severity::Warning,
                format!("Couldn't remember the {what}: the saved settings file is protected"),
            );
            return;
        }
        if let Err(err) = state::save(&self.paths, &self.ui_state) {
            self.report(Severity::Warning, format!("Couldn't remember the {what}: {err}"));
        }
    }
}

#[cfg(test)]
mod tests;
