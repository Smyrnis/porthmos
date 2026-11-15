use porthmos_core::i18n;
use porthmos_core::{UpdateMode, UpdateState};

pub const BADGE_FULL_WIDTH: f32 = 900.0;
pub const BADGE_VERSION_WIDTH: f32 = 640.0;
const ARROW: &str = "\u{2191}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    Ready,
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    pub version: String,
    pub kind: BadgeKind,
}

impl Badge {
    pub fn tiers(&self) -> [String; 3] {
        let suffix = match self.kind {
            BadgeKind::Ready => "installs on quit",
            BadgeKind::Available => "available",
        };
        [format!("{ARROW} {} {suffix}", self.version), format!("{ARROW} {}", self.version), ARROW.to_string()]
    }
}

pub fn badge_text(badge: &Badge, window_width: f32) -> String {
    let [full, version_only, arrow] = badge.tiers();
    if window_width >= BADGE_FULL_WIDTH {
        full
    } else if window_width >= BADGE_VERSION_WIDTH {
        version_only
    } else {
        arrow
    }
}

#[derive(Debug)]
pub struct UpdateView {
    state: Option<UpdateState>,
    ready: Option<String>,
    mode: UpdateMode,
    announced: Option<(String, BadgeKind)>,
}

impl UpdateView {
    pub fn new(mode: UpdateMode) -> Self {
        Self { state: None, ready: None, mode, announced: None }
    }

    pub fn apply(&mut self, state: UpdateState) -> Option<String> {
        if let UpdateState::Ready { version } = &state {
            self.ready = Some(version.clone());
        }
        self.state = Some(state);
        let badge = self.badge()?;
        let announced = (badge.version, badge.kind);
        if self.announced.as_ref() == Some(&announced) {
            return None;
        }
        self.announced = Some(announced);
        self.announcement()
    }

    pub fn badge(&self) -> Option<Badge> {
        if let Some(version) = &self.ready {
            return Some(Badge { version: version.clone(), kind: BadgeKind::Ready });
        }
        let (version, kind) = match self.state.as_ref()? {
            UpdateState::Ready { version } => (version, BadgeKind::Ready),
            UpdateState::Available { version } if self.mode == UpdateMode::Notify => (version, BadgeKind::Available),
            UpdateState::Managed { version, .. } => (version, BadgeKind::Available),
            UpdateState::Available { .. } | UpdateState::Downloading { .. } => return None,
        };
        Some(Badge { version: version.clone(), kind })
    }

    pub fn announcement(&self) -> Option<String> {
        let badge = self.badge()?;
        let version = &badge.version;
        Some(match (&self.ready, &self.state, badge.kind) {
            (None, Some(UpdateState::Managed { reason, .. }), _) => {
                i18n::text_with("update.managed_available", &[("version", version), ("reason", reason)])
            }
            (_, _, BadgeKind::Ready) => i18n::text_with("update.ready_notice", &[("version", version)]),
            (_, _, BadgeKind::Available) => i18n::text_with("update.available_notice", &[("version", version)]),
        })
    }
}

#[cfg(test)]
mod tests;
