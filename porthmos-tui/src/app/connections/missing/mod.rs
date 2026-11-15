use super::super::*;
use crate::widgets::dialog::message::MessageDialog;
use porthmos_core::i18n;

fn unannounced(entries: &[ConnectionEntry], source: ConnectionSource, announced: &HashSet<String>) -> Vec<String> {
    let mut names: Vec<String> = entries
        .iter()
        .filter(|entry| entry.source == source && !announced.contains(&entry.name))
        .map(|entry| entry.name.clone())
        .collect();
    names.sort();
    names
}

impl App {
    pub(in crate::app) fn announce_missing_hosts(&mut self) {
        if self.dialog.is_some() {
            return;
        }
        let entries = self.connections.entries();
        let missing = unannounced(entries, ConnectionSource::MissingSshHost, &self.announced_missing);
        let shadowed = unannounced(entries, ConnectionSource::ShadowedSshHost, &self.announced_missing);
        if missing.is_empty() && shadowed.is_empty() {
            return;
        }
        let mut sentences = Vec::new();
        if !missing.is_empty() {
            sentences.push(i18n::text_with("missing.gone", &[("hosts", &missing.join(", "))]));
        }
        if !shadowed.is_empty() {
            sentences.push(i18n::text_with("missing.hidden", &[("hosts", &shadowed.join(", "))]));
        }
        sentences.push(i18n::text("missing.marked"));
        self.announced_missing.extend(missing.into_iter().chain(shadowed));
        self.dialog = Some(Dialog::Message(MessageDialog::new(i18n::text("missing.title"), sentences.join(" "))));
    }

    pub(in crate::app) fn open_missing_host_dialog(&mut self) {
        let Some(entry) = self.connections.selected_entry() else {
            return;
        };
        let name = entry.name.clone();
        let title = if entry.source == ConnectionSource::ShadowedSshHost {
            i18n::text_with("missing.item_hidden", &[("name", &name)])
        } else {
            i18n::text_with("connection.no_longer_in_ssh_config", &[("name", &name)])
        };
        let items = vec![i18n::text("missing.move"), i18n::text("missing.forget"), i18n::text("common.cancel")];
        self.dialog = Some(Dialog::List(ListDialog::new(title, items)));
        self.pending_action = Some(PendingAction::FixMissingHost { name });
    }

    pub(in crate::app) fn apply_missing_host_choice(&mut self, name: String, index: usize) {
        self.dialog = None;
        match index {
            0 => {
                let candidates: Vec<String> = self
                    .connections
                    .entries()
                    .iter()
                    .filter(|entry| entry.source == ConnectionSource::SshConfig)
                    .filter(|entry| entry.group.is_none() && entry.tags.is_empty())
                    .map(|entry| entry.name.clone())
                    .collect();
                if candidates.is_empty() {
                    self.notifications.push(Severity::Warning, i18n::text("missing.no_target"));
                    return;
                }
                self.dialog = Some(Dialog::List(ListDialog::new(
                    i18n::text_with("missing.move_title", &[("name", &name)]),
                    candidates.clone(),
                )));
                self.pending_action = Some(PendingAction::MoveLabels { from: name, candidates });
            }
            1 => self.core.send(Command::ForgetSshLabels { name }),
            _ => {}
        }
    }

    pub(in crate::app) fn move_labels_to(&mut self, from: String, candidates: Vec<String>, index: usize) {
        self.dialog = None;
        if let Some(to) = candidates.into_iter().nth(index) {
            self.core.send(Command::MoveSshLabels { from, to });
        }
    }
}

#[cfg(test)]
mod tests;
