use super::*;

#[test]
fn only_web_and_mail_schemes_are_openable() {
    let openable = ["https://a.b", "HTTP://a.b/x?y=1", "mailto:x@y.z"];
    let refused = [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "ssh://h",
        "https://a b",
        "https://a\u{7}b",
        "https://",
        "https:a.b",
        "",
    ];

    for uri in openable {
        assert!(is_openable(uri), "{uri}");
    }
    for uri in refused {
        assert!(!is_openable(uri), "{uri}");
    }
}

#[test]
fn url_at_finds_a_url_and_trims_punctuation() {
    let text = "see https://example.com/x, ok";

    assert_eq!(url_at(text, 10), Some(4..25));
    assert_eq!(url_at(text, 4), Some(4..25));
    assert_eq!(url_at(text, 1), None);
    assert_eq!(url_at(text, 26), None);
    assert_eq!(url_at(text, 500), None);
}

#[test]
fn url_at_handles_brackets_and_quotes() {
    assert_eq!(url_at("(https://a.b/c)", 5), Some(1..14));
    assert_eq!(url_at("https://en.wikipedia.org/wiki/Foo_(bar)", 10), Some(0..39));
    assert_eq!(url_at("\"https://a.b\"", 4), Some(1..12));
    assert_eq!(url_at("<mailto:me@x.org>", 4), Some(1..16));
}

#[test]
fn invisible_formatting_characters_make_a_link_unopenable() {
    for hidden in ['\u{202e}', '\u{200b}', '\u{2066}', '\u{feff}', '\u{ad}'] {
        let uri = format!("https://example.com/{hidden}evil");
        assert!(!is_openable(&uri), "{hidden:?}");
    }
}
