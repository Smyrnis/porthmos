use porthmos_core::{NetInterface, PortsSnapshot, Socket, SocketProtocol, SocketState};

use super::*;

pub(crate) fn socket(
    protocol: SocketProtocol, local: &str, remote: &str, state: SocketState, owner: Option<(u32, &str)>,
) -> Socket {
    Socket {
        protocol,
        local: local.to_string(),
        remote: remote.to_string(),
        state,
        owner: owner.map(|(pid, name)| (pid, name.to_string())),
    }
}

pub(crate) fn snapshot() -> PortsSnapshot {
    PortsSnapshot {
        listening: vec![
            socket(SocketProtocol::Tcp, "0.0.0.0:22", "0.0.0.0:0", SocketState::Listen, Some((812, "sshd"))),
            socket(SocketProtocol::Tcp, "127.0.0.1:5432", "0.0.0.0:0", SocketState::Listen, None),
            socket(SocketProtocol::Udp, "0.0.0.0:68", "0.0.0.0:0", SocketState::Listen, Some((500, "dhclient"))),
        ],
        connections: vec![
            socket(SocketProtocol::Tcp, "10.0.0.2:22", "10.0.0.9:50000", SocketState::Established, Some((900, "sshd"))),
            socket(SocketProtocol::Tcp, "10.0.0.2:80", "10.0.0.7:4000", SocketState::TimeWait, None),
        ],
        interfaces: vec![
            NetInterface {
                name: "eth0".into(),
                up: Some(true),
                addresses: vec!["10.0.0.2/24".into(), "fe80::1/64".into()],
                mac: Some("52:54:00:aa:bb:cc".into()),
                mtu: Some(1500),
                rx_bytes: 2048,
                tx_bytes: 1024,
            },
            NetInterface {
                name: "lo".into(),
                up: None,
                addresses: Vec::new(),
                mac: None,
                mtu: None,
                rx_bytes: 0,
                tx_bytes: 0,
            },
        ],
        uid: 1000,
        problems: Vec::new(),
    }
}

fn snapshot_event() -> PortsEvent {
    PortsEvent::Snapshot(Box::new(snapshot()))
}

#[test]
fn a_new_state_starts_on_listening_with_nothing_shown() {
    let state = PortsState::default();

    assert_eq!(state.pane, PortsPane::Listening);
    assert!(state.state.is_none() && state.snapshot().is_none() && !state.failing());
    assert!(state.filter.is_empty());
}

#[test]
fn events_for_another_session_are_ignored() {
    let mut state = PortsState::default();
    state.begin(3);

    state.apply(4, snapshot_event());
    assert!(state.state.is_none());
    state.apply(3, snapshot_event());
    assert!(state.snapshot().is_some());

    state.clear();
    state.apply(3, snapshot_event());
    assert!(state.state.is_none());
}

#[test]
fn failing_keeps_the_last_snapshot_and_a_new_snapshot_replaces_it() {
    let mut state = PortsState::default();
    state.begin(3);
    state.apply(3, snapshot_event());

    state.apply(3, PortsEvent::Failing("no answer".into()));
    state.apply(3, PortsEvent::Failing("still no answer".into()));

    assert!(state.failing());
    assert_eq!(state.snapshot().map(|snapshot| snapshot.listening.len()), Some(3));
    let mut newer = snapshot();
    newer.listening.truncate(1);
    state.apply(3, PortsEvent::Snapshot(Box::new(newer)));
    assert!(!state.failing());
    assert_eq!(state.snapshot().map(|snapshot| snapshot.listening.len()), Some(1));
}

#[test]
fn failing_before_any_snapshot_and_unavailable_show_no_snapshot() {
    let mut state = PortsState::default();
    state.begin(3);

    state.apply(3, PortsEvent::Failing("no answer".into()));
    assert!(state.failing() && state.snapshot().is_none());

    state.apply(3, snapshot_event());
    state.apply(3, PortsEvent::Unavailable("not linux".into()));
    assert!(!state.failing() && state.snapshot().is_none());
    state.apply(3, PortsEvent::Failing("no answer".into()));
    assert!(state.snapshot().is_none());
}

#[test]
fn begin_and_clear_forget_everything_shown_but_keep_pane_and_filter() {
    let mut state = PortsState { pane: PortsPane::Connections, filter: "ssh".into(), ..PortsState::default() };
    state.begin(3);
    state.apply(3, snapshot_event());

    state.begin(4);
    assert!(state.state.is_none() && state.snapshot().is_none());
    assert_eq!(state.session, Some(4));
    state.apply(4, snapshot_event());
    state.clear();

    assert!(state.state.is_none() && state.snapshot().is_none());
    assert_eq!(state.session, None);
    assert_eq!((state.pane, state.filter.as_str()), (PortsPane::Connections, "ssh"));
}

#[test]
fn owner_text_names_the_process_or_shows_a_dash() {
    let named = socket(SocketProtocol::Tcp, "a", "b", SocketState::Listen, Some((812, "sshd")));
    let unreadable = socket(SocketProtocol::Tcp, "a", "b", SocketState::Listen, None);

    assert_eq!(owner_text(&named), "sshd (812)");
    assert_eq!(owner_text(&unreadable), "\u{2014}");
}

#[test]
fn the_root_hint_is_for_unreadable_owners_when_not_root() {
    let named = socket(SocketProtocol::Tcp, "a", "b", SocketState::Listen, Some((812, "sshd")));
    let unreadable = socket(SocketProtocol::Tcp, "a", "b", SocketState::Listen, None);

    assert_eq!(owner_hint(1000, &named), None);
    assert_eq!(owner_hint(1000, &unreadable).as_deref(), Some("Run as root to see the process"));
    assert_eq!(owner_hint(0, &unreadable), None);
}

#[test]
fn protocol_and_state_names_are_the_usual_network_words() {
    assert_eq!(protocol_text(SocketProtocol::Tcp), "TCP");
    assert_eq!(protocol_text(SocketProtocol::Udp), "UDP");
    for (state, text) in [
        (SocketState::Listen, "LISTEN"),
        (SocketState::Established, "ESTABLISHED"),
        (SocketState::SynSent, "SYN_SENT"),
        (SocketState::SynRecv, "SYN_RECV"),
        (SocketState::FinWait1, "FIN_WAIT1"),
        (SocketState::FinWait2, "FIN_WAIT2"),
        (SocketState::TimeWait, "TIME_WAIT"),
        (SocketState::Close, "CLOSE"),
        (SocketState::CloseWait, "CLOSE_WAIT"),
        (SocketState::LastAck, "LAST_ACK"),
        (SocketState::Closing, "CLOSING"),
        (SocketState::Other, "\u{2014}"),
    ] {
        assert_eq!(socket_state_text(state), text);
    }
}

#[test]
fn interface_text_covers_unknown_values() {
    let snapshot = snapshot();

    assert_eq!(interface_state_text(Some(true)), "up");
    assert_eq!(interface_state_text(Some(false)), "down");
    assert_eq!(interface_state_text(None), "\u{2014}");
    assert_eq!(addresses_text(&snapshot.interfaces[0]), "10.0.0.2/24, fe80::1/64");
    assert_eq!(addresses_text(&snapshot.interfaces[1]), "\u{2014}");
    assert_eq!(mac_text(&snapshot.interfaces[0]), "52:54:00:aa:bb:cc");
    assert_eq!(mac_text(&snapshot.interfaces[1]), "\u{2014}");
    assert_eq!(mtu_text(&snapshot.interfaces[0]), "1500");
    assert_eq!(mtu_text(&snapshot.interfaces[1]), "\u{2014}");
}
