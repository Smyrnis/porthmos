use super::*;
use crate::Token;

const DEBIAN: &str = include_str!("../../tests/fixtures/ports_debian.txt");
const ALPINE: &str = include_str!("../../tests/fixtures/ports_alpine.txt");
const CONTAINER: &str = include_str!("../../tests/fixtures/ports_container.txt");
const MACOS: &str = include_str!("../../tests/fixtures/ports_macos.txt");

const SOCKET_HEADER: &str =
    "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode";
const NOBODY: &str = "00000000:0000";
const ANY_V4_22: &str = "00000000:0016";

fn token() -> Token {
    Token::from_hex("00112233445566778899aabbccddeeff").unwrap()
}

fn render(fixture: &str) -> Vec<u8> {
    fixture.replace("@@T:", &format!("@@{}:", token().as_str())).into_bytes()
}

fn linux(output: &[u8]) -> Box<PortsSnapshot> {
    match parse_ports(output, &token()) {
        ParsedPorts::Linux(snapshot) => snapshot,
        other => panic!("expected a Linux snapshot, got {other:?}"),
    }
}

fn socket_line(local: &str, remote: &str, state: &str, inode: u64) -> String {
    format!(
        "   0: {local} {remote} {state} 00000000:00000000 00:00000000 00000000     0        0 {inode} 1 0 100 0 0 10 0"
    )
}

fn synthetic(tcp_lines: &[String]) -> Vec<u8> {
    let mut text = format!("@@T:os\nLinux\n@@T:uid\n0\n@@T:tcp\n{SOCKET_HEADER}\n");
    for line in tcp_lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("@@T:end\n");
    render(&text)
}

fn without_section(fixture: &str, name: &str) -> String {
    let mut kept = Vec::new();
    let mut skipping = false;
    for line in fixture.lines() {
        if let Some(found) = line.strip_prefix("@@T:") {
            skipping = found == name;
        }
        if !skipping {
            kept.push(line);
        }
    }
    kept.join("\n") + "\n"
}

fn find<'a>(sockets: &'a [Socket], protocol: SocketProtocol, local: &str) -> &'a Socket {
    sockets
        .iter()
        .find(|socket| socket.protocol == protocol && socket.local == local)
        .unwrap_or_else(|| panic!("no {protocol:?} socket on {local} in {sockets:?}"))
}

fn socket(protocol: SocketProtocol, local: &str, remote: &str, state: SocketState, owner: Option<&str>) -> Socket {
    Socket {
        protocol,
        local: local.to_string(),
        remote: remote.to_string(),
        state,
        owner: owner.map(|name| (1, name.to_string())),
    }
}

fn locals(sockets: &[&Socket]) -> Vec<String> {
    sockets.iter().map(|socket| socket.local.clone()).collect()
}

#[test]
fn parses_ipv4_listen_and_established_from_the_debian_fixture() {
    let snapshot = linux(&render(DEBIAN));

    let ssh = find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22");
    assert_eq!(ssh.state, SocketState::Listen);
    assert_eq!(ssh.remote, "0.0.0.0:0");
    assert_eq!(ssh.owner, Some((812, "sshd".to_string())));
    let cups = find(&snapshot.listening, SocketProtocol::Tcp, "127.0.0.1:631");
    assert_eq!(cups.owner, Some((731, "cupsd".to_string())));

    let session = snapshot
        .connections
        .iter()
        .find(|socket| socket.local == "10.0.0.2:22" && socket.state == SocketState::Established)
        .expect("the established ssh session");
    assert_eq!(session.remote, "10.0.0.9:51234");
    assert_eq!(session.owner, Some((2301, "sshd".to_string())));
    let waiting = snapshot.connections.iter().find(|socket| socket.state == SocketState::TimeWait).unwrap();
    assert_eq!(waiting.remote, "10.0.0.9:51235");
    assert_eq!(waiting.owner, None);
    assert_eq!(snapshot.uid, 0);
    assert!(snapshot.problems.is_empty());
}

#[test]
fn parses_ipv6_and_shows_mapped_ipv4_as_ipv4() {
    let snapshot = linux(&render(DEBIAN));

    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "[::]:80").owner, Some((905, "nginx".to_string())));
    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "[::1]:631").owner, Some((731, "cupsd".to_string())));
    let mapped = find(&snapshot.connections, SocketProtocol::Tcp, "10.0.0.2:443");
    assert_eq!(mapped.remote, "10.0.0.5:40000");
    assert_eq!(mapped.state, SocketState::Established);
    assert_eq!(mapped.owner, Some((905, "nginx".to_string())));
    for shown in snapshot.listening.iter().chain(&snapshot.connections) {
        assert!(!shown.local.to_lowercase().contains("ffff"), "{shown:?}");
        assert!(!shown.remote.to_lowercase().contains("ffff"), "{shown:?}");
    }
}

#[test]
fn udp_sockets_with_no_remote_are_listening() {
    let snapshot = linux(&render(DEBIAN));

    let dhcp = find(&snapshot.listening, SocketProtocol::Udp, "0.0.0.0:68");
    assert_eq!(dhcp.state, SocketState::Listen);
    assert_eq!(dhcp.owner, Some((640, "dhclient".to_string())));
    assert_eq!(find(&snapshot.listening, SocketProtocol::Udp, "[::]:5353").owner.as_ref().unwrap().1, "avahi-daemon");
    assert!(snapshot.connections.iter().all(|socket| socket.protocol == SocketProtocol::Tcp));

    let connected = socket_line("0200000A:0044", "0900000A:0035", "01", 77);
    let text = format!("@@T:os\nLinux\n@@T:tcp\n{SOCKET_HEADER}\n@@T:udp\n{SOCKET_HEADER}\n{connected}\n@@T:end\n");
    let snapshot = linux(&render(&text));
    assert!(snapshot.listening.is_empty());
    assert_eq!(snapshot.connections.len(), 1);
    assert_eq!(snapshot.connections[0].protocol, SocketProtocol::Udp);
    assert_eq!(snapshot.connections[0].remote, "10.0.0.9:53");
    assert_eq!(snapshot.connections[0].state, SocketState::Established);
}

#[test]
fn every_tcp_state_code_maps() {
    let expected = [
        ("01", SocketState::Established),
        ("02", SocketState::SynSent),
        ("03", SocketState::SynRecv),
        ("04", SocketState::FinWait1),
        ("05", SocketState::FinWait2),
        ("06", SocketState::TimeWait),
        ("07", SocketState::Close),
        ("08", SocketState::CloseWait),
        ("09", SocketState::LastAck),
        ("0A", SocketState::Listen),
        ("0B", SocketState::Closing),
        ("0C", SocketState::Other),
        ("FF", SocketState::Other),
    ];
    for (code, state) in expected {
        let snapshot = linux(&synthetic(&[socket_line("0100007F:1F90", "0200000A:0050", code, 1)]));
        let all: Vec<&Socket> = snapshot.listening.iter().chain(&snapshot.connections).collect();

        assert_eq!(all.len(), 1, "{code}");
        assert_eq!(all[0].state, state, "{code}");
        assert_eq!(snapshot.listening.len(), usize::from(state == SocketState::Listen), "{code}");
    }
    let broken = linux(&synthetic(&[socket_line("0100007F:1F90", "0200000A:0050", "ZZ", 1)]));
    assert!(broken.listening.is_empty() && broken.connections.is_empty());
}

#[test]
fn owner_is_none_when_the_fd_links_are_unreadable() {
    let snapshot = linux(&render(CONTAINER));

    let web = find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:8080");
    assert_eq!(web.owner, None);
    assert_eq!(snapshot.connections[0].local, "127.0.0.11:8080");
    assert_eq!(snapshot.connections[0].remote, "127.0.0.1:54321");
    assert_eq!(snapshot.connections[0].owner, None);
    assert_eq!(snapshot.uid, 1000);
    assert!(snapshot.problems.is_empty());
}

#[test]
fn a_socket_without_a_readable_process_name_has_no_owner_and_a_forged_name_line_voids_the_owner() {
    let forged = DEBIAN.replace("/proc/812/comm:sshd\n", "/proc/812/comm:sshd\n/proc/812/comm:evil\n");
    let snapshot = linux(&render(&forged));
    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22").owner, None);

    let nameless = without_section(DEBIAN, "comm").replace("@@T:end", "@@T:comm\n@@T:end");
    let snapshot = linux(&render(&nameless));
    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22").owner, None);
}

#[test]
fn only_a_link_to_exactly_a_socket_names_an_owner() {
    let faked = DEBIAN.replace("-> socket:[12001]", "-> /tmp/x -> socket:[12001]");
    let snapshot = linux(&render(&faked));
    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22").owner, None);

    let path = DEBIAN.replace("-> socket:[12001]", "-> /tmp/socket:[12001]");
    let snapshot = linux(&render(&path));
    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22").owner, None);

    let quoted = DEBIAN.replace("-> socket:[12001]", "-> 'socket:[12001]'");
    let snapshot = linux(&render(&quoted));
    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22").owner.as_ref().unwrap().0, 812);
}

#[test]
fn interfaces_come_from_proc_net_dev_even_without_sys_or_ip() {
    let snapshot = linux(&render(ALPINE));

    let names: Vec<&str> = snapshot.interfaces.iter().map(|interface| interface.name.as_str()).collect();
    assert_eq!(names, ["eth0", "lo"]);
    let eth0 = &snapshot.interfaces[0];
    assert_eq!((eth0.rx_bytes, eth0.tx_bytes), (98765432, 12345678));
    assert_eq!(eth0.up, None);
    assert!(eth0.addresses.is_empty());
    assert_eq!((eth0.mac.as_deref(), eth0.mtu), (None, None));
    let lo = &snapshot.interfaces[1];
    assert_eq!((lo.rx_bytes, lo.tx_bytes), (1234567, 1234567));
    assert!(snapshot.problems.is_empty());
}

#[test]
fn interfaces_join_sys_and_ip_data_by_name() {
    let snapshot = linux(&render(DEBIAN));

    let eth0 = &snapshot.interfaces[0];
    assert_eq!(eth0.name, "eth0");
    assert_eq!(eth0.up, Some(true));
    assert_eq!(eth0.mac.as_deref(), Some("52:54:00:12:34:56"));
    assert_eq!(eth0.mtu, Some(1500));
    assert_eq!(eth0.addresses, ["10.0.0.2/24", "fe80::5054:ff:fe12:3456/64"]);
    let lo = &snapshot.interfaces[1];
    assert_eq!((lo.name.as_str(), lo.up, lo.mtu), ("lo", None, Some(65536)));
    assert_eq!(lo.addresses, ["127.0.0.1/8", "::1/128"]);

    let container = linux(&render(CONTAINER));
    assert_eq!(container.interfaces.len(), 1);
    assert_eq!(container.interfaces[0].up, Some(true));
    assert!(container.interfaces[0].addresses.is_empty());
}

#[test]
fn a_missing_sockets_section_blanks_only_that_section_and_records_the_problem() {
    let snapshot = linux(&render(&without_section(DEBIAN, "tcp")));
    assert!(snapshot.listening.is_empty() && snapshot.connections.is_empty());
    assert_eq!(snapshot.problems, [PortsSection::Sockets]);
    assert_eq!(snapshot.interfaces.len(), 2);

    let snapshot = linux(&render(&without_section(DEBIAN, "netdev")));
    assert!(snapshot.interfaces.is_empty());
    assert_eq!(snapshot.problems, [PortsSection::Interfaces]);
    assert!(!snapshot.listening.is_empty());

    let hollow = without_section(DEBIAN, "tcp").replace("@@T:tcp6", "@@T:tcp\n@@T:tcp6");
    assert_eq!(linux(&render(&hollow)).problems, [PortsSection::Sockets]);
}

#[test]
fn non_linux_is_unsupported() {
    match parse_ports(&render(MACOS), &token()) {
        ParsedPorts::Unsupported { os } => assert_eq!(os, "Darwin"),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn empty_truncated_and_non_utf8_output_is_garbled_or_partial_never_a_panic() {
    assert!(matches!(parse_ports(b"", &token()), ParsedPorts::Garbled));
    assert!(matches!(parse_ports(b"no markers here\n", &token()), ParsedPorts::Garbled));
    assert!(matches!(parse_ports(&[0xff, 0xfe, 0x00, 0x80], &token()), ParsedPorts::Garbled));

    let full = render(DEBIAN);
    for length in 0..full.len() {
        match parse_ports(&full[..length], &token()) {
            ParsedPorts::Garbled | ParsedPorts::Linux(_) => {}
            ParsedPorts::Unsupported { os } => panic!("truncated at {length} became {os}"),
        }
    }

    let mut hostile = full.clone();
    for byte in hostile.iter_mut().filter(|byte| **byte == b'd') {
        *byte = 0xff;
    }
    assert!(matches!(parse_ports(&hostile, &token()), ParsedPorts::Linux(_) | ParsedPorts::Garbled));
    let broken_names = DEBIAN.replace("sshd", "ss\u{1}\u{202e}h");
    let snapshot = linux(&render(&broken_names));
    let owner = find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22").owner.clone().unwrap();
    assert!(owner.1.chars().all(|character| !character.is_control() && character != '\u{202e}'), "{owner:?}");
}

#[test]
fn hostile_input_is_capped() {
    let many: Vec<String> =
        (0..5000).map(|index| socket_line("0100007F:1F90", "0200000A:0050", "01", index + 1)).collect();
    let snapshot = linux(&synthetic(&many));
    assert_eq!(snapshot.connections.len(), MAX_SOCKETS);
    assert!(snapshot.listening.is_empty());

    let listeners: Vec<String> = (0..3000).map(|index| socket_line("0100007F:1F90", NOBODY, "0A", index + 1)).collect();
    let mixed: Vec<String> = listeners.iter().cloned().chain(many.iter().take(3000).cloned()).collect();
    let snapshot = linux(&synthetic(&mixed));
    assert_eq!(snapshot.listening.len(), MAX_SOCKETS);
    assert!(snapshot.connections.is_empty());

    let valid = socket_line(ANY_V4_22, NOBODY, "0A", 1);
    let oversized = format!("{valid} {}", "a".repeat(900_000));
    let snapshot = linux(&synthetic(&[valid.clone(), oversized, socket_line("00000000:0050", NOBODY, "0A", 2)]));
    let ports: Vec<&str> = snapshot.listening.iter().map(|socket| socket.local.as_str()).collect();
    assert_eq!(ports, ["0.0.0.0:22", "0.0.0.0:80"]);

    let wall_of_text = format!("{}\n", "a".repeat(1_000_000));
    let mut flooded = String::from("@@T:os\nLinux\n@@T:tcp\n");
    flooded.push_str(&wall_of_text);
    flooded.push_str("@@T:end\n");
    assert!(linux(&render(&flooded)).listening.is_empty());
}

#[test]
fn arranged_sockets_filters_case_insensitively_and_sorts() {
    let all = [
        socket(SocketProtocol::Tcp, "0.0.0.0:443", NOBODY, SocketState::Listen, Some("Nginx")),
        socket(SocketProtocol::Tcp, "[::]:22", NOBODY, SocketState::Listen, Some("sshd")),
        socket(SocketProtocol::Udp, "0.0.0.0:80", NOBODY, SocketState::Listen, None),
        socket(SocketProtocol::Tcp, "10.0.0.2:22", "10.0.0.9:2", SocketState::Established, Some("sshd")),
        socket(SocketProtocol::Tcp, "10.0.0.2:22", "1.1.1.1:1", SocketState::SynSent, None),
        socket(SocketProtocol::Tcp, "10.0.0.2:22", "10.0.0.1:5", SocketState::Established, None),
    ];

    assert_eq!(
        locals(&arranged_sockets(&all, "")),
        ["[::]:22", "0.0.0.0:80", "0.0.0.0:443", "10.0.0.2:22", "10.0.0.2:22", "10.0.0.2:22"]
    );
    let remotes: Vec<&str> = arranged_sockets(&all, "").iter().skip(3).map(|socket| socket.remote.as_str()).collect();
    assert_eq!(remotes, ["10.0.0.1:5", "10.0.0.9:2", "1.1.1.1:1"]);
    assert_eq!(locals(&arranged_sockets(&all, "NGINX")), ["0.0.0.0:443"]);
    assert_eq!(locals(&arranged_sockets(&all, "  SSHD ")).len(), 2);
    assert_eq!(arranged_sockets(&all, "1.1.1.1").len(), 1);
    assert_eq!(locals(&arranged_sockets(&all, "[::]")), ["[::]:22"]);
    assert!(arranged_sockets(&all, "nothing matches").is_empty());
}

#[test]
fn ports_command_is_posix_sh_with_every_marker_once_in_order() {
    let script = ports_command(&token());

    for forbidden in ["[[", "<(", ">(", "local ", "$'", "function ", "echo -e", "source ", "=(", "&>", "${!"] {
        assert!(!script.contains(forbidden), "the script uses {forbidden:?}");
    }
    assert!(script.ends_with('\n'));
    assert!(script.len() < 8 * 1024, "{} bytes", script.len());
    assert!(!script.contains("@@T:"));
    assert_eq!(script.matches("00112233445566778899aabbccddeeff").count(), 1);
    let names: Vec<&str> = script
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("m "))
        .map(|rest| rest.split(';').next().unwrap().trim())
        .collect();
    assert_eq!(
        names,
        ["os", "end", "uid", "tcp", "tcp6", "udp", "udp6", "netdev", "sysnet", "ipaddr", "comm", "fdlinks", "end"]
    );
    assert!(!script.contains("head -n 2001"));
    assert!(script.contains("head -c 300000"));
    assert!(script.contains("head -c 100000"));
    assert!(script.contains("uname -s"));
}

#[test]
fn tcp_listeners_are_selected_apart_from_the_other_rows_so_a_busy_server_cannot_crowd_them_out() {
    let script = ports_command(&token());

    for file in ["tcp", "tcp6"] {
        let header = script.find(&format!("head -n 1 /proc/net/{file} ")).unwrap_or_else(|| panic!("{file} header"));
        let listeners =
            script.find(&format!("grep ' 0A ' /proc/net/{file} ")).unwrap_or_else(|| panic!("{file} listeners"));
        let others =
            script.find(&format!("grep -v ' 0A ' /proc/net/{file} ")).unwrap_or_else(|| panic!("{file} others"));
        assert!(header < listeners && listeners < others, "{file}");
    }
}

#[test]
fn udp_unconnected_sockets_are_selected_apart_from_the_connected_ones() {
    let script = ports_command(&token());

    for file in ["udp", "udp6"] {
        let header = script.find(&format!("head -n 1 /proc/net/{file} ")).unwrap_or_else(|| panic!("{file} header"));
        let unconnected =
            script.find(&format!("grep ' 07 ' /proc/net/{file} ")).unwrap_or_else(|| panic!("{file} unconnected"));
        let connected =
            script.find(&format!("grep -v ' 07 ' /proc/net/{file} ")).unwrap_or_else(|| panic!("{file} connected"));
        assert!(header < unconnected && unconnected < connected, "{file}");
    }
}

#[test]
fn listeners_survive_in_the_emitted_order_behind_thousands_of_other_rows() {
    let listeners: Vec<String> = (0..3).map(|index| socket_line("0100007F:1F90", NOBODY, "0A", 900 + index)).collect();
    let others: Vec<String> =
        (0..3000).map(|index| socket_line("0100007F:1F90", "0200000A:0050", "01", 5000 + index)).collect();
    let emitted: Vec<String> = listeners.into_iter().chain(others).collect();

    let snapshot = linux(&synthetic(&emitted));

    assert_eq!(snapshot.listening.len(), 3);
    assert_eq!(snapshot.connections.len(), MAX_SOCKETS - 3);
}

#[cfg(target_os = "linux")]
#[test]
fn the_script_keeps_listeners_when_the_kernel_lists_thousands_of_other_sockets_first() {
    let directory = std::env::temp_dir().join(format!("porthmos-ports-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut table = format!("{SOCKET_HEADER}\n");
    for index in 0..3000 {
        table.push_str(&socket_line("0100007F:1F90", "0200000A:0050", "01", 800_000_000 + index));
        table.push('\n');
    }
    for port in ["0016", "0050", "01BB"] {
        table.push_str(&socket_line(&format!("00000000:{port}"), NOBODY, "0A", 900_000_000));
        table.push('\n');
    }
    std::fs::write(directory.join("tcp"), table).unwrap();
    let token = Token::new();
    let script = ports_command(&token).replace("/proc/net/", &format!("{}/", directory.display()));

    let output = std::process::Command::new("sh").arg("-c").arg(script).output();
    let _ = std::fs::remove_dir_all(&directory);
    let Ok(output) = output else { return };

    let ParsedPorts::Linux(snapshot) = parse_ports(&output.stdout, &token) else {
        panic!("a Linux host should parse as Linux");
    };
    let listening: Vec<&str> = snapshot.listening.iter().map(|socket| socket.local.as_str()).collect();
    assert_eq!(listening, ["0.0.0.0:22", "0.0.0.0:80", "0.0.0.0:443"]);
    assert_eq!(snapshot.connections.len(), MAX_SOCKETS - 3);
}

#[cfg(target_os = "linux")]
#[test]
fn the_script_keeps_unconnected_udp_sockets_when_thousands_of_connected_ones_come_first() {
    let directory = std::env::temp_dir().join(format!("porthmos-ports-udp-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for (file, any) in [("tcp", ""), ("tcp6", ""), ("udp", "00000000"), ("udp6", "00000000000000000000000000000000")] {
        let mut table = format!("{SOCKET_HEADER}\n");
        if file.starts_with("udp") {
            for index in 0..3000 {
                table.push_str(&socket_line(&format!("{any}:1F90"), &format!("{any}:0035"), "01", 800_000_000 + index));
                table.push('\n');
            }
            for port in ["0044", "14E9"] {
                table.push_str(&socket_line(&format!("{any}:{port}"), &format!("{any}:0000"), "07", 900_000_000));
                table.push('\n');
            }
        }
        std::fs::write(directory.join(file), table).unwrap();
    }
    let token = Token::new();
    let script = ports_command(&token).replace("/proc/net/", &format!("{}/", directory.display()));

    let output = std::process::Command::new("sh").arg("-c").arg(script).output();
    let _ = std::fs::remove_dir_all(&directory);
    let Ok(output) = output else { return };

    let ParsedPorts::Linux(snapshot) = parse_ports(&output.stdout, &token) else {
        panic!("a Linux host should parse as Linux");
    };
    let listening: Vec<&str> = snapshot.listening.iter().map(|socket| socket.local.as_str()).collect();
    assert_eq!(listening.iter().filter(|local| local.ends_with(":68")).count(), 2);
    assert_eq!(listening.iter().filter(|local| local.ends_with(":5353")).count(), 2);
    assert!(snapshot.listening.iter().all(|socket| socket.protocol == SocketProtocol::Udp));
}

#[test]
fn when_several_processes_share_a_socket_the_lowest_pid_owns_it_whatever_the_listing_order() {
    let shared = DEBIAN.replace(
        "/proc/812/fd:\n",
        "/proc/9999/fd:\ntotal 0\nlrwx------ 1 root root 64 Nov 27 10:00 3 -> socket:[12001]\n/proc/812/fd:\n",
    );
    let with_names = shared.replace("/proc/812/comm:sshd\n", "/proc/812/comm:sshd\n/proc/9999/comm:child\n");

    let snapshot = linux(&render(&with_names));

    assert_eq!(find(&snapshot.listening, SocketProtocol::Tcp, "0.0.0.0:22").owner, Some((812, "sshd".to_string())));
}

#[cfg(target_os = "linux")]
fn assert_a_real_run_parses(shell: &[&str]) {
    let Some((program, arguments)) = shell.split_first() else { return };
    let token = Token::new();
    let Ok(output) = std::process::Command::new(program).args(arguments).arg("-c").arg(ports_command(&token)).output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let ParsedPorts::Linux(snapshot) = parse_ports(&output.stdout, &token) else {
        panic!("a Linux host should parse as Linux under {shell:?}");
    };

    assert!(snapshot.interfaces.iter().any(|interface| interface.name == "lo"), "{shell:?}");
    assert!(!snapshot.problems.contains(&PortsSection::Sockets), "{shell:?}");
    assert!(!snapshot.problems.contains(&PortsSection::Interfaces), "{shell:?}");
}

#[cfg(target_os = "linux")]
#[test]
fn the_script_runs_under_the_local_sh_dash_and_busybox_and_its_output_parses() {
    assert_a_real_run_parses(&["sh"]);
    assert_a_real_run_parses(&["dash"]);
    assert_a_real_run_parses(&["busybox", "sh"]);
}
