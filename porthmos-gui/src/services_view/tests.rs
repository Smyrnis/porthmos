use porthmos_core::{ActiveState, BootState, ServicesSnapshot, ServicesState as ServicesEvent, Unit};

use super::*;

pub(crate) fn unit(name: &str, active: ActiveState, sub: &str, boot: Option<BootState>) -> Unit {
    Unit {
        name: name.into(),
        description: format!("{name} description"),
        load: "loaded".into(),
        active,
        sub: sub.into(),
        boot,
    }
}

pub(crate) fn snapshot() -> ServicesSnapshot {
    let mut masked = unit("masked.service", ActiveState::Inactive, "dead", Some(BootState::Masked));
    masked.load = "masked".into();
    let mut missing = unit("missing.service", ActiveState::Inactive, "dead", None);
    missing.load = "not-found".into();
    ServicesSnapshot {
        units: vec![
            unit("nginx.service", ActiveState::Active, "running", Some(BootState::Enabled)),
            unit("cron.service", ActiveState::Active, "running", Some(BootState::Enabled)),
            unit("backup.service", ActiveState::Failed, "failed", Some(BootState::Enabled)),
            unit("bluetooth.service", ActiveState::Inactive, "dead", Some(BootState::Disabled)),
            unit("static.service", ActiveState::Inactive, "dead", Some(BootState::Static)),
            unit("odd\\x2dname.service", ActiveState::Active, "running", Some(BootState::Enabled)),
            masked,
            missing,
        ],
        uid: 1000,
    }
}

fn snapshot_event() -> ServicesEvent {
    ServicesEvent::Snapshot(Box::new(snapshot()))
}

fn names(units: &[&Unit]) -> Vec<String> {
    units.iter().map(|unit| unit.name.clone()).collect()
}

#[test]
fn a_new_state_shows_everything_with_nothing_selected() {
    let state = ServicesState::default();

    assert_eq!(state.status, StatusFilter::All);
    assert!(state.filter.is_empty() && state.selected.is_none() && state.status_text.is_none());
    assert!(state.state.is_none() && state.snapshot().is_none() && !state.failing());
}

#[test]
fn events_for_another_session_are_ignored() {
    let mut state = ServicesState::default();
    state.begin(3);

    state.apply(4, snapshot_event());
    assert!(state.state.is_none());
    state.apply(3, snapshot_event());
    assert!(state.snapshot().is_some());
}

#[test]
fn a_failure_keeps_the_last_list_and_a_recovery_or_unavailable_drops_it() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.apply(3, snapshot_event());

    state.apply(3, ServicesEvent::Failing("down".into()));
    assert!(state.failing() && state.snapshot().is_some());
    state.apply(3, ServicesEvent::Failing("still down".into()));
    assert!(state.failing() && state.snapshot().is_some());
    state.apply(3, ServicesEvent::Unavailable("no systemd".into()));
    assert!(!state.failing() && state.snapshot().is_none());
}

#[test]
fn a_failure_without_a_list_has_nothing_to_keep() {
    let mut state = ServicesState::default();
    state.begin(3);

    state.apply(3, ServicesEvent::Failing("down".into()));

    assert!(state.failing() && state.snapshot().is_none());
}

#[test]
fn beginning_or_clearing_forgets_the_list_the_selection_and_the_status() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.apply(3, snapshot_event());
    state.select("nginx.service".into());
    state.status_arrived(3, "nginx.service", "text".into());

    state.begin(4);
    assert!(state.state.is_none() && state.selected.is_none() && state.status_text.is_none());
    state.apply(4, snapshot_event());
    state.select("nginx.service".into());
    state.clear();
    assert!(state.session.is_none() && state.snapshot().is_none() && state.selected.is_none());
}

#[test]
fn selecting_the_same_unit_again_deselects_it_and_a_change_drops_the_old_status() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.select("nginx.service".into());
    state.status_arrived(3, "nginx.service", "text".into());

    state.select("cron.service".into());
    assert_eq!(state.selected.as_deref(), Some("cron.service"));
    assert!(state.status_text.is_none());
    state.select("cron.service".into());
    assert!(state.selected.is_none());
}

#[test]
fn a_unit_that_left_the_list_is_deselected() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.apply(3, snapshot_event());
    state.select("nginx.service".into());
    state.status_arrived(3, "nginx.service", "text".into());
    let mut smaller = snapshot();
    smaller.units.retain(|unit| unit.name != "nginx.service");

    state.apply(3, ServicesEvent::Snapshot(Box::new(smaller)));

    assert!(state.selected.is_none() && state.status_text.is_none());
}

#[test]
fn the_selection_survives_a_failure() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.apply(3, snapshot_event());
    state.select("nginx.service".into());

    state.apply(3, ServicesEvent::Failing("down".into()));

    assert_eq!(state.selected.as_deref(), Some("nginx.service"));
}

#[test]
fn status_text_is_kept_only_for_the_selected_unit_of_the_shown_session() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.select("nginx.service".into());

    state.status_arrived(4, "nginx.service", "other session".into());
    assert!(state.status_text.is_none());
    state.status_arrived(3, "cron.service", "other unit".into());
    assert!(state.status_text.is_none());
    state.status_arrived(3, "nginx.service", "active".into());
    assert_eq!(state.status_text, Some(("nginx.service".to_string(), "active".to_string())));
}

#[test]
fn requesting_the_status_shows_a_reading_note_until_the_text_arrives() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.select("nginx.service".into());

    state.status_requested("nginx.service");
    assert_eq!(state.status_text, Some(("nginx.service".to_string(), "Reading the status\u{2026}".to_string())));
    state.status_arrived(3, "nginx.service", "done".into());
    assert_eq!(state.status_text.map(|(_, text)| text), Some("done".to_string()));
}

#[test]
fn units_come_failed_first_then_running_then_by_name() {
    let snapshot = snapshot();
    let state = ServicesState::default();

    let shown = shown_units(&state, &snapshot);

    assert_eq!(
        names(&shown),
        [
            "backup.service",
            "cron.service",
            "nginx.service",
            "odd\\x2dname.service",
            "bluetooth.service",
            "masked.service",
            "missing.service",
            "static.service"
        ]
    );
}

#[test]
fn the_status_filter_and_the_text_filter_combine() {
    let snapshot = snapshot();
    let mut state = ServicesState { status: StatusFilter::Failed, ..ServicesState::default() };

    assert_eq!(names(&shown_units(&state, &snapshot)), ["backup.service"]);
    state.status = StatusFilter::Running;
    assert_eq!(shown_units(&state, &snapshot).len(), 3);
    state.filter = "NGINX".into();
    assert_eq!(names(&shown_units(&state, &snapshot)), ["nginx.service"]);
    state.status = StatusFilter::All;
    state.filter = "bluetooth.service description".into();
    assert_eq!(names(&shown_units(&state, &snapshot)), ["bluetooth.service"]);
    state.filter = "no such unit".into();
    assert!(shown_units(&state, &snapshot).is_empty());
}

#[test]
fn every_verb_is_enabled_for_a_loaded_enabled_or_disabled_unit() {
    let snapshot = snapshot();

    for name in ["nginx.service", "bluetooth.service"] {
        for verb in ALL_VERBS {
            assert_eq!(action_state(&snapshot, Some(name), verb), ActionState::Allowed, "{name} {verb:?}");
        }
    }
}

#[test]
fn a_static_unit_can_start_stop_and_restart_but_not_change_its_boot_state() {
    let snapshot = snapshot();

    for verb in [ServiceVerb::Start, ServiceVerb::Stop, ServiceVerb::Restart] {
        assert_eq!(action_state(&snapshot, Some("static.service"), verb), ActionState::Allowed);
    }
    for verb in [ServiceVerb::Enable, ServiceVerb::Disable] {
        assert_eq!(action_state(&snapshot, Some("static.service"), verb), ActionState::NotAvailable);
    }
}

#[test]
fn masked_and_missing_units_get_no_action_at_all() {
    let snapshot = snapshot();

    for name in ["masked.service", "missing.service"] {
        for verb in ALL_VERBS {
            assert_eq!(action_state(&snapshot, Some(name), verb), ActionState::NotAvailable, "{name} {verb:?}");
        }
    }
}

#[test]
fn a_unit_with_an_unsafe_name_gets_no_action_and_no_status() {
    let snapshot = snapshot();
    let name = Some("odd\\x2dname.service");

    for verb in ALL_VERBS {
        assert_eq!(action_state(&snapshot, name, verb), ActionState::NotAvailable, "{verb:?}");
    }
    assert!(!status_allowed(&snapshot, name));
    assert_eq!(unavailable_reason(&snapshot, name, ServiceVerb::Start), Some(i18n::text("gui.services.unsafe_name")));
}

#[test]
fn no_selection_or_an_unknown_unit_means_nothing_is_enabled() {
    let snapshot = snapshot();

    for selected in [None, Some("gone.service")] {
        for verb in ALL_VERBS {
            assert_eq!(action_state(&snapshot, selected, verb), ActionState::NoSelection);
        }
        assert!(!status_allowed(&snapshot, selected));
    }
}

#[test]
fn status_is_allowed_for_every_safely_named_unit() {
    let snapshot = snapshot();

    for name in ["nginx.service", "static.service", "masked.service", "missing.service"] {
        assert!(status_allowed(&snapshot, Some(name)), "{name}");
    }
}

#[test]
fn the_status_button_explains_itself_only_when_disabled() {
    let snapshot = snapshot();

    assert_eq!(status_reason(&snapshot, Some("nginx.service")), None);
    assert_eq!(status_reason(&snapshot, Some("odd\\x2dname.service")), Some(i18n::text("gui.services.unsafe_name")));
    assert_eq!(status_reason(&snapshot, None), Some(i18n::text("gui.services.select_first")));
}

#[test]
fn a_disabled_button_always_has_a_reason_and_an_enabled_one_has_none() {
    let snapshot = snapshot();
    let mut reasons = Vec::new();

    for name in ["nginx.service", "static.service", "masked.service", "missing.service", "odd\\x2dname.service"] {
        for verb in ALL_VERBS {
            let reason = unavailable_reason(&snapshot, Some(name), verb);
            let allowed = action_state(&snapshot, Some(name), verb) == ActionState::Allowed;
            assert_eq!(reason.is_none(), allowed, "{name} {verb:?}");
            reasons.extend(reason);
        }
    }
    assert!(reasons.iter().all(|reason| !reason.trim().is_empty()));
    assert_eq!(unavailable_reason(&snapshot, None, ServiceVerb::Start), Some(i18n::text("gui.services.select_first")));
    let masked = unavailable_reason(&snapshot, Some("masked.service"), ServiceVerb::Start);
    let missing = unavailable_reason(&snapshot, Some("missing.service"), ServiceVerb::Start);
    let static_boot = unavailable_reason(&snapshot, Some("static.service"), ServiceVerb::Enable);
    assert!(masked != missing && missing != static_boot && masked != static_boot);
}

#[test]
fn every_active_and_boot_state_has_a_text() {
    for state in [
        ActiveState::Active,
        ActiveState::Inactive,
        ActiveState::Failed,
        ActiveState::Activating,
        ActiveState::Deactivating,
        ActiveState::Other,
    ] {
        assert!(!active_text(state).is_empty(), "{state:?}");
    }
    for state in [
        BootState::Enabled,
        BootState::Disabled,
        BootState::Static,
        BootState::Masked,
        BootState::Alias,
        BootState::Indirect,
        BootState::Generated,
        BootState::Other,
    ] {
        assert!(!boot_text(Some(state)).is_empty(), "{state:?}");
    }
    assert_eq!(boot_text(None), "\u{2014}");
    assert_eq!(active_text(ActiveState::Failed), "failed");
    assert_eq!(boot_text(Some(BootState::Masked)), "masked");
}

#[test]
fn the_row_colour_marks_failed_units_and_quiets_the_inactive_ones() {
    use crate::theme::{Palette, ThemeSettings};
    use porthmos_core::theme::ColorScheme;
    for scheme in [ColorScheme::dark(), ColorScheme::light(), ColorScheme::high_contrast()] {
        let palette = Palette::new(&ThemeSettings::default().with_scheme(scheme));
        let failed = unit("a.service", ActiveState::Failed, "failed", None);
        let running = unit("b.service", ActiveState::Active, "running", None);
        let idle = unit("c.service", ActiveState::Inactive, "dead", None);

        assert_eq!(row_color(&failed, false, false, &palette), palette.err);
        assert_eq!(row_color(&running, false, false, &palette), palette.fg);
        assert_eq!(row_color(&idle, false, false, &palette), palette.fg_muted);
        assert_eq!(row_color(&running, true, false, &palette), palette.fg_focus);
        assert_eq!(row_color(&failed, false, true, &palette), palette.fg_dim);
    }
}

#[test]
fn every_new_text_exists_in_both_languages() {
    let english = include_str!("../../../porthmos-core/locales/en.yml");
    let greek = include_str!("../../../porthmos-core/locales/el.yml");
    for label in [
        "Services",
        "Show services",
        "Filter services",
        "All",
        "Active",
        "Boot",
        "Description",
        "Enable",
        "Disable",
        "Start service",
        "Stop service",
        "Restart service",
        "Enable service",
        "Disable service",
    ] {
        let key = format!("\n  {}:", porthmos_core::i18n::label_key(label).trim_start_matches("label."));
        assert!(english.contains(&key), "en lacks {label}");
        assert!(greek.contains(&key), "el lacks {label}");
    }
}

fn pending_state() -> ServicesState {
    let mut state = ServicesState::default();
    state.begin(3);
    state.apply(3, snapshot_event());
    state.select("nginx.service".into());
    state.status_requested("nginx.service");
    assert!(state.status_text.is_some());
    state
}

#[test]
fn the_reading_note_is_dropped_when_a_failure_arrives() {
    let mut state = pending_state();

    state.apply(3, ServicesEvent::Failing("down".into()));

    assert!(state.status_text.is_none());
}

#[test]
fn the_reading_note_is_dropped_when_the_services_become_unavailable() {
    let mut state = pending_state();

    state.apply(3, ServicesEvent::Unavailable("no systemd".into()));

    assert!(state.status_text.is_none());
}

#[test]
fn the_reading_note_survives_a_fresh_snapshot() {
    let mut state = pending_state();

    state.apply(3, snapshot_event());

    assert!(state.status_text.is_some());
    state.status_arrived(3, "nginx.service", "late text".into());
    assert_eq!(state.status_text.map(|(_, text)| text), Some("late text".to_string()));
}

#[test]
fn the_reading_note_is_dropped_when_the_session_changes_or_is_cleared() {
    let mut changed = pending_state();
    changed.begin(4);
    assert!(changed.status_text.is_none());

    let mut cleared = pending_state();
    cleared.clear();
    assert!(cleared.status_text.is_none());
}

#[test]
fn the_reading_note_is_dropped_when_the_selection_changes_and_does_not_come_back() {
    let mut state = pending_state();

    state.select("cron.service".into());
    assert!(state.status_text.is_none());
    state.select("nginx.service".into());
    state.apply(3, ServicesEvent::Failing("down".into()));
    assert!(state.status_text.is_none());
}

#[test]
fn a_status_that_arrived_stays_through_a_failure() {
    let mut state = pending_state();
    state.status_arrived(3, "nginx.service", "active".into());

    state.apply(3, ServicesEvent::Failing("down".into()));

    assert_eq!(state.status_text.map(|(_, text)| text), Some("active".to_string()));
}

#[test]
fn while_updating_has_failed_every_verb_and_status_is_blocked_with_one_reason() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.apply(3, snapshot_event());
    state.select("nginx.service".into());
    let snapshot = snapshot();
    for verb in ALL_VERBS {
        assert_eq!(verb_blocked(&state, &snapshot, verb), None, "{verb:?}");
    }
    assert_eq!(status_blocked(&state, &snapshot), None);

    state.apply(3, ServicesEvent::Failing("down".into()));

    let reason = i18n::text("gui.services.retry_first");
    assert_eq!(reason, "Updating failed, retry first");
    for verb in ALL_VERBS {
        assert_eq!(verb_blocked(&state, &snapshot, verb), Some(reason.clone()), "{verb:?}");
    }
    assert_eq!(status_blocked(&state, &snapshot), Some(reason));
}

#[test]
fn without_a_failure_the_blocking_reason_is_the_unit_specific_one() {
    let mut state = ServicesState::default();
    state.begin(3);
    state.apply(3, snapshot_event());
    state.select("masked.service".into());
    let snapshot = snapshot();

    assert_eq!(
        verb_blocked(&state, &snapshot, ServiceVerb::Start),
        unavailable_reason(&snapshot, Some("masked.service"), ServiceVerb::Start)
    );
    assert_eq!(status_blocked(&state, &snapshot), None);
}
