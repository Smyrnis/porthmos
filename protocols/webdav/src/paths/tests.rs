use super::*;

fn nextcloud() -> Locator {
    Locator::new("https://cloud.example.com:443", "/remote.php/dav/files/alice/")
}

#[test]
fn the_root_of_an_empty_root_is_a_slash() {
    assert_eq!(Locator::new("http://h:80", "/").href(&VfsPath::new("/"), true), "/");
}

#[test]
fn files_are_encoded_under_the_root() {
    assert_eq!(nextcloud().href(&VfsPath::new("/docs/a b.txt"), false), "/remote.php/dav/files/alice/docs/a%20b.txt");
}

#[test]
fn collections_end_with_a_slash() {
    assert_eq!(nextcloud().href(&VfsPath::new("/docs"), true), "/remote.php/dav/files/alice/docs/");
    assert_eq!(nextcloud().href(&VfsPath::new("/"), true), "/remote.php/dav/files/alice/");
}

#[test]
fn reserved_and_non_ascii_characters_are_percent_encoded() {
    assert_eq!(
        Locator::new("http://h:80", "/").href(&VfsPath::new("/a #1 100% & ü.txt"), false),
        "/a%20%231%20100%25%20%26%20%C3%BC.txt"
    );
}

#[test]
fn parent_segments_never_escape_the_root() {
    assert_eq!(nextcloud().href(&VfsPath::new("/../../etc"), false), "/remote.php/dav/files/alice/etc");
    assert_eq!(normalize(&VfsPath::new("/a/../../b/./c")), VfsPath::new("/b/c"));
}

#[test]
fn urls_join_the_origin_and_href() {
    assert_eq!(
        nextcloud().url("/remote.php/dav/files/alice/"),
        "https://cloud.example.com:443/remote.php/dav/files/alice/"
    );
}

#[test]
fn hrefs_map_back_to_paths_under_the_root() {
    let locator = nextcloud();

    assert_eq!(locator.path_of_href("/remote.php/dav/files/alice/docs/a%20b.txt"), Some(VfsPath::new("/docs/a b.txt")));
    assert_eq!(
        locator.path_of_href("https://cloud.example.com/remote.php/dav/files/alice/docs/"),
        Some(VfsPath::new("/docs"))
    );
    assert_eq!(locator.path_of_href("/remote.php/dav/files/alice/"), Some(VfsPath::new("/")));
    assert_eq!(
        locator.path_of_href("/remote.php/dav/files/alice/Tom%20&%20Jerry.txt"),
        Some(VfsPath::new("/Tom & Jerry.txt"))
    );
}

#[test]
fn hrefs_outside_the_root_are_ignored() {
    assert_eq!(nextcloud().path_of_href("/remote.php/dav/files/bob/x"), None);
}

#[test]
fn a_percent_encoded_root_is_not_encoded_twice() {
    let locator = Locator::new("https://cloud:443", "/remote.php/dav/files/john%40example.com/");

    assert_eq!(locator.href(&VfsPath::new("/"), true), "/remote.php/dav/files/john%40example.com/");
    assert_eq!(locator.path_of_href("/remote.php/dav/files/john@example.com/a.txt"), Some(VfsPath::new("/a.txt")));
}

#[test]
fn a_name_with_a_backslash_or_a_drive_prefix_is_one_segment() {
    let locator = nextcloud();

    let backslash = locator.href(&VfsPath::new("/docs/a\\b"), false);
    let drive = locator.href(&VfsPath::new("/docs/x:1.txt"), false);

    assert_eq!(backslash, "/remote.php/dav/files/alice/docs/a%5Cb");
    assert_eq!(drive, "/remote.php/dav/files/alice/docs/x%3A1.txt");
    assert_eq!(locator.path_of_href(&backslash), Some(VfsPath::new("/docs/a\\b")));
    assert_eq!(locator.path_of_href(&drive), Some(VfsPath::new("/docs/x:1.txt")));
}

#[test]
fn parent_directory_segments_still_pop_a_segment() {
    assert_eq!(normalize(&VfsPath::new("/a/b/../c")), VfsPath::new("/a/c"));
    assert_eq!(normalize(&VfsPath::new("/..")), VfsPath::root());
}
