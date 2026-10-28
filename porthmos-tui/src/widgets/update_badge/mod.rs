use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    Ready,
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateBadge {
    pub version: String,
    pub kind: BadgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadgeFit {
    pub badge: Option<String>,
    pub status_budget: usize,
}

pub const GAP: usize = 3;
const ARROW: &str = "\u{2191}";

impl UpdateBadge {
    pub fn tiers(&self) -> [String; 3] {
        let suffix = match self.kind {
            BadgeKind::Ready => "installs on quit",
            BadgeKind::Available => "available",
        };
        [format!("{ARROW} {} {suffix}", self.version), format!("{ARROW} {}", self.version), ARROW.to_string()]
    }

    pub fn fit(&self, room: usize, status_width: usize) -> BadgeFit {
        let gap = if status_width > 0 { GAP } else { 0 };
        let beside_status = self.tiers().into_iter().find(|tier| status_width + gap + tier.width() <= room);
        if let Some(tier) = beside_status {
            return BadgeFit { badge: Some(tier), status_budget: status_width };
        }
        if room < ARROW.width() {
            return BadgeFit { badge: None, status_budget: 0 };
        }
        BadgeFit { badge: Some(ARROW.to_string()), status_budget: room.saturating_sub(ARROW.width() + GAP) }
    }
}

#[cfg(test)]
mod tests;
