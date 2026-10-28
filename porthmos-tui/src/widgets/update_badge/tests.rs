use unicode_width::UnicodeWidthStr;

use super::*;

fn badge(version: &str, kind: BadgeKind) -> UpdateBadge {
    UpdateBadge { version: version.to_string(), kind }
}

#[test]
fn tiers_for_a_ready_update_read_installs_on_quit_then_version_then_arrow() {
    assert_eq!(
        badge("0.4.0", BadgeKind::Ready).tiers(),
        ["\u{2191} 0.4.0 installs on quit", "\u{2191} 0.4.0", "\u{2191}"]
    );
}

#[test]
fn tiers_for_an_available_update_read_available() {
    assert_eq!(
        badge("0.4.0", BadgeKind::Available).tiers(),
        ["\u{2191} 0.4.0 available", "\u{2191} 0.4.0", "\u{2191}"]
    );
}

#[test]
fn the_fullest_tier_that_fits_beside_the_status_is_chosen() {
    let badge = badge("0.4.0", BadgeKind::Ready);

    assert_eq!(badge.fit(80, 11), BadgeFit { badge: Some(badge.tiers()[0].clone()), status_budget: 11 });
    let room = 11 + 3 + badge.tiers()[1].width();
    assert_eq!(badge.fit(room, 11), BadgeFit { badge: Some(badge.tiers()[1].clone()), status_budget: 11 });
    assert_eq!(badge.fit(11 + 3 + 1, 11), BadgeFit { badge: Some("\u{2191}".to_string()), status_budget: 11 });
}

#[test]
fn without_a_status_there_is_no_gap() {
    let badge = badge("0.4.0", BadgeKind::Ready);

    let full = badge.tiers()[0].width();

    assert_eq!(badge.fit(full, 0), BadgeFit { badge: Some(badge.tiers()[0].clone()), status_budget: 0 });
}

#[test]
fn the_arrow_shrinks_the_status_when_nothing_else_fits() {
    let badge = badge("0.4.0", BadgeKind::Ready);

    let fit = badge.fit(8, 11);

    assert_eq!(fit, BadgeFit { badge: Some("\u{2191}".to_string()), status_budget: 4 });
}

#[test]
fn the_status_is_dropped_when_only_the_arrow_fits() {
    let badge = badge("0.4.0", BadgeKind::Ready);

    assert_eq!(badge.fit(1, 11), BadgeFit { badge: Some("\u{2191}".to_string()), status_budget: 0 });
    assert_eq!(badge.fit(3, 11), BadgeFit { badge: Some("\u{2191}".to_string()), status_budget: 0 });
}

#[test]
fn nothing_fits_in_a_room_of_zero() {
    assert_eq!(badge("0.4.0", BadgeKind::Ready).fit(0, 11), BadgeFit { badge: None, status_budget: 0 });
}

#[test]
fn a_long_prerelease_version_steps_down_instead_of_overflowing() {
    let badge = badge("0.4.0-rc.1", BadgeKind::Ready);

    for room in 0..60 {
        for status_width in [0, 11] {
            let fit = badge.fit(room, status_width);
            let used = fit.badge.as_ref().map_or(0, |text| text.width())
                + if fit.badge.is_some() && fit.status_budget > 0 { 3 } else { 0 }
                + fit.status_budget;
            assert!(used <= room, "room {room}, status {status_width}: {fit:?}");
        }
    }
}
