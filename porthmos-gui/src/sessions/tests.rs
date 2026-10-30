use super::{Sessions, TabInfo};

fn three() -> Sessions {
    let mut sessions = Sessions::default();
    sessions.insert(1, "one".to_string());
    sessions.insert(2, "two".to_string());
    sessions.insert(3, "three".to_string());
    sessions
}

fn active_id(sessions: &Sessions) -> Option<u64> {
    sessions.active().map(|session| session.id)
}

#[test]
fn inserting_a_session_makes_it_active() {
    let mut sessions = Sessions::default();
    assert!(sessions.active().is_none());

    sessions.insert(7, "alpha".to_string());

    assert_eq!(active_id(&sessions), Some(7));
    assert!(sessions.active().unwrap().healthy);
}

#[test]
fn removing_the_active_tab_activates_the_one_that_takes_its_place() {
    let mut sessions = three();
    sessions.set_active(1);

    assert!(sessions.remove(2).is_some());

    assert_eq!(active_id(&sessions), Some(3));
}

#[test]
fn removing_the_active_last_tab_activates_the_previous_one() {
    let mut sessions = three();

    sessions.remove(3);

    assert_eq!(active_id(&sessions), Some(2));
}

#[test]
fn removing_a_tab_left_of_the_active_one_keeps_the_active_tab() {
    let mut sessions = three();

    sessions.remove(1);

    assert_eq!(active_id(&sessions), Some(3));
}

#[test]
fn removing_the_last_tab_leaves_nothing_active() {
    let mut sessions = Sessions::default();
    sessions.insert(1, "one".to_string());

    sessions.remove(1);

    assert!(sessions.active().is_none());
    assert!(sessions.is_empty());
}

#[test]
fn removing_an_unknown_session_changes_nothing() {
    let mut sessions = three();

    assert!(sessions.remove(99).is_none());

    assert_eq!(sessions.names().len(), 3);
    assert_eq!(active_id(&sessions), Some(3));
}

#[test]
fn next_wraps_around_the_tabs() {
    let mut sessions = three();

    sessions.next();
    assert_eq!(active_id(&sessions), Some(1));
    sessions.next();
    assert_eq!(active_id(&sessions), Some(2));
}

#[test]
fn set_active_ignores_an_index_past_the_tabs() {
    let mut sessions = three();

    assert!(!sessions.set_active(3));

    assert_eq!(active_id(&sessions), Some(3));
}

#[test]
fn names_marks_the_active_and_the_unhealthy_tab() {
    let mut sessions = three();
    sessions.by_id_mut(1).unwrap().healthy = false;

    assert_eq!(
        sessions.names(),
        vec![
            TabInfo { id: 1, name: "one".to_string(), healthy: false, active: false },
            TabInfo { id: 2, name: "two".to_string(), healthy: true, active: false },
            TabInfo { id: 3, name: "three".to_string(), healthy: true, active: true },
        ]
    );
}

#[test]
fn activate_selects_a_tab_by_session_id() {
    let mut sessions = three();

    assert!(sessions.activate(1));
    assert_eq!(active_id(&sessions), Some(1));
    assert!(!sessions.activate(42));
    assert_eq!(active_id(&sessions), Some(1));
}

#[test]
fn previous_wraps_around_the_tabs() {
    let mut sessions = three();

    sessions.previous();
    assert_eq!(active_id(&sessions), Some(2));
    sessions.set_active(0);
    sessions.previous();
    assert_eq!(active_id(&sessions), Some(3));
}

#[test]
fn next_and_previous_do_nothing_without_tabs() {
    let mut sessions = Sessions::default();

    sessions.next();
    sessions.previous();

    assert!(sessions.active().is_none());
}

#[test]
fn a_session_is_found_by_its_name() {
    let sessions = three();

    assert_eq!(sessions.by_name("two").map(|session| session.id), Some(2));
    assert!(sessions.by_name("four").is_none());
}
