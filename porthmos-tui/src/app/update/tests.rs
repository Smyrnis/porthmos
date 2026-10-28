use porthmos_core::{Command, Event, Severity, UpdateMode, UpdateState, VfsPath};

use crate::{
    app::testing::{TestApp, test_app},
    widgets::update_badge::{BadgeKind, UpdateBadge},
};

fn app_in(mode: UpdateMode) -> TestApp {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app = test.app.with_update_mode(mode);
    test
}

fn ready(version: &str) -> Event {
    Event::Update(UpdateState::Ready { version: version.to_string() })
}

fn notices(test: &mut TestApp) -> Vec<(Severity, String)> {
    let mut found = Vec::new();
    while let Some(current) = test.app.notifications.current() {
        found.push((current.severity, current.message.clone()));
        test.app.notifications.dismiss_current();
    }
    found
}

fn badge(version: &str, kind: BadgeKind) -> Option<UpdateBadge> {
    Some(UpdateBadge { version: version.to_string(), kind })
}

#[test]
fn ready_shows_a_ready_badge_and_one_notice() {
    let mut test = app_in(UpdateMode::Auto);

    test.app.apply_core_event(ready("0.4.0"));

    assert_eq!(test.app.update_badge(), badge("0.4.0", BadgeKind::Ready));
    assert_eq!(
        notices(&mut test),
        [(Severity::Info, "Porthmos 0.4.0 is ready and will be installed when you quit".to_string())]
    );
}

#[test]
fn repeated_ready_events_for_one_version_give_one_notice() {
    let mut test = app_in(UpdateMode::Auto);

    test.app.apply_core_event(ready("0.4.0"));
    test.app.apply_core_event(Event::Update(UpdateState::Downloading {
        version: "0.4.0".to_string(),
        downloaded: 5,
        total: 10,
    }));
    test.app.apply_core_event(ready("0.4.0"));

    assert_eq!(notices(&mut test).len(), 1);
}

#[test]
fn a_newer_ready_version_gets_its_own_notice() {
    let mut test = app_in(UpdateMode::Auto);

    test.app.apply_core_event(ready("0.4.0"));
    test.app.apply_core_event(ready("0.5.0"));

    let found = notices(&mut test);
    assert_eq!(found.len(), 2);
    assert!(found[1].1.contains("0.5.0"));
    assert_eq!(test.app.update_badge(), badge("0.5.0", BadgeKind::Ready));
}

#[test]
fn available_in_auto_mode_shows_no_badge_and_no_notice() {
    let mut test = app_in(UpdateMode::Auto);

    test.app.apply_core_event(Event::Update(UpdateState::Available { version: "0.4.0".to_string() }));

    assert_eq!(test.app.update_badge(), None);
    assert!(notices(&mut test).is_empty());
}

#[test]
fn downloading_shows_no_badge() {
    let mut test = app_in(UpdateMode::Auto);

    test.app.apply_core_event(Event::Update(UpdateState::Downloading {
        version: "0.4.0".to_string(),
        downloaded: 1,
        total: 2,
    }));

    assert_eq!(test.app.update_badge(), None);
    assert!(notices(&mut test).is_empty());
}

#[test]
fn available_in_notify_mode_shows_an_available_badge_and_the_available_notice() {
    let mut test = app_in(UpdateMode::Notify);

    test.app.apply_core_event(Event::Update(UpdateState::Available { version: "0.4.0".to_string() }));

    assert_eq!(test.app.update_badge(), badge("0.4.0", BadgeKind::Available));
    assert_eq!(notices(&mut test), [(Severity::Info, "Porthmos 0.4.0 is available".to_string())]);
}

#[test]
fn managed_shows_an_available_badge_and_the_notice_with_the_reason() {
    let mut test = app_in(UpdateMode::Auto);

    test.app.apply_core_event(Event::Update(UpdateState::Managed {
        version: "0.4.0".to_string(),
        reason: "installed under /usr".to_string(),
    }));

    assert_eq!(test.app.update_badge(), badge("0.4.0", BadgeKind::Available));
    assert_eq!(
        notices(&mut test),
        [(
            Severity::Info,
            "Porthmos 0.4.0 is available. This copy is managed (installed under /usr); update it with your package manager."
                .to_string()
        )]
    );
}

#[test]
fn checking_for_updates_sends_a_manual_check_when_nothing_is_showing() {
    let mut test = app_in(UpdateMode::Auto);

    test.app.check_for_updates();

    assert_eq!(test.sent(), [Command::CheckForUpdate { manual: true }]);
}

#[test]
fn checking_for_updates_with_a_badge_showing_repeats_the_notice_and_sends_nothing() {
    let mut test = app_in(UpdateMode::Auto);
    test.app.apply_core_event(ready("0.4.0"));
    notices(&mut test);

    test.app.check_for_updates();

    assert!(test.sent().is_empty());
    assert_eq!(
        notices(&mut test),
        [(Severity::Info, "Porthmos 0.4.0 is ready and will be installed when you quit".to_string())]
    );
}

#[test]
fn a_ready_badge_survives_a_newer_version_being_found_and_downloaded() {
    let mut test = app_in(UpdateMode::Auto);
    test.app.apply_core_event(ready("0.4.0"));

    test.app.apply_core_event(Event::Update(UpdateState::Available { version: "0.5.0".to_string() }));
    assert_eq!(test.app.update_badge(), badge("0.4.0", BadgeKind::Ready));

    test.app.apply_core_event(Event::Update(UpdateState::Downloading {
        version: "0.5.0".to_string(),
        downloaded: 1,
        total: 2,
    }));
    assert_eq!(test.app.update_badge(), badge("0.4.0", BadgeKind::Ready));

    test.app.apply_core_event(ready("0.5.0"));
    assert_eq!(test.app.update_badge(), badge("0.5.0", BadgeKind::Ready));
}
