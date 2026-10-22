use super::super::{Engine, Event};
use crate::{Severity, state};

impl Engine {
    pub(crate) fn load_ui_state(&mut self) {
        let loaded = state::load(&self.paths);
        if let Some(warning) = loaded.warning {
            self.report(Severity::Warning, warning);
        }
        self.state_protected = loaded.protected;
        self.ui_state = loaded.state;
        self.ui_state.speed_limit = loaded.state.speed_limit.filter(|rate| *rate > 0);
        self.emit(Event::SaveChoice { save: loaded.state.save_passwords_in_keyring });
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
