use porthmos_vfs::IncomingConnection;
use tokio::sync::mpsc;

use super::*;
use crate::client::ServerId;

fn sender() -> mpsc::Sender<IncomingConnection> {
    mpsc::channel(1).0
}

#[test]
fn the_registry_routes_by_address_and_port_and_forgets_on_unregister() {
    let registry = ForwardRegistry::default();
    registry.register("127.0.0.1", 8080, sender());

    assert!(registry.route("127.0.0.1", 8080).is_some());
    assert!(registry.route("127.0.0.1", 8081).is_none());
    assert!(registry.route("0.0.0.0", 8080).is_none());

    registry.unregister("127.0.0.1", 8080);
    assert!(registry.route("127.0.0.1", 8080).is_none());
}

#[test]
fn clearing_forgets_every_entry() {
    let registry = ForwardRegistry::default();
    registry.register("127.0.0.1", 1, sender());
    registry.register("::1", 2, sender());

    registry.clear();

    assert!(registry.route("127.0.0.1", 1).is_none() && registry.route("::1", 2).is_none());
}

#[test]
fn a_clone_shares_its_entries() {
    let registry = ForwardRegistry::default();
    let other = registry.clone();

    registry.register("127.0.0.1", 9, sender());

    assert!(other.route("127.0.0.1", 9).is_some());
}

#[test]
fn every_connections_server_id_has_its_own_empty_registry() {
    let first = ServerId::default();
    let second = ServerId::default();
    first.forwards().register("127.0.0.1", 5, sender());

    assert!(first.forwards().route("127.0.0.1", 5).is_some());
    assert!(second.forwards().route("127.0.0.1", 5).is_none());
}
