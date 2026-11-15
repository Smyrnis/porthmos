use porthmos_core::{UpdateMode, UpdateState};

use super::{BADGE_FULL_WIDTH, BADGE_VERSION_WIDTH, Badge, BadgeKind, UpdateView, badge_text};

fn ready(version: &str) -> UpdateState {
    UpdateState::Ready { version: version.to_string() }
}

fn available(version: &str) -> UpdateState {
    UpdateState::Available { version: version.to_string() }
}

fn downloading(version: &str) -> UpdateState {
    UpdateState::Downloading { version: version.to_string(), downloaded: 5, total: 10 }
}

fn managed(version: &str) -> UpdateState {
    UpdateState::Managed { version: version.to_string(), reason: "apt".to_string() }
}

fn badge(version: &str, kind: BadgeKind) -> Option<Badge> {
    Some(Badge { version: version.to_string(), kind })
}

#[test]
fn a_ready_update_shows_an_installs_on_quit_badge() {
    let mut view = UpdateView::new(UpdateMode::Auto);

    let announced = view.apply(ready("0.4.0"));

    assert_eq!(view.badge(), badge("0.4.0", BadgeKind::Ready));
    assert_eq!(announced.as_deref(), Some("Porthmos 0.4.0 is ready and will be installed when you quit"));
    assert_eq!(view.badge().unwrap().tiers()[0], "\u{2191} 0.4.0 installs on quit");
}

#[test]
fn available_shows_a_badge_only_in_notify_mode() {
    let mut auto = UpdateView::new(UpdateMode::Auto);
    let mut notify = UpdateView::new(UpdateMode::Notify);

    assert_eq!(auto.apply(available("0.4.0")), None);
    let announced = notify.apply(available("0.4.0"));

    assert_eq!(auto.badge(), None);
    assert_eq!(notify.badge(), badge("0.4.0", BadgeKind::Available));
    assert_eq!(announced.as_deref(), Some("Porthmos 0.4.0 is available"));
    assert_eq!(notify.badge().unwrap().tiers()[0], "\u{2191} 0.4.0 available");
}

#[test]
fn managed_shows_an_available_badge_with_the_reason_in_the_announcement() {
    let mut view = UpdateView::new(UpdateMode::Auto);

    let announced = view.apply(managed("0.4.0"));

    assert_eq!(view.badge(), badge("0.4.0", BadgeKind::Available));
    assert_eq!(
        announced.as_deref(),
        Some("Porthmos 0.4.0 is available. This copy is managed (apt); update it with your package manager.")
    );
}

#[test]
fn downloading_shows_no_badge() {
    let mut view = UpdateView::new(UpdateMode::Notify);

    assert_eq!(view.apply(downloading("0.4.0")), None);

    assert_eq!(view.badge(), None);
    assert_eq!(view.announcement(), None);
}

#[test]
fn ready_is_sticky_when_a_later_state_says_available() {
    let mut view = UpdateView::new(UpdateMode::Notify);
    view.apply(ready("0.4.0"));

    let announced = view.apply(available("0.4.0"));
    view.apply(downloading("0.4.0"));

    assert_eq!(announced, None);
    assert_eq!(view.badge(), badge("0.4.0", BadgeKind::Ready));
    assert_eq!(view.announcement().as_deref(), Some("Porthmos 0.4.0 is ready and will be installed when you quit"));
}

#[test]
fn apply_announces_once_per_version_and_kind() {
    let mut view = UpdateView::new(UpdateMode::Notify);

    assert!(view.apply(available("0.4.0")).is_some());
    assert_eq!(view.apply(available("0.4.0")), None);
    assert!(view.apply(ready("0.4.0")).is_some());
    assert_eq!(view.apply(ready("0.4.0")), None);
    assert_eq!(view.apply(downloading("0.4.0")), None);
    assert_eq!(view.apply(ready("0.4.0")), None);
    assert!(view.apply(ready("0.5.0")).is_some());
}

#[test]
fn the_badge_tiers_shrink_to_an_arrow() {
    let badge = Badge { version: "1.2.3".to_string(), kind: BadgeKind::Available };

    assert_eq!(
        badge.tiers(),
        ["\u{2191} 1.2.3 available".to_string(), "\u{2191} 1.2.3".to_string(), "\u{2191}".to_string()]
    );
}

#[test]
fn badge_text_picks_the_tier_by_window_width() {
    let badge = Badge { version: "1.2.3".to_string(), kind: BadgeKind::Ready };

    assert_eq!(BADGE_FULL_WIDTH, 900.0);
    assert_eq!(BADGE_VERSION_WIDTH, 640.0);
    assert_eq!(badge_text(&badge, 900.0), "\u{2191} 1.2.3 installs on quit");
    assert_eq!(badge_text(&badge, 899.0), "\u{2191} 1.2.3");
    assert_eq!(badge_text(&badge, 640.0), "\u{2191} 1.2.3");
    assert_eq!(badge_text(&badge, 639.0), "\u{2191}");
}
