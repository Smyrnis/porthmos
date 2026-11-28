use super::*;

fn running(tabs: &mut LogTabs, index: usize, session: SessionId) -> LogId {
    let id = tabs.take_id();
    tabs.tabs[index].running = Some((id, session));
    id
}

fn with_tabs(count: usize) -> LogTabs {
    let mut tabs = LogTabs::default();
    for _ in 1..count {
        assert!(tabs.add());
    }
    tabs
}

#[test]
fn a_new_container_has_one_empty_active_tab() {
    let tabs = LogTabs::default();

    assert_eq!(tabs.tabs().len(), 1);
    assert_eq!(tabs.active_index(), 0);
    assert!(tabs.active().is_pristine());
}

#[test]
fn add_makes_the_new_tab_active_and_stops_at_six() {
    let mut tabs = LogTabs::default();

    for expected in 1..MAX_LOG_TABS {
        assert!(tabs.add());
        assert_eq!(tabs.active_index(), expected);
    }
    assert!(!tabs.add());
    assert_eq!(tabs.tabs().len(), MAX_LOG_TABS);
    assert_eq!(tabs.active_index(), MAX_LOG_TABS - 1);
}

#[test]
fn the_cap_is_six() {
    assert_eq!(MAX_LOG_TABS, 6);
}

#[test]
fn can_add_is_false_at_the_cap() {
    assert!(with_tabs(5).can_add());
    assert!(!with_tabs(6).can_add());
}

#[test]
fn take_id_gives_increasing_ids_across_tabs() {
    let mut tabs = with_tabs(2);

    let first = tabs.take_id();
    let second = tabs.take_id();

    assert!(second > first);
}

#[test]
fn select_ignores_an_out_of_range_index() {
    let mut tabs = with_tabs(2);

    assert!(!tabs.select(2));
    assert_eq!(tabs.active_index(), 1);
}

#[test]
fn selecting_a_tab_marks_it_to_restore_its_scroll() {
    let mut tabs = with_tabs(2);
    tabs.tabs[0].was_visible = true;

    assert!(tabs.select(0));

    assert_eq!(tabs.active_index(), 0);
    assert!(!tabs.active().was_visible);
}

#[test]
fn close_returns_the_running_id_of_that_tab_only() {
    let mut tabs = with_tabs(2);
    let first = running(&mut tabs, 0, 3);
    let second = running(&mut tabs, 1, 3);

    assert_eq!(tabs.close(0), Some(first));

    assert_eq!(tabs.tabs().len(), 1);
    assert_eq!(tabs.active().running, Some((second, 3)));
}

#[test]
fn closing_an_idle_tab_returns_no_id() {
    let mut tabs = with_tabs(2);

    assert_eq!(tabs.close(1), None);
    assert_eq!(tabs.tabs().len(), 1);
}

#[test]
fn closing_the_only_tab_leaves_one_fresh_tab() {
    let mut tabs = LogTabs::default();
    let id = running(&mut tabs, 0, 3);
    tabs.active_mut().path = "/var/log/x".to_string();

    assert_eq!(tabs.close(0), Some(id));

    assert_eq!(tabs.tabs().len(), 1);
    assert!(tabs.active().is_pristine());
}

#[test]
fn closing_an_earlier_tab_keeps_the_same_tab_active() {
    let mut tabs = with_tabs(3);
    tabs.tabs[2].path = "/c".to_string();
    assert!(tabs.select(2));

    tabs.close(0);

    assert_eq!(tabs.active_index(), 1);
    assert_eq!(tabs.active().path, "/c");
}

#[test]
fn closing_the_active_last_tab_clamps_the_index() {
    let mut tabs = with_tabs(3);

    tabs.close(2);

    assert_eq!(tabs.tabs().len(), 2);
    assert_eq!(tabs.active_index(), 1);
}

#[test]
fn closing_the_active_middle_tab_activates_the_one_that_took_its_place() {
    let mut tabs = with_tabs(3);
    tabs.tabs[2].path = "/c".to_string();
    assert!(tabs.select(1));

    tabs.close(1);

    assert_eq!(tabs.active_index(), 1);
    assert_eq!(tabs.active().path, "/c");
}

#[test]
fn closing_an_out_of_range_index_changes_nothing() {
    let mut tabs = with_tabs(2);

    assert_eq!(tabs.close(5), None);
    assert_eq!(tabs.tabs().len(), 2);
}

#[test]
fn by_id_mut_finds_the_owning_tab_and_none_for_a_closed_id() {
    let mut tabs = with_tabs(2);
    let first = running(&mut tabs, 0, 3);
    let second = running(&mut tabs, 1, 4);

    assert_eq!(tabs.by_id_mut(second).map(|tab| tab.running), Some(Some((second, 4))));
    assert_eq!(tabs.by_id_mut(first).map(|tab| tab.running), Some(Some((first, 3))));
    tabs.close(0);
    assert!(tabs.by_id_mut(first).is_none());
    assert!(tabs.by_id_mut(999).is_none());
}

fn journal(tabs: &mut LogTabs, index: usize, unit: &str) {
    tabs.tabs[index].kind = SourceKind::Journal;
    tabs.tabs[index].unit = unit.to_string();
}

#[test]
fn following_finds_a_running_journal_tab_for_that_unit_and_session() {
    let mut tabs = with_tabs(3);
    journal(&mut tabs, 1, "nginx.service");
    running(&mut tabs, 1, 3);

    assert_eq!(tabs.following("nginx.service", 3), Some(1));
    assert_eq!(tabs.following(" nginx.service ", 3), Some(1));
}

#[test]
fn following_ignores_other_units_sessions_stopped_tabs_and_files() {
    let mut tabs = with_tabs(3);
    journal(&mut tabs, 0, "nginx.service");
    journal(&mut tabs, 1, "sshd.service");
    running(&mut tabs, 1, 3);
    tabs.tabs[2].path = "nginx.service".to_string();
    running(&mut tabs, 2, 3);

    assert_eq!(tabs.following("nginx.service", 3), None);
    assert_eq!(tabs.following("sshd.service", 4), None);
    assert_eq!(tabs.following("cron.service", 3), None);
}

#[test]
fn trim_all_trims_every_tab() {
    let mut tabs = with_tabs(2);
    for tab in tabs.tabs.iter_mut() {
        let lines =
            (0..10).map(|n| porthmos_core::LogLine { text: n.to_string(), level: porthmos_core::LogLevel::Info });
        tab.append(lines.collect(), 0, 100);
    }

    tabs.trim_all(3);

    assert!(tabs.tabs().iter().all(|tab| tab.lines.len() == 3));
}

#[test]
fn closing_the_active_tab_marks_the_tab_underneath_to_restore_its_scroll() {
    let mut tabs = with_tabs(2);
    tabs.tabs[0].was_visible = true;
    tabs.tabs[1].was_visible = true;

    tabs.close(1);

    assert_eq!(tabs.active_index(), 0);
    assert!(!tabs.active().was_visible);
}

#[test]
fn closing_another_tab_leaves_the_active_tabs_restore_flag_alone() {
    let mut tabs = with_tabs(3);
    tabs.tabs[2].was_visible = true;

    tabs.close(0);

    assert_eq!(tabs.active_index(), 1);
    assert!(tabs.active().was_visible);
}

#[test]
fn the_tabs_full_text_names_the_cap() {
    assert!(tabs_full_text().contains(&MAX_LOG_TABS.to_string()));
}

#[test]
fn following_file_finds_the_running_tab_on_that_session_only() {
    let mut tabs = with_tabs(3);
    tabs.tabs[1].kind = SourceKind::File;
    tabs.tabs[1].path = "/var/log/app.log".to_string();
    tabs.tabs[1].running = Some((9, 3));
    tabs.tabs[2].kind = SourceKind::Journal;
    tabs.tabs[2].unit = "/var/log/app.log".to_string();
    tabs.tabs[2].running = Some((10, 3));

    assert_eq!(tabs.following_file("/var/log/app.log", 3), Some(1));
    assert_eq!(tabs.following_file("/var/log/app.log", 4), None);
    assert_eq!(tabs.following_file("/var/log/other.log", 3), None);
    tabs.tabs[1].running = None;
    assert_eq!(tabs.following_file("/var/log/app.log", 3), None);
}
