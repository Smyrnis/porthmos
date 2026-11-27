use iced::Task;
use iced::widget::operation::{self, RelativeOffset};
use porthmos_core::Command;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{LogId, LogState};

use super::{App, Message};
use crate::drawer::DrawerView;
use crate::logs_view::LogStatus;

impl App {
    pub fn logs(&self) -> &crate::logs_view::LogsState {
        &self.logs
    }

    pub fn logs_tab_label(&self) -> String {
        tr("Logs")
    }

    pub(in crate::app) fn show_logs(&mut self) {
        self.dismiss_inline_editors();
        self.drawer.view = DrawerView::Logs;
        self.drawer.open = true;
        self.drawer.focused = false;
    }

    pub fn logs_list_id() -> iced::widget::Id {
        iced::widget::Id::new("logs-list")
    }

    pub fn logs_shown(&self) -> bool {
        self.health_target().is_some() || !self.logs.lines.is_empty() || self.logs.status != LogStatus::Idle
    }

    pub(in crate::app) fn logs_list_present(&self) -> bool {
        self.drawer.open && self.drawer.view == DrawerView::Logs && self.logs_shown() && self.logs.shown_len() > 0
    }

    pub(in crate::app) fn logs_snap(&mut self) -> Task<Message> {
        let visible = self.logs_list_present();
        let appeared = self.logs.appeared(visible);
        let wanted = self.logs.take_snap();
        if !visible {
            return Task::none();
        }
        if wanted || appeared && self.logs.follow {
            operation::snap_to(App::logs_list_id(), RelativeOffset::END)
        } else if appeared {
            operation::scroll_to(App::logs_list_id(), operation::AbsoluteOffset { x: None, y: Some(self.logs.scroll) })
        } else {
            Task::none()
        }
    }

    pub(in crate::app) fn sync_logs(&mut self) {
        let Some((_, session)) = self.logs.running else {
            return;
        };
        if !self.sessions.by_id(session).is_some_and(|session| session.healthy) {
            self.logs_stop_because(&i18n::text("logs.connection_lost"));
        }
    }

    pub(in crate::app) fn logs_session_chosen(&mut self, name: String) {
        let Some(chosen) = self.sessions.by_name(&name).map(|session| session.id) else {
            return;
        };
        self.health.chosen = Some(chosen);
        if self.logs.running.is_some_and(|(_, session)| session != chosen) {
            self.logs_stop_because(&i18n::text("gui.logs.other_connection"));
        }
    }

    fn logs_stop_because(&mut self, reason: &str) {
        self.logs_stop();
        self.logs.status = LogStatus::Ended(i18n::text_with("gui.logs.stopped_because", &[("reason", reason)]));
    }

    pub(in crate::app) fn logs_start(&mut self) {
        let (Some(session), Some(source)) = (self.health_target(), self.logs.source()) else {
            return;
        };
        self.logs_stop();
        self.logs.start_fresh();
        let id = self.logs.next_id;
        self.logs.next_id += 1;
        self.logs.running = Some((id, session));
        self.logs.status = LogStatus::Starting;
        let initial_lines = self.gui_settings.logs.initial_lines;
        self.core.send(Command::StartLog { id, session, source, initial_lines });
    }

    pub(in crate::app) fn logs_stop(&mut self) {
        if let Some((id, _)) = self.logs.running.take() {
            self.core.send(Command::StopLog { id });
            self.logs.status = LogStatus::Ended(i18n::text("gui.logs.stopped"));
        }
    }

    pub(in crate::app) fn logs_clear(&mut self) {
        self.logs.clear_buffer();
    }

    pub(in crate::app) fn logs_pause_toggled(&mut self) {
        self.logs.pause_toggled(self.gui_settings.logs.max_lines as usize);
    }

    pub(in crate::app) fn logs_arrived(&mut self, id: LogId, state: LogState) {
        if self.logs.running.map(|(running, _)| running) != Some(id) {
            return;
        }
        match state {
            LogState::Started => self.logs.status = LogStatus::Running,
            LogState::Lines { lines, skipped } => {
                let max_lines = self.gui_settings.logs.max_lines as usize;
                self.logs.append(lines, skipped, max_lines);
            }
            LogState::Ended(message) => {
                self.logs.running = None;
                self.logs.status = LogStatus::Ended(message.to_string());
            }
            LogState::Unavailable(message) => {
                self.logs.running = None;
                self.logs.status = LogStatus::Unavailable(message.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests;
