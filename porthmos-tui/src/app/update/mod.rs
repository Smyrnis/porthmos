use super::*;
use crate::widgets::update_badge::{BadgeKind, UpdateBadge};
use porthmos_core::i18n;

impl App {
    pub(crate) fn update_badge(&self) -> Option<UpdateBadge> {
        if let Some(version) = &self.ready_update {
            return Some(UpdateBadge { version: version.clone(), kind: BadgeKind::Ready });
        }
        let (version, kind) = match self.update_state.as_ref()? {
            UpdateState::Ready { version } => (version, BadgeKind::Ready),
            UpdateState::Available { version } if self.update_mode == UpdateMode::Notify => {
                (version, BadgeKind::Available)
            }
            UpdateState::Managed { version, .. } => (version, BadgeKind::Available),
            UpdateState::Available { .. } | UpdateState::Downloading { .. } => return None,
        };
        Some(UpdateBadge { version: version.clone(), kind })
    }

    pub(super) fn apply_update_event(&mut self, state: UpdateState) {
        if let UpdateState::Ready { version } = &state {
            self.ready_update = Some(version.clone());
        }
        self.update_state = Some(state);
        let Some(badge) = self.update_badge() else { return };
        let announced = (badge.version.clone(), badge.kind);
        if self.announced_update.as_ref() != Some(&announced) {
            self.announced_update = Some(announced);
            self.announce_update();
        }
    }

    pub(super) fn check_for_updates(&mut self) {
        if self.update_badge().is_some() {
            self.announce_update();
        } else {
            self.core.send(Command::CheckForUpdate { manual: true });
        }
    }

    fn announce_update(&mut self) {
        let Some(badge) = self.update_badge() else { return };
        let version = &badge.version;
        let message = match (&self.ready_update, &self.update_state, badge.kind) {
            (None, Some(UpdateState::Managed { reason, .. }), _) => {
                i18n::text_with("update.managed_available", &[("version", &version.to_string()), ("reason", reason)])
            }
            (_, _, BadgeKind::Ready) => i18n::text_with("update.ready_notice", &[("version", &version.to_string())]),
            (_, _, BadgeKind::Available) => {
                i18n::text_with("update.available_notice", &[("version", &version.to_string())])
            }
        };
        self.notifications.push(Severity::Info, message);
    }
}

#[cfg(test)]
mod tests;
