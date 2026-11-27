use iced::Color;
use porthmos_core::i18n;
use porthmos_core::{
    ActiveState, BootState, ServiceVerb, ServicesSnapshot, ServicesState as ServicesEvent, SessionId, StatusFilter,
    Unit, UnitName, arranged_units, verb_allowed,
};

use crate::theme::Palette;

const MISSING: &str = "\u{2014}";

pub const ALL_VERBS: [ServiceVerb; 5] =
    [ServiceVerb::Start, ServiceVerb::Stop, ServiceVerb::Restart, ServiceVerb::Enable, ServiceVerb::Disable];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionState {
    NoSelection,
    NotAvailable,
    Allowed,
}

#[derive(Debug)]
pub struct ServicesState {
    pub session: Option<SessionId>,
    pub started: Option<SessionId>,
    pub state: Option<ServicesEvent>,
    pub filter: String,
    pub status: StatusFilter,
    pub selected: Option<String>,
    pub status_text: Option<(String, String)>,
    status_pending: bool,
    kept: Option<Box<ServicesSnapshot>>,
}

impl Default for ServicesState {
    fn default() -> Self {
        Self {
            session: None,
            started: None,
            state: None,
            filter: String::new(),
            status: StatusFilter::All,
            selected: None,
            status_text: None,
            status_pending: false,
            kept: None,
        }
    }
}

impl ServicesState {
    pub fn begin(&mut self, session: SessionId) {
        self.clear();
        self.session = Some(session);
    }

    pub fn clear(&mut self) {
        self.session = None;
        self.state = None;
        self.kept = None;
        self.selected = None;
        self.status_text = None;
        self.status_pending = false;
    }

    pub fn apply(&mut self, session: SessionId, state: ServicesEvent) {
        if self.session != Some(session) {
            return;
        }
        if let Some(ServicesEvent::Snapshot(snapshot)) = self.state.take() {
            self.kept = Some(snapshot);
        }
        if !matches!(state, ServicesEvent::Failing(_)) {
            self.kept = None;
        }
        if !matches!(state, ServicesEvent::Snapshot(_)) && std::mem::take(&mut self.status_pending) {
            self.status_text = None;
        }
        if let (Some(name), ServicesEvent::Snapshot(snapshot)) = (&self.selected, &state)
            && !snapshot.units.iter().any(|unit| &unit.name == name)
        {
            self.selected = None;
            self.status_text = None;
        }
        self.state = Some(state);
    }

    pub fn snapshot(&self) -> Option<&ServicesSnapshot> {
        match &self.state {
            Some(ServicesEvent::Snapshot(snapshot)) => Some(snapshot),
            Some(ServicesEvent::Failing(_)) => self.kept.as_deref(),
            _ => None,
        }
    }

    pub fn failing(&self) -> bool {
        matches!(self.state, Some(ServicesEvent::Failing(_)))
    }

    pub fn select(&mut self, name: String) {
        self.status_text = None;
        self.status_pending = false;
        self.selected = if self.selected.as_ref() == Some(&name) { None } else { Some(name) };
    }

    pub fn status_requested(&mut self, unit: &str) {
        self.status_text = Some((unit.to_string(), i18n::text("gui.services.reading_status")));
        self.status_pending = true;
    }

    pub fn status_arrived(&mut self, session: SessionId, unit: &str, text: String) {
        if self.session == Some(session) && self.selected.as_deref() == Some(unit) {
            self.status_text = Some((unit.to_string(), text));
            self.status_pending = false;
        }
    }
}

pub fn shown_units<'a>(state: &ServicesState, snapshot: &'a ServicesSnapshot) -> Vec<&'a Unit> {
    arranged_units(&snapshot.units, &state.filter, state.status)
}

fn selected_unit<'a>(snapshot: &'a ServicesSnapshot, selected: Option<&str>) -> Option<&'a Unit> {
    let name = selected?;
    snapshot.units.iter().find(|unit| unit.name == name)
}

fn name_is_safe(unit: &Unit) -> bool {
    UnitName::new(&unit.name).is_some()
}

pub fn action_state(snapshot: &ServicesSnapshot, selected: Option<&str>, verb: ServiceVerb) -> ActionState {
    match selected_unit(snapshot, selected) {
        None => ActionState::NoSelection,
        Some(unit) if name_is_safe(unit) && verb_allowed(unit, verb) => ActionState::Allowed,
        Some(_) => ActionState::NotAvailable,
    }
}

pub fn status_allowed(snapshot: &ServicesSnapshot, selected: Option<&str>) -> bool {
    selected_unit(snapshot, selected).is_some_and(name_is_safe)
}

pub fn status_reason(snapshot: &ServicesSnapshot, selected: Option<&str>) -> Option<String> {
    let key = match selected_unit(snapshot, selected) {
        _ if status_allowed(snapshot, selected) => return None,
        None => "select_first",
        Some(_) => "unsafe_name",
    };
    Some(i18n::text(&format!("gui.services.{key}")))
}

pub fn unavailable_reason(snapshot: &ServicesSnapshot, selected: Option<&str>, verb: ServiceVerb) -> Option<String> {
    let key = match (selected_unit(snapshot, selected), action_state(snapshot, selected, verb)) {
        (_, ActionState::Allowed) => return None,
        (None, _) => "select_first",
        (Some(unit), _) if !name_is_safe(unit) => "unsafe_name",
        (Some(unit), _) if unit.boot == Some(BootState::Masked) || unit.load == "masked" => "masked",
        (Some(unit), _) if unit.load != "loaded" => "not_loaded",
        _ => "no_boot_change",
    };
    Some(i18n::text(&format!("gui.services.{key}")))
}

pub fn verb_blocked(state: &ServicesState, snapshot: &ServicesSnapshot, verb: ServiceVerb) -> Option<String> {
    if state.failing() {
        return Some(i18n::text("gui.services.retry_first"));
    }
    unavailable_reason(snapshot, state.selected.as_deref(), verb)
}

pub fn status_blocked(state: &ServicesState, snapshot: &ServicesSnapshot) -> Option<String> {
    if state.failing() {
        return Some(i18n::text("gui.services.retry_first"));
    }
    status_reason(snapshot, state.selected.as_deref())
}

pub fn active_text(state: ActiveState) -> String {
    let key = match state {
        ActiveState::Active => "active",
        ActiveState::Inactive => "inactive",
        ActiveState::Failed => "failed",
        ActiveState::Activating => "activating",
        ActiveState::Deactivating => "deactivating",
        ActiveState::Other => return MISSING.to_string(),
    };
    i18n::text(&format!("gui.services.active_{key}"))
}

pub fn boot_text(state: Option<BootState>) -> String {
    let key = match state {
        Some(BootState::Enabled) => "enabled",
        Some(BootState::Disabled) => "disabled",
        Some(BootState::Static) => "static",
        Some(BootState::Masked) => "masked",
        Some(BootState::Alias) => "alias",
        Some(BootState::Indirect) => "indirect",
        Some(BootState::Generated) => "generated",
        Some(BootState::Other) | None => return MISSING.to_string(),
    };
    i18n::text(&format!("gui.services.boot_{key}"))
}

pub fn row_color(unit: &Unit, selected: bool, dimmed: bool, palette: &Palette) -> Color {
    if dimmed {
        palette.fg_dim
    } else if selected {
        palette.fg_focus
    } else if unit.active == ActiveState::Failed {
        palette.err
    } else if unit.sub == "running" {
        palette.fg
    } else {
        palette.fg_muted
    }
}

#[cfg(test)]
pub(crate) mod tests;
