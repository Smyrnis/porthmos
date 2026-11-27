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
