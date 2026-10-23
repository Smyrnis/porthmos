use reqwest::header::{HeaderMap, HeaderValue};

use super::*;

fn headers(pairs: &[(&'static str, &[u8])]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(*name, HeaderValue::from_bytes(value).unwrap());
    }
    map
}

#[test]
fn the_server_header_is_reported() {
    let map = headers(&[("server", b"nginx/1.25.3"), ("dav", b"1,2")]);

    assert_eq!(server_info(&map).as_deref(), Some("nginx/1.25.3"));
}

#[test]
fn the_dav_header_is_the_fallback() {
    assert_eq!(server_info(&headers(&[("dav", b"1, 2, 3")])).as_deref(), Some("DAV 1, 2, 3"));
}

#[test]
fn no_useful_headers_means_none() {
    assert_eq!(server_info(&HeaderMap::new()), None);
    assert_eq!(server_info(&headers(&[("server", b"   "), ("dav", b"")])), None);
    assert_eq!(server_info(&headers(&[("content-type", b"text/xml")])), None);
}

#[test]
fn a_blank_server_header_falls_back_to_dav() {
    assert_eq!(server_info(&headers(&[("server", b" "), ("dav", b"1")])).as_deref(), Some("DAV 1"));
}

#[test]
fn only_the_first_server_header_is_used() {
    let map = headers(&[("server", b"first"), ("server", b"second")]);

    assert_eq!(server_info(&map).as_deref(), Some("first"));
}

#[test]
fn header_text_is_cleaned_and_cut() {
    assert_eq!(server_info(&headers(&[("server", b"a\xffb")])).as_deref(), Some("a\u{fffd}b"));
    let long = "x".repeat(200);
    let cut = server_info(&headers(&[("server", long.as_bytes())])).unwrap();
    assert!(cut.chars().count() <= 40 && cut.ends_with('\u{2026}'), "{cut}");
}
