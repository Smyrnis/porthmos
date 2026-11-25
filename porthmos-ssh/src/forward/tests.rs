use porthmos_vfs::ErrorKind;

use super::*;

#[test]
fn a_requested_port_is_kept_whatever_the_server_answered() {
    assert_eq!(granted_port(8080, 0).unwrap(), 8080);
    assert_eq!(granted_port(8080, 9999).unwrap(), 8080);
}

#[test]
fn port_zero_takes_the_servers_answer() {
    assert_eq!(granted_port(0, 41000).unwrap(), 41000);
    assert_eq!(granted_port(0, 65535).unwrap(), 65535);
}

#[test]
fn port_zero_with_no_usable_answer_is_a_connect_error_naming_the_problem() {
    for answered in [0, 65536, u32::MAX] {
        let error = granted_port(0, answered).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::Connect);
        assert!(error.to_string().contains("did not say which port"), "{error}");
    }
}
