use std::path::Path;

use anyhow::Result;
use porthmos_tunnel::{Endpoint, Kind, Listen, SpecError, TunnelKey, TunnelSpec};
use serde::{Deserialize, Serialize};

use crate::persist::{Loaded, read_or_set_aside, toml_problem, write_atomic};

const LOCAL: &str = "local";
const DYNAMIC: &str = "dynamic";
const REMOTE: &str = "remote";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelsError {
    Invalid(SpecError),
    Duplicate,
    Protected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tunnels {
    items: Vec<TunnelSpec>,
    skipped: Vec<RawTunnel>,
    protected: bool,
}

impl Tunnels {
    #[cfg(test)]
    pub(crate) fn protected_for_test() -> Self {
        Self { items: Vec::new(), skipped: Vec::new(), protected: true }
    }

    pub fn iter(&self) -> impl Iterator<Item = &TunnelSpec> {
        self.items.iter()
    }

    pub fn get(&self, key: &TunnelKey) -> Option<&TunnelSpec> {
        self.items.iter().find(|spec| spec.key() == *key)
    }

    pub fn for_connection(&self, connection: &str) -> Vec<&TunnelSpec> {
        self.items.iter().filter(|spec| spec.connection == connection).collect()
    }

    pub fn is_protected(&self) -> bool {
        self.protected
    }

    pub fn upsert(&mut self, original: Option<&TunnelKey>, spec: TunnelSpec) -> Result<(), TunnelsError> {
        if self.protected {
            return Err(TunnelsError::Protected);
        }
        spec.validate().map_err(TunnelsError::Invalid)?;
        let position = original.and_then(|key| self.items.iter().position(|existing| existing.key() == *key));
        let new_key = spec.key();
        let taken =
            self.items.iter().enumerate().any(|(index, existing)| existing.key() == new_key && Some(index) != position);
        if taken {
            return Err(TunnelsError::Duplicate);
        }
        match position {
            Some(index) => self.items[index] = spec,
            None => self.items.push(spec),
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &TunnelKey) -> Option<TunnelSpec> {
        let position = self.items.iter().position(|spec| spec.key() == *key)?;
        Some(self.items.remove(position))
    }

    pub fn rename_connection(&mut self, old: &str, new: &str) {
        for spec in self.items.iter_mut().filter(|spec| spec.connection == old) {
            spec.connection = new.to_string();
        }
        for raw in self.skipped.iter_mut().filter(|raw| raw.connection == old) {
            raw.connection = new.to_string();
        }
        let mut seen = std::collections::HashSet::new();
        self.items.retain(|spec| seen.insert(spec.key()));
    }

    pub fn remove_connection(&mut self, connection: &str) -> Vec<TunnelSpec> {
        let (removed, kept) =
            std::mem::take(&mut self.items).into_iter().partition(|spec| spec.connection == connection);
        self.items = kept;
        self.skipped.retain(|raw| raw.connection != connection);
        removed
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
struct RawTunnel {
    #[serde(default)]
    connection: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    listen: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    allow_external: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    auto_start: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct TunnelsFile {
    #[serde(default, rename = "tunnel")]
    tunnel: Vec<RawTunnel>,
}

fn problem_text(error: SpecError) -> String {
    error.to_string()
}

pub fn spec_error_text(error: SpecError) -> String {
    let key = match error {
        SpecError::EmptyName => "empty_name",
        SpecError::NameTooLong => "name_too_long",
        SpecError::NameHasControl => "name_has_control",
        SpecError::EmptyConnection => "empty_connection",
        SpecError::EmptyHost => "empty_host",
        SpecError::HostTooLong => "host_too_long",
        SpecError::HostHasControl => "host_has_control",
        SpecError::BadHost => "bad_host",
        SpecError::BadPort => "bad_port",
        SpecError::ExternalNotAllowed => "external_not_allowed",
    };
    crate::i18n::text(&format!("tunnels.problem.{key}"))
}

fn into_spec(raw: RawTunnel) -> Result<TunnelSpec, String> {
    let parsed = if raw.kind == REMOTE { Listen::parse_server_side(&raw.listen) } else { raw.listen.parse::<Listen>() };
    let listen = parsed.map_err(|error| format!("the listen address: {}", problem_text(error)))?;
    let kind = match raw.kind.as_str() {
        LOCAL => {
            let target = raw.target.as_deref().ok_or_else(|| "a local tunnel needs a target".to_string())?;
            let target = target.parse::<Endpoint>().map_err(|error| format!("the target: {}", problem_text(error)))?;
            Kind::Local { listen, target }
        }
        DYNAMIC => Kind::Dynamic { listen },
        REMOTE => {
            let target = raw.target.as_deref().ok_or_else(|| "a remote tunnel needs a target".to_string())?;
            let target = target.parse::<Endpoint>().map_err(|error| format!("the target: {}", problem_text(error)))?;
            Kind::Remote { listen, target }
        }
        other => return Err(format!("the kind \"{other}\" is not local, remote or dynamic")),
    };
    let spec = TunnelSpec {
        connection: raw.connection,
        name: raw.name,
        kind,
        allow_external: raw.allow_external,
        auto_start: raw.auto_start,
    };
    Ok(spec)
}

fn into_raw(spec: &TunnelSpec) -> RawTunnel {
    let (kind, target) = match &spec.kind {
        Kind::Local { target, .. } => (LOCAL, Some(target.to_string())),
        Kind::Dynamic { .. } => (DYNAMIC, None),
        Kind::Remote { target, .. } => (REMOTE, Some(target.to_string())),
    };
    RawTunnel {
        connection: spec.connection.clone(),
        name: spec.name.clone(),
        kind: kind.to_string(),
        listen: spec.kind.listen().to_string(),
        target,
        allow_external: spec.allow_external,
        auto_start: spec.auto_start,
    }
}

fn from_file(file: TunnelsFile) -> (Tunnels, Vec<super::StartupWarning>) {
    let mut tunnels = Tunnels::default();
    let mut warnings = Vec::new();
    for raw in file.tunnel {
        let label = format!("\"{}\" of \"{}\"", raw.name, raw.connection);
        let kept = raw.clone();
        let outcome = into_spec(raw).and_then(|spec| {
            tunnels.upsert(None, spec).map_err(|error| match error {
                TunnelsError::Duplicate => "another tunnel of that connection already has the name".to_string(),
                TunnelsError::Invalid(error) => problem_text(error),
                TunnelsError::Protected => String::new(),
            })
        });
        if let Err(reason) = outcome {
            tunnels.skipped.push(kept);
            warnings.push(super::StartupWarning(format!(
                "Skipped the tunnel {label} in tunnels.toml: {reason}; the entry stays in the file as written"
            )));
        }
    }
    (tunnels, warnings)
}

pub fn load(paths: &crate::Paths) -> Result<(Tunnels, Vec<super::StartupWarning>)> {
    load_from(&paths.tunnels_file())
}

fn load_from(path: &Path) -> Result<(Tunnels, Vec<super::StartupWarning>)> {
    Ok(
        match read_or_set_aside(path, "tunnels", |text| {
            toml::from_str::<TunnelsFile>(text).map_err(|err| toml_problem(text, &err))
        }) {
            Loaded::Missing => (Tunnels::default(), Vec::new()),
            Loaded::Ready(file) => from_file(file),
            Loaded::SetAside { warning, protected } => {
                (Tunnels { items: Vec::new(), skipped: Vec::new(), protected }, vec![super::StartupWarning(warning)])
            }
        },
    )
}

pub fn save_to(path: &Path, tunnels: &Tunnels) -> Result<()> {
    if tunnels.protected {
        anyhow::bail!("tunnels.toml could not be read or set aside, so it was left alone");
    }
    let mut entries: Vec<RawTunnel> = tunnels.items.iter().map(into_raw).collect();
    entries.extend(tunnels.skipped.iter().cloned());
    let file = TunnelsFile { tunnel: entries };
    write_atomic(path, toml::to_string_pretty(&file)?.as_bytes(), 0o600)
}

#[cfg(test)]
mod tests;
