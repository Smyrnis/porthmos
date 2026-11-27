use porthmos_core::{Event, Location, PortsState as PortsEvent};

use super::*;
use crate::app::Message;
use crate::ports_view::PortsPane;
use crate::ports_view::tests::snapshot;
use crate::testing::Harness;

fn connect(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
    harness.feed(Event::Listed {
        location: Location::Session(session),
        path: porthmos_core::VfsPath::new("/"),
        entries: Vec::new(),
    });
}

fn shown_in(harness: &mut Harness, state: Option<PortsEvent>) {
    if let Some(state) = state {
        harness.feed(Event::Ports { session: 3, state });
    }
    let _ = view(&harness.app);
}

#[test]
fn the_tab_builds_in_every_state() {
    let mut harness = Harness::new();
    harness.press(Message::PortsViewSelected);
    shown_in(&mut harness, None);

    connect(&mut harness, 3, "alpha");
    shown_in(&mut harness, None);
    for pane in [PortsPane::Listening, PortsPane::Connections, PortsPane::Interfaces] {
        harness.press(Message::PortsViewChosen(pane));
        shown_in(&mut harness, Some(PortsEvent::Snapshot(Box::new(snapshot()))));
        shown_in(&mut harness, Some(PortsEvent::Failing("no answer".into())));
        shown_in(&mut harness, Some(PortsEvent::Unavailable("Ports are only available for Linux servers".into())));
        shown_in(&mut harness, Some(PortsEvent::Failing("no answer".into())));
        shown_in(&mut harness, None);
    }
    let mut empty = snapshot();
    empty.listening.clear();
    empty.connections.clear();
    empty.interfaces.clear();
    harness.press(Message::PortsFilterChanged("nothing matches this".into()));
    for pane in [PortsPane::Listening, PortsPane::Connections, PortsPane::Interfaces] {
        harness.press(Message::PortsViewChosen(pane));
        shown_in(&mut harness, Some(PortsEvent::Snapshot(Box::new(empty.clone()))));
        shown_in(&mut harness, Some(PortsEvent::Snapshot(Box::new(snapshot()))));
    }
    connect(&mut harness, 4, "beta");
    shown_in(&mut harness, None);
}

#[test]
fn a_snapshot_with_problems_and_a_huge_number_of_sockets_builds() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    let mut big = snapshot();
    big.problems = vec![porthmos_core::PortsSection::Sockets, porthmos_core::PortsSection::Interfaces];
    let template = big.listening[0].clone();
    big.listening = vec![template; 2000];

    shown_in(&mut harness, Some(PortsEvent::Snapshot(Box::new(big))));
}

#[test]
fn the_filter_narrows_the_socket_rows_and_ignores_case() {
    let snapshot = snapshot();
    let mut state = PortsState::default();

    assert_eq!(shown_listening(&state, &snapshot).len(), 3);
    assert_eq!(shown_connections(&state, &snapshot).len(), 2);
    state.filter = "SSHD".into();
    assert_eq!(shown_listening(&state, &snapshot).len(), 1);
    assert_eq!(shown_connections(&state, &snapshot).len(), 1);
    state.filter = ":5432".into();
    assert_eq!(shown_listening(&state, &snapshot).len(), 1);
    assert!(shown_connections(&state, &snapshot).is_empty());
    state.filter = "no such socket".into();
    assert!(shown_listening(&state, &snapshot).is_empty());
}

#[test]
fn the_filter_does_not_apply_to_interfaces() {
    let mut harness = Harness::new();
    connect(&mut harness, 3, "alpha");
    harness.press(Message::PortsViewSelected);
    harness.press(Message::PortsFilterChanged("no such socket".into()));
    harness.press(Message::PortsViewChosen(PortsPane::Interfaces));

    assert!(!shows_filter(harness.app.ports().pane));
    assert!(shows_filter(PortsPane::Listening) && shows_filter(PortsPane::Connections));
    shown_in(&mut harness, Some(PortsEvent::Snapshot(Box::new(snapshot()))));
}

#[test]
fn listening_rows_come_in_port_order() {
    let snapshot = snapshot();
    let state = PortsState::default();

    let locals: Vec<&str> = shown_listening(&state, &snapshot).iter().map(|socket| socket.local.as_str()).collect();

    assert_eq!(locals, ["0.0.0.0:22", "0.0.0.0:68", "127.0.0.1:5432"]);
}

#[test]
fn dimmed_rows_use_a_quieter_colour_in_every_scheme() {
    use crate::theme::{Palette, ThemeSettings};
    use porthmos_core::theme::ColorScheme;
    for scheme in [ColorScheme::dark(), ColorScheme::light(), ColorScheme::high_contrast()] {
        let palette = Palette::new(&ThemeSettings::default().with_scheme(scheme));

        assert_eq!(row_color(false, &palette), palette.fg);
        assert_eq!(row_color(true, &palette), palette.fg_dim);
    }
}

#[test]
fn a_cut_address_or_owner_offers_its_whole_text_as_a_tooltip_and_a_short_one_offers_nothing() {
    use crate::ports_view::tests::socket;
    use porthmos_core::{SocketProtocol, SocketState};
    let long_address = format!("[{}]:8080", "fe80:0000:0000:0000:0202:b3ff:fe1e:8329");
    let long_name = "n".repeat(OWNER_CELLS + 10);
    let owned = socket(SocketProtocol::Tcp, "0.0.0.0:22", "0.0.0.0:0", SocketState::Listen, Some((812, &long_name)));
    let short = socket(SocketProtocol::Tcp, "0.0.0.0:22", "0.0.0.0:0", SocketState::Listen, Some((812, "sshd")));
    let unknown = socket(SocketProtocol::Tcp, "0.0.0.0:22", "0.0.0.0:0", SocketState::Listen, None);

    assert_eq!(address_tooltip(&long_address), Some(long_address.clone()));
    assert_eq!(address_tooltip("0.0.0.0:22"), None);
    assert_eq!(owner_tooltip(0, &owned), Some(format!("{long_name} (812)")));
    assert_eq!(owner_tooltip(0, &short), None);
    assert_eq!(owner_tooltip(1000, &unknown), Some(porthmos_core::i18n::text("gui.ports.run_as_root")));
    assert_eq!(owner_tooltip(0, &unknown), None);
}
