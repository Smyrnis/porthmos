use super::Screen;
use crate::{
    input::{ALL_ACTIONS, Action, KeyBindings, format_key_spec},
    widgets::{
        frame::{Hint, Hints},
        help::{HelpRow, HelpSection},
    },
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

pub(super) fn screen_actions(screen: Screen) -> &'static [(Action, &'static str)] {
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

pub(super) fn form_hints() -> Hints {
    Hints {
        entries: vec![
            Hint { key: "tab".to_string(), label: "next field" },
            Hint { key: "\u{2191}\u{2193}".to_string(), label: "field" },
            Hint { key: "ctrl+s".to_string(), label: "save" },
        ],
        pinned: None,
        right: Some(Hint { key: "esc".to_string(), label: "cancel" }),
    }
}

pub(super) fn search_hints() -> Hints {
    Hints {
        entries: vec![
            Hint { key: "\u{2191}\u{2193}".to_string(), label: "move" },
            Hint { key: "enter".to_string(), label: "open" },
        ],
        pinned: None,
        right: Some(Hint { key: "esc".to_string(), label: "close" }),
    }
}

pub(super) fn hints_for(screen: Screen, bindings: &KeyBindings) -> Hints {
    Hints {
        entries: screen_actions(screen).iter().filter_map(|(action, label)| pair(bindings, *action, label)).collect(),
        pinned: pair(bindings, Action::Help, "help"),
        right: if screen == Screen::Files { None } else { pair(bindings, Action::Back, "back") },
    }
}

const HELP_SCREENS: &[(&str, Screen)] = &[
    ("files", Screen::Files),
    ("connections", Screen::Connections),
    ("transfers", Screen::Transfers),
    ("history", Screen::History),
    ("sync", Screen::Sync),
];

pub(super) fn help_sections(bindings: &KeyBindings) -> Vec<HelpSection> {
    let key_of = |action: Action| bindings.key_for(action).map(format_key_spec);
    let mut sections: Vec<HelpSection> = HELP_SCREENS
        .iter()
        .map(|(title, screen)| HelpSection {
            title,
            rows: screen_actions(*screen)
                .iter()
                .map(|(action, label)| HelpRow { key: key_of(*action), action: label })
                .collect(),
        })
        .collect();

    let listed = |action: &Action| {
        HELP_SCREENS.iter().any(|(_, screen)| screen_actions(*screen).iter().any(|(listed, _)| listed == action))
    };
    let movement_key = match (key_of(Action::Up), key_of(Action::Down)) {
        (Some(up), Some(down)) => Some(format!("{up}/{down}")),
        _ => None,
    };
    let mut global = vec![HelpRow { key: movement_key, action: "move" }];
    global.extend(
        ALL_ACTIONS
            .iter()
            .filter(|action| !matches!(action, Action::Up | Action::Down) && !listed(action))
            .map(|action| HelpRow { key: key_of(*action), action: action.name() }),
    );
    sections.push(HelpSection { title: "global", rows: global });
    sections
}

#[cfg(test)]
mod tests;
