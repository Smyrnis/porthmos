use super::Screen;
use crate::{
    input::{Action, KeyBindings, format_key_spec},
    widgets::frame::{Hint, Hints},
};

const FILES: &[(Action, &str)] = &[
    (Action::Copy, "copy"),
    (Action::Mkdir, "mkdir"),
    (Action::Delete, "delete"),
    (Action::OpenConnections, "connections"),
    (Action::Quit, "quit"),
    (Action::Edit, "edit"),
    (Action::Rename, "rename"),
    (Action::Filter, "filter"),
    (Action::ToggleSelect, "select"),
    (Action::SwitchPanel, "panel"),
];
const CONNECTIONS: &[(Action, &str)] = &[
    (Action::Open, "connect"),
    (Action::AddConnection, "new"),
    (Action::Rename, "edit"),
    (Action::DeleteConnection, "delete"),
    (Action::Delete, "disconnect"),
];
const HISTORY: &[(Action, &str)] = &[(Action::Open, "details"), (Action::Delete, "clear")];
const TRANSFERS: &[(Action, &str)] =
    &[(Action::Open, "retry"), (Action::Delete, "cancel"), (Action::Refresh, "clear done")];
const SYNC: &[(Action, &str)] = &[(Action::Open, "run"), (Action::ToggleSelect, "tick")];

fn entries_of(screen: Screen) -> &'static [(Action, &'static str)] {
    match screen {
        Screen::Files => FILES,
        Screen::Connections => CONNECTIONS,
        Screen::History => HISTORY,
        Screen::Sync => SYNC,
        Screen::Transfers => TRANSFERS,
        Screen::Search => &[],
    }
}

fn pair(bindings: &KeyBindings, action: Action, label: &'static str) -> Option<Hint> {
    bindings.key_for(action).map(|spec| Hint { key: format_key_spec(spec), label })
}

pub(super) fn hints_for(screen: Screen, bindings: &KeyBindings) -> Hints {
    Hints {
        entries: entries_of(screen).iter().filter_map(|(action, label)| pair(bindings, *action, label)).collect(),
        pinned: pair(bindings, Action::Help, "help"),
        right: if screen == Screen::Files { None } else { pair(bindings, Action::Back, "back") },
    }
}

#[cfg(test)]
mod tests;
