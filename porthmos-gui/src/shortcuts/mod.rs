use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    Home,
    End,
    Open,
    Parent,
    SwitchPanel,
    ToggleSelect,
    SelectAll,
    ToggleHidden,
    CycleSort,
    Refresh,
    StartFilter,
    ClearFilter,
    NextTab,
    PreviousTab,
    NewFolder,
    Rename,
    Delete,
    Copy,
    Move,
    ToggleQueue,
    Preview,
    Edit,
    Terminal,
    Search,
    Bookmarks,
    BookmarkHere,
    History,
    Quit,
}

const NAMES: [(Action, &str); 32] = [
    (Action::MoveUp, "move_up"),
    (Action::MoveDown, "move_down"),
    (Action::PageUp, "page_up"),
    (Action::PageDown, "page_down"),
    (Action::Home, "home"),
    (Action::End, "end"),
    (Action::Open, "open"),
    (Action::Parent, "parent"),
    (Action::SwitchPanel, "switch_panel"),
    (Action::ToggleSelect, "toggle_select"),
    (Action::SelectAll, "select_all"),
    (Action::ToggleHidden, "toggle_hidden"),
    (Action::CycleSort, "cycle_sort"),
    (Action::Refresh, "refresh"),
    (Action::StartFilter, "start_filter"),
    (Action::ClearFilter, "clear_filter"),
    (Action::NextTab, "next_tab"),
    (Action::PreviousTab, "previous_tab"),
    (Action::NewFolder, "new_folder"),
    (Action::Rename, "rename"),
    (Action::Delete, "delete"),
    (Action::Copy, "copy"),
    (Action::Move, "move"),
    (Action::ToggleQueue, "toggle_queue"),
    (Action::Preview, "preview"),
    (Action::Edit, "edit"),
    (Action::Terminal, "terminal"),
    (Action::Search, "search"),
    (Action::Bookmarks, "bookmarks"),
    (Action::BookmarkHere, "bookmark_here"),
    (Action::History, "history"),
    (Action::Quit, "quit"),
];

const NAMED_KEYS: [(&str, Named); 30] = [
    ("enter", Named::Enter),
    ("return", Named::Enter),
    ("escape", Named::Escape),
    ("esc", Named::Escape),
    ("tab", Named::Tab),
    ("space", Named::Space),
    ("backspace", Named::Backspace),
    ("delete", Named::Delete),
    ("del", Named::Delete),
    ("insert", Named::Insert),
    ("up", Named::ArrowUp),
    ("down", Named::ArrowDown),
    ("left", Named::ArrowLeft),
    ("right", Named::ArrowRight),
    ("pageup", Named::PageUp),
    ("pagedown", Named::PageDown),
    ("home", Named::Home),
    ("end", Named::End),
    ("f1", Named::F1),
    ("f2", Named::F2),
    ("f3", Named::F3),
    ("f4", Named::F4),
    ("f5", Named::F5),
    ("f6", Named::F6),
    ("f7", Named::F7),
    ("f8", Named::F8),
    ("f9", Named::F9),
    ("f10", Named::F10),
    ("f11", Named::F11),
    ("f12", Named::F12),
];

impl Action {
    fn from_name(name: &str) -> Option<Action> {
        NAMES.iter().find(|(_, known)| *known == name).map(|(action, _)| *action)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    key: Key,
    modifiers: Modifiers,
}

impl Chord {
    pub fn new(key: Key, modifiers: Modifiers) -> Chord {
        Chord { key: normalized(&key), modifiers }
    }

    fn parse(text: &str) -> Option<Chord> {
        let text = text.trim();
        let (modifier_text, key_text) = match text.strip_suffix('+') {
            Some(rest) => (rest.strip_suffix('+').unwrap_or(rest), "+"),
            None => text.rsplit_once('+').unwrap_or(("", text)),
        };
        let mut modifiers = Modifiers::empty();
        for token in modifier_text.split('+').map(str::trim).filter(|token| !token.is_empty()) {
            modifiers |= match token.to_lowercase().as_str() {
                "ctrl" | "control" => Modifiers::CTRL,
                "shift" => Modifiers::SHIFT,
                "alt" | "option" => Modifiers::ALT,
                "cmd" | "command" | "super" | "meta" | "logo" | "win" => Modifiers::LOGO,
                "mod" => Modifiers::COMMAND,
                _ => return None,
            };
        }
        let lowered = key_text.to_lowercase();
        let named = NAMED_KEYS.iter().find(|(name, _)| *name == lowered).map(|(_, key)| *key);
        let key = match (named, lowered.chars().count()) {
            (Some(named), _) => Key::Named(named),
            (None, 1) => Key::Character(lowered.as_str().into()),
            _ => return None,
        };
        Some(Chord { key, modifiers })
    }
}

impl Chord {
    fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.control() {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.alt() {
            parts.push("Alt".to_string());
        }
        if self.modifiers.shift() {
            parts.push("Shift".to_string());
        }
        if self.modifiers.logo() {
            parts.push(if cfg!(target_os = "macos") { "Cmd" } else { "Super" }.to_string());
        }
        parts.push(match &self.key {
            Key::Named(named) => {
                let name = NAMED_KEYS.iter().find(|(_, known)| known == named).map_or("", |(name, _)| *name);
                let mut letters = name.chars();
                letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default()
            }
            Key::Character(text) => text.to_uppercase(),
            Key::Unidentified => String::new(),
        });
        parts.join("+")
    }
}

fn normalized(key: &Key) -> Key {
    match key {
        Key::Character(text) => Key::Character(text.to_lowercase().as_str().into()),
        other => other.clone(),
    }
}

pub fn is_reserved(chord: &Chord) -> bool {
    let modifiers = chord.modifiers;
    let alt_only = modifiers == Modifiers::ALT;
    let window_switch = alt_only && matches!(chord.key, Key::Named(Named::F4 | Named::Tab));
    let fullscreen = modifiers.is_empty() && chord.key == Key::Named(Named::F11);
    let terminal_chord = modifiers.contains(Modifiers::CTRL | Modifiers::SHIFT);
    let system_key = cfg!(not(target_os = "macos")) && modifiers.logo();
    window_switch || fullscreen || terminal_chord || system_key
}

#[derive(Debug, Clone)]
pub struct KeyPress {
    pub key: Key,
    pub modified_key: Key,
    pub modifiers: Modifiers,
    pub text: Option<String>,
}

pub fn effective(press: &KeyPress) -> (Key, Modifiers) {
    if press.modifiers == Modifiers::SHIFT
        && let Key::Character(typed) = &press.modified_key
        && let mut characters = typed.chars()
        && let (Some(only), None) = (characters.next(), characters.next())
        && !only.is_alphanumeric()
    {
        return (press.modified_key.clone(), Modifiers::empty());
    }
    (press.key.clone(), press.modifiers)
}

#[derive(Debug, Clone)]
pub struct Shortcuts {
    bindings: Vec<(Chord, Action)>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        let character = |text: &str| Key::Character(text.into());
        let named = Key::Named;
        let none = Modifiers::empty();
        let table = [
            (named(Named::ArrowUp), none, Action::MoveUp),
            (character("k"), none, Action::MoveUp),
            (named(Named::ArrowDown), none, Action::MoveDown),
            (character("j"), none, Action::MoveDown),
            (named(Named::PageUp), none, Action::PageUp),
            (named(Named::PageDown), none, Action::PageDown),
            (named(Named::Home), none, Action::Home),
            (named(Named::End), none, Action::End),
            (named(Named::Enter), none, Action::Open),
            (named(Named::Backspace), none, Action::Parent),
            (named(Named::Tab), none, Action::SwitchPanel),
            (named(Named::Space), none, Action::ToggleSelect),
            (character("a"), Modifiers::COMMAND, Action::SelectAll),
            (character("h"), Modifiers::CTRL, Action::ToggleHidden),
            (character("s"), Modifiers::CTRL, Action::CycleSort),
            (character("r"), Modifiers::CTRL, Action::Refresh),
            (character("/"), none, Action::StartFilter),
            (named(Named::Escape), none, Action::ClearFilter),
            (character("n"), Modifiers::CTRL, Action::NextTab),
            (named(Named::F5), none, Action::Copy),
            (named(Named::F6), none, Action::Move),
            (character("t"), Modifiers::CTRL, Action::ToggleQueue),
            (character("p"), none, Action::Preview),
            (character("e"), none, Action::Edit),
            (named(Named::F3), none, Action::Edit),
            (named(Named::F4), none, Action::Terminal),
            (character("f"), Modifiers::CTRL, Action::Search),
            (character("b"), Modifiers::CTRL, Action::Bookmarks),
            (character("d"), Modifiers::CTRL, Action::BookmarkHere),
            (character("y"), Modifiers::CTRL, Action::History),
            (named(Named::F7), none, Action::NewFolder),
            (named(Named::F2), none, Action::Rename),
            (named(Named::Delete), none, Action::Delete),
            (named(Named::F8), none, Action::Delete),
            (character("q"), Modifiers::COMMAND, Action::Quit),
        ];
        let bindings = table.into_iter().map(|(key, modifiers, action)| (Chord::new(key, modifiers), action)).collect();
        Shortcuts { bindings }
    }
}

impl Shortcuts {
    #[cfg(test)]
    pub fn bindings(&self) -> &[(Chord, Action)] {
        &self.bindings
    }

    pub fn label_for(&self, action: Action) -> Option<String> {
        self.bindings.iter().find(|(_, bound)| *bound == action).map(|(chord, _)| chord.label())
    }

    pub fn action_for(&self, key: &Key, modifiers: Modifiers) -> Option<Action> {
        let key = normalized(key);
        self.bindings.iter().find(|(chord, _)| chord.key == key && chord.modifiers == modifiers).map(|(_, a)| *a)
    }

    pub fn from_frontend(frontend: &toml::Table) -> (Shortcuts, Vec<String>) {
        let mut shortcuts = Shortcuts::default();
        let mut warnings = Vec::new();
        let Some(gui) = frontend.get("gui").and_then(toml::Value::as_table) else {
            return (shortcuts, warnings);
        };
        let Some(keys) = gui.get("keys") else {
            return (shortcuts, warnings);
        };
        let Some(keys) = keys.as_table() else {
            warnings.push("[gui.keys] must be a table; using the default shortcuts".to_string());
            return (shortcuts, warnings);
        };
        for (name, value) in keys {
            let Some(action) = Action::from_name(name) else {
                warnings.push(format!("unknown gui key action \"{name}\""));
                continue;
            };
            let chords = shortcuts.parse_chords(name, value, &mut warnings);
            shortcuts.rebind(action, chords);
        }
        (shortcuts, warnings)
    }

    fn parse_chords(&self, name: &str, value: &toml::Value, warnings: &mut Vec<String>) -> Vec<Chord> {
        let texts: Vec<&str> = match value {
            toml::Value::String(text) => vec![text.as_str()],
            toml::Value::Array(items) => items.iter().filter_map(toml::Value::as_str).collect(),
            _ => Vec::new(),
        };
        let well_formed = match value {
            toml::Value::String(_) => true,
            toml::Value::Array(items) => items.iter().all(toml::Value::is_str),
            _ => false,
        };
        if !well_formed {
            warnings.push(format!("gui key {name} must be a chord or a list of chords; keeping the default"));
        }
        let mut chords = Vec::new();
        for text in texts {
            match Chord::parse(text) {
                Some(chord) if is_reserved(&chord) => {
                    warnings.push(format!("gui key \"{text}\" for {name} is reserved by the window manager; ignored"))
                }
                Some(chord) => chords.push(chord),
                None => warnings.push(format!("invalid gui key \"{text}\" for {name}; keeping the default")),
            }
        }
        chords
    }

    fn rebind(&mut self, action: Action, chords: Vec<Chord>) {
        if chords.is_empty() {
            return;
        }
        self.bindings.retain(|(chord, bound)| *bound != action && !chords.contains(chord));
        self.bindings.extend(chords.into_iter().map(|chord| (chord, action)));
    }
}

#[cfg(test)]
mod tests;
