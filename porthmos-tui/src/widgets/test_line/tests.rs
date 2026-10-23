use std::time::Duration;

use porthmos_core::{NotTrusted, StartFolder, TestOutcome, TestReport, TestStage};
use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, Theme, ThemeSettings},
    widgets::dialog::TestState,
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn render(state: &TestState, width: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new(line(state, usize::from(width), &theme)), frame.area());
        })
        .unwrap();
    terminal
}

fn text(state: &TestState, width: u16) -> String {
    screen_rows(&render(state, width))[0].trim_end().to_string()
}

fn report(millis: u64, server: Option<&str>, start_folder: StartFolder) -> TestState {
    TestState::Finished(TestOutcome::Connected(TestReport {
        elapsed: Duration::from_millis(millis),
        server: server.map(str::to_string),
        start_folder,
    }))
}

fn opens(path: &str) -> StartFolder {
    StartFolder::Opens(path.to_string())
}

#[test]
fn running_shows_a_dim_testing_line() {
    let theme = indexed();
    let terminal = render(&TestState::Running, 80);

    assert_eq!(screen_rows(&terminal)[0].trim_end(), "\u{25cb} testing\u{2026}");
    assert_eq!(cell_fg(&terminal, 0, 0), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, 2, 0), theme.fg_dim);
}

#[test]
fn connected_shows_time_server_and_folder_opens() {
    let theme = indexed();
    let state = report(142, Some("OpenSSH_9.6"), opens("/home/deploy"));
    let terminal = render(&state, 100);

    assert_eq!(
        screen_rows(&terminal)[0].trim_end(),
        "\u{25cf} connected \u{b7} 142 ms \u{b7} OpenSSH_9.6 \u{b7} /home/deploy opens"
    );
    assert_eq!(cell_fg(&terminal, 0, 0), theme.ok);
    let (x, y) = find_text(&terminal, "142 ms").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    let (x, y) = find_text(&terminal, "OpenSSH_9.6").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
}

#[test]
fn connected_without_server_info_omits_that_part() {
    let state = report(142, None, opens("/home/deploy"));

    assert_eq!(text(&state, 100), "\u{25cf} connected \u{b7} 142 ms \u{b7} /home/deploy opens");
}

#[test]
fn a_missing_or_unreadable_start_folder_is_bright_and_named() {
    let theme = indexed();
    let missing = report(90, Some("vsFTPd 3.0.5"), StartFolder::Missing("/srv".to_string()));
    let terminal = render(&missing, 100);
    assert!(screen_rows(&terminal)[0].contains("\u{b7} start folder /srv not found"));
    let (x, y) = find_text(&terminal, "start folder /srv not found").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);

    let denied = report(90, None, StartFolder::Unreadable { path: "/srv".to_string(), message: "denied".to_string() });
    assert!(text(&denied, 100).ends_with("\u{b7} /srv not readable"));
}

#[test]
fn an_unchecked_folder_says_so_in_dim() {
    let theme = indexed();
    let terminal = render(&report(90, None, StartFolder::Unchecked), 100);

    assert!(
        screen_rows(&terminal)[0].ends_with("start folder not checked")
            || screen_rows(&terminal)[0].trim_end().ends_with("start folder not checked")
    );
    let (x, y) = find_text(&terminal, "start folder not checked").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn a_host_key_not_trusted_yet_shows_the_fingerprint_in_bright() {
    let theme = indexed();
    let state = TestState::Finished(TestOutcome::NotTrusted(NotTrusted::HostKey {
        key_type: "ssh-ed25519".to_string(),
        fingerprint: "SHA256:ab12CD".to_string(),
    }));
    let terminal = render(&state, 100);

    assert_eq!(
        screen_rows(&terminal)[0].trim_end(),
        "\u{25cf} host reachable \u{b7} host key not trusted yet \u{b7} SHA256:ab12CD"
    );
    assert_eq!(cell_fg(&terminal, 0, 0), theme.fg_bright);
}

#[test]
fn a_certificate_not_trusted_yet_shows_the_fingerprint_in_bright() {
    let state = TestState::Finished(TestOutcome::NotTrusted(NotTrusted::Certificate {
        fingerprint: "AA:BB:CC".to_string(),
        subject: "CN=files.example".to_string(),
    }));

    assert_eq!(text(&state, 100), "\u{25cf} host reachable \u{b7} certificate not trusted yet \u{b7} AA:BB:CC");
}

#[test]
fn failures_use_err_and_their_wording() {
    let theme = indexed();
    let failed =
        |stage, message: &str| TestState::Finished(TestOutcome::Failed { stage, message: message.to_string() });

    let login = render(&failed(TestStage::Login, "Authentication failed for nas"), 100);
    assert_eq!(screen_rows(&login)[0].trim_end(), "\u{25cf} failed at login \u{b7} Authentication failed for nas");
    assert_eq!(cell_fg(&login, 0, 0), theme.err);
    assert_eq!(
        text(&failed(TestStage::Connect, "connection refused"), 100),
        "\u{25cf} failed to connect \u{b7} connection refused"
    );
    assert_eq!(text(&failed(TestStage::StartFolder, "gone"), 100), "\u{25cf} failed at the start folder \u{b7} gone");
    assert_eq!(text(&failed(TestStage::Settings, "Host can't be empty"), 100), "\u{25cf} Host can't be empty");
}

#[test]
fn timed_out_names_the_stage_and_fifteen_seconds() {
    let theme = indexed();
    let timed = |stage| TestState::Finished(TestOutcome::TimedOut(stage));

    let connect = render(&timed(TestStage::Connect), 100);
    assert_eq!(screen_rows(&connect)[0].trim_end(), "\u{25cf} timed out after 15 s while connecting");
    assert_eq!(cell_fg(&connect, 0, 0), theme.err);
    assert_eq!(text(&timed(TestStage::Login), 100), "\u{25cf} timed out after 15 s while logging in");
    assert_eq!(
        text(&timed(TestStage::StartFolder), 100),
        "\u{25cf} timed out after 15 s while checking the start folder"
    );
}

#[test]
fn cancelled_is_dim() {
    let theme = indexed();
    let terminal = render(&TestState::Finished(TestOutcome::Cancelled), 100);

    assert_eq!(screen_rows(&terminal)[0].trim_end(), "\u{25cf} test cancelled");
    assert_eq!(cell_fg(&terminal, 0, 0), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, 2, 0), theme.fg_dim);
}

#[test]
fn every_state_starts_with_a_glyph() {
    let states = [
        TestState::Running,
        report(1, None, opens("/")),
        TestState::Finished(TestOutcome::Cancelled),
        TestState::Finished(TestOutcome::TimedOut(TestStage::Connect)),
        TestState::Finished(TestOutcome::Failed { stage: TestStage::Login, message: "x".into() }),
        TestState::Finished(TestOutcome::NotTrusted(NotTrusted::HostKey {
            key_type: "k".into(),
            fingerprint: "f".into(),
        })),
    ];
    for state in states {
        let first = text(&state, 80).chars().next().unwrap();
        assert!(first == '\u{25cf}' || first == '\u{25cb}', "{state:?}");
    }
}

#[test]
fn the_time_is_milliseconds_below_ten_seconds_and_seconds_with_one_decimal_above() {
    assert_eq!(format_elapsed(Duration::from_millis(0)), "0 ms");
    assert_eq!(format_elapsed(Duration::from_millis(142)), "142 ms");
    assert_eq!(format_elapsed(Duration::from_millis(9_999)), "9999 ms");
    assert_eq!(format_elapsed(Duration::from_millis(10_000)), "10.0 s");
    assert_eq!(format_elapsed(Duration::from_millis(12_345)), "12.3 s");
}

#[test]
fn server_text_and_messages_are_sanitised_and_cut_but_the_time_never_is() {
    let hostile = "a\u{1b}[2J\u{202e}b\nc".repeat(50);
    let state = report(142, Some(&hostile), opens(&"\u{65e5}".repeat(300)));
    let failed = TestState::Finished(TestOutcome::Failed {
        stage: TestStage::Connect,
        message: format!("first\nsecond\u{1b}[31m{}", "x".repeat(500)),
    });

    for width in [20u16, 40, 80, 130] {
        let terminal = render(&state, width);
        assert!(!has_unprintable_cell(&terminal), "{width}");
        assert!(find_text(&terminal, "142 ms").is_some(), "{width}");
        let terminal = render(&failed, width);
        assert!(!has_unprintable_cell(&terminal), "{width}");
        assert!(!screen_rows(&terminal)[0].contains("second"), "{width}");
    }
}

#[test]
fn the_line_fits_one_row_at_every_width_and_never_panics() {
    let theme = indexed();
    let states = [
        TestState::Running,
        report(142, Some("OpenSSH_9.6p1 Ubuntu-3ubuntu13.4"), opens("/home/deploy/projects")),
        TestState::Finished(TestOutcome::Failed { stage: TestStage::Login, message: "x".repeat(200) }),
        TestState::Finished(TestOutcome::NotTrusted(NotTrusted::HostKey {
            key_type: "k".into(),
            fingerprint: "SHA256:".to_string() + &"a".repeat(43),
        })),
    ];
    for state in &states {
        for width in 0..=130usize {
            let built = line(state, width, &theme);
            if width >= 2 {
                assert!(built.width() <= width, "{state:?} {width}: {}", built.width());
            }
            render(state, width as u16);
        }
    }
}

#[test]
fn an_indexed_theme_keeps_ok_bright_err_and_dim_distinct() {
    let theme = indexed();
    let colours = [theme.ok, theme.fg_bright, theme.err, theme.fg_dim];
    for (index, colour) in colours.iter().enumerate() {
        assert!(colours.iter().skip(index + 1).all(|other| other != colour));
    }
}
