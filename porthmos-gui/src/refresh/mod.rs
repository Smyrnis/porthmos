use std::time::Duration;

use porthmos_core::{RefreshSetting, RefreshSettings, RefreshTab};

const FAILING_FLOOR: Duration = Duration::from_secs(30);

pub const ALL: [RefreshTab; 3] = [RefreshTab::Health, RefreshTab::Services, RefreshTab::Ports];

pub fn index(tab: RefreshTab) -> usize {
    match tab {
        RefreshTab::Health => 0,
        RefreshTab::Services => 1,
        RefreshTab::Ports => 2,
    }
}

pub fn setting(settings: &RefreshSettings, tab: RefreshTab) -> &RefreshSetting {
    match tab {
        RefreshTab::Health => &settings.health,
        RefreshTab::Services => &settings.services,
        RefreshTab::Ports => &settings.ports,
    }
}

pub fn setting_mut(settings: &mut RefreshSettings, tab: RefreshTab) -> &mut RefreshSetting {
    match tab {
        RefreshTab::Health => &mut settings.health,
        RefreshTab::Services => &mut settings.services,
        RefreshTab::Ports => &mut settings.ports,
    }
}

pub fn tick_interval(setting: &RefreshSetting, failing: bool) -> Option<Duration> {
    let interval = Duration::from_secs(setting.interval_seconds);
    setting.auto.then(|| if failing { interval.max(FAILING_FLOOR) } else { interval })
}

#[cfg(test)]
mod tests;
