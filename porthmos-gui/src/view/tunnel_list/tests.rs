use porthmos_core::theme::ColorScheme;
use porthmos_core::{Message, TunnelInfo, TunnelKind, TunnelSpec, TunnelStats, TunnelStatus};

use super::*;
use crate::theme::ThemeSettings;

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default().with_scheme(ColorScheme::dark()))
}

fn info(stats: TunnelStats) -> TunnelInfo {
    TunnelInfo {
        spec: TunnelSpec {
            connection: "a".into(),
            name: "x".into(),
            kind: TunnelKind::Dynamic { listen: "127.0.0.1:1080".parse().unwrap() },
            allow_external: false,
            auto_start: false,
        },
        status: TunnelStatus::Stopped,
        stats,
    }
}

fn running() -> TunnelStatus {
    TunnelStatus::Running { local: "127.0.0.1:1080".parse().unwrap() }
}

#[test]
fn only_a_stopped_lost_or_failed_tunnel_can_be_started() {
    assert!(start_enabled(&TunnelStatus::Stopped));
    assert!(start_enabled(&TunnelStatus::Lost));
    assert!(start_enabled(&TunnelStatus::Failed(Message::new("x"))));
    assert!(!start_enabled(&TunnelStatus::Offline));
    assert!(!start_enabled(&TunnelStatus::Starting));
    assert!(!start_enabled(&running()));
}

#[test]
fn a_running_or_starting_tunnel_can_be_stopped() {
    assert!(stoppable(&running()));
    assert!(stoppable(&TunnelStatus::Starting));
    assert!(!stoppable(&TunnelStatus::Stopped));
    assert!(!stoppable(&TunnelStatus::Offline));
}

#[test]
fn every_status_has_a_glyph_so_colour_never_carries_the_meaning_alone() {
    let all = [
        TunnelStatus::Stopped,
        TunnelStatus::Offline,
        TunnelStatus::Starting,
        running(),
        TunnelStatus::Failed(Message::new("x")),
        TunnelStatus::Lost,
    ];
    for status in &all {
        assert!(!status_glyph(status).is_empty());
    }
    assert_ne!(status_glyph(&running()), status_glyph(&TunnelStatus::Stopped));
    assert_ne!(status_glyph(&TunnelStatus::Failed(Message::new("x"))), status_glyph(&TunnelStatus::Stopped));
}

#[test]
fn status_colours_use_palette_roles() {
    let palette = palette();

    assert_eq!(status_color(&running(), &palette), palette.ok);
    assert_eq!(status_color(&TunnelStatus::Failed(Message::new("x")), &palette), palette.err);
    assert_eq!(status_color(&TunnelStatus::Stopped, &palette), palette.fg_dim);
}

#[test]
fn status_text_names_the_state_in_words() {
    assert_eq!(status_text(&TunnelStatus::Stopped), "");
    assert_eq!(status_text(&TunnelStatus::Offline), "offline");
    assert_eq!(status_text(&TunnelStatus::Lost), "connection lost");
    assert_eq!(status_text(&TunnelStatus::Failed(Message::new("port in use"))), "port in use");
}

#[test]
fn the_tooltip_counts_connections_errors_and_refusals() {
    let text = counts_text(&info(TunnelStats { connections: 7, errors: 2, refused: 3, ..TunnelStats::default() }));

    assert!(text.contains('7') && text.contains('2') && text.contains('3'), "{text}");
}

#[test]
fn traffic_text_shows_rates_and_totals() {
    let text = traffic_text(1536, 0, &info(TunnelStats { bytes_up: 2048, bytes_down: 10, ..TunnelStats::default() }));

    assert!(
        text.contains("1.5 KB/s") && text.contains("0 B/s") && text.contains("2.0 KB") && text.contains("10 B"),
        "{text}"
    );
}
