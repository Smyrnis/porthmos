use std::path::PathBuf;

use super::{list, parse};

#[test]
fn a_plain_path_becomes_a_file_uri_with_crlf() {
    assert_eq!(list(&[PathBuf::from("/tmp/a.txt")]), "file:///tmp/a.txt\r\n");
}

#[test]
fn spaces_and_non_ascii_letters_are_percent_encoded() {
    let uri = list(&[PathBuf::from("/tmp/ünï-文件 with spaces.txt")]);

    assert_eq!(uri, "file:///tmp/%C3%BCn%C3%AF-%E6%96%87%E4%BB%B6%20with%20spaces.txt\r\n");
}

#[test]
fn encoding_then_parsing_returns_the_same_paths() {
    let paths = vec![PathBuf::from("/tmp/ünï-文件 with spaces.txt"), PathBuf::from("/tmp/100%.txt")];

    assert_eq!(parse(&list(&paths)), paths);
}

#[test]
fn comments_blank_lines_and_other_schemes_are_skipped() {
    let text = "# comment\r\n\r\nhttp://example.com/x\r\nfile:///tmp/ok.txt\r\n";

    assert_eq!(parse(text), vec![PathBuf::from("/tmp/ok.txt")]);
}

#[test]
fn a_localhost_authority_is_accepted() {
    assert_eq!(parse("file://localhost/tmp/x\r\n"), vec![PathBuf::from("/tmp/x")]);
}
