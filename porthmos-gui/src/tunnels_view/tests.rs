use std::time::Duration;

use porthmos_core::{Endpoint, Listen, TunnelInfo, TunnelKey, TunnelKind, TunnelSpec, TunnelStats, TunnelStatus};

use super::*;

fn spec(connection: &str, name: &str, kind: TunnelKind) -> TunnelSpec {
    TunnelSpec {
        connection: connection.to_string(),
        name: name.to_string(),
        kind,
        allow_external: false,
        auto_start: false,
    }
}

fn local(listen: &str, target: &str) -> TunnelKind {
    TunnelKind::Local { listen: listen.parse::<Listen>().unwrap(), target: target.parse::<Endpoint>().unwrap() }
}

fn dynamic(listen: &str) -> TunnelKind {
    TunnelKind::Dynamic { listen: listen.parse::<Listen>().unwrap() }
}

fn info(connection: &str, name: &str, status: TunnelStatus) -> TunnelInfo {
    TunnelInfo {
        spec: spec(connection, name, local("127.0.0.1:5432", "db.internal:5432")),
        status,
        stats: TunnelStats::default(),
    }
}

fn key(connection: &str, name: &str) -> TunnelKey {
    TunnelKey { connection: connection.to_string(), name: name.to_string() }
}

fn running() -> TunnelStatus {
    TunnelStatus::Running { local: "127.0.0.1:5432".parse().unwrap() }
}

fn stats(up: u64, down: u64) -> TunnelStats {
    TunnelStats { bytes_up: up, bytes_down: down, ..TunnelStats::default() }
}

#[test]
fn the_list_replaces_the_rows_and_status_and_stats_update_one_row_each() {
    let mut state = TunnelsState::default();
    state.apply_list(vec![info("a", "one", TunnelStatus::Stopped), info("a", "two", TunnelStatus::Stopped)]);
    state.apply_status(&key("a", "two"), running());
    state.apply_stats(&[(key("a", "one"), stats(10, 20))], Duration::from_secs(1));

    assert_eq!(state.rows[0].status, TunnelStatus::Stopped);
    assert_eq!(state.rows[0].stats.bytes_up, 10);
    assert_eq!(state.rows[1].status, running());
    assert_eq!(state.rows[1].stats.bytes_up, 0);

    state.apply_list(vec![info("b", "only", TunnelStatus::Offline)]);

    assert_eq!(state.rows.len(), 1);
    assert_eq!(state.rows[0].spec.connection, "b");
}

#[test]
fn an_update_for_an_unknown_tunnel_changes_nothing() {
    let mut state = TunnelsState::default();
    state.apply_list(vec![info("a", "one", TunnelStatus::Stopped)]);

    state.apply_status(&key("a", "ghost"), running());
    state.apply_stats(&[(key("a", "ghost"), stats(5, 5))], Duration::from_secs(1));

    assert_eq!(state.rows[0].status, TunnelStatus::Stopped);
    assert_eq!(state.rows[0].stats, TunnelStats::default());
}

#[test]
fn running_count_counts_only_running_rows() {
    let mut state = TunnelsState::default();
    state.apply_list(vec![
        info("a", "one", running()),
        info("a", "two", TunnelStatus::Starting),
        info("a", "three", TunnelStatus::Stopped),
        info("a", "four", TunnelStatus::Failed(porthmos_core::Message::new("x"))),
        info("a", "five", running()),
    ]);

    assert_eq!(state.running_count(), 2);
}

#[test]
fn path_text_for_local_and_dynamic_and_ipv6() {
    assert_eq!(
        path_text(&spec("a", "x", local("127.0.0.1:5432", "db.internal:5432"))),
        "127.0.0.1:5432 → db.internal:5432"
    );
    assert_eq!(path_text(&spec("a", "x", dynamic("127.0.0.1:1080"))), "127.0.0.1:1080 (SOCKS)");
    assert_eq!(path_text(&spec("a", "x", local("[::1]:8080", "[fe80::1]:80"))), "[::1]:8080 → [fe80::1]:80");
}

#[test]
fn rate_and_total_text_use_binary_free_decimal_units_and_never_panic_on_huge_values() {
    assert_eq!(rate_text(0), "0 B/s");
    assert_eq!(rate_text(1536), "1.5 KB/s");
    assert_eq!(rate_text(2 * 1024 * 1024), "2.0 MB/s");
    assert_eq!(total_text(0), "0 B");
    assert_eq!(total_text(1536), "1.5 KB");
    assert!(!rate_text(u64::MAX).is_empty());
    assert!(!total_text(u64::MAX).is_empty());
}

#[test]
fn rates_come_from_two_successive_stats_batches() {
    let mut state = TunnelsState::default();
    state.apply_list(vec![info("a", "one", running())]);
    let one = key("a", "one");

    state.apply_stats(&[(one.clone(), stats(1000, 4000))], Duration::from_secs(1));
    assert_eq!(state.rate(&one), (1000, 4000));

    state.apply_stats(&[(one.clone(), stats(3000, 4000))], Duration::from_secs(2));
    assert_eq!(state.rate(&one), (1000, 0));

    state.apply_stats(&[(one.clone(), stats(10, 0))], Duration::from_secs(1));
    assert_eq!(state.rate(&one), (0, 0));
}

#[test]
fn a_zero_elapsed_time_gives_a_zero_rate_instead_of_dividing() {
    let mut state = TunnelsState::default();
    state.apply_list(vec![info("a", "one", running())]);

    state.apply_stats(&[(key("a", "one"), stats(1000, 1000))], Duration::ZERO);

    assert_eq!(state.rate(&key("a", "one")), (0, 0));
}

#[test]
fn a_tunnel_that_stops_has_no_rate() {
    let mut state = TunnelsState::default();
    state.apply_list(vec![info("a", "one", running())]);
    state.apply_stats(&[(key("a", "one"), stats(1000, 1000))], Duration::from_secs(1));

    state.apply_status(&key("a", "one"), TunnelStatus::Stopped);

    assert_eq!(state.rate(&key("a", "one")), (0, 0));
}

#[test]
fn the_tab_label_shows_the_running_count_only_above_zero() {
    let mut state = TunnelsState::default();
    assert_eq!(state.tab_label(), "Tunnels");

    state.apply_list(vec![info("a", "one", running()), info("a", "two", running())]);

    assert_eq!(state.tab_label(), "Tunnels · 2");
}
