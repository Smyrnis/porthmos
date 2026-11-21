use porthmos_vfs::{Environment, Protocol};

use super::*;

#[test]
fn describes_itself_as_scp_on_port_22() {
    let protocol = Scp::default();

    assert_eq!((protocol.id(), protocol.display_name(), protocol.default_port()), ("scp", "SCP", 22));
}

#[test]
fn the_form_matches_sftp_with_an_identity_file() {
    let form = Scp::default().connection_form();

    assert_eq!(form.option("identity_file").map(|field| field.label), Some("Identity file"));
    assert!(form.reserved_key_collisions().is_empty());
}

#[test]
fn the_form_offers_the_same_optional_jump_host() {
    let form = Scp::default().connection_form();

    let jump = form.option("proxy_jump").expect("a jump host field");
    assert_eq!((jump.label, jump.required), ("Jump host", false));
    assert_eq!(jump, &porthmos_ssh::PROXY_JUMP_FIELD);
}

#[test]
fn the_form_offers_the_same_optional_proxy_and_proxy_command() {
    let form = Scp::default().connection_form();

    assert_eq!(form.option("proxy"), Some(&porthmos_ssh::PROXY_FIELD));
    assert_eq!(form.option("proxy_command"), Some(&porthmos_ssh::PROXY_COMMAND_FIELD));
}

#[test]
fn the_form_offers_a_proxy_login_with_the_password_kept_secret() {
    let form = Scp::default().connection_form();

    assert_eq!(form.option("proxy_user"), Some(&porthmos_ssh::PROXY_USER_FIELD));
    assert_eq!(form.option("proxy_password"), Some(&porthmos_ssh::PROXY_PASSWORD_FIELD));
    assert_eq!(porthmos_ssh::PROXY_PASSWORD_FIELD.kind, porthmos_vfs::OptionKind::Secret);
}

#[test]
fn scp_does_not_discover_hosts() {
    assert!(Scp::default().discover(&Environment::default()).unwrap().is_empty());
}
