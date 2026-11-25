use std::time::Duration;

use iced::keyboard::{Key, Modifiers};
use porthmos_core::{
    Command, Endpoint, Entry, Event, Handover, Listen, Location, Severity, TunnelInfo, TunnelKey, TunnelKind,
    TunnelSpec, TunnelStats, TunnelStatus, VfsPath,
};
use tokio::io::AsyncReadExt;

use crate::app::Message;
use crate::drawer::{DrawerView, TerminalSource};
use crate::modal::Modal;
use crate::shortcuts::{Action, KeyPress};
use crate::testing::{Harness, ShellEnd, shell_pipe};
use crate::tools::Tool;
use crate::tunnel_form::{DraftChange, DraftKind, Field};

fn info(connection: &str, name: &str, status: TunnelStatus) -> TunnelInfo {
    TunnelInfo {
        spec: TunnelSpec {
            connection: connection.to_string(),
            name: name.to_string(),
            kind: TunnelKind::Local {
                listen: "127.0.0.1:5432".parse::<Listen>().unwrap(),
                target: "db.internal:5432".parse::<Endpoint>().unwrap(),
            },
            allow_external: false,
            auto_start: false,
        },
        status,
        stats: TunnelStats::default(),
    }
}

fn running() -> TunnelStatus {
    TunnelStatus::Running { local: "127.0.0.1:5432".parse().unwrap() }
}

fn key(connection: &str, name: &str) -> TunnelKey {
    TunnelKey { connection: connection.to_string(), name: name.to_string() }
}

fn feed_tunnels(harness: &mut Harness, rows: Vec<TunnelInfo>) {
    harness.feed(Event::Tunnels(rows));
}

fn connect(harness: &mut Harness, session: u64, name: &str, folder: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: VfsPath::new(folder),
        entries: Vec::<Entry>::new(),
    });
}

async fn opened_remote(harness: &mut Harness) -> ShellEnd {
    connect(harness, 3, "alpha", "/srv/data");
    harness.app.open_embedded_terminal(TerminalSource::Remote(3));
    let id = harness
        .sent()
        .into_iter()
        .find_map(|command| match command {
            Command::OpenTerminal { id, .. } => Some(id),
            _ => None,
        })
        .expect("an OpenTerminal command");
    let (stream, mut end) = shell_pipe();
    harness.feed(Event::TerminalOpened { id, session: 3, stream: Handover::new(stream) });
    let cd = format!("cd -- {}\r", porthmos_ssh::shell_quote("/srv/data"));
    let mut bytes = vec![0u8; cd.len()];
    tokio::time::timeout(Duration::from_secs(2), end.from_client.read_exact(&mut bytes)).await.unwrap().unwrap();
    end
}

fn type_key(harness: &mut Harness, character: &str) {
    let key = Key::Character(character.into());
    harness.press(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::empty(),
        text: Some(character.into()),
    }));
}

async fn expect_silence(end: &mut ShellEnd) {
    let mut buffer = [0u8; 1];
    let waited = tokio::time::timeout(Duration::from_millis(150), end.from_client.read(&mut buffer)).await;
    assert!(waited.is_err(), "nothing should have been written, got {buffer:?}");
}

#[tokio::test]
async fn the_tunnels_entry_is_first_in_the_strip_and_shows_the_running_count() {
    let mut harness = Harness::new();
    assert_eq!(harness.app.tunnels_tab_label(), "Tunnels");

    feed_tunnels(&mut harness, vec![info("a", "one", running()), info("a", "two", TunnelStatus::Stopped)]);

    assert_eq!(harness.app.tunnels_tab_label(), "Tunnels · 1");
    assert_eq!(crate::view::drawer::strip_labels(&harness.app).first().map(String::as_str), Some("Tunnels · 1"));
}

#[tokio::test]
async fn selecting_the_tunnels_entry_shows_the_list_and_keys_no_longer_reach_a_shell() {
    let mut harness = Harness::new();
    let mut end = opened_remote(&mut harness).await;

    harness.press(Message::TunnelsViewSelected);
    type_key(&mut harness, "x");

    assert_eq!(harness.app.drawer().view, DrawerView::Tunnels);
    assert!(harness.app.drawer().open);
    assert!(!harness.app.terminal_has_keys());
    expect_silence(&mut end).await;
}

#[tokio::test]
async fn selecting_a_terminal_tab_returns_to_terminals() {
    let mut harness = Harness::new();
    let mut end = opened_remote(&mut harness).await;
    harness.press(Message::TunnelsViewSelected);
    let tab = harness.app.drawer().active_tab().unwrap().id;

    harness.press(Message::TerminalTabSelected(tab));
    type_key(&mut harness, "x");

    assert_eq!(harness.app.drawer().view, DrawerView::Terminals);
    let mut byte = [0u8; 1];
    tokio::time::timeout(Duration::from_secs(2), end.from_client.read_exact(&mut byte)).await.unwrap().unwrap();
    assert_eq!(&byte, b"x");
}

#[tokio::test]
async fn the_show_tunnels_action_opens_the_drawer_on_the_tunnels_view_even_without_a_terminal() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::ShowTunnels));

    assert!(harness.app.drawer().open);
    assert_eq!(harness.app.drawer().view, DrawerView::Tunnels);
    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[tokio::test]
async fn start_stop_restart_and_delete_buttons_send_the_matching_commands() {
    let mut harness = Harness::new();
    let one = key("a", "one");
    harness.sent();

    harness.press(Message::TunnelStartRequested(one.clone()));
    harness.press(Message::TunnelStopRequested(one.clone()));
    harness.press(Message::TunnelRestartRequested(one.clone()));
    harness.press(Message::TunnelDeleteRequested(one.clone()));
    harness.press(Message::ModalAccepted);

    let sent = harness.sent();
    assert!(matches!(&sent[0], Command::StartTunnel { key } if *key == one), "{sent:?}");
    assert!(matches!(&sent[1], Command::StopTunnel { key } if *key == one), "{sent:?}");
    assert!(matches!(&sent[2], Command::RestartTunnel { key } if *key == one), "{sent:?}");
    assert!(matches!(&sent[3], Command::DeleteTunnel { key } if *key == one), "{sent:?}");
}

#[tokio::test]
async fn an_offline_tunnels_start_is_disabled() {
    assert!(!crate::view::tunnel_list::start_enabled(&TunnelStatus::Offline));
    assert!(!crate::view::tunnel_list::start_enabled(&TunnelStatus::Starting));
    assert!(!crate::view::tunnel_list::start_enabled(&running()));
    assert!(crate::view::tunnel_list::start_enabled(&TunnelStatus::Stopped));
    assert!(crate::view::tunnel_list::start_enabled(&TunnelStatus::Lost));
}

#[tokio::test]
async fn status_and_stats_events_update_the_rows() {
    let mut harness = Harness::new();
    feed_tunnels(&mut harness, vec![info("a", "one", TunnelStatus::Stopped)]);

    harness.feed(Event::TunnelStatus { key: key("a", "one"), status: running() });
    harness.feed(Event::TunnelStats(vec![(
        key("a", "one"),
        TunnelStats { bytes_up: 7, bytes_down: 9, errors: 2, ..TunnelStats::default() },
    )]));

    let row = &harness.app.tunnels().rows[0];
    assert_eq!(row.status, running());
    assert_eq!((row.stats.bytes_up, row.stats.bytes_down, row.stats.errors), (7, 9, 2));
}

#[tokio::test]
async fn terminal_chords_are_ignored_in_the_tunnels_view() {
    let mut harness = Harness::new();
    let _end = opened_remote(&mut harness).await;
    harness.press(Message::TunnelsViewSelected);
    harness.sent();
    let panes = harness.app.drawer().panes.len();

    let key = Key::Character("d".into());
    harness.press(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::CTRL | Modifiers::SHIFT,
        text: None,
    }));

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.drawer().panes.len(), panes);
    assert_eq!(harness.app.drawer().view, DrawerView::Tunnels);
}

#[tokio::test]
async fn the_drawer_builds_in_the_tunnels_view_with_rows_of_every_status() {
    let mut harness = Harness::new();
    feed_tunnels(
        &mut harness,
        vec![
            info("a", "stopped", TunnelStatus::Stopped),
            info("a", "offline", TunnelStatus::Offline),
            info("a", "starting", TunnelStatus::Starting),
            info("a", "running", running()),
            info("a", "failed", TunnelStatus::Failed(porthmos_core::Message::new("port in use"))),
            info("a", "lost", TunnelStatus::Lost),
        ],
    );
    harness.press(Message::TunnelsViewSelected);

    assert!(crate::view::drawer::view(&harness.app).is_some());
}

#[tokio::test]
async fn an_empty_tunnels_view_still_builds() {
    let mut harness = Harness::new();
    harness.press(Message::TunnelsViewSelected);

    assert!(crate::view::drawer::view(&harness.app).is_some());
    assert!(!matches!(harness.app.tool(), Some(Tool::Palette(_))));
}

fn fill(harness: &mut Harness, name: &str) {
    for change in [
        DraftChange::Name(name.to_string()),
        DraftChange::ListenPort("5432".to_string()),
        DraftChange::TargetHost("db.internal".to_string()),
        DraftChange::TargetPort("5432".to_string()),
    ] {
        harness.press(Message::TunnelDraftChanged(change));
    }
}

fn saves(sent: &[Command]) -> Vec<(Option<TunnelKey>, String)> {
    sent.iter()
        .filter_map(|command| match command {
            Command::SaveTunnel { original, spec } => Some((original.clone(), spec.name.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn new_tunnel_opens_the_form_for_a_connected_connection_and_submit_saves_it() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv");
    harness.sent();

    harness.press(Message::ActionRequested(Action::NewTunnel));

    let Some(Modal::Tunnel { draft, connections, .. }) = harness.app.modal() else {
        panic!("the form should be open");
    };
    assert_eq!(draft.connection, "alpha");
    assert_eq!(connections, &vec!["alpha".to_string()]);
    assert_eq!(harness.app.drawer().view, DrawerView::Tunnels);

    fill(&mut harness, "postgres");
    harness.press(Message::ModalAccepted);

    assert!(harness.app.modal().is_none());
    assert_eq!(saves(&harness.sent()), vec![(None, "postgres".to_string())]);
}

#[tokio::test]
async fn submitting_an_invalid_draft_keeps_the_form_open_and_sends_nothing() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv");
    harness.press(Message::ActionRequested(Action::NewTunnel));
    harness.sent();
    harness.press(Message::TunnelDraftChanged(DraftChange::ListenPort("80a".to_string())));

    harness.press(Message::ModalAccepted);

    let Some(Modal::Tunnel { errors, .. }) = harness.app.modal() else {
        panic!("the form should stay open");
    };
    assert!(errors.iter().any(|(field, _)| *field == Field::ListenPort), "{errors:?}");
    assert!(errors.iter().any(|(field, _)| *field == Field::Name), "{errors:?}");
    assert!(harness.sent().is_empty());
}

#[tokio::test]
async fn typing_clears_the_error_of_the_field_being_edited() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha", "/srv");
    harness.press(Message::ActionRequested(Action::NewTunnel));
    harness.press(Message::ModalAccepted);

    harness.press(Message::TunnelDraftChanged(DraftChange::Name("x".to_string())));

    let Some(Modal::Tunnel { errors, .. }) = harness.app.modal() else {
        panic!("the form should stay open");
    };
    assert!(!errors.iter().any(|(field, _)| *field == Field::Name), "{errors:?}");
    assert!(errors.iter().any(|(field, _)| *field == Field::ListenPort), "{errors:?}");
}

#[tokio::test]
async fn new_tunnel_without_a_connected_connection_says_so() {
    let mut harness = Harness::new();

    harness.press(Message::ActionRequested(Action::NewTunnel));

    assert!(harness.app.modal().is_none());
    let notice = harness.app.notices().last().expect("a notice");
    assert_eq!(notice.severity, Severity::Warning);
}

#[tokio::test]
async fn editing_a_stopped_tunnel_saves_without_asking() {
    let mut harness = Harness::new();
    feed_tunnels(&mut harness, vec![info("alpha", "one", TunnelStatus::Stopped)]);
    harness.sent();

    harness.press(Message::TunnelFormRequested(Some(key("alpha", "one"))));
    harness.press(Message::TunnelDraftChanged(DraftChange::Name("renamed".to_string())));
    harness.press(Message::ModalAccepted);

    assert!(harness.app.modal().is_none());
    let sent = harness.sent();
    assert_eq!(saves(&sent), vec![(Some(key("alpha", "one")), "renamed".to_string())]);
    assert!(!sent.iter().any(|command| matches!(command, Command::RestartTunnel { .. })));
}

#[tokio::test]
async fn the_connection_of_an_edited_tunnel_is_fixed() {
    let mut harness = Harness::new();
    feed_tunnels(&mut harness, vec![info("alpha", "one", TunnelStatus::Stopped)]);

    harness.press(Message::TunnelFormRequested(Some(key("alpha", "one"))));

    let Some(Modal::Tunnel { draft, connections, .. }) = harness.app.modal() else {
        panic!("the form should be open");
    };
    assert_eq!(draft.original, Some(key("alpha", "one")));
    assert_eq!(connections, &vec!["alpha".to_string()]);
}

#[tokio::test]
async fn editing_a_running_tunnel_asks_to_restart_and_restart_sends_save_then_restart() {
    let mut harness = Harness::new();
    feed_tunnels(&mut harness, vec![info("alpha", "one", running())]);
    harness.sent();

    harness.press(Message::TunnelFormRequested(Some(key("alpha", "one"))));
    harness.press(Message::TunnelDraftChanged(DraftChange::Name("renamed".to_string())));
    harness.press(Message::ModalAccepted);

    assert!(
        matches!(harness.app.modal(), Some(Modal::ConfirmRestartTunnel { key: asked }) if *asked == key("alpha", "renamed"))
    );
    let before = harness.sent();
    assert_eq!(saves(&before).len(), 1);
    assert!(!before.iter().any(|command| matches!(command, Command::RestartTunnel { .. })));

    harness.press(Message::ModalAccepted);

    let after = harness.sent();
    assert!(
        matches!(after.as_slice(), [Command::RestartTunnel { key: restarted }] if *restarted == key("alpha", "renamed")),
        "{after:?}"
    );
}

#[tokio::test]
async fn declining_the_restart_saves_only() {
    let mut harness = Harness::new();
    feed_tunnels(&mut harness, vec![info("alpha", "one", running())]);
    harness.press(Message::TunnelFormRequested(Some(key("alpha", "one"))));
    harness.press(Message::ModalAccepted);
    harness.sent();

    harness.press(Message::ModalDeclined);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[tokio::test]
async fn deleting_asks_first_and_only_the_confirmation_sends_the_command() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::TunnelDeleteRequested(key("alpha", "one")));

    assert!(matches!(harness.app.modal(), Some(Modal::ConfirmDeleteTunnel { .. })));
    assert!(harness.sent().is_empty());

    harness.press(Message::ModalDeclined);
    assert!(harness.sent().is_empty());

    harness.press(Message::TunnelDeleteRequested(key("alpha", "one")));
    harness.press(Message::ModalAccepted);
    assert!(
        matches!(harness.sent().as_slice(), [Command::DeleteTunnel { key: deleted }] if *deleted == key("alpha", "one"))
    );
}

#[tokio::test]
async fn enter_in_the_delete_prompt_does_not_delete() {
    let mut harness = Harness::new();
    harness.press(Message::TunnelDeleteRequested(key("alpha", "one")));
    harness.sent();

    let enter = Key::Named(iced::keyboard::key::Named::Enter);
    harness.press(Message::KeyPressed(KeyPress {
        key: enter.clone(),
        modified_key: enter,
        modifiers: Modifiers::empty(),
        text: None,
    }));

    assert!(harness.sent().is_empty());
    assert!(harness.app.modal().is_none());
}

#[tokio::test]
async fn a_draft_change_without_the_form_open_does_nothing() {
    let mut harness = Harness::new();

    harness.press(Message::TunnelDraftChanged(DraftChange::Kind(DraftKind::Dynamic)));

    assert!(harness.app.modal().is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_dropped_on_the_tunnels_view_is_not_typed_into_the_hidden_shell() {
    let mut harness = Harness::new();
    let scratch = tempfile::tempdir().unwrap();
    let wanted = crate::drawer::Wanted {
        id: 1,
        source: TerminalSource::Local,
        directory: porthmos_core::from_os_path(&scratch.path().canonicalize().unwrap()),
        placement: crate::drawer::Placement::NewTab,
    };
    harness.app.start_local_terminal(wanted);
    harness.press(Message::TunnelsViewSelected);
    let size = harness.app.window_size();
    let at = iced::Point::new(size.width / 2.0, crate::geometry::drawer_top(size, 0.0) + 10.0);
    let dropped = scratch.path().join("zzz-dropped-marker");

    let swallowed = harness.app.drop_into_terminal(at, &[dropped]);

    assert!(swallowed);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let snapshot = harness.app.drawer().active_pane().unwrap().snapshot().unwrap();
    let screen: Vec<String> = (0..snapshot.size.rows).map(|row| snapshot.row_text(row)).collect();
    assert!(!screen.iter().any(|line| line.contains("zzz-dropped-marker")), "{screen:?}");
}
