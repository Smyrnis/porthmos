use porthmos_core::theme::ColorScheme;
use porthmos_core::{Command, Event, Location, LogLevel, LogLine, LogState};

use super::*;
use crate::app::Message;
use crate::logs_view::{LogStatus, LogTabs, MAX_LOG_TABS, SourceKind, StoredLine};
use crate::testing::Harness;
use crate::theme::ThemeSettings;

fn palettes() -> Vec<Palette> {
    [ColorScheme::dark(), ColorScheme::light(), ColorScheme::high_contrast()]
        .into_iter()
        .map(|scheme| Palette::new(&ThemeSettings::default().with_scheme(scheme)))
        .collect()
}

fn connect(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: porthmos_core::VfsPath::new("/"),
        entries: Vec::new(),
    });
}

fn stored(level: LogLevel, skipped: Option<u64>) -> StoredLine {
    StoredLine { seq: 0, line: LogLine { text: "x".to_string(), level }, skipped }
}

fn line(text: &str, level: LogLevel) -> LogLine {
    LogLine { text: text.to_string(), level }
}

fn running_state() -> LogsState {
    LogsState { running: Some((1, 3)), path: "/var/log/syslog".to_string(), ..LogsState::default() }
}

#[test]
fn a_window_never_covers_more_than_the_viewport_plus_overscan() {
    let total = 20_000;
    let viewport = 220.0;

    for offset in [0.0, 1234.0, 100_000.0, 439_000.0] {
        let range = window(total, offset, viewport, false);

        assert!(range.len() <= 10 + 2 * OVERSCAN + 1, "{offset}: {range:?}");
        assert!(range.end <= total);
    }
}

#[test]
fn a_window_starts_at_the_row_under_the_offset_less_the_overscan() {
    let range = window(1000, 22.0 * 100.0, 22.0 * 10.0, false);

    assert_eq!(range, (100 - OVERSCAN)..(110 + OVERSCAN));
}

#[test]
fn following_pins_the_window_to_the_end() {
    let total = 5000;

    let range = window(total, 0.0, 220.0, true);

    assert_eq!(range.end, total);
    assert!(range.start >= total - 10 - OVERSCAN - 1, "{range:?}");
}

#[test]
fn a_short_buffer_is_shown_whole_and_an_empty_one_has_no_window() {
    assert_eq!(window(3, 0.0, 220.0, true), 0..3);
    assert_eq!(window(3, 500.0, 220.0, false), 0..3);
    assert_eq!(window(0, 0.0, 220.0, true), 0..0);
}

#[test]
fn a_stale_offset_past_the_end_is_held_at_the_end() {
    let range = window(1000, 1_000_000.0, 220.0, false);

    assert_eq!(range.end, 1000);
    assert!(!range.is_empty(), "{range:?}");
    assert!(range.start >= 1000 - 10 - 2 * OVERSCAN - 1, "{range:?}");
}

#[test]
fn clearing_while_scrolled_up_then_new_lines_are_shown_from_the_top() {
    let mut state = LogsState::default();
    let many: Vec<LogLine> = (0..5000).map(|n| line(&format!("l{n}"), LogLevel::Info)).collect();
    state.append(many, 0, 20_000);
    state.scrolled(3000.0, 220.0, 110_000.0);
    assert!(!state.follow);

    state.clear_buffer();
    state.append(vec![line("a", LogLevel::Info), line("b", LogLevel::Info), line("c", LogLevel::Info)], 0, 20_000);

    let range = window(state.shown_len(), state.scroll, 220.0, state.follow);
    assert_eq!(range, 0..3);
}

#[test]
fn the_end_offset_is_the_content_less_the_viewport_and_never_negative() {
    assert_eq!(end_offset(100, 220.0), 100.0 * ROW_HEIGHT - 220.0);
    assert_eq!(end_offset(2, 220.0), 0.0);
}

#[test]
fn levels_pick_their_palette_role_in_every_scheme() {
    for palette in palettes() {
        assert_eq!(line_color(&stored(LogLevel::Error, None), &palette), palette.err);
        assert_eq!(line_color(&stored(LogLevel::Warn, None), &palette), palette.fg_bright);
        for level in [LogLevel::Info, LogLevel::Debug, LogLevel::Unknown] {
            assert_eq!(line_color(&stored(level, None), &palette), palette.fg, "{level:?}");
        }
        assert_eq!(line_color(&stored(LogLevel::Error, Some(5)), &palette), palette.fg_dim);
    }
}

#[test]
fn a_skipped_marker_says_how_many_lines_were_skipped() {
    assert_eq!(skipped_text(1), "1 line skipped");
    assert_eq!(skipped_text(250), "250 lines skipped");
}

#[test]
fn start_is_blocked_without_a_source_and_says_why() {
    let state = LogsState::default();
    let (press, reason) = start_press(&state);
    assert!(press.is_none());
    assert_eq!(reason.as_deref(), Some("Enter the path of a file to follow"));

    let state = LogsState { kind: SourceKind::Journal, unit: "nginx".to_string(), ..LogsState::default() };
    let (press, reason) = start_press(&state);
    assert!(press.is_none());
    assert_eq!(reason.as_deref(), Some("Its name can't be used safely in a command"));
}

#[test]
fn start_is_allowed_with_a_path_or_a_journal_with_or_without_a_unit() {
    for state in [
        LogsState { path: "/var/log/syslog".to_string(), ..LogsState::default() },
        LogsState { kind: SourceKind::Journal, ..LogsState::default() },
        LogsState { kind: SourceKind::Journal, unit: "nginx.service".to_string(), ..LogsState::default() },
    ] {
        let (press, reason) = start_press(&state);

        assert!(matches!(press, Some(Message::LogsStartRequested)), "{state:?}");
        assert_eq!(reason, None);
    }
}

#[test]
fn stop_pause_and_clear_wait_for_something_to_act_on() {
    let idle = LogsState::default();
    for (press, reason) in [stop_press(&idle), pause_press(&idle), clear_press(&idle)] {
        assert!(press.is_none());
        assert!(reason.is_some());
    }
    assert_eq!(stop_press(&idle).1.as_deref(), Some("Nothing is being followed"));
    assert_eq!(clear_press(&idle).1.as_deref(), Some("Nothing to clear"));

    let mut busy = running_state();
    busy.append(vec![line("a", LogLevel::Info)], 0, 10);
    assert!(matches!(stop_press(&busy).0, Some(Message::LogsStopRequested)));
    assert!(matches!(pause_press(&busy).0, Some(Message::LogsPauseToggled)));
    assert!(matches!(clear_press(&busy).0, Some(Message::LogsClearRequested)));
    assert_eq!((stop_press(&busy).1, pause_press(&busy).1, clear_press(&busy).1), (None, None, None));
}

#[test]
fn resume_stays_available_after_the_log_ended_while_paused() {
    let mut state = LogsState::default();
    state.append(vec![line("a", LogLevel::Info)], 0, 10);
    state.pause_toggled(100);
    state.status = LogStatus::Ended("done".into());

    assert!(matches!(pause_press(&state).0, Some(Message::LogsPauseToggled)));
    assert_eq!(pause_title(true), "Resume");
    assert_eq!(pause_title(false), "Pause");
}

#[test]
fn the_follow_note_says_paused_or_following_and_nothing_when_scrolled_away() {
    assert_eq!(follow_text(true, true, true, 0).as_deref(), Some("Paused"));
    assert_eq!(follow_text(true, false, true, 0).as_deref(), Some("Paused"));
    assert_eq!(follow_text(false, true, true, 0).as_deref(), Some("Following"));
    assert_eq!(follow_text(false, false, true, 0), None);
}

#[test]
fn the_follow_note_says_following_only_while_a_log_runs() {
    assert_eq!(follow_text(false, true, false, 0), None);
    assert_eq!(follow_text(true, true, false, 0).as_deref(), Some("Paused"));
}

#[test]
fn the_paused_note_counts_the_lines_that_arrived_meanwhile() {
    assert_eq!(follow_text(true, true, true, 1).as_deref(), Some("Paused, 1 new line"));
    assert_eq!(follow_text(true, false, true, 37).as_deref(), Some("Paused, 37 new lines"));
}

#[test]
fn a_zero_viewport_shows_only_the_overscan_rows() {
    let following = window(1000, 0.0, 0.0, true);
    assert!(following.len() <= 2 * OVERSCAN + 1, "{following:?}");
    assert_eq!(following.end, 1000);
    let scrolled = window(1000, 22.0 * 100.0, 0.0, false);
    assert!(scrolled.len() <= 2 * OVERSCAN + 1, "{scrolled:?}");
    assert!(scrolled.contains(&100), "{scrolled:?}");
    assert!(window(0, 0.0, 0.0, true).is_empty());
}

#[test]
fn a_stop_reason_shows_in_the_footer() {
    let stopped = LogStatus::Ended(porthmos_core::i18n::text("gui.logs.stopped"));
    assert_eq!(footer_text(&stopped).as_deref(), Some("Stopped"));
    let reason = porthmos_core::i18n::text_with(
        "gui.logs.stopped_because",
        &[("reason", &porthmos_core::i18n::text("gui.logs.other_connection"))],
    );
    assert_eq!(reason, "Stopped: Another connection was chosen.");
}

#[test]
fn the_empty_area_says_what_state_the_log_is_in() {
    assert_eq!(empty_text(&LogStatus::Idle), "Choose a source and press Start");
    assert_eq!(empty_text(&LogStatus::Starting), "Starting\u{2026}");
    assert_eq!(empty_text(&LogStatus::Running), "Waiting for lines\u{2026}");
    assert_eq!(empty_text(&LogStatus::Ended("The log ended.".into())), "The log ended.");
    assert_eq!(empty_text(&LogStatus::Unavailable("No journal".into())), "No journal");
}

#[test]
fn the_footer_only_appears_once_the_log_has_ended_or_is_unavailable() {
    assert_eq!(footer_text(&LogStatus::Idle), None);
    assert_eq!(footer_text(&LogStatus::Starting), None);
    assert_eq!(footer_text(&LogStatus::Running), None);
    assert_eq!(footer_text(&LogStatus::Ended("The log ended.".into())).as_deref(), Some("The log ended."));
    assert_eq!(footer_text(&LogStatus::Unavailable("No journal".into())).as_deref(), Some("No journal"));
}

#[test]
fn the_view_builds_in_every_state() {
    let mut harness = Harness::new();
    harness.press(Message::LogsViewSelected);
    let _ = view(&harness.app);

    connect(&mut harness, 3, "alpha");
    let _ = view(&harness.app);
    harness.press(Message::LogsPathChanged("/var/log/syslog".into()));
    let _ = view(&harness.app);
    harness.press(Message::LogsStartRequested);
    let id = harness
        .sent()
        .iter()
        .find_map(|command| if let Command::StartLog { id, .. } = command { Some(*id) } else { None })
        .expect("a start");
    let _ = view(&harness.app);

    harness.feed(Event::Log { id, state: LogState::Started });
    let _ = view(&harness.app);
    let many: Vec<LogLine> = (0..3000)
        .map(|n| line(&format!("line {n}\t\u{1b}[31m"), [LogLevel::Error, LogLevel::Warn, LogLevel::Info][n % 3]))
        .collect();
    harness.feed(Event::Log { id, state: LogState::Lines { lines: many, skipped: 40 } });
    let _ = view(&harness.app);
    harness.press(Message::LogsPauseToggled);
    harness.feed(Event::Log { id, state: LogState::Lines { lines: vec![line("late", LogLevel::Info)], skipped: 0 } });
    let _ = view(&harness.app);
    harness.press(Message::LogsPauseToggled);
    harness.press(Message::LogsScrolled { offset: 100.0, viewport: 200.0, content: 66_000.0 });
    let _ = view(&harness.app);
    harness.press(Message::LogsKindChosen(SourceKind::Journal));
    harness.press(Message::LogsUnitChanged("nginx.service".into()));
    let _ = view(&harness.app);
    harness.feed(Event::Log { id, state: LogState::Ended("The log ended.".into()) });
    let _ = view(&harness.app);
    harness.press(Message::LogsClearRequested);
    let _ = view(&harness.app);
    harness.feed(Event::Log { id, state: LogState::Unavailable("No journal".into()) });
    connect(&mut harness, 4, "beta");
    let _ = view(&harness.app);
}

#[test]
fn the_picker_shows_for_two_healthy_sessions_only() {
    let mut harness = Harness::new();
    assert!(!shows_session_picker(&harness.app));
    connect(&mut harness, 3, "alpha");
    assert!(!shows_session_picker(&harness.app));
    connect(&mut harness, 4, "beta");
    assert!(shows_session_picker(&harness.app));
}

#[test]
fn every_new_text_exists_in_both_languages() {
    let english = include_str!("../../../../porthmos-core/locales/en.yml");
    let greek = include_str!("../../../../porthmos-core/locales/el.yml");
    for label in [
        "Journal",
        "File",
        "Follow",
        "Pause",
        "Resume",
        "Clear",
        "Path of a log file",
        "Unit (empty for the whole journal)",
        "View logs",
    ] {
        let key = format!("\n  {}:", porthmos_core::i18n::label_key(label).trim_start_matches("label."));
        assert!(english.contains(&key), "en lacks {label}");
        assert!(greek.contains(&key), "el lacks {label}");
    }
}

fn connected_harness() -> Harness {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    connect(&mut harness, 4, "beta");
    harness
}

fn titles(harness: &Harness) -> Vec<String> {
    (0..harness.app.log_tabs().tabs().len()).map(|index| tab_title(&harness.app, index)).collect()
}

fn run_on(harness: &mut Harness, session: &str, path: &str) {
    harness.press(Message::LogsSessionChosen(session.into()));
    harness.press(Message::LogsPathChanged(path.into()));
    harness.press(Message::LogsStartRequested);
}

#[test]
fn an_empty_tab_is_titled_new_log() {
    let harness = Harness::new();

    assert_eq!(titles(&harness), [i18n::text("gui.logs.new_tab")]);
}

#[test]
fn a_file_tab_is_titled_by_its_file_name() {
    let mut harness = connected_harness();
    harness.press(Message::LogsPathChanged("/var/log/syslog".into()));

    assert_eq!(titles(&harness), ["syslog"]);
}

#[test]
fn a_unit_tab_is_titled_by_its_unit_and_the_whole_journal_by_its_own_word() {
    let mut harness = connected_harness();
    harness.press(Message::LogsKindChosen(SourceKind::Journal));
    assert_eq!(titles(&harness), [i18n::text("gui.logs.journal_title")]);

    harness.press(Message::LogsUnitChanged("nginx.service".into()));

    assert_eq!(titles(&harness), ["nginx.service"]);
}

#[test]
fn a_title_never_carries_control_characters() {
    let mut harness = connected_harness();
    harness.press(Message::LogsPathChanged("/var/log/a\u{1b}[31mb".into()));

    assert!(titles(&harness)[0].chars().all(|character| !character.is_control()));
}

#[test]
fn titles_get_the_connection_name_only_when_running_tabs_span_connections() {
    let mut harness = connected_harness();
    run_on(&mut harness, "alpha", "/a");
    assert_eq!(titles(&harness), ["a"]);

    harness.press(Message::LogsTabAdded);
    assert_eq!(titles(&harness), ["a", i18n::text("gui.logs.new_tab").as_str()]);
    run_on(&mut harness, "alpha", "/b");
    assert_eq!(titles(&harness), ["a", "b"]);

    harness.press(Message::LogsTabAdded);
    run_on(&mut harness, "beta", "/c");

    assert_eq!(titles(&harness), ["a · alpha", "b · alpha", "c · beta"]);
}

#[test]
fn an_out_of_range_tab_has_an_empty_title() {
    assert_eq!(tab_title(&Harness::new().app, 7), "");
}

#[test]
fn the_add_button_is_pressable_below_the_cap_and_explains_itself_at_it() {
    let mut tabs = LogTabs::default();
    assert!(matches!(add_press(&tabs), (Some(Message::LogsTabAdded), None)));

    for _ in 1..MAX_LOG_TABS {
        tabs.add();
    }

    assert!(add_press(&tabs).0.is_none());
    assert_eq!(add_press(&tabs).1, Some(i18n::text_with("gui.logs.tabs_full", &[("count", "6")])));
}
