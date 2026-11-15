use super::*;

#[test]
fn printable_drops_escape_and_control_characters() {
    assert_eq!(printable("a\u{1b}[31mb\u{0}c"), "a[31mbc");
    assert_eq!(printable("tab\there\nnewline"), "tabherenewline");
}

#[test]
fn printable_keeps_ordinary_unicode() {
    assert_eq!(printable("café 東京"), "café 東京");
}

#[test]
fn name_trims_and_rejects_empty() {
    assert_eq!(name("  x \n"), Some("x".to_string()));
    assert_eq!(name("  \t"), None);
    assert_eq!(name("\u{1b}\u{0}"), None);
}

#[test]
fn group_joins_folders_with_slash() {
    assert_eq!(group_from_folders(&["Work", "Prod"]), Some("Work/Prod".to_string()));
}

#[test]
fn slash_inside_a_folder_name_does_not_split_the_group() {
    assert_eq!(group_from_folders(&["A/B"]), Some("A-B".to_string()));
}

#[test]
fn empty_folders_give_no_group() {
    assert_eq!(group_from_folders(&[]), None);
    assert_eq!(group_from_folders(&["  ", "\u{1b}"]), None);
}

#[test]
fn empty_folders_between_real_ones_are_dropped() {
    assert_eq!(group_from_folders(&["a", " ", "b"]), Some("a/b".to_string()));
}

#[test]
fn user_and_host_splits_at_the_last_at_sign_only_without_a_username() {
    assert_eq!(user_and_host("bob@h.example", String::new()), Ok(("bob".to_string(), "h.example".to_string())));
    assert_eq!(user_and_host("h.example", "ann".to_string()), Ok(("ann".to_string(), "h.example".to_string())));
    assert_eq!(user_and_host(" h.example ", String::new()), Ok((String::new(), "h.example".to_string())));
}

#[test]
fn a_password_or_url_in_the_host_field_is_refused_without_echoing_it() {
    for raw in ["u:SECRETPW@h.example", "ftp://user:SECRETPW@h.example/path", "sftp://h.example", "u/x@h.example"] {
        assert_eq!(user_and_host(raw, String::new()), Err("invalid host"), "{raw}");
    }
}

#[test]
fn a_host_with_a_path_or_whitespace_is_refused() {
    for raw in ["h.example/path", "h y", "h.example\u{1b}"] {
        assert_eq!(user_and_host(raw, String::new()), Err("invalid host"), "{raw:?}");
    }
    assert_eq!(user_and_host("", String::new()), Err("no host"));
    assert_eq!(user_and_host("bob@", String::new()), Err("no host"));
}

#[test]
fn printable_drops_characters_that_hide_or_reorder_text() {
    assert_eq!(printable("a\u{202e}gnp.exe"), "agnp.exe");
    assert_eq!(printable("ad\u{200b}min"), "admin");
    assert_eq!(printable("\u{feff}host"), "host");
    assert_eq!(printable("x\u{2066}y\u{2069}z"), "xyz");
    assert_eq!(printable("a\u{200d}b\u{200e}c\u{200f}d"), "abcd");
    assert_eq!(printable("soft\u{ad}hyphen"), "softhyphen");
    assert_eq!(printable("a\u{2060}b"), "ab");
}

#[test]
fn a_name_made_only_of_invisible_characters_is_empty() {
    assert_eq!(name("\u{200b}\u{202e}"), None);
}

#[test]
fn a_host_with_a_hidden_character_is_refused() {
    assert_eq!(user_and_host("exa\u{200b}mple.com", String::new()), Err("invalid host"));
}
