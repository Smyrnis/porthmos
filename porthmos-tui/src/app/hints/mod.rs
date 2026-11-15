use super::Screen;
use crate::{
    input::{ALL_ACTIONS, Action, KeyBindings, SESSION_ACTIONS, Scope, format_key_spec},
    widgets::{
        frame::{Hint, Hints},
        help::{HelpRow, HelpSection},
    },
};
use porthmos_core::i18n;

const FILES: &[(Action, &str)] = &[
    (Action::Copy, "hint.copy"),
    (Action::Move, "hint.move_selection"),
    (Action::Mkdir, "hint.mkdir"),
    (Action::Delete, "hint.delete"),
    (Action::OpenConnections, "hint.connections"),
    (Action::Quit, "hint.quit"),
    (Action::Edit, "hint.edit"),
    (Action::Preview, "hint.preview"),
    (Action::Rename, "hint.rename"),
    (Action::Filter, "hint.filter"),
    (Action::ToggleSelect, "hint.select"),
    (Action::SwitchPanel, "hint.panel"),
];
const FILES_HELP_ONLY: &[(Action, &str)] = &[
    (Action::GoToPath, "hint.go_to_path"),
    (Action::Parent, "hint.parent_folder"),
    (Action::Permissions, "hint.permissions"),
    (Action::Properties, "hint.properties"),
];
const LIST_HELP: &[(Action, &str)] = &[
    (Action::PageUp, "hint.page_up"),
    (Action::PageDown, "hint.page_down"),
    (Action::First, "hint.first_row"),
    (Action::Last, "hint.last_row"),
];
const CONNECTIONS: &[(Action, &str)] = &[
    (Action::Open, "hint.connect"),
    (Action::AddConnection, "hint.new"),
    (Action::Rename, "hint.edit"),
    (Action::DeleteConnection, "hint.delete"),
    (Action::Disconnect, "hint.disconnect"),
    (Action::ReopenConnections, "hint.reopen"),
];
const HISTORY: &[(Action, &str)] = &[(Action::Open, "hint.details"), (Action::Delete, "hint.clear")];
const TRANSFERS: &[(Action, &str)] = &[
    (Action::Pause, "hint.pause"),
    (Action::RowUp, "hint.move_earlier"),
    (Action::RowDown, "hint.move_later"),
    (Action::Open, "hint.retry"),
    (Action::Delete, "hint.cancel"),
    (Action::Refresh, "hint.clear_done"),
    (Action::SpeedLimit, "hint.limit"),
];
const SYNC: &[(Action, &str)] = &[(Action::Open, "hint.run"), (Action::ToggleSelect, "hint.tick")];

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
    bindings.key_for(scope, action).map(|spec| Hint { key: format_key_spec(spec), label: i18n::text(label) })
}

pub(super) fn form_hints() -> Hints {
    Hints {
        entries: vec![
            Hint { key: "tab".to_string(), label: i18n::text("hint.next_field") },
            Hint { key: "\u{2191}\u{2193}".to_string(), label: i18n::text("hint.field") },
            Hint { key: "ctrl+t".to_string(), label: i18n::text("hint.test") },
            Hint { key: "ctrl+s".to_string(), label: i18n::text("hint.save") },
        ],
        pinned: None,
        right: Some(Hint { key: "esc".to_string(), label: i18n::text("hint.cancel") }),
    }
}

pub(super) fn path_edit_hints() -> Hints {
    Hints {
        entries: vec![
            Hint { key: "enter".to_string(), label: i18n::text("hint.go") },
            Hint { key: "tab".to_string(), label: i18n::text("hint.complete") },
        ],
        pinned: None,
        right: Some(Hint { key: "esc".to_string(), label: i18n::text("hint.cancel") }),
    }
}

pub(super) fn search_hints(with_preview: bool) -> Hints {
    let mut entries = vec![
        Hint { key: "\u{2191}\u{2193}".to_string(), label: i18n::text("hint.move_selection") },
        Hint { key: "enter".to_string(), label: i18n::text("hint.open") },
    ];
    if with_preview {
        entries.push(Hint { key: "PgUp/PgDn".to_string(), label: i18n::text("hint.scroll_preview") });
    }
    Hints { entries, pinned: None, right: Some(Hint { key: "esc".to_string(), label: i18n::text("hint.close") }) }
}

const REORDER_LABELS: [&str; 2] = ["hint.move_earlier", "hint.move_later"];

fn merge_reorder(entries: Vec<Hint>) -> Vec<Hint> {
    let (reorder, mut rest): (Vec<Hint>, Vec<Hint>) =
        entries.into_iter().partition(|hint| REORDER_LABELS.iter().any(|key| i18n::text(key) == hint.label));
    if reorder.is_empty() {
        return rest;
    }
    let key = reorder.iter().map(|hint| hint.key.as_str()).collect::<Vec<_>>().join(" ");
    let at = rest.iter().position(|hint| hint.label == i18n::text("hint.pause")).map_or(0, |index| index + 1);
    rest.insert(at, Hint { key, label: i18n::text("hint.reorder") });
    rest
}

pub(super) fn hints_for(screen: Screen, bindings: &KeyBindings) -> Hints {
    let scope = screen.scope();
    Hints {
        entries: merge_reorder(
            screen_actions(screen).iter().filter_map(|(action, label)| pair(bindings, scope, *action, label)).collect(),
        ),
        pinned: pair(bindings, scope, Action::Help, "hint.help"),
        right: if screen == Screen::Files { None } else { pair(bindings, scope, Action::Back, "hint.back") },
    }
}

const HELP_SCREENS: &[(&str, Screen)] = &[
    ("help.section.files", Screen::Files),
    ("help.section.connections", Screen::Connections),
    ("help.section.transfers", Screen::Transfers),
    ("help.section.history", Screen::History),
    ("help.section.sync", Screen::Sync),
];

const HELP_KEYS_SHOWN: usize = 2;

fn action_text(action: &Action) -> String {
    i18n::text(&format!("action.{}", action.name()))
}

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
        return vec![HelpRow { key: Some("1-9".to_string()), action: i18n::text("hint.session") }];
    }
    keys.into_iter()
        .filter_map(|(action, key)| key.map(|key| HelpRow { key: Some(key), action: action_text(&action) }))
        .collect()
}

pub(super) fn help_sections(bindings: &KeyBindings) -> Vec<HelpSection> {
    let mut sections: Vec<HelpSection> = HELP_SCREENS
        .iter()
        .map(|(title, screen)| HelpSection {
            title: i18n::text(title),
            rows: screen_actions(*screen)
                .iter()
                .map(|(action, label)| HelpRow {
                    key: key_text(bindings, screen.scope(), *action),
                    action: i18n::text(label),
                })
                .collect(),
        })
        .collect();
    if let Some(files) = sections.first_mut() {
        files.rows.extend(session_rows(bindings));
        files.rows.extend(FILES_HELP_ONLY.iter().map(|(action, label)| HelpRow {
            key: key_text(bindings, Scope::Files, *action),
            action: i18n::text(label),
        }));
    }

    let listed = |action: &Action| {
        HELP_SCREENS.iter().any(|(_, screen)| screen_actions(*screen).iter().any(|(listed, _)| listed == action))
            || FILES_HELP_ONLY.iter().any(|(listed, _)| listed == action)
            || LIST_HELP.iter().any(|(listed, _)| listed == action)
    };
    let first = |action: Action| bindings.key_for(Scope::Global, action).map(format_key_spec);
    let movement_key = match (first(Action::Up), first(Action::Down)) {
        (Some(up), Some(down)) => Some(format!("{up}/{down}")),
        _ => None,
    };
    let mut global = vec![HelpRow { key: movement_key, action: i18n::text("hint.move_cursor") }];
    global.extend(
        LIST_HELP.iter().map(|(action, label)| HelpRow {
            key: key_text(bindings, Scope::Global, *action),
            action: i18n::text(label),
        }),
    );
    global.extend(
        ALL_ACTIONS
            .iter()
            .filter(|action| !matches!(action, Action::Up | Action::Down) && !listed(action))
            .map(|action| HelpRow { key: key_text(bindings, Scope::Global, *action), action: action_text(action) }),
    );
    sections.push(HelpSection { title: i18n::text("help.section.global"), rows: global });
    sections
}

#[cfg(test)]
mod tests;
