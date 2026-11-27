use std::time::Duration;

use iced::Subscription;
use porthmos_core::i18n::label as tr;
use porthmos_core::{Command, PortsState as PortsEvent, RefreshTab, SessionId};

use super::{App, Message};
use crate::drawer::DrawerView;
use crate::refresh;

impl App {
    pub fn ports(&self) -> &crate::ports_view::PortsState {
        &self.ports
    }

    pub fn ports_tab_label(&self) -> String {
        tr("Ports")
    }

    fn ports_visible(&self) -> bool {
        self.drawer.open && self.drawer.view == DrawerView::Ports
    }

    pub(in crate::app) fn show_ports(&mut self) {
        self.dismiss_inline_editors();
        self.drawer.view = DrawerView::Ports;
        self.drawer.open = true;
        self.drawer.focused = false;
    }

    pub(in crate::app) fn sync_ports(&mut self) {
        let wanted = if self.ports_visible() { self.health_target() } else { None };
        if wanted == self.ports.started {
            return;
        }
        if let Some(old) = self.ports.started.take() {
            self.core.send(Command::StopPorts { session: old });
        }
        match wanted {
            Some(session) => {
                self.ports.begin(session);
                self.core.send(Command::StartPorts { session });
                self.ports.started = Some(session);
            }
            None => self.ports.clear(),
        }
    }

    pub(in crate::app) fn ports_arrived(&mut self, session: SessionId, state: PortsEvent) {
        self.ports.apply(session, state);
    }

    pub(in crate::app) fn ports_refresh(&mut self) {
        if let Some(session) = self.ports.started {
            self.core.send(Command::RefreshPorts { session });
        }
    }

    pub(in crate::app) fn ports_tick(&mut self) {
        if self.ports_visible() {
            self.ports_refresh();
        }
    }

    pub fn ports_tick_interval(&self) -> Option<Duration> {
        if !self.ports_visible() || self.ports.started.is_none() {
            return None;
        }
        refresh::tick_interval(refresh::setting(&self.gui_settings.refresh, RefreshTab::Ports), self.ports.failing())
    }

    pub(in crate::app) fn ports_subscription(&self) -> Subscription<Message> {
        match self.ports_tick_interval() {
            Some(interval) => iced::time::every(interval).map(|_| Message::RefreshTick(RefreshTab::Ports)),
            None => Subscription::none(),
        }
    }
}

#[cfg(test)]
mod tests;
