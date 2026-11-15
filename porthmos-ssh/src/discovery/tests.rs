use std::path::Path;

use super::*;

#[test]
fn an_identity_file_under_the_home_tilde_is_expanded() {
    let home = Path::new("/home/alice");

    assert_eq!(identity_path("~/.ssh/id_ed25519", Some(home)), Path::new("/home/alice/.ssh/id_ed25519"));
    assert_eq!(identity_path("~", Some(home)), Path::new("/home/alice"));
}

#[test]
fn other_identity_file_paths_are_used_as_written() {
    let home = Path::new("/home/alice");

    assert_eq!(identity_path("/keys/id", Some(home)), Path::new("/keys/id"));
    assert_eq!(identity_path("~bob/id", Some(home)), Path::new("~bob/id"));
    assert_eq!(identity_path("~/.ssh/id", None), Path::new("~/.ssh/id"));
}

fn home_with(config: &str) -> (tempfile::TempDir, Environment) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".ssh")).unwrap();
    std::fs::write(dir.path().join(".ssh").join("config"), config).unwrap();
    let env =
        Environment { home: Some(dir.path().to_path_buf()), user: Some("alice".into()), ..Environment::default() };
    (dir, env)
}

#[test]
fn a_discovered_host_carries_its_proxy_jump() {
    let (_dir, env) = home_with("Host box\n  HostName 10.0.0.2\n  ProxyJump bastion\n");

    let targets = discover(&env).unwrap();

    assert_eq!(targets[0].option("proxy_jump"), Some("bastion"));
}

#[test]
fn a_discovered_host_without_a_jump_has_no_such_option() {
    let (_dir, env) = home_with("Host box\n  HostName 10.0.0.2\n");

    assert_eq!(discover(&env).unwrap()[0].option("proxy_jump"), None);
}

#[test]
fn a_profile_that_says_nothing_about_a_jump_still_matches_its_alias() {
    let (_dir, env) = home_with("Host box\n  HostName 10.0.0.2\n  ProxyJump bastion\n");
    let mut target = discover(&env).unwrap().remove(0);
    target.options.remove("proxy_jump");

    assert!(matches_its_ssh_config_alias(&target, &env));
}

#[test]
fn a_profile_with_another_jump_than_its_alias_no_longer_matches_the_alias() {
    let (_dir, env) = home_with("Host box\n  HostName 10.0.0.2\n  ProxyJump bastion\n");
    let mut target = discover(&env).unwrap().remove(0);

    assert!(matches_its_ssh_config_alias(&target, &env));
    target.options.insert("proxy_jump".to_string(), "elsewhere".to_string());
    assert!(!matches_its_ssh_config_alias(&target, &env));
}
