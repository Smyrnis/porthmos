use porthmos_core::{
    Command, Entry, Event, Location, LogLevel, LogLine, LogSource, LogState, LogsSettings, UnitName, VfsPath,
};

use crate::app::Message;
use crate::drawer::DrawerView;
use crate::logs_view::{LogStatus, SourceKind};
use crate::settings_view::GuiSettings;
use crate::shortcuts::Action;
use crate::testing::Harness;

fn connect(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: VfsPath::new("/srv"),
        entries: Vec::<Entry>::new(),
    });
}

fn line(text: &str) -> LogLine {
    LogLine { text: text.to_string(), level: LogLevel::Info }
}

fn batch(texts: &[&str], skipped: u64) -> LogState {
    LogState::Lines { lines: texts.iter().map(|text| line(text)).collect(), skipped }
}

fn starts(sent: &[Command]) -> Vec<(u64, u64, LogSource, u32)> {
    sent.iter()
        .filter_map(|command| match command {
            Command::StartLog { id, session, source, initial_lines } => {
                Some((*id, *session, source.clone(), *initial_lines))
            }
            _ => None,
        })
        .collect()
}

fn stops(sent: &[Command]) -> Vec<u64> {
    sent.iter().filter_map(|command| if let Command::StopLog { id } = command { Some(*id) } else { None }).collect()
}

fn ready() -> Harness {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::LogsPathChanged("/var/log/syslog".into()));
    harness.sent();
    harness
}

fn started(harness: &mut Harness) -> u64 {
    harness.press(Message::LogsStartRequested);
    let sent = harness.sent();
    let id = starts(&sent)[0].0;
    harness.feed(Event::Log { id, state: LogState::Started });
    id
}

fn shown(harness: &Harness) -> Vec<String> {
    harness.app.logs().lines.iter().map(|stored| stored.line.text.clone()).collect()
}

#[test]
fn starting_sends_the_source_the_session_and_the_initial_line_count() {
    let mut harness = ready();

    harness.press(Message::LogsStartRequested);

    let sent = harness.sent();
    let all = starts(&sent);
    assert_eq!(all.len(), 1, "{sent:?}");
    let (_, session, source, initial) = all[0].clone();
    assert_eq!(session, 3);
    assert_eq!(source, LogSource::File("/var/log/syslog".to_string()));
    assert_eq!(initial, LogsSettings::default().initial_lines);
    assert_eq!(harness.app.logs().status, LogStatus::Starting);
}

#[test]
fn the_initial_line_count_follows_the_setting() {
    let mut settings = GuiSettings::default();
    settings.logs.initial_lines = 42;
    let mut harness = Harness::with_gui_settings(settings);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::LogsPathChanged("/a".into()));
    harness.sent();

    harness.press(Message::LogsStartRequested);

    assert_eq!(starts(&harness.sent())[0].3, 42);
}

#[test]
fn starting_without_a_connection_or_a_usable_source_sends_nothing() {
    let mut bare = Harness::new();
    bare.sent();
    bare.press(Message::LogsPathChanged("/a".into()));
    bare.press(Message::LogsStartRequested);
    assert!(bare.sent().is_empty());

    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.sent();
    harness.press(Message::LogsStartRequested);
    assert!(harness.sent().is_empty());

    harness.press(Message::LogsKindChosen(SourceKind::Journal));
    harness.press(Message::LogsUnitChanged("x.service; reboot".into()));
    harness.press(Message::LogsStartRequested);
    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.logs().running, None);
}

#[test]
fn a_journal_source_starts_with_the_checked_unit() {
    let mut harness = ready();
    harness.press(Message::LogsKindChosen(SourceKind::Journal));
    harness.press(Message::LogsUnitChanged("nginx.service".into()));

    harness.press(Message::LogsStartRequested);

    assert_eq!(starts(&harness.sent())[0].2, LogSource::Journal(UnitName::new("nginx.service")));
}

#[test]
fn starting_clears_the_old_buffer_and_uses_a_fresh_id() {
    let mut harness = ready();
    let first = started(&mut harness);
    harness.feed(Event::Log { id: first, state: batch(&["a", "b"], 5) });
    harness.press(Message::LogsStopRequested);
    harness.sent();

    harness.press(Message::LogsStartRequested);

    let sent = harness.sent();
    let second = starts(&sent)[0].0;
    assert_ne!(first, second);
    assert!(harness.app.logs().lines.is_empty());
    assert_eq!(harness.app.logs().running, Some((second, 3)));
}

#[test]
fn starting_while_running_stops_the_old_log_first() {
    let mut harness = ready();
    let first = started(&mut harness);

    harness.press(Message::LogsStartRequested);

    let sent = harness.sent();
    assert_eq!(stops(&sent), [first]);
    let stop_at = sent.iter().position(|command| matches!(command, Command::StopLog { .. })).unwrap();
    let start_at = sent.iter().position(|command| matches!(command, Command::StartLog { .. })).unwrap();
    assert!(stop_at < start_at, "{sent:?}");
    assert_ne!(starts(&sent)[0].0, first);
}

#[test]
fn stopping_sends_the_stop_and_says_stopped_and_a_second_stop_sends_nothing() {
    let mut harness = ready();
    let id = started(&mut harness);

    harness.press(Message::LogsStopRequested);
    assert_eq!(stops(&harness.sent()), [id]);
    assert_eq!(harness.app.logs().status, LogStatus::Ended(porthmos_core::i18n::text("gui.logs.stopped")));
    assert_eq!(harness.app.logs().running, None);

    harness.press(Message::LogsStopRequested);
    assert!(harness.sent().is_empty());
}

#[test]
fn started_sets_running_and_lines_append() {
    let mut harness = ready();
    let id = started(&mut harness);
    assert_eq!(harness.app.logs().status, LogStatus::Running);

    harness.feed(Event::Log { id, state: batch(&["a", "b"], 0) });
    harness.feed(Event::Log { id, state: batch(&["c"], 0) });

    assert_eq!(shown(&harness), ["a", "b", "c"]);
}

#[test]
fn lines_for_another_id_are_ignored() {
    let mut harness = ready();
    let id = started(&mut harness);

    harness.feed(Event::Log { id: id + 100, state: batch(&["stale"], 2) });
    harness.feed(Event::Log { id: id + 100, state: LogState::Started });
    harness.feed(Event::Log { id: id + 100, state: LogState::Ended("x".into()) });

    assert!(harness.app.logs().lines.is_empty());
    assert_eq!(harness.app.logs().status, LogStatus::Running);
    assert_eq!(harness.app.logs().running, Some((id, 3)));
}

#[test]
fn lines_with_nothing_running_are_ignored() {
    let mut harness = ready();

    harness.feed(Event::Log { id: 0, state: batch(&["stray"], 0) });

    assert!(harness.app.logs().lines.is_empty());
}

#[test]
fn a_late_batch_after_stopping_is_ignored() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.press(Message::LogsStopRequested);

    harness.feed(Event::Log { id, state: batch(&["late"], 0) });
    harness.feed(Event::Log { id, state: LogState::Ended("stopped".into()) });

    assert!(harness.app.logs().lines.is_empty());
    assert_eq!(harness.app.logs().status, LogStatus::Ended(porthmos_core::i18n::text("gui.logs.stopped")));
}

#[test]
fn skipped_lines_add_a_marker() {
    let mut harness = ready();
    let id = started(&mut harness);

    harness.feed(Event::Log { id, state: batch(&["a"], 9) });

    assert_eq!(harness.app.logs().lines.len(), 2);
    assert_eq!(harness.app.logs().lines[0].skipped, Some(9));
}

#[test]
fn the_cap_follows_the_max_lines_setting() {
    let mut settings = GuiSettings::default();
    settings.logs.max_lines = 1000;
    let mut harness = Harness::with_gui_settings(settings);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::LogsPathChanged("/a".into()));
    harness.sent();
    let id = started(&mut harness);
    let many: Vec<String> = (0..1005).map(|n| n.to_string()).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();

    harness.feed(Event::Log { id, state: batch(&refs, 0) });

    assert_eq!(harness.app.logs().lines.len(), 1000);
    assert_eq!(harness.app.logs().lines[0].line.text, "5");
}

#[test]
fn ended_and_unavailable_set_the_status_and_keep_the_buffer() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["a"], 0) });

    harness.feed(Event::Log { id, state: LogState::Ended("log ended".into()) });
    assert_eq!(harness.app.logs().status, LogStatus::Ended("log ended".to_string()));
    assert_eq!(shown(&harness), ["a"]);
    assert_eq!(harness.app.logs().running, None);

    let second = started(&mut harness);
    harness.feed(Event::Log { id: second, state: LogState::Unavailable("no journal".into()) });
    assert_eq!(harness.app.logs().status, LogStatus::Unavailable("no journal".to_string()));
    assert_eq!(harness.app.logs().running, None);
}

#[test]
fn clearing_empties_the_buffer_and_pausing_freezes_the_shown_lines() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["a", "b"], 0) });

    harness.press(Message::LogsPauseToggled);
    harness.feed(Event::Log { id, state: batch(&["c"], 0) });
    assert!(harness.app.logs().paused());
    assert_eq!(harness.app.logs().shown().count(), 2);
    assert_eq!(harness.app.logs().unseen_since_pause(), 1);

    harness.press(Message::LogsPauseToggled);
    assert_eq!(harness.app.logs().shown().count(), 3);

    harness.press(Message::LogsClearRequested);
    assert!(harness.app.logs().lines.is_empty());
}

fn stopped_because(reason: &str) -> LogStatus {
    LogStatus::Ended(porthmos_core::i18n::text_with("gui.logs.stopped_because", &[("reason", reason)]))
}

fn lost() -> LogStatus {
    stopped_because(&porthmos_core::i18n::text("logs.connection_lost"))
}

fn two_connections() -> Harness {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::LogsPathChanged("/a".into()));
    harness.sent();
    harness
}

fn running_session(harness: &Harness) -> Option<u64> {
    harness.app.logs().running.map(|(_, session)| session)
}

#[test]
fn switching_the_active_connection_tab_keeps_the_log_on_its_own_connection() {
    let mut harness = two_connections();
    let id = started(&mut harness);
    let first = running_session(&harness).unwrap();
    let other = if first == 3 { "beta" } else { "alpha" };
    let index = harness.app.sessions().names().iter().position(|tab| tab.name == other).unwrap();

    harness.press(Message::TabClicked(index));
    harness.press(Message::HealthSessionChosen(other.into()));

    assert!(stops(&harness.sent()).is_empty());
    assert_eq!(harness.app.logs().running, Some((id, first)));
    assert_eq!(harness.app.logs().status, LogStatus::Running);
}

#[test]
fn choosing_another_connection_in_the_logs_picker_stops_the_log_and_says_why() {
    let mut harness = two_connections();
    harness.press(Message::LogsSessionChosen("alpha".into()));
    let id = started(&mut harness);
    assert_eq!(running_session(&harness), Some(3));
    harness.sent();

    harness.press(Message::LogsSessionChosen("alpha".into()));
    assert!(stops(&harness.sent()).is_empty());

    harness.press(Message::LogsSessionChosen("beta".into()));

    assert_eq!(stops(&harness.sent()), [id]);
    assert_eq!(harness.app.logs().running, None);
    assert_eq!(harness.app.logs().status, stopped_because(&porthmos_core::i18n::text("gui.logs.other_connection")));
    assert_eq!(harness.app.health_target(), Some(4));
}

#[test]
fn an_unhealthy_connection_stops_its_log_and_says_why() {
    let mut harness = two_connections();
    harness.press(Message::LogsSessionChosen("alpha".into()));
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["kept"], 0) });
    harness.sent();

    harness.feed(Event::SessionHealth { session: 4, healthy: false });
    assert!(stops(&harness.sent()).is_empty());

    harness.feed(Event::SessionHealth { session: 3, healthy: false });

    assert_eq!(stops(&harness.sent()), [id]);
    assert_eq!(harness.app.logs().running, None);
    assert_eq!(harness.app.logs().status, lost());
    assert_eq!(shown(&harness), ["kept"]);
}

#[test]
fn closing_the_connection_stops_the_running_log() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["kept"], 0) });
    harness.sent();

    harness.feed(Event::Disconnected { session: 3, name: "alpha".into() });

    assert_eq!(stops(&harness.sent()), [id]);
    assert_eq!(harness.app.logs().running, None);
    assert_eq!(harness.app.logs().status, lost());
    assert_eq!(shown(&harness), ["kept"]);
}

#[test]
fn lowering_the_line_cap_in_settings_trims_the_buffer_at_once() {
    let mut settings = GuiSettings::default();
    settings.logs.max_lines = 5000;
    let mut harness = Harness::with_gui_settings(settings);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::LogsPathChanged("/a".into()));
    let id = started(&mut harness);
    let many: Vec<String> = (0..3000).map(|n| n.to_string()).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    harness.feed(Event::Log { id, state: batch(&refs, 0) });
    harness.press(Message::ActionRequested(Action::Settings));

    harness.press(Message::SettingsLogsTyped(crate::settings_view::LogsField::MaxLines, "1000".into()));
    harness.press(Message::SettingsLogsApplied(crate::settings_view::LogsField::MaxLines));

    assert_eq!(harness.app.gui_settings().logs.max_lines, 1000);
    assert_eq!(harness.app.logs().lines.len(), 1000);
    assert_eq!(harness.app.logs().lines[0].line.text, "2000");
}

#[test]
fn the_log_keeps_running_when_the_drawer_shows_another_tab() {
    let mut harness = ready();
    harness.press(Message::LogsViewSelected);
    started(&mut harness);
    harness.sent();

    harness.press(Message::TunnelsViewSelected);

    assert!(stops(&harness.sent()).is_empty());
    assert!(harness.app.logs().running.is_some());
}

#[test]
fn the_show_logs_action_opens_the_drawer_on_logs() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::ShowLogs));

    assert!(harness.app.drawer().open);
    assert_eq!(harness.app.drawer().view, DrawerView::Logs);
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[test]
fn the_logs_entry_follows_ports_in_the_strip() {
    let harness = Harness::new();

    let labels = crate::view::drawer::strip_labels(&harness.app);

    assert_eq!(&labels[..5], ["Tunnels", "Health", "Services", "Ports", "Logs"]);
}

#[test]
fn show_logs_is_named_titled_and_has_no_default_chord() {
    assert_eq!(Action::ShowLogs.name(), "show_logs");
    assert_eq!(Action::ShowLogs.title(), "Show logs");
    let shortcuts = crate::shortcuts::Shortcuts::default();
    assert!(shortcuts.label_for(Action::ShowLogs).is_none());
}

#[test]
fn the_form_messages_set_the_source_and_follow() {
    let mut harness = Harness::new();
    assert!(harness.app.logs().follow);

    harness.press(Message::LogsFollowToggled(false));
    harness.press(Message::LogsKindChosen(SourceKind::Journal));
    harness.press(Message::LogsUnitChanged("a.service".into()));
    harness.press(Message::LogsPathChanged("/p".into()));

    let logs = harness.app.logs();
    assert!(!logs.follow);
    assert_eq!((logs.kind, logs.unit.as_str(), logs.path.as_str()), (SourceKind::Journal, "a.service", "/p"));
}

#[test]
fn starting_a_new_log_drops_an_earlier_pause_so_its_lines_show() {
    let mut harness = ready();
    let first = started(&mut harness);
    harness.feed(Event::Log { id: first, state: batch(&["a", "b"], 0) });
    harness.press(Message::LogsPauseToggled);
    assert!(harness.app.logs().paused());

    let second = started(&mut harness);
    harness.feed(Event::Log { id: second, state: batch(&["c"], 0) });

    assert!(!harness.app.logs().paused());
    assert_eq!(harness.app.logs().shown().count(), 1);
    assert_eq!(harness.app.logs().unseen_since_pause(), 0);
}

#[test]
fn scrolling_up_stops_following_and_the_end_resumes_it() {
    let mut harness = ready();
    assert!(harness.app.logs().follow);

    harness.press(Message::LogsScrolled { offset: 0.0, viewport: 200.0, content: 2000.0 });
    assert!(!harness.app.logs().follow);

    harness.press(Message::LogsScrolled { offset: 1800.0, viewport: 200.0, content: 2000.0 });
    assert!(harness.app.logs().follow);

    harness.press(Message::LogsFollowToggled(false));
    assert!(!harness.app.logs().follow);
    harness.press(Message::LogsFollowToggled(true));
    assert!(harness.app.logs().follow);
}

#[test]
fn the_buffer_and_the_reason_stay_shown_after_the_only_connection_is_lost() {
    let mut harness = Harness::new();
    harness.press(Message::LogsViewSelected);
    assert!(!harness.app.logs_shown());
    connect(&mut harness, 3, "alpha");
    assert!(harness.app.logs_shown());
    harness.press(Message::LogsPathChanged("/a".into()));
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["kept"], 0) });

    harness.feed(Event::Disconnected { session: 3, name: "alpha".into() });

    assert_eq!(harness.app.health_target(), None);
    assert!(harness.app.logs_shown());
    assert!(harness.app.logs_list_present());
    assert_eq!(harness.app.logs().status, lost());
}

#[test]
fn the_list_exists_only_with_a_connection_a_shown_logs_tab_and_lines_to_show() {
    let mut harness = Harness::new();
    harness.press(Message::LogsViewSelected);
    assert!(!harness.app.logs_list_present());

    connect(&mut harness, 3, "alpha");
    harness.press(Message::LogsPathChanged("/var/log/syslog".into()));
    let id = started(&mut harness);
    assert!(!harness.app.logs_list_present());

    harness.feed(Event::Log { id, state: batch(&["a"], 0) });
    assert!(harness.app.logs_list_present());

    harness.press(Message::LogsPauseToggled);
    harness.press(Message::LogsClearRequested);
    harness.press(Message::LogsPauseToggled);
    assert!(!harness.app.logs_list_present());

    harness.feed(Event::Log { id, state: batch(&["b"], 0) });
    assert!(harness.app.logs_list_present());

    harness.press(Message::HealthViewSelected);
    assert!(!harness.app.logs_list_present());
}

fn started_on_another_tab(harness: &mut Harness, session: &str, path: &str) -> u64 {
    harness.press(Message::LogsTabAdded);
    harness.press(Message::LogsSessionChosen(session.into()));
    harness.press(Message::LogsPathChanged(path.into()));
    started(harness)
}

fn two_running_tabs() -> (Harness, u64, u64) {
    let mut harness = two_connections();
    harness.press(Message::LogsSessionChosen("alpha".into()));
    let first = started(&mut harness);
    let second = started_on_another_tab(&mut harness, "beta", "/b");
    harness.sent();
    (harness, first, second)
}

fn tab_lines(harness: &Harness, index: usize) -> Vec<String> {
    harness.app.log_tabs().tabs()[index].lines.iter().map(|stored| stored.line.text.clone()).collect()
}

#[test]
fn a_second_tab_starts_its_own_log_with_a_new_id_and_leaves_the_first_running() {
    let mut harness = two_connections();
    harness.press(Message::LogsSessionChosen("alpha".into()));
    let first = started(&mut harness);

    let second = started_on_another_tab(&mut harness, "beta", "/b");

    assert_ne!(first, second);
    assert_eq!(harness.app.log_tabs().tabs().len(), 2);
    assert_eq!(harness.app.log_tabs().tabs()[0].running, Some((first, 3)));
    assert_eq!(harness.app.log_tabs().tabs()[1].running, Some((second, 4)));
    assert!(stops(&harness.sent()).is_empty());
}

#[test]
fn lines_arrive_in_the_tab_that_owns_the_id() {
    let (mut harness, first, second) = two_running_tabs();

    harness.feed(Event::Log { id: second, state: batch(&["b1"], 0) });
    harness.feed(Event::Log { id: first, state: batch(&["a1", "a2"], 0) });

    assert_eq!(tab_lines(&harness, 0), ["a1", "a2"]);
    assert_eq!(tab_lines(&harness, 1), ["b1"]);
    harness.press(Message::LogsTabSelected(0));
    assert_eq!(shown(&harness), ["a1", "a2"]);
    harness.press(Message::LogsTabSelected(1));
    assert_eq!(shown(&harness), ["b1"]);
}

#[test]
fn closing_a_running_tab_stops_only_that_log() {
    let (mut harness, first, second) = two_running_tabs();

    harness.press(Message::LogsTabClosed(0));

    assert_eq!(stops(&harness.sent()), [first]);
    assert_eq!(harness.app.log_tabs().tabs().len(), 1);
    assert_eq!(harness.app.logs().running, Some((second, 4)));
}

#[test]
fn closing_an_idle_tab_sends_no_stop() {
    let mut harness = ready();
    harness.press(Message::LogsTabAdded);
    harness.sent();

    harness.press(Message::LogsTabClosed(1));

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.log_tabs().tabs().len(), 1);
}

#[test]
fn closing_the_last_tab_keeps_one_empty_tab_and_stops_its_log() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.sent();

    harness.press(Message::LogsTabClosed(0));

    assert_eq!(stops(&harness.sent()), [id]);
    assert_eq!(harness.app.log_tabs().tabs().len(), 1);
    assert!(harness.app.logs().is_pristine());
}

#[test]
fn events_for_a_closed_tab_are_ignored() {
    let (mut harness, first, second) = two_running_tabs();
    harness.feed(Event::Log { id: second, state: batch(&["keep"], 0) });
    harness.press(Message::LogsTabClosed(0));
    harness.sent();

    harness.feed(Event::Log { id: first, state: LogState::Started });
    harness.feed(Event::Log { id: first, state: batch(&["ghost"], 0) });
    harness.feed(Event::Log { id: first, state: LogState::Ended(porthmos_core::Message::new("gone")) });

    assert!(harness.sent().is_empty());
    assert_eq!(shown(&harness), ["keep"]);
    assert_eq!(harness.app.logs().running, Some((second, 4)));
    assert_eq!(harness.app.logs().status, LogStatus::Running);
}

#[test]
fn adding_past_six_tabs_does_nothing() {
    let mut harness = ready();

    for _ in 0..8 {
        harness.press(Message::LogsTabAdded);
    }

    assert_eq!(harness.app.log_tabs().tabs().len(), 6);
    assert_eq!(harness.app.log_tabs().active_index(), 5);
}

#[test]
fn losing_a_connection_ends_only_its_tabs() {
    let (mut harness, first, second) = two_running_tabs();
    harness.feed(Event::Log { id: second, state: LogState::Started });
    harness.sent();

    harness.feed(Event::SessionHealth { session: 3, healthy: false });

    assert_eq!(stops(&harness.sent()), [first]);
    assert_eq!(harness.app.log_tabs().tabs()[0].running, None);
    assert_eq!(harness.app.log_tabs().tabs()[0].status, lost());
    assert_eq!(harness.app.log_tabs().tabs()[1].running, Some((second, 4)));
    assert_eq!(harness.app.log_tabs().tabs()[1].status, LogStatus::Running);
}

#[test]
fn switching_tabs_keeps_each_tabs_pause_and_scroll() {
    let (mut harness, first, _) = two_running_tabs();
    harness.press(Message::LogsTabSelected(0));
    harness.feed(Event::Log { id: first, state: batch(&["a"], 0) });
    harness.press(Message::LogsPauseToggled);
    harness.press(Message::LogsScrolled { offset: 40.0, viewport: 100.0, content: 1000.0 });

    harness.press(Message::LogsTabSelected(1));
    assert!(!harness.app.logs().paused());
    harness.press(Message::LogsTabSelected(0));

    assert!(harness.app.logs().paused());
    assert_eq!(harness.app.logs().scroll, 40.0);
    assert!(!harness.app.logs().follow);
}

fn showing_the_first_of_two_tabs_with_lines() -> Harness {
    let (mut harness, first, second) = two_running_tabs();
    harness.feed(Event::Log { id: first, state: batch(&["a", "b"], 0) });
    harness.feed(Event::Log { id: second, state: batch(&["c", "d"], 0) });
    harness.press(Message::LogsViewSelected);
    harness.press(Message::LogsTabSelected(0));
    harness.press(Message::LogsTabSelected(1));
    harness
}

#[test]
fn selecting_a_tab_asks_the_list_to_restore_its_position() {
    let mut harness = showing_the_first_of_two_tabs_with_lines();
    assert!(harness.app.log_tabs().tabs()[0].was_visible);

    harness.app.logs.select(0);

    assert!(harness.app.logs_snap().units() > 0);
}

#[test]
fn closing_the_active_tab_asks_the_list_to_restore_the_position_of_the_next() {
    let mut harness = showing_the_first_of_two_tabs_with_lines();
    assert!(harness.app.log_tabs().tabs()[0].was_visible);

    harness.app.logs.close(1);

    assert!(harness.app.logs_snap().units() > 0);
}

#[test]
fn starting_a_running_tab_again_keeps_it_on_its_own_connection() {
    let (mut harness, first, _) = two_running_tabs();
    harness.press(Message::LogsTabSelected(0));
    assert_eq!(harness.app.logs().running, Some((first, 3)));
    harness.press(Message::LogsPathChanged("/changed".into()));

    harness.press(Message::LogsStartRequested);

    let sent = harness.sent();
    assert_eq!(starts(&sent).iter().map(|start| start.1).collect::<Vec<_>>(), [3]);
    assert_eq!(stops(&sent), [first]);
    assert_eq!(harness.app.logs().running.map(|(_, session)| session), Some(3));
}

#[test]
fn stopped_tabs_stay_reachable_after_every_connection_is_lost() {
    let (mut harness, first, _) = two_running_tabs();
    harness.feed(Event::Log { id: first, state: batch(&["kept"], 0) });
    harness.press(Message::LogsTabAdded);

    harness.feed(Event::SessionHealth { session: 3, healthy: false });
    harness.feed(Event::SessionHealth { session: 4, healthy: false });

    assert_eq!(harness.app.health_target(), None);
    assert!(harness.app.logs_shown());
    assert_eq!(tab_lines(&harness, 0), ["kept"]);
}

#[test]
fn a_flood_for_a_background_tab_does_not_touch_the_active_tab() {
    let mut settings = GuiSettings::default();
    settings.logs.max_lines = 1000;
    let mut harness = Harness::with_gui_settings(settings);
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness.press(Message::LogsSessionChosen("alpha".into()));
    harness.press(Message::LogsPathChanged("/a".into()));
    let first = started(&mut harness);
    let second = started_on_another_tab(&mut harness, "beta", "/b");
    harness.press(Message::LogsTabSelected(0));
    harness.feed(Event::Log { id: first, state: batch(&["steady"], 0) });

    let flood: Vec<String> = (0..5000).map(|n| n.to_string()).collect();
    let refs: Vec<&str> = flood.iter().map(String::as_str).collect();
    harness.feed(Event::Log { id: second, state: batch(&refs, 0) });

    assert_eq!(tab_lines(&harness, 0), ["steady"]);
    assert_eq!(tab_lines(&harness, 1).len(), 1000);
}

#[test]
fn lowering_the_line_cap_trims_every_tab() {
    let mut settings = GuiSettings::default();
    settings.logs.max_lines = 5000;
    let mut harness = Harness::with_gui_settings(settings);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::LogsPathChanged("/a".into()));
    let first = started(&mut harness);
    let second = started_on_another_tab(&mut harness, "alpha", "/b");
    let many: Vec<String> = (0..3000).map(|n| n.to_string()).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    harness.feed(Event::Log { id: first, state: batch(&refs, 0) });
    harness.feed(Event::Log { id: second, state: batch(&refs, 0) });
    harness.press(Message::ActionRequested(Action::Settings));

    harness.press(Message::SettingsLogsTyped(crate::settings_view::LogsField::MaxLines, "1000".into()));
    harness.press(Message::SettingsLogsApplied(crate::settings_view::LogsField::MaxLines));

    assert_eq!(tab_lines(&harness, 0).len(), 1000);
    assert_eq!(tab_lines(&harness, 1).len(), 1000);
}

fn leveled_batch(items: &[(&str, LogLevel)]) -> LogState {
    LogState::Lines {
        lines: items.iter().map(|(text, level)| LogLine { text: text.to_string(), level: *level }).collect(),
        skipped: 0,
    }
}

fn view_len(harness: &Harness, index: usize) -> usize {
    harness.app.log_tabs().tabs()[index].view_len()
}

#[test]
fn typing_a_filter_narrows_the_active_tab_only() {
    let (mut harness, first, second) = two_running_tabs();
    harness.feed(Event::Log { id: first, state: batch(&["disk a", "net b"], 0) });
    harness.feed(Event::Log { id: second, state: batch(&["disk c", "net d"], 0) });

    harness.press(Message::LogsFilterChanged("disk".into()));

    assert_eq!(view_len(&harness, 1), 1);
    assert_eq!(view_len(&harness, 0), 2);
}

#[test]
fn each_tab_keeps_its_own_filter_when_switching() {
    let (mut harness, _, _) = two_running_tabs();
    harness.press(Message::LogsFilterChanged("beta".into()));
    harness.press(Message::LogsTabSelected(0));
    harness.press(Message::LogsFilterChanged("alpha".into()));

    harness.press(Message::LogsTabSelected(1));
    assert_eq!(harness.app.logs().filter.text, "beta");
    harness.press(Message::LogsTabSelected(0));
    assert_eq!(harness.app.logs().filter.text, "alpha");
}

#[test]
fn a_level_pill_toggled_twice_is_off_again() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: leveled_batch(&[("boom", LogLevel::Error), ("fine", LogLevel::Info)]) });

    harness.press(Message::LogsLevelToggled(LogLevel::Error));
    assert_eq!(harness.app.logs().view_len(), 1);

    harness.press(Message::LogsLevelToggled(LogLevel::Error));
    assert_eq!(harness.app.logs().view_len(), 2);
    assert!(!harness.app.logs().filtering());
}

#[test]
fn changing_the_filter_makes_the_tab_follow_the_end_again() {
    let mut harness = ready();
    started(&mut harness);
    harness.press(Message::LogsFollowToggled(false));
    assert!(!harness.app.logs().follow);

    harness.press(Message::LogsFilterChanged("x".into()));

    assert!(harness.app.logs().follow);
}

#[test]
fn the_list_is_absent_when_the_filter_matches_nothing_and_back_when_it_does() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["disk a", "net b"], 0) });
    harness.press(Message::LogsViewSelected);
    assert!(harness.app.logs_list_present());

    harness.press(Message::LogsFilterChanged("zzz".into()));
    assert!(!harness.app.logs_list_present());

    harness.press(Message::LogsFilterChanged("disk".into()));
    assert!(harness.app.logs_list_present());
}

#[test]
fn new_lines_that_match_appear_and_others_do_not() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.press(Message::LogsFilterChanged("err".into()));

    harness.feed(Event::Log { id, state: batch(&["error one", "fine", "ERR two"], 0) });

    assert_eq!(harness.app.logs().view_len(), 2);
    assert_eq!(shown(&harness).len(), 3);
}

#[test]
fn starting_the_log_again_keeps_the_filter_and_rebuilds_the_view() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["disk old"], 0) });
    harness.press(Message::LogsFilterChanged("disk".into()));
    assert_eq!(harness.app.logs().view_len(), 1);

    let next = started(&mut harness);
    assert_eq!(harness.app.logs().view_len(), 0);
    harness.feed(Event::Log { id: next, state: batch(&["disk new", "net new"], 0) });

    assert_eq!(harness.app.logs().filter.text, "disk");
    assert_eq!(harness.app.logs().view_len(), 1);
}

#[test]
fn closing_a_tab_drops_its_filter_with_it() {
    let (mut harness, _, _) = two_running_tabs();
    harness.press(Message::LogsFilterChanged("beta".into()));

    harness.press(Message::LogsTabClosed(1));

    assert_eq!(harness.app.log_tabs().tabs().len(), 1);
    assert!(!harness.app.logs().filtering());
}

#[test]
fn the_list_snaps_to_the_end_when_the_view_grows_back_from_nothing() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["disk a", "net b"], 0) });
    harness.press(Message::LogsViewSelected);
    harness.app.logs.active_mut().set_filter_text("zzz".into());
    let _ = harness.app.logs_snap();
    assert!(!harness.app.logs_list_present());

    harness.app.logs.active_mut().set_filter_text("disk".into());

    assert!(harness.app.logs_snap().units() > 0);
}

#[test]
fn changing_the_filter_while_paused_counts_only_lines_before_the_pause() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["disk a", "net b"], 0) });
    harness.press(Message::LogsPauseToggled);
    harness.feed(Event::Log { id, state: batch(&["disk later"], 0) });

    harness.press(Message::LogsFilterChanged("disk".into()));

    assert_eq!(harness.app.logs().view_len(), 1);
    harness.press(Message::LogsPauseToggled);
    assert_eq!(harness.app.logs().view_len(), 2);
}

#[test]
fn lowering_the_line_cap_keeps_the_filtered_view_consistent() {
    let mut settings = GuiSettings::default();
    settings.logs.max_lines = 5000;
    let mut harness = Harness::with_gui_settings(settings);
    connect(&mut harness, 3, "alpha");
    harness.press(Message::LogsPathChanged("/a".into()));
    let id = started(&mut harness);
    let many: Vec<String> = (0..3000).map(|n| format!("n{n}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    harness.feed(Event::Log { id, state: batch(&refs, 0) });
    harness.press(Message::LogsFilterChanged("n".into()));
    assert_eq!(harness.app.logs().view_len(), 3000);
    harness.press(Message::ActionRequested(Action::Settings));

    harness.press(Message::SettingsLogsTyped(crate::settings_view::LogsField::MaxLines, "1000".into()));
    harness.press(Message::SettingsLogsApplied(crate::settings_view::LogsField::MaxLines));

    assert_eq!(harness.app.logs().view_len(), 1000);
    let rows = harness.app.logs().view_lines(0..1000);
    assert_eq!(rows.len(), 1000);
    assert_eq!(rows[0].line.text, "n2000");
}

fn notice_texts(harness: &Harness) -> Vec<(porthmos_core::Severity, String)> {
    harness.app.notices().iter().map(|notice| (notice.severity, notice.message.to_string())).collect()
}

#[test]
fn saving_with_nothing_shown_sends_no_task() {
    let mut harness = ready();
    started(&mut harness);

    assert_eq!(harness.app.logs_save_requested().units(), 0);
}

#[test]
fn saving_with_lines_shown_starts_the_dialog_task() {
    let mut harness = ready();
    let id = started(&mut harness);
    harness.feed(Event::Log { id, state: batch(&["a"], 0) });

    assert!(harness.app.logs_save_requested().units() > 0);
}

#[test]
fn a_cancelled_dialog_changes_nothing() {
    let mut harness = ready();
    let before = notice_texts(&harness).len();

    harness.press(Message::LogsSaved(Ok(None)));

    assert_eq!(notice_texts(&harness).len(), before);
}

#[test]
fn a_saved_file_says_where() {
    let mut harness = ready();

    harness.press(Message::LogsSaved(Ok(Some("/tmp/out.log".into()))));

    let notices = notice_texts(&harness);
    let (severity, text) = notices.last().unwrap();
    assert_eq!(*severity, porthmos_core::Severity::Info);
    assert!(text.contains("/tmp/out.log"), "{text}");
}

#[test]
fn a_failed_save_shows_the_reason() {
    let mut harness = ready();

    harness.press(Message::LogsSaved(Err("disk full".into())));

    let notices = notice_texts(&harness);
    let (severity, text) = notices.last().unwrap();
    assert_eq!(*severity, porthmos_core::Severity::Error);
    assert!(text.contains("disk full"), "{text}");
}

fn list_remote(harness: &mut Harness, session: u64, entries: Vec<porthmos_core::Entry>) {
    harness.feed(Event::Listed { location: Location::Session(session), path: VfsPath::new("/srv"), entries });
}

fn remote_file(name: &str) -> porthmos_core::Entry {
    porthmos_core::Entry { name: name.to_string(), path: VfsPath::new("/srv").join(name), ..Default::default() }
}

fn remote_folder(name: &str) -> porthmos_core::Entry {
    porthmos_core::Entry { is_dir: true, ..remote_file(name) }
}

fn exec_ready(can_exec: bool) -> Harness {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.feed(Event::SessionCapabilities { session: 3, preserves_times: false, embedded_shell: false, can_exec });
    list_remote(&mut harness, 3, vec![remote_file("app.log"), remote_file("other.log"), remote_folder("dir")]);
    harness.sent();
    harness
}

fn follow_target(harness: &Harness) -> Option<bool> {
    match harness.app.context_menu().map(|menu| menu.target.clone()) {
        Some(crate::menus::MenuTarget::Entries { follow_log, .. }) => Some(follow_log),
        _ => None,
    }
}

fn follow_from_menu(harness: &mut Harness, row: usize) {
    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, row));
    harness.press(Message::MenuItemChosen(crate::menus::MenuItem::FollowAsLog));
}

#[test]
fn the_menu_offers_follow_as_log_for_a_remote_file_on_a_session_that_can_exec() {
    let mut harness = exec_ready(true);

    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 2));

    assert_eq!(follow_target(&harness), Some(true));
}

#[test]
fn the_menu_does_not_offer_it_on_a_session_without_exec() {
    let mut harness = exec_ready(false);

    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 2));

    assert_eq!(follow_target(&harness), Some(false));
}

#[test]
fn the_menu_does_not_offer_it_for_a_folder_the_parent_row_or_two_files() {
    let mut harness = exec_ready(true);

    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 1));
    assert_eq!(follow_target(&harness), Some(false));

    harness.press(Message::ContextMenuDismissed);
    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 0));
    assert_eq!(follow_target(&harness), Some(false));

    harness.press(Message::ContextMenuDismissed);
    harness.press(Message::ModifiersChanged(iced::keyboard::Modifiers::empty()));
    harness.press(Message::PanelClicked(crate::app::PanelSide::Remote, 2));
    harness.press(Message::ModifiersChanged(iced::keyboard::Modifiers::COMMAND));
    harness.press(Message::PanelClicked(crate::app::PanelSide::Remote, 3));
    harness.press(Message::ModifiersChanged(iced::keyboard::Modifiers::empty()));
    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 2));
    assert_eq!(follow_target(&harness), Some(false));
}

#[test]
fn the_menu_does_not_offer_it_on_the_local_panel() {
    let mut harness = exec_ready(true);
    let folder = harness.start_folder();
    let local = porthmos_core::Entry { name: "a".into(), path: folder.join("a"), ..Default::default() };
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries: vec![local] });

    harness.press(Message::RowRightPressed(crate::app::PanelSide::Local, 1));

    assert_eq!(follow_target(&harness), Some(false));
}

#[test]
fn choosing_it_opens_the_logs_and_starts_the_file_on_that_session() {
    let mut harness = exec_ready(true);

    follow_from_menu(&mut harness, 3);

    let sent = harness.sent();
    let started = starts(&sent);
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].1, 3);
    assert_eq!(started[0].2, LogSource::File("/srv/other.log".to_string()));
    assert_eq!(harness.app.drawer().view, DrawerView::Logs);
    assert!(harness.app.drawer().open);
}

#[test]
fn choosing_it_twice_focuses_the_same_tab() {
    let mut harness = exec_ready(true);
    follow_from_menu(&mut harness, 2);
    let id = starts(&harness.sent())[0].0;
    harness.feed(Event::Log { id, state: LogState::Started });

    follow_from_menu(&mut harness, 2);

    assert!(starts(&harness.sent()).is_empty());
    assert_eq!(harness.app.log_tabs().tabs().len(), 1);
}

#[test]
fn a_second_file_opens_a_second_tab() {
    let mut harness = exec_ready(true);
    follow_from_menu(&mut harness, 2);
    let first = starts(&harness.sent())[0].0;
    harness.feed(Event::Log { id: first, state: LogState::Started });

    follow_from_menu(&mut harness, 3);

    let sent = harness.sent();
    assert_eq!(starts(&sent).len(), 1);
    assert!(stops(&sent).is_empty());
    assert_eq!(harness.app.log_tabs().tabs().len(), 2);
    assert_eq!(harness.app.logs().path, "/srv/other.log");
}

#[test]
fn with_six_busy_tabs_it_says_so_and_stops_nothing() {
    let mut harness = exec_ready(true);
    for index in 0..6 {
        harness.press(Message::LogsPathChanged(format!("/f{index}")));
        if index < 5 {
            harness.press(Message::LogsTabAdded);
        }
    }
    harness.press(Message::LogsSessionChosen("alpha".into()));
    harness.press(Message::LogsStartRequested);
    harness.sent();
    let running = harness.app.logs().running;

    follow_from_menu(&mut harness, 2);

    let sent = harness.sent();
    assert!(starts(&sent).is_empty() && stops(&sent).is_empty(), "{sent:?}");
    assert_eq!(harness.app.logs().running, running);
    assert_eq!(harness.app.logs().path, "/f5");
    let said = crate::logs_view::tabs_full_text();
    assert!(harness.app.notices().iter().any(|notice| notice.message == said));
}

fn two_exec_connections() -> Harness {
    let mut harness = Harness::new();
    for (session, name) in [(3, "alpha"), (4, "beta")] {
        connect(&mut harness, session, name);
        harness.feed(Event::SessionCapabilities {
            session,
            preserves_times: false,
            embedded_shell: false,
            can_exec: true,
        });
        list_remote(&mut harness, session, vec![remote_file("x.log")]);
    }
    harness.sent();
    harness
}

#[test]
fn following_starts_on_the_clicked_connection_and_leaves_a_running_tab_alone() {
    let mut harness = two_exec_connections();
    harness.press(Message::LogsSessionChosen("alpha".into()));
    harness.press(Message::LogsPathChanged("/a".into()));
    let first = started(&mut harness);
    assert_eq!(harness.app.sessions().active().map(|session| session.id), Some(4));

    follow_from_menu(&mut harness, 1);

    let sent = harness.sent();
    let begun = starts(&sent);
    assert_eq!(begun.len(), 1);
    assert_eq!(begun[0].1, 4);
    assert!(stops(&sent).is_empty());
    assert_eq!(harness.app.log_tabs().tabs()[0].running, Some((first, 3)));
    assert_eq!(harness.app.logs().running.map(|(_, session)| session), Some(4));
}

#[test]
fn losing_the_connection_closes_an_open_context_menu() {
    let mut harness = exec_ready(true);
    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 2));
    assert!(harness.app.context_menu().is_some());

    harness.feed(Event::Disconnected { session: 3, name: "alpha".into() });

    assert!(harness.app.context_menu().is_none());
}

#[test]
fn an_unhealthy_session_is_not_offered_follow_as_log() {
    let mut harness = exec_ready(true);
    harness.feed(Event::SessionHealth { session: 3, healthy: false });

    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 2));

    assert_eq!(follow_target(&harness), Some(false));
}

#[test]
fn choosing_follow_as_log_from_a_menu_that_did_not_offer_it_does_nothing() {
    let mut harness = exec_ready(false);
    harness.press(Message::RowRightPressed(crate::app::PanelSide::Remote, 2));

    harness.press(Message::MenuItemChosen(crate::menus::MenuItem::FollowAsLog));

    assert!(starts(&harness.sent()).is_empty());
    assert_ne!(harness.app.drawer().view, DrawerView::Logs);
    assert!(harness.app.log_tabs().active().is_pristine());
}

#[test]
fn following_a_file_that_runs_in_another_tab_selects_that_tab() {
    let mut harness = exec_ready(true);
    follow_from_menu(&mut harness, 2);
    let id = starts(&harness.sent())[0].0;
    harness.feed(Event::Log { id, state: LogState::Started });
    harness.press(Message::LogsTabAdded);
    assert_eq!(harness.app.log_tabs().active_index(), 1);

    follow_from_menu(&mut harness, 2);

    assert!(starts(&harness.sent()).is_empty());
    assert_eq!(harness.app.log_tabs().active_index(), 0);
    assert_eq!(harness.app.log_tabs().tabs().len(), 2);
}

#[test]
fn a_saved_path_is_shown_without_control_characters() {
    let mut harness = ready();

    harness.press(Message::LogsSaved(Ok(Some("/tmp/a\u{1b}[31mb.log".into()))));

    let notices = notice_texts(&harness);
    assert!(!notices.last().unwrap().1.chars().any(char::is_control));
}
