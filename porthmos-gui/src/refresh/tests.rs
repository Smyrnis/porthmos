use std::time::Duration;

use porthmos_core::{RefreshSetting, RefreshSettings, RefreshTab};

use super::{setting, setting_mut, tick_interval};

fn on(seconds: u64) -> RefreshSetting {
    RefreshSetting { auto: true, interval_seconds: seconds }
}

#[test]
fn there_is_no_tick_while_auto_refresh_is_off() {
    assert_eq!(tick_interval(&RefreshSetting::default(), false), None);
    assert_eq!(tick_interval(&RefreshSetting { auto: false, interval_seconds: 60 }, true), None);
}

#[test]
fn the_tick_follows_the_interval_while_healthy() {
    assert_eq!(tick_interval(&on(10), false), Some(Duration::from_secs(10)));
    assert_eq!(tick_interval(&on(300), false), Some(Duration::from_secs(300)));
}

#[test]
fn a_failing_tab_slows_down_to_thirty_seconds() {
    assert_eq!(tick_interval(&on(10), true), Some(Duration::from_secs(30)));
    assert_eq!(tick_interval(&on(5), true), Some(Duration::from_secs(30)));
}

#[test]
fn a_failing_tab_never_speeds_up_past_a_longer_interval() {
    assert_eq!(tick_interval(&on(60), true), Some(Duration::from_secs(60)));
    assert_eq!(tick_interval(&on(30), true), Some(Duration::from_secs(30)));
}

#[test]
fn each_tab_reaches_its_own_setting() {
    let mut settings = RefreshSettings::default();

    setting_mut(&mut settings, RefreshTab::Services).auto = true;
    setting_mut(&mut settings, RefreshTab::Ports).interval_seconds = 90;

    assert_eq!(settings.services, on(10));
    assert_eq!(settings.ports, RefreshSetting { auto: false, interval_seconds: 90 });
    assert_eq!(settings.health, RefreshSetting::default());
    assert_eq!(*setting(&settings, RefreshTab::Ports), settings.ports);
}

#[test]
fn every_tab_has_its_own_index() {
    let indexes: Vec<_> = super::ALL.iter().map(|tab| super::index(*tab)).collect();

    assert_eq!(indexes, vec![0, 1, 2]);
}
