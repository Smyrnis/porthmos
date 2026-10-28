use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const SESSION_NAMES: [&str; 9] = [
    "session_1",
    "session_2",
    "session_3",
    "session_4",
    "session_5",
    "session_6",
    "session_7",
    "session_8",
    "session_9",
];

fn session_name(number: u8) -> &'static str {
    usize::from(number).checked_sub(1).and_then(|index| SESSION_NAMES.get(index)).copied().unwrap_or("session")
}

fn session_number(name: &str) -> Option<u8> {
    SESSION_NAMES.iter().position(|candidate| *candidate == name).map(|index| index as u8 + 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    Global,
    Files,
    Connections,
    Transfers,
    History,
    Sync,
}

pub const SCOPES: &[Scope] =
    &[Scope::Global, Scope::Files, Scope::Connections, Scope::Transfers, Scope::History, Scope::Sync];

impl Scope {
    pub fn name(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Files => "files",
            Scope::Connections => "connections",
            Scope::Transfers => "transfers",
            Scope::History => "history",
            Scope::Sync => "sync",
        }
    }

    pub fn from_name(name: &str) -> Option<Scope> {
        SCOPES.iter().copied().find(|scope| scope.name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    SwitchPanel,
    Up,
    Down,
    Open,
    ToggleSelect,
    Rename,
    Mkdir,
    Delete,
    Refresh,
    Copy,
    Move,
    Preview,
    Edit,
    CancelTransfer,
    SpeedLimit,
    Pause,
    RowUp,
    RowDown,
    OpenConnections,
    AddConnection,
    DeleteConnection,
    Disconnect,
    JumpToSession(u8),
    OpenTerminal,
    ToggleHidden,
    CycleSort,
    BookmarkHere,
    OpenBookmarks,
    OpenSearch,
    Filter,
    OpenTransfers,
    OpenHistory,
    OpenSync,
    CheckForUpdates,
    OpenPalette,
    CycleSession,
    Help,
    Back,
    Noop,
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Action::Quit => "quit",
            Action::SwitchPanel => "switch_panel",
            Action::Up => "up",
            Action::Down => "down",
            Action::Open => "open",
            Action::ToggleSelect => "toggle_select",
            Action::Rename => "rename",
            Action::Mkdir => "mkdir",
            Action::Delete => "delete",
            Action::Refresh => "refresh",
            Action::Copy => "copy",
            Action::Move => "move",
            Action::Preview => "preview",
            Action::Edit => "edit",
            Action::CancelTransfer => "cancel_transfer",
            Action::SpeedLimit => "speed_limit",
            Action::Pause => "pause",
            Action::RowUp => "row_up",
            Action::RowDown => "row_down",
            Action::OpenConnections => "open_connections",
            Action::AddConnection => "add_connection",
            Action::DeleteConnection => "delete_connection",
            Action::Disconnect => "disconnect",
            Action::JumpToSession(number) => session_name(*number),
            Action::OpenTerminal => "open_terminal",
            Action::ToggleHidden => "toggle_hidden",
            Action::CycleSort => "cycle_sort",
            Action::BookmarkHere => "bookmark_here",
            Action::OpenBookmarks => "open_bookmarks",
            Action::OpenSearch => "open_search",
            Action::Filter => "filter",
            Action::OpenTransfers => "open_transfers",
            Action::OpenHistory => "open_history",
            Action::OpenSync => "open_sync",
            Action::CheckForUpdates => "check_for_updates",
            Action::OpenPalette => "open_palette",
            Action::CycleSession => "cycle_session",
            Action::Help => "help",
            Action::Back => "back",
            Action::Noop => "noop",
        }
    }

    pub fn from_name(name: &str) -> Option<Action> {
        Some(match name {
            "quit" => Action::Quit,
            "switch_panel" => Action::SwitchPanel,
            "up" => Action::Up,
            "down" => Action::Down,
            "open" => Action::Open,
            "toggle_select" => Action::ToggleSelect,
            "rename" => Action::Rename,
            "mkdir" => Action::Mkdir,
            "delete" => Action::Delete,
            "refresh" => Action::Refresh,
            "copy" => Action::Copy,
            "move" => Action::Move,
            "preview" => Action::Preview,
            "edit" => Action::Edit,
            "cancel_transfer" => Action::CancelTransfer,
            "speed_limit" => Action::SpeedLimit,
            "pause" => Action::Pause,
            "row_up" => Action::RowUp,
            "row_down" => Action::RowDown,
            "open_connections" => Action::OpenConnections,
            "add_connection" => Action::AddConnection,
            "delete_connection" => Action::DeleteConnection,
            "disconnect" => Action::Disconnect,
            "open_terminal" => Action::OpenTerminal,
            "toggle_hidden" => Action::ToggleHidden,
            "cycle_sort" => Action::CycleSort,
            "bookmark_here" => Action::BookmarkHere,
            "open_bookmarks" => Action::OpenBookmarks,
            "open_search" => Action::OpenSearch,
            "filter" => Action::Filter,
            "open_transfers" => Action::OpenTransfers,
            "open_history" => Action::OpenHistory,
            "open_sync" => Action::OpenSync,
            "check_for_updates" => Action::CheckForUpdates,
            "open_palette" => Action::OpenPalette,
            "cycle_session" => Action::CycleSession,
            "help" => Action::Help,
            "back" => Action::Back,
            other => return session_number(other).map(Action::JumpToSession),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseKeyError(pub String);

impl std::fmt::Display for ParseKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseKeyError {}

fn strip_modifier<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &text[prefix.len()..])
}

pub fn parse_key_spec(s: &str) -> Result<KeySpec, ParseKeyError> {
    let mut modifiers = KeyModifiers::NONE;
    let mut remainder = s;

    loop {
        if let Some(rest) = strip_modifier(remainder, "ctrl+") {
            modifiers |= KeyModifiers::CONTROL;
            remainder = rest;
        } else if let Some(rest) = strip_modifier(remainder, "alt+") {
            modifiers |= KeyModifiers::ALT;
            remainder = rest;
        } else if let Some(rest) = strip_modifier(remainder, "shift+") {
            modifiers |= KeyModifiers::SHIFT;
            remainder = rest;
        } else {
            break;
        }
    }

    let code = parse_key_code(remainder, modifiers)?;
    Ok(normalized(KeySpec { code, modifiers }))
}

fn parse_key_code(text: &str, modifiers: KeyModifiers) -> Result<KeyCode, ParseKeyError> {
    let mut characters = text.chars();
    if let (Some(character), None) = (characters.next(), characters.next())
        && character.is_ascii_alphabetic()
    {
        let combined = modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let letter = if combined {
            character.to_ascii_lowercase()
        } else if modifiers.contains(KeyModifiers::SHIFT) {
            character.to_ascii_uppercase()
        } else {
            character
        };
        return Ok(KeyCode::Char(letter));
    }
    let lowered = text.to_lowercase();
    let s = lowered.as_str();
    if let Some(digits) = s.strip_prefix('f')
        && let Ok(num) = digits.parse::<u8>()
        && (1..=12).contains(&num)
    {
        return Ok(KeyCode::F(num));
    }

    match s {
        "tab" => Ok(KeyCode::Tab),
        "enter" => Ok(KeyCode::Enter),
        "esc" | "escape" => Ok(KeyCode::Esc),
        "space" => Ok(KeyCode::Char(' ')),
        "up" => Ok(KeyCode::Up),
        "down" => Ok(KeyCode::Down),
        "left" => Ok(KeyCode::Left),
        "right" => Ok(KeyCode::Right),
        "home" => Ok(KeyCode::Home),
        "end" => Ok(KeyCode::End),
        "backspace" => Ok(KeyCode::Backspace),
        "delete" | "del" => Ok(KeyCode::Delete),
        _ => {
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => Ok(KeyCode::Char(ch)),
                _ => Err(ParseKeyError(format!("unrecognized key \"{s}\""))),
            }
        }
    }
}

pub fn format_key_spec(spec: KeySpec) -> String {
    let mut parts = Vec::new();
    if spec.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if spec.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_string());
    }
    if spec.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift".to_string());
    }

    let key_name = match spec.code {
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Char(c) if spec.modifiers.is_empty() => c.to_string(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        other => format!("{other:?}"),
    };
    parts.push(key_name);

    parts.join("+")
}

fn normalized(mut spec: KeySpec) -> KeySpec {
    if matches!(spec.code, KeyCode::Char(_)) {
        spec.modifiers.remove(KeyModifiers::SHIFT);
    }
    spec
}

pub struct KeyBindings(HashMap<Scope, HashMap<Action, Vec<KeySpec>>>);

fn defaults_table() -> Vec<(Scope, Action, Vec<KeySpec>)> {
    let plain = |code: KeyCode| KeySpec { code, modifiers: KeyModifiers::NONE };
    let ctrl = |character: char| KeySpec { code: KeyCode::Char(character), modifiers: KeyModifiers::CONTROL };
    let function = |number: u8| plain(KeyCode::F(number));
    let letter = |character: char| plain(KeyCode::Char(character));
    let mut table = vec![
        (Scope::Global, Action::Quit, vec![letter('q'), function(10)]),
        (Scope::Global, Action::Up, vec![plain(KeyCode::Up), letter('k')]),
        (Scope::Global, Action::Down, vec![plain(KeyCode::Down), letter('j')]),
        (Scope::Global, Action::Open, vec![plain(KeyCode::Enter)]),
        (Scope::Global, Action::ToggleSelect, vec![plain(KeyCode::Char(' '))]),
        (Scope::Global, Action::Back, vec![plain(KeyCode::Esc)]),
        (Scope::Global, Action::Help, vec![letter('?'), function(1)]),
        (Scope::Global, Action::Filter, vec![plain(KeyCode::Char('/'))]),
        (Scope::Global, Action::OpenConnections, vec![function(9)]),
        (Scope::Global, Action::OpenTerminal, vec![function(4)]),
        (Scope::Global, Action::AddConnection, vec![function(6)]),
        (Scope::Global, Action::Refresh, vec![ctrl('r')]),
        (Scope::Global, Action::CancelTransfer, vec![ctrl('c')]),
        (Scope::Global, Action::ToggleHidden, vec![ctrl('h')]),
        (Scope::Global, Action::CycleSort, vec![ctrl('s')]),
        (Scope::Global, Action::BookmarkHere, vec![ctrl('d')]),
        (Scope::Global, Action::OpenBookmarks, vec![ctrl('b')]),
        (Scope::Global, Action::OpenSearch, vec![ctrl('f')]),
        (Scope::Global, Action::OpenTransfers, vec![ctrl('t')]),
        (Scope::Global, Action::OpenHistory, vec![ctrl('y')]),
        (Scope::Global, Action::OpenSync, vec![ctrl('u')]),
        (Scope::Global, Action::OpenPalette, vec![ctrl('p')]),
        (Scope::Global, Action::CycleSession, vec![ctrl('n')]),
        (Scope::Files, Action::SwitchPanel, vec![plain(KeyCode::Tab)]),
        (Scope::Files, Action::Rename, vec![letter('r'), function(2)]),
        (Scope::Files, Action::Edit, vec![letter('e'), function(3)]),
        (Scope::Files, Action::Copy, vec![letter('c'), function(5)]),
        (Scope::Files, Action::Move, vec![letter('m'), function(6)]),
        (Scope::Files, Action::Preview, vec![letter('p')]),
        (Scope::Files, Action::Mkdir, vec![letter('n'), function(7)]),
        (Scope::Files, Action::Delete, vec![letter('d'), function(8)]),
        (Scope::Files, Action::OpenTransfers, vec![letter('t')]),
        (Scope::Connections, Action::Rename, vec![letter('e'), function(2)]),
        (Scope::Connections, Action::AddConnection, vec![letter('n')]),
        (Scope::Connections, Action::Disconnect, vec![letter('x'), function(8)]),
        (Scope::Connections, Action::DeleteConnection, vec![plain(KeyCode::Delete)]),
        (Scope::Transfers, Action::Open, vec![letter('r')]),
        (Scope::Transfers, Action::Delete, vec![letter('x'), function(8)]),
        (Scope::Transfers, Action::Refresh, vec![letter('c')]),
        (Scope::Transfers, Action::Pause, vec![letter('p')]),
        (Scope::Transfers, Action::RowUp, vec![letter('K')]),
        (Scope::Transfers, Action::RowDown, vec![letter('J')]),
        (Scope::Transfers, Action::SpeedLimit, vec![ctrl('l')]),
        (Scope::History, Action::Delete, vec![letter('c'), function(8)]),
    ];
    for (number, action) in (1..=9u8).zip(SESSION_ACTIONS) {
        table.push((Scope::Files, *action, vec![letter(char::from(b'0' + number))]));
    }
    table
}

fn lookup_order(scope: Scope) -> impl Iterator<Item = Scope> {
    let fallback = (scope != Scope::Global).then_some(Scope::Global);
    std::iter::once(scope).chain(fallback)
}

impl KeyBindings {
    pub fn defaults() -> Self {
        let mut map: HashMap<Scope, HashMap<Action, Vec<KeySpec>>> = HashMap::new();
        for (scope, action, keys) in defaults_table() {
            map.entry(scope).or_default().insert(action, keys);
        }
        Self(map)
    }

    pub fn map_key(&self, scope: Scope, key: KeyEvent) -> Action {
        let spec = normalized(KeySpec { code: key.code, modifiers: key.modifiers });
        for domain in lookup_order(scope) {
            let found = self
                .0
                .get(&domain)
                .and_then(|actions| actions.iter().find(|(_, keys)| keys.contains(&spec)).map(|(action, _)| *action));
            if let Some(action) = found {
                return action;
            }
        }
        Action::Noop
    }

    fn action_in(&self, domain: Scope, spec: &KeySpec) -> Option<Action> {
        self.0
            .get(&domain)
            .and_then(|actions| actions.iter().find(|(_, keys)| keys.contains(spec)).map(|(action, _)| *action))
    }

    pub fn keys_for(&self, scope: Scope, action: Action) -> Vec<KeySpec> {
        let mut keys: Vec<KeySpec> = Vec::new();
        for domain in lookup_order(scope) {
            for spec in self.0.get(&domain).and_then(|actions| actions.get(&action)).into_iter().flatten() {
                let taken = domain != scope && self.action_in(scope, spec).is_some_and(|other| other != action);
                if !taken && !keys.contains(spec) {
                    keys.push(*spec);
                }
            }
        }
        keys
    }

    pub fn key_for(&self, scope: Scope, action: Action) -> Option<KeySpec> {
        self.keys_for(scope, action).first().copied()
    }
}

pub const ALL_ACTIONS: &[Action] = &[
    Action::Quit,
    Action::SwitchPanel,
    Action::Up,
    Action::Down,
    Action::Open,
    Action::ToggleSelect,
    Action::Rename,
    Action::Mkdir,
    Action::Delete,
    Action::Refresh,
    Action::Copy,
    Action::Move,
    Action::Preview,
    Action::Edit,
    Action::CancelTransfer,
    Action::SpeedLimit,
    Action::Pause,
    Action::RowUp,
    Action::RowDown,
    Action::OpenConnections,
    Action::AddConnection,
    Action::DeleteConnection,
    Action::Disconnect,
    Action::OpenTerminal,
    Action::ToggleHidden,
    Action::CycleSort,
    Action::BookmarkHere,
    Action::OpenBookmarks,
    Action::OpenSearch,
    Action::Filter,
    Action::OpenTransfers,
    Action::OpenHistory,
    Action::OpenSync,
    Action::CheckForUpdates,
    Action::OpenPalette,
    Action::CycleSession,
    Action::Help,
    Action::Back,
];

pub const SESSION_ACTIONS: &[Action] = &[
    Action::JumpToSession(1),
    Action::JumpToSession(2),
    Action::JumpToSession(3),
    Action::JumpToSession(4),
    Action::JumpToSession(5),
    Action::JumpToSession(6),
    Action::JumpToSession(7),
    Action::JumpToSession(8),
    Action::JumpToSession(9),
];

mod config;

#[cfg(test)]
mod tests;
