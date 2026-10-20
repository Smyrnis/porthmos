use std::path::{Path, PathBuf};

use porthmos_core::{Command, CoreHandle, Entry, Event, Location, config::settings::PanelSettings};
use ratatui::{Terminal, backend::TestBackend};
use tokio::sync::mpsc::UnboundedReceiver;

use super::App;
use crate::input::KeyBindings;

pub(crate) struct TestApp {
    pub(crate) app: App,
    pub(crate) commands: UnboundedReceiver<Command>,
}

pub(crate) fn test_app(path: &Path) -> TestApp {
    test_app_with_protocols(path, Vec::new())
}

pub(crate) fn test_app_with_protocols(path: &Path, protocols: Vec<porthmos_core::ProtocolInfo>) -> TestApp {
    let (core, commands) = CoreHandle::detached_with(protocols);
    let mut test = TestApp {
        app: App::new(core, path.to_path_buf(), &PanelSettings::default(), KeyBindings::defaults()),
        commands,
    };
    test.sent();
    test
}

pub(crate) fn entry(dir: &Path, name: &str, is_dir: bool) -> Entry {
    Entry { name: name.to_string(), path: dir.join(name), is_dir, size: 1, permissions: None }
}

impl TestApp {
    pub(crate) fn sent(&mut self) -> Vec<Command> {
        std::iter::from_fn(|| self.commands.try_recv().ok()).collect()
    }

    pub(crate) fn list_local(&mut self, entries: Vec<Entry>) {
        let path = self.app.local.path().to_path_buf();
        self.app.apply_core_event(Event::Listed { location: Location::Local, path, entries });
    }

    pub(crate) fn connect(&mut self, session: u64, name: &str) -> u64 {
        self.app.apply_core_event(Event::Connected { session, name: name.to_string(), shell_available: true });
        session
    }

    pub(crate) fn list_remote(&mut self, session: u64, path: &str, entries: Vec<Entry>) {
        self.app.apply_core_event(Event::Listed {
            location: Location::Session(session),
            path: PathBuf::from(path),
            entries,
        });
    }

    pub(crate) fn notification(&self) -> Option<String> {
        self.app.notifications.current().map(|notification| notification.message.clone())
    }
}

pub(crate) fn render_app(app: &App, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal
}

pub(crate) fn find_text(terminal: &Terminal<TestBackend>, needle: &str) -> Option<(u16, u16)> {
    let buffer = terminal.backend().buffer();
    let width = buffer.area.width;
    (0..buffer.area.height).find_map(|y| {
        let row: String = (0..width).map(|x| buffer[(x, y)].symbol().to_string()).collect();
        row.find(needle).map(|byte_offset| (row[..byte_offset].chars().count() as u16, y))
    })
}

pub(crate) fn cell_fg(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> ratatui::style::Color {
    terminal.backend().buffer()[(x, y)].fg
}

pub(crate) fn cell_bg(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> ratatui::style::Color {
    terminal.backend().buffer()[(x, y)].bg
}
