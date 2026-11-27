use crate::ports::contains_ignoring_case;

const MAX_NAME_CHARS: usize = 256;
const SUFFIX: &str = ".service";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitName(String);

impl UnitName {
    pub fn new(text: &str) -> Option<Self> {
        let valid = text.len() <= MAX_NAME_CHARS
            && text.ends_with(SUFFIX)
            && !text.starts_with('-')
            && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || b":_.@-".contains(&byte));
        valid.then(|| Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveState {
    Active,
    Inactive,
    Failed,
    Activating,
    Deactivating,
    Other,
}

impl ActiveState {
    pub(super) fn from_text(text: &str) -> Self {
        match text {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            "failed" => Self::Failed,
            "activating" => Self::Activating,
            "deactivating" => Self::Deactivating,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootState {
    Enabled,
    Disabled,
    Static,
    Masked,
    Alias,
    Indirect,
    Generated,
    Other,
}

impl BootState {
    pub(super) fn from_text(text: &str) -> Self {
        match text {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "static" => Self::Static,
            "masked" | "masked-runtime" => Self::Masked,
            "alias" => Self::Alias,
            "indirect" => Self::Indirect,
            "generated" => Self::Generated,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub name: String,
    pub description: String,
    pub load: String,
    pub active: ActiveState,
    pub sub: String,
    pub boot: Option<BootState>,
}

pub fn verb_allowed(unit: &Unit, verb: super::ServiceVerb) -> bool {
    if unit.load != "loaded" {
        return false;
    }
    match verb {
        super::ServiceVerb::Enable | super::ServiceVerb::Disable => {
            matches!(unit.boot, Some(BootState::Enabled | BootState::Disabled))
        }
        _ => true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Running,
    Failed,
}

fn shown(unit: &Unit, needle: &str, status: StatusFilter) -> bool {
    let status_ok = match status {
        StatusFilter::All => true,
        StatusFilter::Running => unit.sub == "running",
        StatusFilter::Failed => unit.active == ActiveState::Failed,
    };
    status_ok
        && (needle.is_empty()
            || contains_ignoring_case(&unit.name, needle)
            || contains_ignoring_case(&unit.description, needle))
}

pub fn arranged_units<'a>(all: &'a [Unit], filter: &str, status: StatusFilter) -> Vec<&'a Unit> {
    let needle = filter.trim().to_lowercase();
    let mut units: Vec<&Unit> = all.iter().filter(|unit| shown(unit, &needle, status)).collect();
    units.sort_by_cached_key(|unit| {
        let rank = match (unit.active == ActiveState::Failed, unit.sub == "running") {
            (true, _) => 0,
            (false, true) => 1,
            (false, false) => 2,
        };
        (rank, unit.name.clone())
    });
    units
}
