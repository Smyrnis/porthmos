use iced::Subscription;
use porthmos_core::i18n::label as tr;
use porthmos_core::{Command, Handover, Message as Notice, SessionId, ShellSize, ShellStream, TerminalId, VfsPath};
use porthmos_core::{Severity, to_os_path};
use porthmos_term::{LocalShell, TermSize, TerminalConfig, TerminalEvent, TerminalSession, spawn_local};

use super::{App, Message};
use crate::drawer::keys::{KeyInput, is_chord, translate};
use crate::drawer::{Drawer, SharedSession, TerminalSource, TerminalTab, Wanted, into_transport, opening_size};
use crate::shortcuts::KeyPress;
use porthmos_term::encode_key;

fn shell_size(size: TermSize) -> ShellSize {
    ShellSize { columns: size.columns, rows: size.rows }
}

fn change_directory_command(directory: &VfsPath) -> Option<Vec<u8>> {
    let path = directory.to_string_lossy();
    if path.chars().any(|character| character.is_control() || character == char::REPLACEMENT_CHARACTER) {
        return None;
    }
    Some(format!("cd -- {}\r", porthmos_ssh::shell_quote(&path)).into_bytes())
}

impl App {
    pub fn drawer(&self) -> &Drawer {
        &self.drawer
    }

    pub(crate) fn open_embedded_terminal(&mut self, source: TerminalSource) {
        let directory = match source {
            TerminalSource::Local => self.local.panel.path().clone(),
            TerminalSource::Remote(session) => match self.sessions.by_id(session) {
                Some(session) => session.pane.panel.path().clone(),
                None => return self.push_notice(Severity::Warning, tr("Connect to a server first")),
            },
        };
        self.dismiss_inline_editors();
        self.drawer.tab = None;
        let id = self.drawer.allocate_id();
        self.drawer.open = true;
        self.drawer.focused = true;
        let wanted = Wanted { id, source, directory };
        match source {
            TerminalSource::Remote(session) => {
                self.drawer.wanted = Some(wanted);
                self.core.send(Command::OpenTerminal { id, session, size: shell_size(opening_size()) });
            }
            TerminalSource::Local => self.start_local_terminal(wanted),
        }
    }

    pub(in crate::app) fn start_local_terminal(&mut self, wanted: Wanted) {
        let shell = LocalShell { directory: Some(to_os_path(&wanted.directory)), ..LocalShell::default() };
        match spawn_local(&shell, opening_size()) {
            Ok(transport) => self.start_tab(wanted, transport),
            Err(error) => self.push_notice(
                Severity::Error,
                porthmos_core::i18n::text_with("gui.terminal.open_failed", &[("error", &error.to_string())]),
            ),
        }
    }

    fn start_tab(&mut self, wanted: Wanted, transport: porthmos_term::Transport) {
        let size = opening_size();
        let (session, events) = TerminalSession::spawn(transport, size, TerminalConfig::default());
        if matches!(wanted.source, TerminalSource::Remote(_)) {
            match change_directory_command(&wanted.directory) {
                Some(command) => session.write(command),
                None => self.push_notice(Severity::Warning, porthmos_core::i18n::text("gui.terminal.folder_unsafe")),
            }
        }
        self.drawer.tab = Some(TerminalTab {
            id: wanted.id,
            source: wanted.source,
            title: String::new(),
            session: Some(SharedSession::new(session)),
            frozen: None,
            events: Handover::new(events),
            exited: None,
            size,
        });
    }

    pub(in crate::app) fn terminal_opened(&mut self, id: TerminalId, stream: Handover<ShellStream>) {
        let Some(stream) = stream.take() else {
            return;
        };
        if self.drawer.wanted.as_ref().map(|wanted| wanted.id) != Some(id) {
            return;
        }
        let Some(wanted) = self.drawer.wanted.take() else {
            return;
        };
        self.start_tab(wanted, into_transport(stream));
    }

    pub(in crate::app) fn terminal_open_failed(&mut self, id: TerminalId, message: Notice) {
        if self.drawer.wanted.as_ref().map(|wanted| wanted.id) == Some(id) {
            self.drawer.wanted = None;
            self.push_notice(Severity::Error, message);
        }
    }

    pub(in crate::app) fn terminal_event(&mut self, id: TerminalId, event: TerminalEvent) {
        let Some(tab) = self.drawer.tab.as_mut().filter(|tab| tab.id == id) else {
            return;
        };
        match event {
            TerminalEvent::Title(title) => tab.title = title,
            TerminalEvent::Exited(status) => tab.exited = Some(status),
            _ => {}
        }
    }

    pub(in crate::app) fn terminal_session_ended(&mut self, session: SessionId) {
        if let Some(tab) = self.drawer.tab.as_mut().filter(|tab| tab.source == TerminalSource::Remote(session)) {
            tab.end();
        }
        if self.drawer.wanted.as_ref().is_some_and(|wanted| wanted.source == TerminalSource::Remote(session)) {
            self.drawer.wanted = None;
        }
    }

    pub(in crate::app) fn terminal_has_keys(&self) -> bool {
        self.drawer.open
            && self.drawer.focused
            && (self.drawer.wanted.is_some() || self.drawer.tab.as_ref().is_some_and(|tab| tab.exited.is_none()))
            && self.modals.is_empty()
            && self.tool.is_none()
            && !self.drag.is_tracking()
            && self.path_edit.is_none()
            && self.filtering.is_none()
            && self.context_menu.is_none()
            && !self.more_open
            && self.queue.speed_text().is_none()
    }

    pub(in crate::app) fn terminal_key(&mut self, press: &KeyPress) {
        if is_chord(press) {
            self.drawer.focused = false;
            return;
        }
        let Some(tab) = self.drawer.tab.as_ref() else {
            return;
        };
        let Some(session) = tab.session.as_ref() else {
            return;
        };
        let bytes = match translate(press) {
            Some(KeyInput::Key(key, modifiers)) => {
                let modes = session.with_terminal(|terminal| terminal.modes());
                encode_key(key, modifiers, modes)
            }
            Some(KeyInput::Text(text)) => Some(text.into_bytes()),
            None => None,
        };
        if let Some(bytes) = bytes {
            session.write(bytes);
        }
    }

    fn dismiss_inline_editors(&mut self) {
        self.path_edit = None;
        self.more_open = false;
        self.context_menu = None;
        if self.filtering.take().is_some() {
            self.filter_text.clear();
        }
        self.queue.cancel_speed();
    }

    pub(crate) fn toggle_terminal(&mut self) {
        if self.drawer.open {
            self.drawer.open = false;
            self.drawer.focused = false;
        } else {
            self.drawer.open = true;
            self.drawer.focused = self.drawer.tab.is_some();
            if self.drawer.focused {
                self.dismiss_inline_editors();
            }
        }
    }

    pub(in crate::app) fn focus_terminal(&mut self) {
        if self.drawer.tab.is_some() || self.drawer.wanted.is_some() {
            self.dismiss_inline_editors();
            self.drawer.open = true;
            self.drawer.focused = true;
        }
    }

    pub(in crate::app) fn terminal_resized(&mut self, id: TerminalId, size: TermSize) {
        if let Some(tab) = self.drawer.tab.as_mut().filter(|tab| tab.id == id && tab.size != size) {
            tab.size = size;
            if let Some(session) = &tab.session {
                session.resize(size);
            }
        }
    }

    pub(in crate::app) fn close_terminal(&mut self) {
        self.drawer.tab = None;
        self.drawer.wanted = None;
        self.drawer.open = false;
        self.drawer.focused = false;
    }

    pub(in crate::app) fn terminal_subscription(&self) -> Subscription<Message> {
        match &self.drawer.tab {
            Some(tab) => tab.subscription(),
            None => Subscription::none(),
        }
    }
}

#[cfg(test)]
mod tests;
