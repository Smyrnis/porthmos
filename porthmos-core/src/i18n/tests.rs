use std::path::{Path, PathBuf};

use super::*;

#[test]
fn a_known_key_gives_its_english_text() {
    assert_eq!(text_in("en", "folders.compare.identical"), "Comparing folders: they are identical");
}

#[test]
fn an_unknown_key_comes_back_as_itself_so_the_gap_is_visible() {
    assert_eq!(text_in("en", "no.such.key"), "no.such.key");
}

#[test]
fn an_unknown_language_falls_back_to_english() {
    assert_eq!(text_in("xx", "folders.compare.needs_server"), "Connect to a server to compare folders");
}

#[test]
fn placeholders_are_filled_in() {
    assert_eq!(
        fill("Hello %{name}, you have %{count} items", &[("name", "Ada"), ("count", "3")]),
        "Hello Ada, you have 3 items"
    );
}

#[test]
fn a_placeholder_without_a_value_is_left_alone() {
    assert_eq!(fill("keep %{this}", &[("other", "x")]), "keep %{this}");
}

#[test]
fn the_plural_form_follows_the_count() {
    assert_eq!(plural_in("en", "folders.compare.differences", 1), "Comparing folders: 1 difference");
    assert_eq!(plural_in("en", "folders.compare.differences", 0), "Comparing folders: 0 differences");
    assert_eq!(plural_in("en", "folders.compare.differences", 5), "Comparing folders: 5 differences");
}

#[test]
fn the_language_comes_from_the_environment_variable() {
    assert_eq!(language_of(Some("el_GR.UTF-8")), "el");
    assert_eq!(language_of(Some("en_US")), "en");
    assert_eq!(language_of(Some("pt-BR")), "pt");
    assert_eq!(language_of(Some("de")), "de");
}

#[test]
fn no_variable_or_a_neutral_one_means_english() {
    for neutral in [None, Some(""), Some("C"), Some("POSIX"), Some("C.UTF-8")] {
        assert_eq!(language_of(neutral), "en", "{neutral:?}");
    }
}

#[test]
fn a_language_that_is_not_just_letters_means_english() {
    assert_eq!(language_of(Some("../../etc")), "en");
}

fn source_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            source_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

fn keys_asked_for(call: &str) -> Vec<(PathBuf, String)> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the workspace root");
    let mut files = Vec::new();
    for member in std::fs::read_dir(workspace).expect("the workspace").flatten() {
        source_files(&member.path().join("src"), &mut files);
    }
    let mut asked = Vec::new();
    for file in files {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        let needle = format!("{call}(\"");
        for (offset, _) in contents.match_indices(&needle) {
            let rest = &contents[offset + needle.len()..];
            if let Some(end) = rest.find('"') {
                asked.push((file.clone(), rest[..end].to_string()));
            }
        }
    }
    asked
}

#[test]
fn every_text_key_the_code_asks_for_exists_in_the_english_file() {
    let own_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("i18n");
    for call in ["i18n::text", "i18n::text_with"] {
        for (file, key) in keys_asked_for(call) {
            if file.starts_with(&own_source) {
                continue;
            }
            assert_ne!(text_in("en", &key), key, "{key} is asked for in {} but is not in en.yml", file.display());
        }
    }
}

#[test]
fn every_plural_key_the_code_asks_for_has_both_forms() {
    for (file, key) in keys_asked_for("i18n::plural") {
        for form in ["one", "other"] {
            let full = format!("{key}.{form}");
            assert_ne!(text_in("en", &full), full, "{full} is missing from en.yml (asked for in {})", file.display());
        }
    }
}

#[test]
fn a_value_that_looks_like_a_placeholder_is_never_filled_in_again() {
    let filled = fill("Delete \"%{name}\" from %{place}?", &[("name", "%{place}"), ("place", "the server")]);

    assert_eq!(filled, "Delete \"%{place}\" from the server?");
}

#[test]
fn an_unknown_placeholder_is_left_as_it_is() {
    assert_eq!(fill("a %{missing} b", &[("other", "x")]), "a %{missing} b");
}
