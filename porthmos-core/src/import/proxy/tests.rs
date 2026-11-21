use super::{ProxyFields, proxy_options};

fn fields(method: &str, host: &str, port: &str, user: &str) -> ProxyFields {
    let some = |text: &str| Some(text.to_string());
    ProxyFields { method: some(method), host: some(host), port: some(port), user: some(user) }
}

fn address(imported: &super::Imported) -> Option<&str> {
    imported.options.get("proxy").map(String::as_str)
}

#[test]
fn the_method_numbers_map_to_socks4_socks5_and_http() {
    assert_eq!(address(&proxy_options(&fields("1", "gate", "1080", ""))), Some("socks4://gate:1080"));
    assert_eq!(address(&proxy_options(&fields("2", "gate", "1080", ""))), Some("socks5://gate:1080"));
    assert_eq!(address(&proxy_options(&fields("3", "gate", "3128", ""))), Some("http://gate:3128"));
}

#[test]
fn no_method_or_method_zero_imports_no_proxy_and_no_note() {
    for method in [None, Some("0"), Some(""), Some(" 0 ")] {
        let found = ProxyFields {
            method: method.map(str::to_string),
            host: Some("gate".to_string()),
            port: Some("1080".to_string()),
            user: Some("alice".to_string()),
        };

        let imported = proxy_options(&found);

        assert!(imported.options.is_empty(), "{method:?}");
        assert!(imported.notes.is_empty(), "{method:?}");
    }
}

#[test]
fn a_missing_or_zero_port_leaves_the_default_of_the_proxy_type() {
    assert_eq!(address(&proxy_options(&fields("2", "gate", "", ""))), Some("socks5://gate"));
    assert_eq!(address(&proxy_options(&fields("2", "gate", "0", ""))), Some("socks5://gate"));
}

#[test]
fn an_ipv6_host_is_written_in_brackets() {
    assert_eq!(address(&proxy_options(&fields("2", "2001:db8::1", "1080", ""))), Some("socks5://[2001:db8::1]:1080"));
}

#[test]
fn the_user_name_is_imported_next_to_the_proxy() {
    let imported = proxy_options(&fields("3", "gate", "3128", "alice"));

    assert_eq!(imported.options.get("proxy_user").map(String::as_str), Some("alice"));
}

#[test]
fn a_blank_user_name_is_not_imported_and_control_characters_are_stripped() {
    assert!(!proxy_options(&fields("3", "gate", "3128", "  ")).options.contains_key("proxy_user"));
    let imported = proxy_options(&fields("3", "gate", "3128", "al\u{1b}[31mice"));
    assert_eq!(imported.options.get("proxy_user").map(String::as_str), Some("al[31mice"));
}

#[test]
fn proxy_types_porthmos_does_not_import_leave_a_note_and_no_options() {
    for method in ["4", "5", "6", "7", "banana"] {
        let imported = proxy_options(&fields(method, "gate", "1080", "alice"));

        assert!(imported.options.is_empty(), "{method}");
        assert_eq!(imported.notes.len(), 1, "{method}");
        assert!(imported.notes[0].contains("proxy"), "{method}: {:?}", imported.notes);
    }
}

#[test]
fn a_proxy_address_that_cannot_be_written_safely_is_left_out_with_a_note() {
    for host in ["", "a b", "a@b", "a/b", "a#b", "a\nb", "[x", "a]b"] {
        let imported = proxy_options(&fields("2", host, "1080", "alice"));

        assert!(imported.options.is_empty(), "{host:?}");
        assert_eq!(imported.notes.len(), 1, "{host:?}");
    }
}

#[test]
fn a_port_that_is_not_a_port_leaves_the_proxy_out_with_a_note_instead_of_guessing() {
    for port in ["abc", "70000", "-1", "80 80"] {
        let imported = proxy_options(&fields("2", "gate", port, ""));

        assert!(imported.options.is_empty(), "{port}");
        assert_eq!(imported.notes.len(), 1, "{port}");
    }
}

#[test]
fn a_password_is_never_imported_and_a_note_says_to_type_it_in_when_there_is_a_user() {
    let imported = proxy_options(&fields("2", "gate", "1080", "alice"));

    assert!(!imported.options.contains_key("proxy_password"));
    assert_eq!(imported.notes.len(), 1);
    assert!(imported.notes[0].contains("Proxy password"), "{:?}", imported.notes);
}

#[test]
fn without_a_user_there_is_no_password_note() {
    assert!(proxy_options(&fields("2", "gate", "1080", "")).notes.is_empty());
}
