use std::time::Duration;

use iced::Subscription;
use porthmos_core::i18n::label as tr;
use porthmos_core::{
    Command, Privilege, RefreshTab, ServiceVerb, ServicesState as ServicesEvent, SessionId, Severity, UnitName,
};

use super::{App, Message};
use crate::drawer::DrawerView;
use crate::logs_view::{SourceKind, tabs_full_text};
use crate::modal::Modal;
use crate::refresh;
use crate::services_view::{ActionState, action_state, status_allowed};

impl App {
    pub fn services(&self) -> &crate::services_view::ServicesState {
        &self.services
    }

    pub fn services_tab_label(&self) -> String {
        tr("Services")
    }

    fn services_visible(&self) -> bool {
        self.drawer.open && self.drawer.view == DrawerView::Services
    }

    pub(in crate::app) fn show_services(&mut self) {
        self.dismiss_inline_editors();
        self.drawer.view = DrawerView::Services;
        self.drawer.open = true;
        self.drawer.focused = false;
    }

    pub(in crate::app) fn sync_services(&mut self) {
        let wanted = if self.services_visible() { self.health_target() } else { None };
        if wanted == self.services.started {
            return;
        }
        if let Some(old) = self.services.started.take() {
            self.core.send(Command::StopServices { session: old });
        }
        match wanted {
            Some(session) => {
                self.services.begin(session);
                self.core.send(Command::StartServices { session });
                self.services.started = Some(session);
            }
            None => self.services.clear(),
        }
    }

    pub(in crate::app) fn services_arrived(&mut self, session: SessionId, state: ServicesEvent) {
        self.services.apply(session, state);
    }

    pub(in crate::app) fn service_status_arrived(&mut self, session: SessionId, unit: &str, text: String) {
        self.services.status_arrived(session, unit, text);
    }

    pub(in crate::app) fn services_refresh(&mut self) {
        if let Some(session) = self.services.started {
            self.core.send(Command::RefreshServices { session });
        }
    }

    pub(in crate::app) fn services_tick(&mut self) {
        if self.services_visible() {
            self.services_refresh();
        }
    }

    pub fn services_tick_interval(&self) -> Option<Duration> {
        if !self.services_visible() || self.services.started.is_none() {
            return None;
        }
        let setting = refresh::setting(&self.gui_settings.refresh, RefreshTab::Services);
        refresh::tick_interval(setting, self.services.failing())
    }

    pub(in crate::app) fn services_subscription(&self) -> Subscription<Message> {
        match self.services_tick_interval() {
            Some(interval) => iced::time::every(interval).map(|_| Message::RefreshTick(RefreshTab::Services)),
            None => Subscription::none(),
        }
    }

    fn services_selection(&self, verb: Option<ServiceVerb>) -> Option<(SessionId, String, u32)> {
        let session = self.services.started?;
        let Some(ServicesEvent::Snapshot(snapshot)) = &self.services.state else {
            return None;
        };
        let selected = self.services.selected.clone()?;
        let allowed = match verb {
            Some(verb) => action_state(snapshot, Some(&selected), verb) == ActionState::Allowed,
            None => status_allowed(snapshot, Some(&selected)),
        };
        allowed.then_some((session, selected, snapshot.uid))
    }

    pub(in crate::app) fn service_action_requested(&mut self, verb: ServiceVerb) {
        if let Some((session, unit, uid)) = self.services_selection(Some(verb)) {
            self.modals.push(Modal::ConfirmServiceAction { session, unit, verb, privilege: Privilege::for_uid(uid) });
        }
    }

    pub(in crate::app) fn service_logs_requested(&mut self) {
        let Some(unit) = self.services.selected.clone().filter(|name| UnitName::new(name).is_some()) else {
            return;
        };
        if let Some(index) = self.health_target().and_then(|session| self.logs.following(&unit, session)) {
            self.logs.select(index);
            self.show_logs();
            return;
        }
        if !self.logs.active().is_pristine() && !self.logs.add() {
            self.show_logs();
            self.push_notice(Severity::Warning, tabs_full_text());
            return;
        }
        let tab = self.logs.active_mut();
        tab.kind = SourceKind::Journal;
        tab.unit = unit;
        self.show_logs();
        self.logs_start();
    }

    pub(in crate::app) fn service_status_requested(&mut self) {
        if let Some((session, unit, _)) = self.services_selection(None) {
            self.services.status_requested(&unit);
            self.core.send(Command::ServiceStatus { session, unit });
        }
    }
}

#[cfg(test)]
mod tests;
