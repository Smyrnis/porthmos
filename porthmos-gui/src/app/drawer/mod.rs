use iced::{Subscription, Task};
use porthmos_core::i18n::label as tr;
use porthmos_core::{
    Command, Handover, Location, Message as Notice, SessionId, ShellSize, ShellStream, TerminalId, VfsPath,
};
use porthmos_core::{Severity, to_os_path};
use porthmos_term::{
    DirectoryOwner, DirectoryTracker, LocalShell, TermSize, TerminalConfig, TerminalEvent, TerminalSession, spawn_local,
};

use super::editing::JobRole;
use super::{App, Message};
use crate::drawer::keys::{KeyInput, TerminalChord, chord_of, translate};
use crate::drawer::layout::{Direction, Layout, Removal, Toward};
use crate::drawer::links::{Link, is_clickable, link_at, needs_confirmation};
use crate::drawer::paste::needs_paste_confirmation;
use crate::drawer::pointer::{PointerKind, Route, route, selection_kind};
use crate::drawer::search::{SearchState, Viewport, resume_point};
use crate::drawer::{
    Drawer, DrawerView, Gesture, Placement, SharedSession, TerminalPane, TerminalSource, TerminalTab, Wanted,
    dropped_text, into_transport, opening_size,
};
use crate::geometry::drawer_contains;
use crate::modal::Modal;
use crate::processes::ProcessJob;
use crate::shortcuts::KeyPress;
use crate::transfers::queue_height;
use crate::view::terminal_grid::PointerEvent;
use porthmos_core::edit::EditorExit;
use porthmos_core::edit::launch::url_spec;
use porthmos_term::ViewPoint;
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

fn local_names() -> &'static [String] {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(porthmos_term::local_host_names)
}

fn directory_owner(source: TerminalSource) -> DirectoryOwner {
    match source {
        TerminalSource::Local => DirectoryOwner::Local,
        TerminalSource::Remote(_) => DirectoryOwner::Remote,
    }
}

fn change_directory_command(directory: &VfsPath, with_enter: bool) -> Option<Vec<u8>> {
    let path = directory.to_string_lossy();
    if path.chars().any(|character| character.is_control() || character == char::REPLACEMENT_CHARACTER) {
        return None;
    }
    let ending = if with_enter { "\r" } else { "" };
    Some(format!("cd -- {}{ending}", porthmos_ssh::shell_quote(&path)).into_bytes())
}

impl App {
    pub fn drawer(&self) -> &Drawer {
        &self.drawer
    }

    pub(crate) fn open_embedded_terminal(&mut self, source: TerminalSource) {
        self.open_pane(source, Placement::NewTab);
    }

    pub(in crate::app) fn split_terminal(&mut self, direction: Direction) {
        let Some(tab) = self.drawer.active_tab() else {
            return;
        };
        let (tab, target) = (tab.id, tab.focus);
        let Some(source) = self.drawer.pane(target).map(|pane| pane.source) else {
            return;
        };
        self.open_pane(source, Placement::Split { tab, target, direction });
    }

    fn open_pane(&mut self, source: TerminalSource, placement: Placement) {
        let directory = match source {
            TerminalSource::Local => self.local.panel.path().clone(),
            TerminalSource::Remote(session) => match self.sessions.by_id(session) {
                Some(session) => session.pane.panel.path().clone(),
                None => return self.push_notice(Severity::Warning, tr("Connect to a server first")),
            },
        };
        self.dismiss_inline_editors();
        let id = self.drawer.allocate_id();
        self.drawer.view = DrawerView::Terminals;
        self.drawer.open = true;
        self.drawer.focused = true;
        let wanted = Wanted { id, source, directory, placement };
        match source {
            TerminalSource::Remote(session) => {
                self.drawer.wanted.push(wanted);
                self.core.send(Command::OpenTerminal { id, session, size: shell_size(opening_size()) });
            }
            TerminalSource::Local => self.start_local_terminal(wanted),
        }
    }

    pub(in crate::app) fn start_local_terminal(&mut self, wanted: Wanted) {
        let shell = LocalShell { directory: Some(to_os_path(&wanted.directory)), ..LocalShell::default() };
        match spawn_local(&shell, opening_size()) {
            Ok(transport) => self.start_pane(wanted, transport),
            Err(error) => self.push_notice(
                Severity::Error,
                porthmos_core::i18n::text_with("gui.terminal.open_failed", &[("error", &error.to_string())]),
            ),
        }
    }

    fn start_pane(&mut self, wanted: Wanted, transport: porthmos_term::Transport) {
        let position = match wanted.placement {
            Placement::NewTab => None,
            Placement::Split { tab, target, .. } => {
                match self
                    .drawer
                    .tabs
                    .iter()
                    .position(|candidate| candidate.id == tab && candidate.layout.contains(target))
                {
                    Some(position) => Some(position),
                    None => return,
                }
            }
        };
        let size = opening_size();
        let (session, events) = TerminalSession::spawn(transport, size, TerminalConfig::default());
        if matches!(wanted.source, TerminalSource::Remote(_)) {
            match change_directory_command(&wanted.directory, true) {
                Some(command) => session.write(command),
                None => self.push_notice(Severity::Warning, porthmos_core::i18n::text("gui.terminal.folder_unsafe")),
            }
        }
        self.drawer.panes.push(TerminalPane {
            id: wanted.id,
            source: wanted.source,
            title: String::new(),
            session: Some(SharedSession::new(session)),
            frozen: None,
            events: Handover::new(events),
            exited: None,
            size,
            gesture: Gesture::Idle,
            search: SearchState::default(),
            directory: DirectoryTracker::new(directory_owner(wanted.source)),
            hover: None,
        });
        match (wanted.placement, position) {
            (Placement::Split { target, direction, .. }, Some(position)) => {
                if position == self.drawer.active {
                    self.leave_active_pane();
                }
                let tab = &mut self.drawer.tabs[position];
                tab.layout.split(target, wanted.id, direction);
                tab.focus = wanted.id;
            }
            _ => {
                self.drawer.tabs.push(TerminalTab { id: wanted.id, layout: Layout::Leaf(wanted.id), focus: wanted.id });
                self.drawer.active = self.drawer.tabs.len() - 1;
            }
        }
    }

    pub(in crate::app) fn terminal_opened(&mut self, id: TerminalId, stream: Handover<ShellStream>) {
        let Some(stream) = stream.take() else {
            return;
        };
        let Some(position) = self.drawer.wanted.iter().position(|wanted| wanted.id == id) else {
            return;
        };
        let wanted = self.drawer.wanted.remove(position);
        self.start_pane(wanted, into_transport(stream));
    }

    pub(in crate::app) fn terminal_open_failed(&mut self, id: TerminalId, message: Notice) {
        if let Some(position) = self.drawer.wanted.iter().position(|wanted| wanted.id == id) {
            self.drawer.wanted.remove(position);
            self.push_notice(Severity::Error, message);
        }
    }

    pub(in crate::app) fn terminal_event(&mut self, id: TerminalId, event: TerminalEvent) {
        let Some(pane) = self.drawer.pane_mut(id) else {
            return;
        };
        match event {
            TerminalEvent::Title(title) => pane.title = title,
            TerminalEvent::Exited(status) => pane.exited = Some(status),
            TerminalEvent::WorkingDirectory(report) => pane.directory.update(&report, local_names()),
            _ => {}
        }
    }

    pub(in crate::app) fn terminal_session_ended(&mut self, session: SessionId) {
        for pane in self.drawer.panes.iter_mut().filter(|pane| pane.source == TerminalSource::Remote(session)) {
            pane.end();
        }
        self.drawer.wanted.retain(|wanted| wanted.source != TerminalSource::Remote(session));
    }

    pub(in crate::app) fn terminal_has_keys(&self) -> bool {
        self.drawer.open
            && self.drawer.view == DrawerView::Terminals
            && self.drawer.focused
            && (!self.drawer.wanted.is_empty() || self.drawer.active_pane().is_some_and(|pane| pane.exited.is_none()))
            && self.modals.is_empty()
            && self.tool.is_none()
            && !self.drag.is_tracking()
            && !self.inline_editors_open()
            && !self.drawer.search().open
    }

    pub(in crate::app) fn exited_terminal_chord(&mut self, press: &KeyPress) -> Option<Task<Message>> {
        let exited = self.drawer.active_pane().is_some_and(|pane| pane.exited.is_some());
        let free = self.drawer.open
            && self.drawer.view == DrawerView::Terminals
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
            chord if chord.is_layout() => {
                self.layout_chord(chord);
                Some(Task::none())
            }
            _ => None,
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
            Some(chord) if chord.is_layout() => {
                self.layout_chord(chord);
                return Task::none();
            }
            Some(_) | None => {}
        }
        let Some(pane) = self.drawer.active_pane() else {
            return Task::none();
        };
        let Some(session) = pane.session.as_ref() else {
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
        let Some(session) = self.drawer.active_pane().and_then(|pane| pane.session.clone()) else {
            return;
        };
        session.with_terminal(|terminal| {
            let page = i32::from(terminal.size().rows.saturating_sub(1).max(1));
            terminal.scroll(if up { page } else { -page });
        });
    }

    pub(in crate::app) fn terminal_copy_text(&self) -> Option<String> {
        let session = self.drawer.active_pane()?.session.as_ref()?;
        session.with_terminal(|terminal| terminal.selected_text()).filter(|text| !text.is_empty())
    }

    pub(in crate::app) fn terminal_copy(&mut self) -> Task<Message> {
        match self.terminal_copy_text() {
            Some(text) => iced::clipboard::write(text),
            None => Task::none(),
        }
    }

    pub(in crate::app) fn terminal_paste(&mut self) -> Task<Message> {
        let Some(id) = self.drawer.active_pane().filter(|pane| pane.exited.is_none()).map(|pane| pane.id) else {
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
        if self.drawer.pane(id).is_none() {
            return ClipboardRequest::None;
        }
        let mut event = event;
        if let PointerKind::Press(button) = event.kind {
            if self.drawer.click_offset > 0 {
                event.clicks = if event.clicks <= 1 {
                    self.drawer.click_offset = 0;
                    1
                } else {
                    event.clicks.saturating_sub(self.drawer.click_offset).max(1)
                };
            }
            if self.drawer.active_tab().is_some_and(|tab| tab.focus != id) {
                self.focus_pane(id);
                self.set_gesture(id, Gesture::Ignored(button));
                self.drawer.click_offset = 1;
                return ClipboardRequest::None;
            }
        }
        let Some(pane) = self.drawer.pane(id) else {
            return ClipboardRequest::None;
        };
        let Some(session) = pane.session.clone() else {
            return ClipboardRequest::None;
        };
        let live = pane.exited.is_none();
        let modes = session.with_terminal(|terminal| terminal.modes());
        let modifiers = TermModifiers {
            shift: self.modifiers.shift(),
            control: self.modifiers.control(),
            alt: self.modifiers.alt(),
        };
        if event.kind == (PointerKind::Move { dragging: false }) {
            self.update_link_hover(id, event.point);
        }
        let (routed, held) = match event.kind {
            PointerKind::Press(button) => {
                self.close_terminal_search();
                let accepting = self.drawer.open && self.drawer.focused && !self.inline_editors_open();
                let mut routed = route(event.kind, modes, modifiers.shift, accepting);
                if button == MouseButton::Left
                    && routed == Route::Select
                    && self.click_link(id, event.point, event.clicks)
                {
                    routed = Route::Ignore;
                }
                let gesture = match routed {
                    Route::Report if live => Gesture::Reported(button),
                    Route::Select => Gesture::Selecting { point: event.point, side: event.side, clicks: event.clicks },
                    _ => Gesture::Ignored(button),
                };
                self.set_gesture(id, gesture);
                (routed, Some(button))
            }
            PointerKind::Move { .. } | PointerKind::Release(_) => match self.gesture_of(id) {
                Gesture::Idle => match event.kind {
                    PointerKind::Move { dragging: false } if self.drawer.open && self.drawer.focused => {
                        (route(event.kind, modes, modifiers.shift, true), None)
                    }
                    _ => (Route::Ignore, None),
                },
                Gesture::Ignored(button) => {
                    if event.kind == PointerKind::Release(button) {
                        self.set_gesture(id, Gesture::Idle);
                    }
                    (Route::Ignore, None)
                }
                Gesture::Reported(button) => match event.kind {
                    PointerKind::Release(other) if other != button => (Route::Ignore, None),
                    PointerKind::Release(_) => {
                        self.set_gesture(id, Gesture::Idle);
                        (if live { Route::Report } else { Route::Ignore }, Some(button))
                    }
                    _ => (if live { Route::Report } else { Route::Ignore }, Some(button)),
                },
                Gesture::Selecting { .. } => match event.kind {
                    PointerKind::Release(MouseButton::Left) | PointerKind::Move { dragging: true } => {
                        (Route::Select, None)
                    }
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
            Route::Select => return self.select_with_pointer(id, &session, event, modifiers.alt),
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

    fn select_with_pointer(
        &mut self, id: TerminalId, session: &SharedSession, event: PointerEvent, alt: bool,
    ) -> ClipboardRequest {
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
                let gesture = self.take_gesture(id);
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
            .panes
            .iter()
            .find(|pane| pane.id == id && pane.exited.is_none())
            .and_then(|pane| pane.session.clone())
        else {
            return;
        };
        let bracketed = session.with_terminal(|terminal| terminal.modes().bracketed_paste);
        if needs_paste_confirmation(&text, bracketed) {
            self.modals.push(Modal::ConfirmPaste { terminal: id, text });
        } else {
            self.write_paste(id, text);
        }
    }

    pub(in crate::app) fn write_paste(&mut self, id: TerminalId, text: String) {
        let Some(session) = self
            .drawer
            .panes
            .iter()
            .find(|pane| pane.id == id && pane.exited.is_none())
            .and_then(|pane| pane.session.clone())
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

    fn link_under(&self, id: TerminalId, point: ViewPoint) -> Option<Link> {
        if !self.modifiers.control() {
            return None;
        }
        let snapshot = self.drawer.pane(id)?.snapshot()?;
        link_at(&snapshot, point).filter(is_clickable)
    }

    fn update_link_hover(&mut self, id: TerminalId, point: ViewPoint) {
        let link = self.link_under(id, point);
        if let Some(pane) = self.drawer.pane_mut(id) {
            pane.hover = link;
        }
    }

    pub(in crate::app) fn clear_link_hover_without_ctrl(&mut self) {
        if !self.modifiers.control() {
            for pane in &mut self.drawer.panes {
                pane.hover = None;
            }
        }
    }

    fn click_link(&mut self, id: TerminalId, point: ViewPoint, clicks: u8) -> bool {
        let Some(link) = self.link_under(id, point) else {
            return false;
        };
        if clicks > 1 {
            return true;
        }
        if needs_confirmation(&link) {
            self.modals.push(Modal::OpenLink { target: link.target });
        } else {
            self.open_link(link.target);
        }
        true
    }

    pub(in crate::app) fn open_link(&mut self, target: String) {
        let spec = url_spec(&target, self.launcher.platform());
        let token = self.next_process_token;
        self.next_process_token += 1;
        self.jobs.insert(token, JobRole::Link);
        self.launcher.spawn(ProcessJob { token, spec, env: Vec::new(), cleanup: Vec::new() });
    }

    pub(in crate::app) fn link_finished(&mut self, exit: EditorExit) {
        if exit != EditorExit::Success {
            self.push_notice(Severity::Error, tr("Couldn't open the link"));
        }
    }

    pub(in crate::app) fn drop_into_terminal(&mut self, at: iced::Point, paths: &[std::path::PathBuf]) -> bool {
        let queue_below = if self.queue.is_open() { queue_height(self.window_size.height) + 1.0 } else { 0.0 };
        if !self.drawer.open || !drawer_contains(self.window_size, at, queue_below) {
            return false;
        }
        if self.drawer.view != DrawerView::Terminals {
            return true;
        }
        let Some(pane) = self.drawer.active_pane().filter(|pane| pane.exited.is_none()) else {
            return true;
        };
        if matches!(pane.source, TerminalSource::Remote(_)) {
            self.push_notice(Severity::Warning, porthmos_core::i18n::text("gui.terminal.drop_remote"));
            return true;
        }
        let Some(session) = pane.session.clone() else {
            return true;
        };
        let Some(text) = dropped_text(paths) else {
            self.push_notice(Severity::Warning, porthmos_core::i18n::text("gui.terminal.drop_unsafe"));
            return true;
        };
        let bytes = session.with_terminal(|terminal| {
            terminal.scroll_to_bottom();
            terminal.clear_selection();
            encode_paste(&text, terminal.modes())
        });
        for chunk in paste_chunks(&bytes) {
            session.write(chunk);
        }
        true
    }

    fn gesture_of(&self, id: TerminalId) -> Gesture {
        self.drawer.pane(id).map_or(Gesture::Idle, |pane| pane.gesture)
    }

    fn set_gesture(&mut self, id: TerminalId, gesture: Gesture) {
        if let Some(pane) = self.drawer.pane_mut(id) {
            pane.gesture = gesture;
        }
    }

    fn take_gesture(&mut self, id: TerminalId) -> Gesture {
        let gesture = self.gesture_of(id);
        self.set_gesture(id, Gesture::Idle);
        gesture
    }

    pub(in crate::app) fn dismiss_inline_editors(&mut self) {
        if let Some(pane) = self.drawer.active_pane_mut() {
            pane.search.open = false;
        }
        self.path_edit = None;
        self.more_open = false;
        self.context_menu = None;
        if self.filtering.take().is_some() {
            self.filter_text.clear();
        }
        self.queue.cancel_speed();
    }

    pub(in crate::app) fn open_terminal_search(&mut self) -> Task<Message> {
        if self.drawer.active_pane().is_none() {
            return Task::none();
        }
        self.dismiss_inline_editors();
        self.drawer.view = DrawerView::Terminals;
        self.drawer.open = true;
        self.drawer.focused = true;
        if let Some(pane) = self.drawer.active_pane_mut() {
            pane.search.open = true;
        }
        iced::widget::operation::focus(Self::terminal_search_id())
    }

    pub(in crate::app) fn close_terminal_search(&mut self) {
        if self.drawer.search().open {
            if let Some(pane) = self.drawer.active_pane_mut() {
                pane.search.open = false;
            }
            self.drawer.focused = self.drawer.active_pane().is_some();
        }
    }

    pub(in crate::app) fn terminal_search_typed(&mut self, query: String) {
        if let Some(pane) = self.drawer.active_pane_mut() {
            pane.search.query = query;
        }
        self.run_terminal_search(SearchDirection::Forward, false);
    }

    pub(in crate::app) fn terminal_search_step(&mut self, direction: SearchDirection) -> Task<Message> {
        self.run_terminal_search(direction, true);
        iced::widget::operation::focus(Self::terminal_search_id())
    }

    fn run_terminal_search(&mut self, direction: SearchDirection, step: bool) {
        let query = self.drawer.search().query.clone();
        let Some(session) = self.drawer.active_pane().and_then(|pane| pane.session.clone()) else {
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
        if let Some(pane) = self.drawer.active_pane_mut() {
            pane.search.missed = !query.is_empty() && found.is_none();
        }
    }

    pub(crate) fn toggle_terminal(&mut self) {
        if self.drawer.open {
            self.drawer.open = false;
            self.drawer.focused = false;
            if let Some(pane) = self.drawer.active_pane_mut() {
                pane.search.open = false;
            }
        } else {
            self.drawer.open = true;
            self.drawer.focused = self.drawer.view == DrawerView::Terminals && self.drawer.active_pane().is_some();
            if self.drawer.focused {
                self.dismiss_inline_editors();
            }
        }
    }

    pub(in crate::app) fn focus_terminal(&mut self) {
        if self.drawer.active_pane().is_some() || !self.drawer.wanted.is_empty() {
            self.dismiss_inline_editors();
            self.drawer.view = DrawerView::Terminals;
            self.drawer.open = true;
            self.drawer.focused = true;
        }
    }

    pub(in crate::app) fn terminal_resized(&mut self, id: TerminalId, size: TermSize) {
        if let Some(pane) = self.drawer.pane_mut(id).filter(|pane| pane.size != size) {
            pane.size = size;
            if let Some(session) = &pane.session {
                session.resize(size);
            }
        }
    }

    fn directory_target(&self, source: TerminalSource) -> Option<(Location, VfsPath)> {
        match source {
            TerminalSource::Local => Some((Location::Local, self.local.panel.path().clone())),
            TerminalSource::Remote(id) => {
                let session = self.sessions.by_id(id)?;
                Some((Location::Session(id), session.pane.panel.path().clone()))
            }
        }
    }

    pub(in crate::app) fn terminal_new_tab_possible(&self) -> bool {
        match self.drawer.active_pane().map(|pane| pane.source) {
            Some(TerminalSource::Local) => true,
            Some(TerminalSource::Remote(id)) => self.sessions.by_id(id).is_some(),
            None => self.terminal_possible(),
        }
    }

    pub(in crate::app) fn terminal_directory_openable(&self) -> bool {
        self.drawer
            .active_pane()
            .is_some_and(|pane| pane.directory.directory().is_some() && self.directory_target(pane.source).is_some())
    }

    pub(in crate::app) fn terminal_directory_changeable(&self) -> bool {
        let windows_local = |source| cfg!(windows) && source == TerminalSource::Local;
        self.drawer.active_pane().is_some_and(|pane| {
            pane.exited.is_none()
                && pane.directory.directory().is_some()
                && !windows_local(pane.source)
                && self.directory_target(pane.source).is_some()
        })
    }

    pub(in crate::app) fn open_terminal_directory(&mut self) {
        let Some(pane) = self.drawer.active_pane() else {
            return;
        };
        let (Some(path), Some((location, _))) = (pane.directory.directory(), self.directory_target(pane.source)) else {
            return;
        };
        let path = VfsPath::new(path);
        self.navigate(location, path);
    }

    pub(in crate::app) fn change_terminal_directory(&mut self) {
        if !self.terminal_directory_changeable() {
            return;
        }
        let Some(pane) = self.drawer.active_pane() else {
            return;
        };
        let (Some(session), Some((_, path))) = (pane.session.clone(), self.directory_target(pane.source)) else {
            return;
        };
        match change_directory_command(&path, false) {
            Some(command) => session.write(command),
            None => self.push_notice(Severity::Warning, porthmos_core::i18n::text("gui.terminal.change_unsafe")),
        }
    }

    pub(in crate::app) fn layout_chord(&mut self, chord: TerminalChord) {
        match chord {
            TerminalChord::NewTab => self.new_terminal_tab(),
            TerminalChord::ClosePane => self.close_focused_pane(),
            TerminalChord::NextTab => self.step_terminal_tab(true),
            TerminalChord::PreviousTab => self.step_terminal_tab(false),
            TerminalChord::SplitRight => self.split_terminal(Direction::Right),
            TerminalChord::SplitDown => self.split_terminal(Direction::Down),
            TerminalChord::FocusLeft => self.focus_pane_toward(Toward::Left),
            TerminalChord::FocusRight => self.focus_pane_toward(Toward::Right),
            TerminalChord::FocusUp => self.focus_pane_toward(Toward::Up),
            TerminalChord::FocusDown => self.focus_pane_toward(Toward::Down),
            _ => {}
        }
    }

    pub(in crate::app) fn terminal_split_possible(&self) -> bool {
        self.drawer.active_pane().is_some() && self.terminal_new_tab_possible()
    }

    pub(in crate::app) fn terminal_has_panes_to_focus(&self) -> bool {
        self.drawer.active_tab().is_some_and(|tab| tab.layout.leaves().len() > 1)
    }

    pub(in crate::app) fn close_active_terminal_tab(&mut self) {
        if let Some(id) = self.drawer.active_tab().map(|tab| tab.id) {
            self.close_terminal_tab(id);
        }
    }

    pub(in crate::app) fn new_terminal_tab(&mut self) {
        match self.drawer.active_pane().map(|pane| pane.source) {
            Some(source) => self.open_embedded_terminal(source),
            None => self.request_terminal(),
        }
    }

    pub(in crate::app) fn select_terminal_tab(&mut self, id: TerminalId) {
        let Some(position) = self.drawer.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        self.leave_active_pane();
        self.drawer.active = position;
        self.drawer.view = DrawerView::Terminals;
        self.drawer.open = true;
        self.drawer.focused = true;
    }

    fn leave_active_pane(&mut self) {
        self.dismiss_inline_editors();
        if let Some(pane) = self.drawer.active_pane_mut() {
            pane.gesture = Gesture::Idle;
        }
    }

    pub(in crate::app) fn focus_pane(&mut self, id: TerminalId) {
        let Some(tab) = self.drawer.active_tab() else {
            return;
        };
        if !tab.layout.contains(id) {
            return;
        }
        if tab.focus != id {
            self.leave_active_pane();
            if let Some(tab) = self.drawer.active_tab_mut() {
                tab.focus = id;
            }
        }
        self.drawer.view = DrawerView::Terminals;
        self.drawer.open = true;
        self.drawer.focused = true;
    }

    pub(in crate::app) fn focus_pane_toward(&mut self, toward: Toward) {
        let Some(tab) = self.drawer.active_tab() else {
            return;
        };
        if let Some(neighbour) = tab.layout.neighbour(tab.focus, toward) {
            self.focus_pane(neighbour);
        }
    }

    pub(in crate::app) fn step_terminal_tab(&mut self, forward: bool) {
        let count = self.drawer.tabs.len();
        if count < 2 {
            return;
        }
        self.leave_active_pane();
        self.drawer.view = DrawerView::Terminals;
        self.drawer.open = true;
        self.drawer.focused = true;
        self.drawer.active =
            if forward { (self.drawer.active + 1) % count } else { (self.drawer.active + count - 1) % count };
    }

    pub(in crate::app) fn close_focused_pane(&mut self) {
        if let Some(focus) = self.drawer.active_tab().map(|tab| tab.focus) {
            self.close_terminal_pane(focus);
        }
    }

    pub(in crate::app) fn close_terminal_pane(&mut self, id: TerminalId) {
        let Some(position) = self.drawer.tab_index_of(id) else {
            return;
        };
        let tab = &mut self.drawer.tabs[position];
        match tab.layout.remove(id) {
            Removal::Missing => {}
            Removal::Last => {
                let tab = tab.id;
                self.close_terminal_tab(tab);
            }
            Removal::Removed { focus } => {
                if tab.focus == id {
                    tab.focus = focus;
                }
                self.drawer.panes.retain(|pane| pane.id != id);
                self.drawer
                    .wanted
                    .retain(|wanted| !matches!(wanted.placement, Placement::Split { target, .. } if target == id));
            }
        }
    }

    pub(in crate::app) fn close_terminal_tab(&mut self, id: TerminalId) {
        self.drawer
            .wanted
            .retain(|wanted| wanted.id != id && !matches!(wanted.placement, Placement::Split { tab, .. } if tab == id));
        let Some(position) = self.drawer.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let removed = self.drawer.tabs.remove(position);
        self.drawer.panes.retain(|pane| !removed.layout.contains(pane.id));
        if position < self.drawer.active {
            self.drawer.active -= 1;
        }
        self.drawer.active = self.drawer.active.min(self.drawer.tabs.len().saturating_sub(1));
        if self.drawer.tabs.is_empty() && self.drawer.wanted.is_empty() && self.drawer.view == DrawerView::Terminals {
            self.drawer.open = false;
            self.drawer.focused = false;
        }
    }

    pub(in crate::app) fn terminal_subscription(&self) -> Subscription<Message> {
        Subscription::batch(self.drawer.panes.iter().map(TerminalPane::subscription))
    }
}

#[cfg(test)]
mod tests;
