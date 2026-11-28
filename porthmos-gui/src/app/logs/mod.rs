use iced::Task;
use iced::widget::operation::{self, RelativeOffset};
use porthmos_core::Command;
use porthmos_core::edit::printable;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{LogId, LogState, Severity};

use super::{App, Message, PanelSide};
use crate::drawer::DrawerView;
use crate::logs_view::{
    LogStatus, LogTabs, LogsState, SourceKind, default_file_name, save_text, tabs_full_text, write_lines,
};
use crate::view::logs::skipped_text;

async fn save_lines(name: String, text: String) -> Result<Option<std::path::PathBuf>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new().set_file_name(name).save_file().await else {
        return Ok(None);
    };
    let path = handle.path().to_path_buf();
    write_lines(path.clone(), text).await?;
    Ok(Some(path))
}

impl App {
    pub fn logs(&self) -> &LogsState {
        self.logs.active()
    }

    pub fn log_tabs(&self) -> &LogTabs {
        &self.logs
    }

    pub fn logs_tab_label(&self) -> String {
        tr("Logs")
    }

    pub(in crate::app) fn logs_tab_for_new_source(&mut self) -> bool {
        if self.logs.active().is_pristine() || self.logs.add() {
            return true;
        }
        self.show_logs();
        self.push_notice(Severity::Warning, tabs_full_text());
        false
    }

    pub(in crate::app) fn follow_as_log(&mut self) {
        let Some(session) =
            self.sessions.active().filter(|session| session.can_exec && session.healthy).map(|session| session.id)
        else {
            return;
        };
        let Some(path) = self
            .panel_of(PanelSide::Remote)
            .and_then(crate::panel::Panel::cursor_entry)
            .filter(|entry| !entry.is_dir)
            .map(|entry| entry.path.to_string_lossy().into_owned())
        else {
            return;
        };
        if let Some(index) = self.logs.following_file(&path, session) {
            self.logs.select(index);
            self.show_logs();
            return;
        }
        if !self.logs_tab_for_new_source() {
            return;
        }
        self.health.chosen = Some(session);
        let tab = self.logs.active_mut();
        tab.kind = SourceKind::File;
        tab.path = path;
        self.show_logs();
        self.logs_start();
    }

    pub(in crate::app) fn logs_save_requested(&mut self) -> Task<Message> {
        let state = self.logs();
        if state.view_len() == 0 {
            return Task::none();
        }
        let text = save_text(state, skipped_text);
        let name = default_file_name(state, chrono::Local::now().date_naive());
        Task::perform(save_lines(name, text), Message::LogsSaved)
    }

    pub(in crate::app) fn logs_saved(&mut self, result: Result<Option<std::path::PathBuf>, String>) {
        match result {
            Ok(None) => {}
            Ok(Some(path)) => {
                let shown = printable(&path.display().to_string());
                self.push_notice(Severity::Info, i18n::text_with("gui.logs.saved", &[("path", &shown)]));
            }
            Err(reason) => {
                self.push_notice(Severity::Error, i18n::text_with("gui.logs.save_failed", &[("reason", &reason)]));
            }
        }
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
        self.health_target().is_some()
            || self.logs.tabs().iter().any(|tab| !tab.lines.is_empty() || tab.status != LogStatus::Idle)
    }

    pub(in crate::app) fn logs_list_present(&self) -> bool {
        self.drawer.open && self.drawer.view == DrawerView::Logs && self.logs_shown() && self.logs().view_len() > 0
    }

    pub(in crate::app) fn logs_snap(&mut self) -> Task<Message> {
        let visible = self.logs_list_present();
        let tab = self.logs.active_mut();
        let appeared = tab.appeared(visible);
        let wanted = tab.take_snap();
        let (follow, scroll) = (tab.follow, tab.scroll);
        if !visible {
            return Task::none();
        }
        if wanted || appeared && follow {
            operation::snap_to(App::logs_list_id(), RelativeOffset::END)
        } else if appeared {
            operation::scroll_to(App::logs_list_id(), operation::AbsoluteOffset { x: None, y: Some(scroll) })
        } else {
            Task::none()
        }
    }

    pub(in crate::app) fn sync_logs(&mut self) {
        let lost: Vec<usize> = self
            .logs
            .tabs()
            .iter()
            .enumerate()
            .filter(|(_, tab)| {
                tab.running.is_some_and(|(_, session)| !self.sessions.by_id(session).is_some_and(|s| s.healthy))
            })
            .map(|(index, _)| index)
            .collect();
        for index in lost {
            self.stop_tab(index, stopped_because(&i18n::text("logs.connection_lost")));
        }
    }

    pub(in crate::app) fn logs_session_chosen(&mut self, name: String) {
        let Some(chosen) = self.sessions.by_name(&name).map(|session| session.id) else {
            return;
        };
        self.health.chosen = Some(chosen);
        if self.logs().running.is_some_and(|(_, session)| session != chosen) {
            let index = self.logs.active_index();
            self.stop_tab(index, stopped_because(&i18n::text("gui.logs.other_connection")));
        }
    }

    fn stop_tab(&mut self, index: usize, status: LogStatus) {
        let Some(tab) = self.logs.tab_mut(index) else {
            return;
        };
        if let Some((id, _)) = tab.running.take() {
            tab.status = status;
            self.core.send(Command::StopLog { id });
        }
    }

    pub(in crate::app) fn logs_tab_selected(&mut self, index: usize) {
        self.logs.select(index);
    }

    pub(in crate::app) fn logs_tab_added(&mut self) {
        self.logs.add();
    }

    pub(in crate::app) fn logs_tab_closed(&mut self, index: usize) {
        if let Some(id) = self.logs.close(index) {
            self.core.send(Command::StopLog { id });
        }
    }

    pub(in crate::app) fn logs_start(&mut self) {
        let own_session = self.logs().running.map(|(_, session)| session);
        let (Some(session), Some(source)) = (own_session.or_else(|| self.health_target()), self.logs().source()) else {
            return;
        };
        self.logs_stop();
        let id = self.logs.take_id();
        let tab = self.logs.active_mut();
        tab.start_fresh();
        tab.running = Some((id, session));
        tab.status = LogStatus::Starting;
        let initial_lines = self.gui_settings.logs.initial_lines;
        self.core.send(Command::StartLog { id, session, source, initial_lines });
    }

    pub(in crate::app) fn logs_stop(&mut self) {
        let index = self.logs.active_index();
        self.stop_tab(index, LogStatus::Ended(i18n::text("gui.logs.stopped")));
    }

    pub(in crate::app) fn logs_clear(&mut self) {
        self.logs.active_mut().clear_buffer();
    }

    pub(in crate::app) fn logs_pause_toggled(&mut self) {
        let max_lines = self.gui_settings.logs.max_lines as usize;
        self.logs.active_mut().pause_toggled(max_lines);
    }

    pub(in crate::app) fn logs_arrived(&mut self, id: LogId, state: LogState) {
        let max_lines = self.gui_settings.logs.max_lines as usize;
        let Some(tab) = self.logs.by_id_mut(id) else {
            return;
        };
        match state {
            LogState::Started => tab.status = LogStatus::Running,
            LogState::Lines { lines, skipped } => tab.append(lines, skipped, max_lines),
            LogState::Ended(message) => {
                tab.running = None;
                tab.status = LogStatus::Ended(message.to_string());
            }
            LogState::Unavailable(message) => {
                tab.running = None;
                tab.status = LogStatus::Unavailable(message.to_string());
            }
        }
    }
}

fn stopped_because(reason: &str) -> LogStatus {
    LogStatus::Ended(i18n::text_with("gui.logs.stopped_because", &[("reason", reason)]))
}

#[cfg(test)]
mod tests;
