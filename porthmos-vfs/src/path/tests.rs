use std::ffi::OsStr;

use serde::{Deserialize, Serialize};

use super::*;

fn texts(path: &VfsPath) -> Vec<String> {
    path.segments().map(|segment| segment.to_string_lossy().into_owned()).collect()
}

#[test]
fn repeated_slashes_a_trailing_slash_and_dot_segments_are_removed() {
    assert_eq!(VfsPath::new("/a//b/./c/"), VfsPath::new("/a/b/c"));
    assert_eq!(texts(&VfsPath::new("/a/../b")), vec!["a", "..", "b"]);
}

#[test]
fn the_root_is_absolute_and_has_no_parent_or_file_name() {
    let root = VfsPath::root();

    assert!(root.is_absolute());
    assert!(root.is_root());
    assert_eq!(root.parent(), None);
    assert_eq!(root.file_name(), None);
    assert_eq!(VfsPath::new("///"), root);
}

#[test]
fn a_relative_path_is_not_absolute() {
    assert!(!VfsPath::new("a/b").is_absolute());
    assert!(VfsPath::new("/a/b").is_absolute());
}

#[test]
fn the_empty_path_is_empty_and_has_no_parent() {
    let empty = VfsPath::new("");

    assert!(empty.is_empty());
    assert!(!empty.is_absolute());
    assert_eq!(empty.parent(), None);
    assert_eq!(empty, VfsPath::default());
    assert_eq!(VfsPath::new("."), empty);
}

#[test]
fn join_appends_a_relative_path_and_an_absolute_one_replaces_it() {
    assert_eq!(VfsPath::new("/srv").join("a/b"), VfsPath::new("/srv/a/b"));
    assert_eq!(VfsPath::new("/srv").join("/etc"), VfsPath::new("/etc"));
    assert_eq!(VfsPath::new("/srv/").join("a").to_string_lossy(), "/srv/a");
    assert_eq!(VfsPath::new("/srv").join(""), VfsPath::new("/srv"));
    assert_eq!(VfsPath::new("a").join("b"), VfsPath::new("a/b"));
}

#[test]
fn join_accepts_text_and_paths_alike() {
    let base = VfsPath::new("/srv");
    let relative = VfsPath::new("a/b");

    assert_eq!(base.join(&relative), VfsPath::new("/srv/a/b"));
    assert_eq!(base.join(relative), VfsPath::new("/srv/a/b"));
    assert_eq!(base.join(String::from("c")), VfsPath::new("/srv/c"));
    let borrowed = String::from("d");
    assert_eq!(base.join(&borrowed), VfsPath::new("/srv/d"));
    assert_eq!(borrowed, "d");
}

#[test]
fn parent_walks_up_to_the_root_and_a_single_relative_name_has_the_empty_path_as_parent() {
    assert_eq!(VfsPath::new("/a/b").parent(), Some(VfsPath::new("/a")));
    assert_eq!(VfsPath::new("/a").parent(), Some(VfsPath::root()));
    assert_eq!(VfsPath::new("a/b").parent(), Some(VfsPath::new("a")));
    assert_eq!(VfsPath::new("a").parent(), Some(VfsPath::new("")));
}

#[test]
fn file_name_is_the_last_segment_except_for_a_parent_marker() {
    assert_eq!(VfsPath::new("/a/b.txt").file_name(), Some(OsStr::new("b.txt")));
    assert_eq!(VfsPath::new("/a/..").file_name(), None);
}

#[test]
fn names_that_look_like_windows_syntax_are_ordinary_names() {
    let base = VfsPath::new("/srv");
    for name in ["x:1.txt", "a\\b", "con", "name.", "C:", "\\\\server\\share"] {
        let joined = base.join(name);

        assert_eq!(joined.segments().count(), 2, "{name}");
        assert_eq!(joined.file_name(), Some(OsStr::new(name)), "{name}");
        assert_eq!(joined.parent(), Some(base.clone()), "{name}");
        assert!(joined.starts_with(&base), "{name}");
        assert_eq!(joined.to_string_lossy(), format!("/srv/{name}"), "{name}");
    }
}

#[test]
fn starts_with_and_strip_prefix_cut_on_whole_segments() {
    assert!(VfsPath::new("/a/b").starts_with(&VfsPath::new("/a")));
    assert!(!VfsPath::new("/ab").starts_with(&VfsPath::new("/a")));
    assert!(!VfsPath::new("a/b").starts_with(&VfsPath::new("/a")));
    assert_eq!(VfsPath::new("/a/b/c").strip_prefix(&VfsPath::new("/a")), Some(VfsPath::new("b/c")));
    assert_eq!(VfsPath::new("/a").strip_prefix(&VfsPath::new("/a")), Some(VfsPath::new("")));
    assert_eq!(VfsPath::new("/ab").strip_prefix(&VfsPath::new("/a")), None);
    assert_eq!(VfsPath::new("/x").strip_prefix(&VfsPath::new("/a")), None);
}

#[test]
fn ancestors_run_from_the_path_up_to_the_root() {
    let absolute: Vec<VfsPath> = VfsPath::new("/a/b").ancestors().collect();
    assert_eq!(absolute, vec![VfsPath::new("/a/b"), VfsPath::new("/a"), VfsPath::root()]);

    let relative: Vec<VfsPath> = VfsPath::new("a/b").ancestors().collect();
    assert_eq!(relative, vec![VfsPath::new("a/b"), VfsPath::new("a"), VfsPath::new("")]);
}

#[test]
fn with_file_name_and_with_suffix_change_only_the_last_segment() {
    let path = VfsPath::new("/a/b.txt");

    assert_eq!(path.with_file_name("c.txt"), VfsPath::new("/a/c.txt"));
    assert_eq!(path.with_suffix(".part"), VfsPath::new("/a/b.txt.part"));
}

#[test]
fn a_suffix_on_a_path_without_a_name_never_gives_back_the_same_path() {
    for path in [VfsPath::root(), VfsPath::new("")] {
        let suffixed = path.with_suffix(".part");

        assert_ne!(suffixed, path);
        assert_eq!(suffixed.file_name(), Some(std::ffi::OsStr::new(".part")));
    }
}

#[test]
fn a_new_file_name_with_separators_is_split_like_any_other_path() {
    let path = VfsPath::new("/a/b.txt");

    assert_eq!(path.with_file_name("x/y"), VfsPath::new("/a/x/y"));
    assert_eq!(path.with_file_name("x/y").segments().count(), 3);
    assert_eq!(path.with_file_name("./x"), VfsPath::new("/a/x"));
    assert_eq!(path.with_file_name(""), VfsPath::new("/a"));
    assert_eq!(path.with_file_name("."), VfsPath::new("/a"));
}

#[cfg(unix)]
#[test]
fn a_non_utf8_segment_survives_join_parent_file_name_and_with_suffix() {
    use std::os::unix::ffi::OsStrExt;

    let raw = OsStr::from_bytes(b"caf\xe9");
    let path = VfsPath::from_segments(true, ["srv".as_ref(), raw]);

    assert_eq!(path.file_name(), Some(raw));
    assert_eq!(path.parent(), Some(VfsPath::new("/srv")));
    assert_eq!(path.with_suffix(".part").file_name(), Some(OsStr::from_bytes(b"caf\xe9.part")));
    assert_eq!(path.join("x").parent().unwrap().file_name(), Some(raw));
    assert_eq!(path.to_string_lossy(), "/srv/caf\u{fffd}");
}

#[test]
fn from_segments_takes_each_segment_literally_and_drops_dots_and_empty_ones() {
    let path = VfsPath::from_segments(true, ["a\\b", ".", "", "c:"]);

    assert_eq!(texts(&path), vec!["a\\b", "c:"]);
    assert!(path.is_absolute());
}

#[test]
fn display_uses_slashes_and_the_lossy_text() {
    assert_eq!(VfsPath::new("/a/b").to_string(), "/a/b");
    assert_eq!(VfsPath::root().to_string(), "/");
    assert_eq!(VfsPath::new("a/b").to_string(), "a/b");
    assert_eq!(format!("{:?}", VfsPath::new("/a/b")), "VfsPath(\"/a/b\")");
}

#[test]
fn equal_paths_hash_equally() {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let hash = |path: &VfsPath| {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        hasher.finish()
    };

    assert_eq!(hash(&VfsPath::new("/a//b/")), hash(&VfsPath::new("/a/b")));
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Holder {
    path: VfsPath,
}

#[test]
fn serde_round_trips_as_a_plain_string() {
    let text = "path = \"/a/b\"\n";

    let holder: Holder = toml::from_str(text).unwrap();

    assert_eq!(holder.path, VfsPath::new("/a/b"));
    assert_eq!(toml::to_string(&holder).unwrap(), text);
}

#[cfg(unix)]
#[test]
fn serialising_a_non_utf8_path_is_an_error() {
    use std::os::unix::ffi::OsStrExt;

    let holder = Holder { path: VfsPath::from_segments(true, [OsStr::from_bytes(b"caf\xe9")]) };

    assert!(toml::to_string(&holder).is_err());
}
