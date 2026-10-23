use super::Screen;
use crate::{
    input::{ALL_ACTIONS, Action, KeyBindings, SESSION_ACTIONS, Scope, format_key_spec},
    widgets::{
        frame::{Hint, Hints},
        help::{HelpRow, HelpSection},
    },
};

const FILES: &[(Action, &str)] = &[
    (Action::Copy, "copy"),
    (Action::Move, "move"),
    (Action::Mkdir, "mkdir"),
    (Action::Delete, "delete"),
    (Action::OpenConnections, "connections"),
    (Action::Quit, "quit"),
    (Action::Edit, "edit"),
    (Action::Preview, "preview"),
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
    (Action::Disconnect, "disconnect"),
];
const HISTORY: &[(Action, &str)] = &[(Action::Open, "details"), (Action::Delete, "clear")];
const TRANSFERS: &[(Action, &str)] = &[
    (Action::Pause, "pause"),
    (Action::RowUp, "move earlier"),
    (Action::RowDown, "move later"),
    (Action::Open, "retry"),
    (Action::Delete, "cancel"),
    (Action::Refresh, "clear done"),
    (Action::SpeedLimit, "limit"),
];
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

fn pair(bindings: &KeyBindings, scope: Scope, action: Action, label: &'static str) -> Option<Hint> {
    bindings.key_for(scope, action).map(|spec| Hint { key: format_key_spec(spec), label })
}

pub(super) fn form_hints() -> Hints {
    Hints {
        entries: vec![
            Hint { key: "tab".to_string(), label: "next field" },
            Hint { key: "\u{2191}\u{2193}".to_string(), label: "field" },
            Hint { key: "ctrl+t".to_string(), label: "test" },
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

const REORDER_LABELS: [&str; 2] = ["move earlier", "move later"];

fn merge_reorder(entries: Vec<Hint>) -> Vec<Hint> {
    let (reorder, mut rest): (Vec<Hint>, Vec<Hint>) =
        entries.into_iter().partition(|hint| REORDER_LABELS.contains(&hint.label));
    if reorder.is_empty() {
        return rest;
    }
    let key = reorder.iter().map(|hint| hint.key.as_str()).collect::<Vec<_>>().join(" ");
    let at = rest.iter().position(|hint| hint.label == "pause").map_or(0, |index| index + 1);
    rest.insert(at, Hint { key, label: "reorder" });
    rest
}

pub(super) fn hints_for(screen: Screen, bindings: &KeyBindings) -> Hints {
    let scope = screen.scope();
    Hints {
        entries: merge_reorder(
            screen_actions(screen).iter().filter_map(|(action, label)| pair(bindings, scope, *action, label)).collect(),
        ),
        pinned: pair(bindings, scope, Action::Help, "help"),
        right: if screen == Screen::Files { None } else { pair(bindings, scope, Action::Back, "back") },
    }
}

const HELP_SCREENS: &[(&str, Screen)] = &[
    ("files", Screen::Files),
    ("connections", Screen::Connections),
    ("transfers", Screen::Transfers),
    ("history", Screen::History),
    ("sync", Screen::Sync),
];

const HELP_KEYS_SHOWN: usize = 2;

fn key_text(bindings: &KeyBindings, scope: Scope, action: Action) -> Option<String> {
    let keys: Vec<String> =
        bindings.keys_for(scope, action).into_iter().take(HELP_KEYS_SHOWN).map(format_key_spec).collect();
    (!keys.is_empty()).then(|| keys.join(" / "))
}

fn session_rows(bindings: &KeyBindings) -> Vec<HelpRow> {
    let keys: Vec<(Action, Option<String>)> = SESSION_ACTIONS
        .iter()
        .map(|action| (*action, bindings.key_for(Scope::Files, *action).map(format_key_spec)))
        .collect();
    let consecutive_digits = keys
        .iter()
        .zip(1..=9u8)
        .all(|((_, key), digit)| key.as_deref() == Some(char::from(b'0' + digit).to_string().as_str()));
    if consecutive_digits {
        return vec![HelpRow { key: Some("1-9".to_string()), action: "session" }];
    }
    keys.into_iter()
        .filter_map(|(action, key)| key.map(|key| HelpRow { key: Some(key), action: action.name() }))
        .collect()
}

pub(super) fn help_sections(bindings: &KeyBindings) -> Vec<HelpSection> {
    let mut sections: Vec<HelpSection> = HELP_SCREENS
        .iter()
        .map(|(title, screen)| HelpSection {
            title,
            rows: screen_actions(*screen)
                .iter()
                .map(|(action, label)| HelpRow { key: key_text(bindings, screen.scope(), *action), action: label })
                .collect(),
        })
        .collect();
    if let Some(files) = sections.first_mut() {
        files.rows.extend(session_rows(bindings));
    }

    let listed = |action: &Action| {
        HELP_SCREENS.iter().any(|(_, screen)| screen_actions(*screen).iter().any(|(listed, _)| listed == action))
    };
    let first = |action: Action| bindings.key_for(Scope::Global, action).map(format_key_spec);
    let movement_key = match (first(Action::Up), first(Action::Down)) {
        (Some(up), Some(down)) => Some(format!("{up}/{down}")),
        _ => None,
    };
    let mut global = vec![HelpRow { key: movement_key, action: "move cursor" }];
    global.extend(
        ALL_ACTIONS
            .iter()
            .filter(|action| !matches!(action, Action::Up | Action::Down) && !listed(action))
            .map(|action| HelpRow { key: key_text(bindings, Scope::Global, *action), action: action.name() }),
    );
    sections.push(HelpSection { title: "global", rows: global });
    sections
}

#[cfg(test)]
mod tests;
