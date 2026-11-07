use std::collections::HashMap;

use iced::{Size, window};

use crate::main_window;

pub const SETTINGS_WINDOW_SIZE: Size = Size::new(760.0, 600.0);
pub const CONNECTIONS_WINDOW_SIZE: Size = Size::new(860.0, 560.0);
pub const SEARCH_WINDOW_SIZE: Size = Size::new(720.0, 560.0);
const SEARCH_MINIMUM: Size = Size::new(480.0, 360.0);
const SETTINGS_MINIMUM: Size = Size::new(420.0, 320.0);
const CONNECTIONS_MINIMUM: Size = Size::new(420.0, 320.0);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum WindowKind {
    Main,
    Settings,
    Connections,
    Search,
}

pub enum Opened {
    Created,
    Existing,
}

pub struct Windows {
    main: window::Id,
    open: HashMap<window::Id, (WindowKind, Size)>,
}

pub fn settings_for(kind: WindowKind) -> window::Settings {
    let (size, min_size) = match kind {
        WindowKind::Main => return main_window::settings(),
        WindowKind::Settings => (SETTINGS_WINDOW_SIZE, SETTINGS_MINIMUM),
        WindowKind::Connections => (CONNECTIONS_WINDOW_SIZE, CONNECTIONS_MINIMUM),
        WindowKind::Search => (SEARCH_WINDOW_SIZE, SEARCH_MINIMUM),
    };
    window::Settings { size, min_size: Some(min_size), ..main_window::settings() }
}

impl WindowKind {
    pub fn title(self) -> &'static str {
        match self {
            WindowKind::Main => "Porthmos",
            WindowKind::Settings => "Porthmos Settings",
            WindowKind::Connections => "Porthmos Connections",
            WindowKind::Search => "Porthmos Search",
        }
    }
}

impl Windows {
    pub fn new(main: window::Id, main_size: Size) -> Windows {
        Windows { main, open: HashMap::from([(main, (WindowKind::Main, main_size))]) }
    }

    pub fn main(&self) -> window::Id {
        self.main
    }

    pub fn kind_of(&self, id: window::Id) -> Option<WindowKind> {
        self.open.get(&id).map(|(kind, _)| *kind)
    }

    pub fn id_of(&self, kind: WindowKind) -> Option<window::Id> {
        self.open.iter().find_map(|(id, (open_kind, _))| (*open_kind == kind).then_some(*id))
    }

    pub fn open(&mut self, kind: WindowKind, create: impl FnOnce(window::Settings) -> window::Id) -> Opened {
        if self.id_of(kind).is_some() {
            return Opened::Existing;
        }
        let settings = settings_for(kind);
        let size = settings.size;
        let id = create(settings);
        self.open.insert(id, (kind, size));
        Opened::Created
    }

    pub fn close(&mut self, id: window::Id) -> Option<WindowKind> {
        if id == self.main {
            return None;
        }
        self.open.remove(&id).map(|(kind, _)| kind)
    }

    pub fn size_of(&self, id: window::Id) -> Option<Size> {
        self.open.get(&id).map(|(_, size)| *size)
    }

    pub fn set_size(&mut self, id: window::Id, size: Size) {
        if let Some(entry) = self.open.get_mut(&id) {
            entry.1 = size;
        }
    }

    pub fn secondary(&self) -> Vec<window::Id> {
        self.open.keys().copied().filter(|id| *id != self.main).collect()
    }
}

#[cfg(test)]
mod tests;
