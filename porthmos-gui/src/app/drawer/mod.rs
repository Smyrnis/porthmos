use iced::{Subscription, Task};
use porthmos_core::i18n::label as tr;
use porthmos_core::{Command, Handover, Message as Notice, SessionId, ShellSize, ShellStream, TerminalId, VfsPath};
use porthmos_core::{Severity, to_os_path};
use porthmos_term::{LocalShell, TermSize, TerminalConfig, TerminalEvent, TerminalSession, spawn_local};

use super::{App, Message};
use crate::drawer::keys::{KeyInput, TerminalChord, chord_of, translate};
use crate::drawer::pointer::{PointerKind, Route, route, selection_kind};
use crate::drawer::search::{Viewport, resume_point};
use crate::drawer::{
    Drawer, Gesture, SharedSession, TerminalSource, TerminalTab, Wanted, into_transport, opening_size,
};
use crate::shortcuts::KeyPress;
use crate::view::terminal_grid::PointerEvent;
use porthmos_term::{
    Key as TermKey, Modifiers as TermModifiers, MouseAction, MouseButton, MouseEvent, SearchDirection, encode_key,
    encode_mouse, encode_paste,
};

pub(in crate::app) const PRIMARY_SELECTION: bool = cfg!(target_os = "linux");
pub(in crate::app) const PASTE_CHUNK: usize = 16 * 1024;
const NOTCH_LINES: i32 = crate::drawer::pointer::WHEEL_LINES_PER_NOTCH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) enum ClipboardRequest {
    None,
    WritePrimary(String),
    ReadPrimary,
}

pub(in crate::app) fn paste_chunks(bytes: &[u8]) -> Vec<Vec<u8>> {
    bytes.chunks(PASTE_CHUNK).map(<[u8]>::to_vec).collect()
}

fn report_pointer(
    session: &SharedSession, event: PointerEvent, held: Option<MouseButton>, modes: porthmos_term::Modes,
    modifiers: TermModifiers,
) {
    let action = match event.kind {
        PointerKind::Press(button) => Some(MouseAction::Press(button)),
        PointerKind::Release(button) => Some(MouseAction::Release(button)),
        PointerKind::Move { .. } => Some(MouseAction::Move(held)),
        PointerKind::Wheel if event.lines > 0 => Some(MouseAction::WheelUp),
        PointerKind::Wheel if event.lines < 0 => Some(MouseAction::WheelDown),
        PointerKind::Wheel => None,
    };
    let repeats = if event.kind == PointerKind::Wheel { (event.lines.abs() / NOTCH_LINES).max(1) } else { 1 };
    if let Some(action) = action {
        let report = MouseEvent { action, column: event.point.column, row: event.point.row, modifiers };
        for _ in 0..repeats {
            if let Some(bytes) = encode_mouse(report, modes) {
                session.write(bytes);
            }
        }
    }
}

fn viewport_of(terminal: &porthmos_term::Terminal) -> Viewport {
    let size = terminal.size();
    Viewport {
        columns: size.columns,
        rows: size.rows,
        visible_top: -(terminal.display_offset() as i32),
        first_line: -(terminal.history() as i32),
    }
}

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
        self.drawer.gesture = Gesture::Idle;
        self.drawer.search.open = false;
        self.drawer.search.missed = false;
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
            && !self.inline_editors_open()
            && !self.drawer.search.open
    }

    pub(in crate::app) fn exited_terminal_chord(&mut self, press: &KeyPress) -> Option<Task<Message>> {
        let exited = self.drawer.tab.as_ref().is_some_and(|tab| tab.exited.is_some());
        let free = self.drawer.open
            && self.drawer.focused
            && exited
            && self.modals.is_empty()
            && self.tool.is_none()
            && !self.drag.is_tracking()
            && !self.inline_editors_open();
        if !free {
            return None;
        }
        match chord_of(press)? {
            TerminalChord::Copy => Some(self.terminal_copy()),
            TerminalChord::Paste => Some(Task::none()),
            TerminalChord::Search => Some(self.open_terminal_search()),
            chord @ (TerminalChord::PageUp | TerminalChord::PageDown) => {
                self.scroll_page(chord == TerminalChord::PageUp);
                Some(Task::none())
            }
            TerminalChord::Focus => None,
        }
    }

    pub(in crate::app) fn terminal_key(&mut self, press: &KeyPress) -> Task<Message> {
        match chord_of(press) {
            Some(TerminalChord::Focus) => {
                self.drawer.focused = false;
                return Task::none();
            }
            Some(TerminalChord::Copy) => return self.terminal_copy(),
            Some(TerminalChord::Paste) => return self.terminal_paste(),
            Some(TerminalChord::Search) => return self.open_terminal_search(),
            Some(chord @ (TerminalChord::PageUp | TerminalChord::PageDown)) => {
                self.scroll_page(chord == TerminalChord::PageUp);
                return Task::none();
            }
            None => {}
        }
        let Some(tab) = self.drawer.tab.as_ref() else {
            return Task::none();
        };
        let Some(session) = tab.session.as_ref() else {
            return Task::none();
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
            session.with_terminal(|terminal| {
                terminal.scroll_to_bottom();
                terminal.clear_selection();
            });
            session.write(bytes);
        }
        Task::none()
    }

    fn scroll_page(&mut self, up: bool) {
        let Some(session) = self.drawer.tab.as_ref().and_then(|tab| tab.session.clone()) else {
            return;
        };
        session.with_terminal(|terminal| {
            let page = i32::from(terminal.size().rows.saturating_sub(1).max(1));
            terminal.scroll(if up { page } else { -page });
        });
    }

    pub(in crate::app) fn terminal_copy_text(&self) -> Option<String> {
        let session = self.drawer.tab.as_ref()?.session.as_ref()?;
        session.with_terminal(|terminal| terminal.selected_text()).filter(|text| !text.is_empty())
    }

    pub(in crate::app) fn terminal_copy(&mut self) -> Task<Message> {
        match self.terminal_copy_text() {
            Some(text) => iced::clipboard::write(text),
            None => Task::none(),
        }
    }

    pub(in crate::app) fn terminal_paste(&mut self) -> Task<Message> {
        let Some(id) = self.drawer.tab.as_ref().filter(|tab| tab.exited.is_none()).map(|tab| tab.id) else {
            return Task::none();
        };
        iced::clipboard::read().map(move |text| Message::TerminalPasted(id, text))
    }

    fn inline_editors_open(&self) -> bool {
        self.path_edit.is_some()
            || self.filtering.is_some()
            || self.context_menu.is_some()
            || self.more_open
            || self.queue.speed_text().is_some()
    }

    pub(in crate::app) fn terminal_pointer(&mut self, id: TerminalId, event: PointerEvent) -> Task<Message> {
        match self.route_pointer(id, event) {
            ClipboardRequest::None => Task::none(),
            ClipboardRequest::WritePrimary(text) => iced::clipboard::write_primary(text),
            ClipboardRequest::ReadPrimary => {
                iced::clipboard::read_primary().map(move |text| Message::TerminalPasted(id, text))
            }
        }
    }

    pub(in crate::app) fn route_pointer(&mut self, id: TerminalId, event: PointerEvent) -> ClipboardRequest {
        let Some(tab) = self.drawer.tab.as_ref().filter(|tab| tab.id == id) else {
            return ClipboardRequest::None;
        };
        let Some(session) = tab.session.clone() else {
            return ClipboardRequest::None;
        };
        let live = tab.exited.is_none();
        let modes = session.with_terminal(|terminal| terminal.modes());
        let modifiers = TermModifiers {
            shift: self.modifiers.shift(),
            control: self.modifiers.control(),
            alt: self.modifiers.alt(),
        };
        let (routed, held) = match event.kind {
            PointerKind::Press(button) => {
                self.close_terminal_search();
                let accepting = self.drawer.open && self.drawer.focused && !self.inline_editors_open();
                let routed = route(event.kind, modes, modifiers.shift, accepting);
                self.drawer.gesture = match routed {
                    Route::Report if live => Gesture::Reported(button),
                    Route::Select => Gesture::Selecting { point: event.point, side: event.side, clicks: event.clicks },
                    _ => Gesture::Ignored(button),
                };
                (routed, Some(button))
            }
            PointerKind::Move { .. } | PointerKind::Release(_) => match self.drawer.gesture {
                Gesture::Idle => match event.kind {
                    PointerKind::Move { dragging: false } if self.drawer.open && self.drawer.focused => {
                        (route(event.kind, modes, modifiers.shift, true), None)
                    }
                    _ => (Route::Ignore, None),
                },
                Gesture::Ignored(button) => {
                    if event.kind == PointerKind::Release(button) {
                        self.drawer.gesture = Gesture::Idle;
                    }
                    (Route::Ignore, None)
                }
                Gesture::Reported(button) => match event.kind {
                    PointerKind::Release(other) if other != button => (Route::Ignore, None),
                    PointerKind::Release(_) => {
                        self.drawer.gesture = Gesture::Idle;
                        (if live { Route::Report } else { Route::Ignore }, Some(button))
                    }
                    _ => (if live { Route::Report } else { Route::Ignore }, Some(button)),
                },
                Gesture::Selecting { .. } => match event.kind {
                    PointerKind::Release(MouseButton::Left) | PointerKind::Move { .. } => (Route::Select, None),
                    _ => (Route::Ignore, None),
                },
            },
            PointerKind::Wheel => {
                let accepting = self.drawer.open && self.drawer.focused && !self.inline_editors_open();
                (route(event.kind, modes, modifiers.shift, accepting), None)
            }
        };
        match routed {
            Route::FocusOnly => self.focus_terminal(),
            Route::Report if live => report_pointer(&session, event, held, modes, modifiers),
            Route::Select => return self.select_with_pointer(&session, event, modifiers.alt),
            Route::Scroll => session.with_terminal(|terminal| terminal.scroll(event.lines)),
            Route::AlternateScroll if live => {
                let key = if event.lines > 0 { TermKey::Up } else { TermKey::Down };
                let none = TermModifiers { shift: false, control: false, alt: false };
                if let Some(arrow) = encode_key(key, none, modes) {
                    let count = event.lines.unsigned_abs() as usize;
                    session.write(arrow.repeat(count));
                }
            }
            Route::Paste if live => return ClipboardRequest::ReadPrimary,
            _ => {}
        }
        ClipboardRequest::None
    }

    fn select_with_pointer(&mut self, session: &SharedSession, event: PointerEvent, alt: bool) -> ClipboardRequest {
        match event.kind {
            PointerKind::Press(_) => {
                session.with_terminal(|terminal| {
                    terminal.start_selection(selection_kind(event.clicks, alt), event.point, event.side)
                });
            }
            PointerKind::Move { .. } => {
                session.with_terminal(|terminal| terminal.update_selection(event.point, event.side));
            }
            PointerKind::Release(_) => {
                let gesture = std::mem::take(&mut self.drawer.gesture);
                if gesture == (Gesture::Selecting { point: event.point, side: event.side, clicks: 1 }) {
                    session.with_terminal(|terminal| terminal.clear_selection());
                    return ClipboardRequest::None;
                }
                let text = session.with_terminal(|terminal| terminal.selected_text());
                if let Some(text) = text.filter(|text| !text.is_empty() && PRIMARY_SELECTION) {
                    return ClipboardRequest::WritePrimary(text);
                }
            }
            PointerKind::Wheel => {}
        }
        ClipboardRequest::None
    }

    pub(in crate::app) fn terminal_pasted(&mut self, id: TerminalId, text: Option<String>) {
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            return;
        };
        let Some(session) = self
            .drawer
            .tab
            .as_ref()
            .filter(|tab| tab.id == id && tab.exited.is_none())
            .and_then(|tab| tab.session.clone())
        else {
            return;
        };
        let bytes = session.with_terminal(|terminal| {
            terminal.scroll_to_bottom();
            terminal.clear_selection();
            encode_paste(&text, terminal.modes())
        });
        for chunk in paste_chunks(&bytes) {
            session.write(chunk);
        }
    }

    fn dismiss_inline_editors(&mut self) {
        self.drawer.search.open = false;
        self.path_edit = None;
        self.more_open = false;
        self.context_menu = None;
        if self.filtering.take().is_some() {
            self.filter_text.clear();
        }
        self.queue.cancel_speed();
    }

    pub(in crate::app) fn open_terminal_search(&mut self) -> Task<Message> {
        if self.drawer.tab.is_none() {
            return Task::none();
        }
        self.dismiss_inline_editors();
        self.drawer.open = true;
        self.drawer.focused = true;
        self.drawer.search.open = true;
        iced::widget::operation::focus(Self::terminal_search_id())
    }

    pub(in crate::app) fn close_terminal_search(&mut self) {
        if self.drawer.search.open {
            self.drawer.search.open = false;
            self.drawer.focused = self.drawer.tab.is_some();
        }
    }

    pub(in crate::app) fn terminal_search_typed(&mut self, query: String) {
        self.drawer.search.query = query;
        self.run_terminal_search(SearchDirection::Forward, false);
    }

    pub(in crate::app) fn terminal_search_step(&mut self, direction: SearchDirection) -> Task<Message> {
        self.run_terminal_search(direction, true);
        iced::widget::operation::focus(Self::terminal_search_id())
    }

    fn run_terminal_search(&mut self, direction: SearchDirection, step: bool) {
        let query = self.drawer.search.query.clone();
        let Some(session) = self.drawer.tab.as_ref().and_then(|tab| tab.session.clone()) else {
            return;
        };
        let found = session.with_terminal(|terminal| {
            if query.is_empty() {
                terminal.clear_selection();
                return None;
            }
            let view = viewport_of(terminal);
            let current = terminal.selection_match();
            let from = match (step, current) {
                (false, Some(current)) => current.start,
                _ => resume_point(current.filter(|_| step), direction, view),
            };
            let found = terminal.search(&query, from, direction);
            match found {
                Some(found) => terminal.select_match(found),
                None => terminal.clear_selection(),
            }
            found
        });
        self.drawer.search.missed = !query.is_empty() && found.is_none();
    }

    pub(crate) fn toggle_terminal(&mut self) {
        if self.drawer.open {
            self.drawer.open = false;
            self.drawer.focused = false;
            self.drawer.search.open = false;
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
        self.drawer.search.open = false;
        self.drawer.search.missed = false;
        self.drawer.gesture = Gesture::Idle;
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
