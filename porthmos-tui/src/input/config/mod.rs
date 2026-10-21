use std::collections::HashMap;

use super::{Action, KeyBindings, KeySpec, SCOPES, Scope, lookup_order, parse_key_spec};

const WRONG_TYPE: &str = "must be a string, a list of strings or a table";

struct Entry {
    action: Action,
    keys: Vec<(String, KeySpec)>,
}

fn unlisted(map: &mut HashMap<Action, Vec<KeySpec>>, spec: KeySpec, keep: Action) -> Vec<Action> {
    let mut lost = Vec::new();
    for (action, keys) in map.iter_mut() {
        if *action != keep && keys.contains(&spec) {
            keys.retain(|existing| *existing != spec);
            lost.push(*action);
        }
    }
    map.retain(|action, keys| *action == keep || !keys.is_empty());
    lost
}

fn parse_entry(name: &str, value: &toml::Value, warnings: &mut Vec<String>) -> Option<Entry> {
    let Some(action) = Action::from_name(name) else {
        warnings.push(format!("unknown key binding action \"{name}\""));
        return None;
    };
    let texts: Vec<&str> = match value {
        toml::Value::String(text) => vec![text.as_str()],
        toml::Value::Array(items) => match items.iter().map(toml::Value::as_str).collect::<Option<Vec<&str>>>() {
            Some(texts) => texts,
            None => {
                warnings.push(format!("key binding \"{name}\" {WRONG_TYPE}"));
                return None;
            }
        },
        _ => {
            warnings.push(format!("key binding \"{name}\" {WRONG_TYPE}"));
            return None;
        }
    };

    let mut keys: Vec<(String, KeySpec)> = Vec::new();
    for text in &texts {
        match parse_key_spec(text) {
            Ok(spec) if keys.iter().all(|(_, known)| *known != spec) => keys.push((text.to_string(), spec)),
            Ok(_) => {}
            Err(err) => warnings.push(format!("invalid key \"{text}\" for \"{name}\": {err}")),
        }
    }
    if keys.is_empty() && !texts.is_empty() {
        return None;
    }
    Some(Entry { action, keys })
}

fn warn_displaced(displaced: Vec<(KeySpec, Action)>, entry: &Entry, name: &str, warnings: &mut Vec<String>) {
    for (spec, other) in displaced {
        let text = entry.keys.iter().find(|(_, known)| *known == spec).map_or("", |(text, _)| text.as_str());
        warnings.push(format!("key \"{text}\" is already bound to \"{}\"; \"{name}\" now overrides it", other.name()));
    }
}

fn sorted(table: &toml::Table) -> Vec<(&String, &toml::Value)> {
    let mut entries: Vec<(&String, &toml::Value)> = table.iter().collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    entries
}

impl KeyBindings {
    pub fn from_frontend(frontend: &toml::Table) -> (Self, Vec<String>) {
        let mut bindings = Self::defaults();
        let mut warnings = Vec::new();
        match frontend.get("keys") {
            None => {}
            Some(toml::Value::Table(table)) => bindings.apply_table(table, &mut warnings),
            Some(_) => warnings.push("[keys] must be a table; using the default key bindings".to_string()),
        }
        (bindings, warnings)
    }

    #[cfg(test)]
    pub fn from_overrides(overrides: &HashMap<String, String>) -> (Self, Vec<String>) {
        let table: toml::Table =
            overrides.iter().map(|(name, key)| (name.clone(), toml::Value::String(key.clone()))).collect();
        let mut bindings = Self::defaults();
        let mut warnings = Vec::new();
        bindings.apply_table(&table, &mut warnings);
        (bindings, warnings)
    }

    fn apply_table(&mut self, table: &toml::Table, warnings: &mut Vec<String>) {
        let entries = sorted(table);
        for (name, value) in entries.iter().filter(|(_, value)| !value.is_table()) {
            if let Some(entry) = parse_entry(name, value, warnings) {
                let displaced = self.pin_everywhere(&entry);
                warn_displaced(displaced, &entry, name, warnings);
            }
        }
        for (scope_name, value) in entries.iter().filter(|(_, value)| value.is_table()) {
            let Some(scope) = Scope::from_name(scope_name) else {
                warnings.push(format!("unknown key scope \"{scope_name}\""));
                continue;
            };
            let Some(scoped) = value.as_table() else {
                continue;
            };
            for (name, value) in sorted(scoped) {
                if let Some(entry) = parse_entry(name, value, warnings) {
                    let displaced = self.bind_in_scope(scope, &entry);
                    warn_displaced(displaced, &entry, name, warnings);
                }
            }
        }
    }

    fn pin_everywhere(&mut self, entry: &Entry) -> Vec<(KeySpec, Action)> {
        let mut scopes: Vec<Scope> = SCOPES
            .iter()
            .copied()
            .filter(|scope| self.0.get(scope).is_some_and(|map| map.contains_key(&entry.action)))
            .collect();
        if scopes.is_empty() && !entry.keys.is_empty() {
            scopes.push(Scope::Global);
        }
        let mut displaced: Vec<(KeySpec, Action)> = Vec::new();
        for scope in scopes {
            for (_, spec) in &entry.keys {
                for domain in lookup_order(scope) {
                    if let Some(map) = self.0.get_mut(&domain) {
                        for lost in unlisted(map, *spec, entry.action) {
                            if !displaced.contains(&(*spec, lost)) {
                                displaced.push((*spec, lost));
                            }
                        }
                    }
                }
            }
            self.set_keys(scope, entry);
        }
        displaced
    }

    fn bind_in_scope(&mut self, scope: Scope, entry: &Entry) -> Vec<(KeySpec, Action)> {
        let mut displaced: Vec<(KeySpec, Action)> = Vec::new();
        for (_, spec) in &entry.keys {
            if let Some(map) = self.0.get_mut(&scope) {
                for lost in unlisted(map, *spec, entry.action) {
                    displaced.push((*spec, lost));
                }
            }
        }
        self.set_keys(scope, entry);
        displaced
    }

    fn set_keys(&mut self, scope: Scope, entry: &Entry) {
        let map = self.0.entry(scope).or_default();
        if entry.keys.is_empty() {
            map.remove(&entry.action);
        } else {
            map.insert(entry.action, entry.keys.iter().map(|(_, spec)| *spec).collect());
        }
    }
}

#[cfg(test)]
mod tests;
