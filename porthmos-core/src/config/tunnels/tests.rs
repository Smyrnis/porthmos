use std::fs;

use porthmos_tunnel::{Endpoint, Kind, Listen};

use super::*;

fn listen(text: &str) -> Listen {
    text.parse().unwrap()
}

fn local(connection: &str, name: &str, listen_text: &str, target: &str) -> TunnelSpec {
    TunnelSpec {
        connection: connection.to_string(),
        name: name.to_string(),
        kind: Kind::Local { listen: listen(listen_text), target: target.parse::<Endpoint>().unwrap() },
        allow_external: false,
        auto_start: false,
    }
}

fn dynamic(connection: &str, name: &str, listen_text: &str) -> TunnelSpec {
    TunnelSpec {
        connection: connection.to_string(),
        name: name.to_string(),
        kind: Kind::Dynamic { listen: listen(listen_text) },
        allow_external: false,
        auto_start: false,
    }
}

fn key(connection: &str, name: &str) -> TunnelKey {
    TunnelKey { connection: connection.to_string(), name: name.to_string() }
}

fn texts(warnings: &[crate::config::StartupWarning]) -> Vec<&str> {
    warnings.iter().map(|warning| warning.0.as_str()).collect()
}

fn names(tunnels: &Tunnels) -> Vec<String> {
    tunnels.iter().map(|spec| format!("{}/{}", spec.connection, spec.name)).collect()
}

#[test]
fn a_missing_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();

    let (tunnels, warnings) = load_from(&dir.path().join("tunnels.toml")).unwrap();

    assert_eq!(tunnels.iter().count(), 0);
    assert!(warnings.is_empty());
    assert!(!tunnels.is_protected());
}

#[test]
fn a_saved_file_round_trips_both_kinds_and_flags() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    let mut tunnels = Tunnels::default();
    let mut postgres = local("prod-db", "postgres", "127.0.0.1:5432", "db.internal:5432");
    postgres.auto_start = true;
    let mut lan = dynamic("prod-db", "socks", "0.0.0.0:1080");
    lan.allow_external = true;
    tunnels.upsert(None, postgres.clone()).unwrap();
    tunnels.upsert(None, lan.clone()).unwrap();
    tunnels.upsert(None, local("other", "web", "[::1]:8080", "[::1]:80")).unwrap();

    save_to(&path, &tunnels).unwrap();
    let (loaded, warnings) = load_from(&path).unwrap();

    assert!(warnings.is_empty(), "{:?}", texts(&warnings));
    assert_eq!(loaded.iter().cloned().collect::<Vec<_>>(), tunnels.iter().cloned().collect::<Vec<_>>());
    assert_eq!(loaded.get(&key("prod-db", "postgres")), Some(&postgres));
    assert_eq!(loaded.get(&key("prod-db", "socks")), Some(&lan));
}

#[test]
fn the_written_file_matches_the_documented_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    let mut tunnels = Tunnels::default();
    let mut postgres = local("prod-db", "postgres", "127.0.0.1:5432", "db.internal:5432");
    postgres.auto_start = true;
    tunnels.upsert(None, postgres).unwrap();
    tunnels.upsert(None, dynamic("prod-db", "socks", "127.0.0.1:1080")).unwrap();

    save_to(&path, &tunnels).unwrap();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[[tunnel]]\nconnection = \"prod-db\"\nname = \"postgres\"\nkind = \"local\"\nlisten = \"127.0.0.1:5432\"\ntarget = \"db.internal:5432\"\nauto_start = true\n\n[[tunnel]]\nconnection = \"prod-db\"\nname = \"socks\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:1080\"\n"
    );
}

#[test]
fn an_unparseable_file_is_set_aside_with_a_warning_and_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(&path, "not [ valid").unwrap();

    let (tunnels, warnings) = load_from(&path).unwrap();

    assert_eq!(tunnels.iter().count(), 0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].0.contains("tunnels.toml.broken"), "{}", warnings[0].0);
    assert_eq!(fs::read_to_string(dir.path().join("tunnels.toml.broken")).unwrap(), "not [ valid");
}

#[cfg(unix)]
#[test]
fn a_protected_file_refuses_every_change_and_is_never_overwritten() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let paths = crate::Paths::in_dir(dir.path());
    fs::create_dir_all(&paths.config_dir).unwrap();
    fs::write(paths.tunnels_file(), "garbage [").unwrap();
    fs::set_permissions(&paths.config_dir, fs::Permissions::from_mode(0o500)).unwrap();
    if fs::File::create(paths.config_dir.join("probe")).is_ok() {
        fs::set_permissions(&paths.config_dir, fs::Permissions::from_mode(0o700)).unwrap();
        return;
    }

    let (mut tunnels, _) = load(&paths).unwrap();
    fs::set_permissions(&paths.config_dir, fs::Permissions::from_mode(0o700)).unwrap();

    assert!(tunnels.is_protected());
    assert_eq!(tunnels.upsert(None, dynamic("a", "b", "127.0.0.1:1")), Err(TunnelsError::Protected));
    assert!(save_to(&paths.tunnels_file(), &tunnels).is_err());
    assert_eq!(fs::read_to_string(paths.tunnels_file()).unwrap(), "garbage [");
}

#[test]
fn a_non_loopback_listener_without_the_opt_in_is_dropped_with_a_warning_and_the_rest_survive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(
        &path,
        "[[tunnel]]\nconnection = \"a\"\nname = \"open\"\nkind = \"dynamic\"\nlisten = \"0.0.0.0:1080\"\n\n[[tunnel]]\nconnection = \"a\"\nname = \"fine\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:1081\"\n\n[[tunnel]]\nconnection = \"a\"\nname = \"allowed\"\nkind = \"dynamic\"\nlisten = \"0.0.0.0:1082\"\nallow_external = true\n",
    )
    .unwrap();

    let (tunnels, warnings) = load_from(&path).unwrap();

    assert_eq!(names(&tunnels), ["a/fine", "a/allowed"]);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].0.contains("open") && warnings[0].0.contains("tunnels.toml"), "{}", warnings[0].0);
}

#[test]
fn broken_entries_are_dropped_one_by_one_with_a_warning_each() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(
        &path,
        concat!(
            "[[tunnel]]\nconnection = \"a\"\nname = \"weird\"\nkind = \"reverse\"\nlisten = \"127.0.0.1:1\"\n\n",
            "[[tunnel]]\nconnection = \"a\"\nname = \"no-target\"\nkind = \"local\"\nlisten = \"127.0.0.1:2\"\n\n",
            "[[tunnel]]\nconnection = \"a\"\nname = \"bad-listen\"\nkind = \"dynamic\"\nlisten = \"localhost:3\"\n\n",
            "[[tunnel]]\nconnection = \"a\"\nname = \"bad-target\"\nkind = \"local\"\nlisten = \"127.0.0.1:4\"\ntarget = \"nowhere\"\n\n",
            "[[tunnel]]\nconnection = \"\"\nname = \"no-connection\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:5\"\n\n",
            "[[tunnel]]\nconnection = \"a\"\nname = \"\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:6\"\n\n",
            "[[tunnel]]\nconnection = \"a\"\nname = \"missing-fields\"\n\n",
            "[[tunnel]]\nconnection = \"a\"\nname = \"good\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:7\"\n",
        ),
    )
    .unwrap();

    let (tunnels, warnings) = load_from(&path).unwrap();

    assert_eq!(names(&tunnels), ["a/good"]);
    assert_eq!(warnings.len(), 7, "{:?}", texts(&warnings));
}

#[test]
fn a_duplicate_name_on_one_connection_keeps_the_first_and_warns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(
        &path,
        "[[tunnel]]\nconnection = \"a\"\nname = \"x\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:1\"\n\n[[tunnel]]\nconnection = \"a\"\nname = \"x\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:2\"\n\n[[tunnel]]\nconnection = \"b\"\nname = \"x\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:3\"\n",
    )
    .unwrap();

    let (tunnels, warnings) = load_from(&path).unwrap();

    assert_eq!(names(&tunnels), ["a/x", "b/x"]);
    assert_eq!(tunnels.get(&key("a", "x")).unwrap().kind.listen().port, 1);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn upsert_refuses_duplicates_and_invalid_specs() {
    let mut tunnels = Tunnels::default();
    tunnels.upsert(None, dynamic("a", "x", "127.0.0.1:1")).unwrap();
    tunnels.upsert(None, dynamic("a", "y", "127.0.0.1:2")).unwrap();

    assert_eq!(tunnels.upsert(None, dynamic("a", "x", "127.0.0.1:3")), Err(TunnelsError::Duplicate));
    assert_eq!(tunnels.upsert(Some(&key("a", "y")), dynamic("a", "x", "127.0.0.1:3")), Err(TunnelsError::Duplicate));
    assert_eq!(
        tunnels.upsert(None, dynamic("a", "", "127.0.0.1:3")),
        Err(TunnelsError::Invalid(porthmos_tunnel::SpecError::EmptyName))
    );
    assert_eq!(
        tunnels.upsert(None, dynamic("a", "z", "0.0.0.0:3")),
        Err(TunnelsError::Invalid(porthmos_tunnel::SpecError::ExternalNotAllowed))
    );
    assert_eq!(names(&tunnels), ["a/x", "a/y"]);
}

#[test]
fn upsert_can_edit_and_rename_in_place_keeping_the_position() {
    let mut tunnels = Tunnels::default();
    for (name, port) in [("one", 1), ("two", 2), ("three", 3)] {
        tunnels.upsert(None, dynamic("a", name, &format!("127.0.0.1:{port}"))).unwrap();
    }

    tunnels.upsert(Some(&key("a", "two")), dynamic("a", "two", "127.0.0.1:20")).unwrap();
    tunnels.upsert(Some(&key("a", "two")), dynamic("a", "second", "127.0.0.1:20")).unwrap();

    assert_eq!(names(&tunnels), ["a/one", "a/second", "a/three"]);
    assert_eq!(tunnels.get(&key("a", "second")).unwrap().kind.listen().port, 20);
    assert!(tunnels.get(&key("a", "two")).is_none());
}

#[test]
fn the_same_name_on_two_connections_is_fine_and_lists_by_connection() {
    let mut tunnels = Tunnels::default();
    tunnels.upsert(None, dynamic("a", "x", "127.0.0.1:1")).unwrap();
    tunnels.upsert(None, dynamic("b", "x", "127.0.0.1:2")).unwrap();
    tunnels.upsert(None, dynamic("a", "y", "127.0.0.1:3")).unwrap();

    let for_a: Vec<_> = tunnels.for_connection("a").iter().map(|spec| spec.name.clone()).collect();

    assert_eq!(for_a, ["x", "y"]);
    assert!(tunnels.for_connection("missing").is_empty());
}

#[test]
fn remove_returns_the_tunnel_and_unknown_keys_do_nothing() {
    let mut tunnels = Tunnels::default();
    tunnels.upsert(None, dynamic("a", "x", "127.0.0.1:1")).unwrap();

    assert!(tunnels.remove(&key("a", "nope")).is_none());
    assert_eq!(tunnels.remove(&key("a", "x")).unwrap().name, "x");
    assert_eq!(tunnels.iter().count(), 0);
}

#[test]
fn renaming_a_connection_moves_its_tunnels_and_deleting_removes_them() {
    let mut tunnels = Tunnels::default();
    tunnels.upsert(None, dynamic("old", "x", "127.0.0.1:1")).unwrap();
    tunnels.upsert(None, dynamic("keep", "y", "127.0.0.1:2")).unwrap();
    tunnels.upsert(None, dynamic("old", "z", "127.0.0.1:3")).unwrap();

    tunnels.rename_connection("old", "new");
    assert_eq!(names(&tunnels), ["new/x", "keep/y", "new/z"]);

    let removed = tunnels.remove_connection("new");
    assert_eq!(removed.iter().map(|spec| spec.name.as_str()).collect::<Vec<_>>(), ["x", "z"]);
    assert_eq!(names(&tunnels), ["keep/y"]);
}

#[test]
fn renaming_onto_a_connection_with_the_same_tunnel_name_keeps_the_existing_one() {
    let mut tunnels = Tunnels::default();
    tunnels.upsert(None, dynamic("new", "x", "127.0.0.1:1")).unwrap();
    tunnels.upsert(None, dynamic("old", "x", "127.0.0.1:2")).unwrap();

    tunnels.rename_connection("old", "new");

    assert_eq!(names(&tunnels), ["new/x"]);
    assert_eq!(tunnels.get(&key("new", "x")).unwrap().kind.listen().port, 1);
}

#[cfg(unix)]
#[test]
fn the_file_is_written_with_private_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");

    save_to(&path, &Tunnels::default()).unwrap();

    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn a_skipped_entry_is_written_back_untouched_by_the_next_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(
        &path,
        "[[tunnel]]\nconnection = \"a\"\nname = \"typo\"\nkind = \"dynamic\"\nlisten = \"0.0.0.0:1080\"\n\n[[tunnel]]\nconnection = \"a\"\nname = \"fine\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:1081\"\n",
    )
    .unwrap();
    let (mut tunnels, warnings) = load_from(&path).unwrap();
    assert_eq!(warnings.len(), 1);

    tunnels.upsert(None, dynamic("a", "new", "127.0.0.1:1082")).unwrap();
    save_to(&path, &tunnels).unwrap();

    let (reloaded, warnings) = load_from(&path).unwrap();
    assert_eq!(names(&reloaded), ["a/fine", "a/new"]);
    assert_eq!(warnings.len(), 1);
    assert!(fs::read_to_string(&path).unwrap().contains("0.0.0.0:1080"));
}

#[test]
fn skipped_entries_follow_their_connection_when_it_is_renamed_or_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(
        &path,
        "[[tunnel]]\nconnection = \"a\"\nname = \"typo\"\nkind = \"dynamic\"\nlisten = \"0.0.0.0:1080\"\n",
    )
    .unwrap();
    let (mut tunnels, _) = load_from(&path).unwrap();

    tunnels.rename_connection("a", "b");
    save_to(&path, &tunnels).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("connection = \"b\""));

    tunnels.remove_connection("b");
    save_to(&path, &tunnels).unwrap();
    assert!(!fs::read_to_string(&path).unwrap().contains("typo"));
}

fn remote(connection: &str, name: &str, listen_text: &str, target: &str) -> TunnelSpec {
    TunnelSpec {
        connection: connection.to_string(),
        name: name.to_string(),
        kind: Kind::Remote {
            listen: Listen::parse_server_side(listen_text).unwrap(),
            target: target.parse::<Endpoint>().unwrap(),
        },
        allow_external: false,
        auto_start: false,
    }
}

#[test]
fn a_remote_tunnel_round_trips_with_port_zero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    let mut tunnels = Tunnels::default();
    let spec = remote("prod", "preview", "127.0.0.1:0", "127.0.0.1:3000");
    tunnels.upsert(None, spec.clone()).unwrap();

    save_to(&path, &tunnels).unwrap();
    let (loaded, warnings) = load_from(&path).unwrap();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[[tunnel]]\nconnection = \"prod\"\nname = \"preview\"\nkind = \"remote\"\nlisten = \"127.0.0.1:0\"\ntarget = \"127.0.0.1:3000\"\n"
    );
    assert!(warnings.is_empty(), "{:?}", texts(&warnings));
    assert_eq!(loaded.get(&key("prod", "preview")), Some(&spec));
}

#[test]
fn a_non_loopback_remote_bind_needs_the_opt_in_on_load_and_the_entry_stays_in_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    let entry = |extra: &str| {
        format!(
            "[[tunnel]]\nconnection = \"a\"\nname = \"open\"\nkind = \"remote\"\nlisten = \"0.0.0.0:8080\"\ntarget = \"127.0.0.1:3000\"\n{extra}"
        )
    };
    fs::write(&path, entry("")).unwrap();

    let (tunnels, warnings) = load_from(&path).unwrap();

    assert_eq!(tunnels.iter().count(), 0);
    assert_eq!(warnings.len(), 1, "{:?}", texts(&warnings));
    save_to(&path, &tunnels).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("0.0.0.0:8080"));
    fs::write(&path, entry("allow_external = true\n")).unwrap();
    let (allowed, warnings) = load_from(&path).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(names(&allowed), ["a/open"]);
}

#[test]
fn a_local_tunnel_with_port_zero_is_still_refused_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(
        &path,
        "[[tunnel]]\nconnection = \"a\"\nname = \"l\"\nkind = \"local\"\nlisten = \"127.0.0.1:0\"\ntarget = \"db:1\"\n\n[[tunnel]]\nconnection = \"a\"\nname = \"d\"\nkind = \"dynamic\"\nlisten = \"127.0.0.1:0\"\n",
    )
    .unwrap();

    let (tunnels, warnings) = load_from(&path).unwrap();

    assert_eq!(tunnels.iter().count(), 0);
    assert_eq!(warnings.len(), 2);
}

#[test]
fn a_remote_entry_without_a_target_is_skipped_with_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tunnels.toml");
    fs::write(&path, "[[tunnel]]\nconnection = \"a\"\nname = \"r\"\nkind = \"remote\"\nlisten = \"127.0.0.1:8080\"\n")
        .unwrap();

    let (tunnels, warnings) = load_from(&path).unwrap();

    assert_eq!(tunnels.iter().count(), 0);
    assert!(texts(&warnings)[0].contains("needs a target"), "{:?}", texts(&warnings));
}
