use super::*;

#[test]
fn a_line_split_across_chunks_comes_out_whole() {
    let mut splitter = LineSplitter::default();

    assert_eq!(splitter.push(b"/a/one\n/a/tw"), vec!["/a/one".to_string()]);
    assert_eq!(splitter.push(b"o\n/a/three\n"), vec!["/a/two".to_string(), "/a/three".to_string()]);
    assert_eq!(splitter.finish(), None);
}

#[test]
fn the_last_line_without_a_newline_comes_out_at_the_end() {
    let mut splitter = LineSplitter::default();

    assert_eq!(splitter.push(b"/a/one\n/a/two"), vec!["/a/one".to_string()]);
    assert_eq!(splitter.finish(), Some("/a/two".to_string()));
}

#[test]
fn empty_lines_are_dropped() {
    let mut splitter = LineSplitter::default();

    assert_eq!(splitter.push(b"\n/a\n\n\n/b\n"), vec!["/a".to_string(), "/b".to_string()]);
    assert_eq!(splitter.finish(), None);
}

#[test]
fn a_character_split_across_chunks_is_not_damaged() {
    let bytes = "/a/caf\u{e9}\n".as_bytes();
    let split_inside_the_accent = bytes.len() - 2;
    let mut splitter = LineSplitter::default();

    assert!(splitter.push(&bytes[..split_inside_the_accent]).is_empty());
    assert_eq!(splitter.push(&bytes[split_inside_the_accent..]), vec!["/a/caf\u{e9}".to_string()]);
}

#[test]
fn bytes_that_are_not_text_are_replaced() {
    let mut splitter = LineSplitter::default();

    assert_eq!(splitter.push(b"/a/\xff\n"), vec!["/a/\u{fffd}".to_string()]);
}

#[test]
fn a_chunk_without_a_newline_produces_nothing_yet() {
    let mut splitter = LineSplitter::default();

    assert!(splitter.push(b"/a/one").is_empty());
    assert!(splitter.push(b"/two").is_empty());
    assert_eq!(splitter.finish(), Some("/a/one/two".to_string()));
}
