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

#[test]
fn english_and_greek_are_offered_under_their_own_names() {
    let languages = available_languages();

    assert_eq!(languages.iter().map(|language| language.code).collect::<Vec<_>>(), ["en", "el"]);
    assert_eq!(languages[0].name, "English");
    assert_eq!(languages[1].name, "Ελληνικά");
}

#[test]
fn only_the_offered_languages_are_available() {
    assert!(is_available("en"));
    assert!(is_available("el"));
    assert!(!is_available("klingon"));
    assert!(!is_available(""));
}

#[test]
fn a_chosen_language_wins_over_the_environment() {
    assert_eq!(chosen_language(Some("el"), Some("en_US.UTF-8")), "el");
    assert_eq!(chosen_language(Some("en"), Some("el_GR")), "en");
}

#[test]
fn without_a_choice_the_environment_decides() {
    assert_eq!(chosen_language(None, Some("el_GR.UTF-8")), "el");
    assert_eq!(chosen_language(None, None), "en");
}

#[test]
fn a_choice_that_is_not_offered_is_ignored() {
    assert_eq!(chosen_language(Some("klingon"), Some("el_GR")), "el");
}

#[test]
fn an_environment_language_that_is_not_offered_means_english() {
    assert_eq!(chosen_language(None, Some("de_DE.UTF-8")), "en");
}

fn flatten_locale(file: &str) -> Vec<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales").join(file);
    let contents = std::fs::read_to_string(&path).expect("the locale file");
    let mut stack: Vec<String> = Vec::new();
    let mut entries = Vec::new();
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let depth = (line.len() - line.trim_start().len()) / 2;
        let (name, value) = line.trim().split_once(':').expect("a key line");
        stack.truncate(depth);
        let value = value.trim();
        if value.is_empty() {
            stack.push(name.to_string());
        } else if name != "_version" {
            let unquoted = value.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).expect("a quoted value");
            entries.push((format!("{}.{name}", stack.join(".")), unquoted.replace("\\\"", "\"")));
        }
    }
    entries
}

fn placeholders(value: &str) -> Vec<String> {
    let mut found: Vec<String> = value
        .match_indices("%{")
        .filter_map(|(offset, _)| {
            value[offset + 2..].find('}').map(|end| value[offset + 2..offset + 2 + end].to_string())
        })
        .collect();
    found.sort();
    found
}

#[test]
fn every_language_file_has_exactly_the_keys_of_the_english_one() {
    let english = flatten_locale("en.yml");
    let english_keys: Vec<&String> = english.iter().map(|(key, _)| key).collect();
    for language in available_languages().iter().filter(|language| language.code != "en") {
        let translated = flatten_locale(&format!("{}.yml", language.code));
        let translated_keys: Vec<&String> = translated.iter().map(|(key, _)| key).collect();
        for key in &english_keys {
            assert!(translated_keys.contains(key), "{} lacks {key}", language.code);
        }
        for key in &translated_keys {
            assert!(english_keys.contains(key), "{} has {key}, which en.yml does not", language.code);
        }
    }
}

#[test]
fn every_translation_keeps_the_placeholders_of_the_english_text() {
    let english = flatten_locale("en.yml");
    for language in available_languages().iter().filter(|language| language.code != "en") {
        for (key, value) in flatten_locale(&format!("{}.yml", language.code)) {
            let original = &english.iter().find(|(name, _)| *name == key).expect("a matching key").1;
            assert_eq!(placeholders(&value), placeholders(original), "{} {key}", language.code);
        }
    }
}

#[test]
fn a_greek_key_gives_greek_text() {
    assert_eq!(text_in("el", "folders.compare.needs_server"), "Συνδεθείτε σε διακομιστή για να συγκρίνετε φακέλους");
}

#[test]
fn a_plural_can_carry_more_values_than_its_count() {
    assert_eq!(
        plural_with_in("en", "transfers.restored", 2, &[("names", "alpha or beta")]),
        "2 transfers from last time are in the transfers list. Connect to alpha or beta and retry them to continue."
    );
    assert_eq!(
        plural_with_in("en", "transfers.restored", 1, &[("names", "alpha")]),
        "1 transfer from last time is in the transfers list. Connect to alpha and retry it to continue."
    );
}

#[test]
fn a_label_key_is_the_english_text_with_its_letters_and_digits_joined_by_underscores() {
    assert_eq!(label_key("Host"), "label.Host");
    assert_eq!(label_key("host"), "label.host");
    assert_eq!(label_key("Private key (optional)"), "label.Private_key_optional_");
    assert_eq!(label_key("  S3 region / endpoint  "), "label.S3_region_endpoint_");
    assert_eq!(label_key("Search"), "label.Search");
    assert_eq!(label_key("Search\u{2026}"), "label.Search_");
}

#[test]
fn a_label_without_a_translation_stays_english() {
    assert_eq!(label_in("el", "Not in the file at all"), "Not in the file at all");
    assert_eq!(label_in("en", "Not in the file at all"), "Not in the file at all");
}

#[test]
fn a_label_with_a_translation_comes_back_translated() {
    assert_eq!(label_in("el", "Host"), "Διακομιστής");
}

#[test]
fn text_or_gives_the_fallback_for_a_key_that_is_not_in_the_file() {
    assert_eq!(text_or_in("en", "no.such.key", "plain words"), "plain words");
    assert_eq!(text_or_in("el", "no.such.key", "plain words"), "plain words");
}

#[test]
fn text_or_gives_the_translation_for_a_key_that_is_in_the_file() {
    assert_eq!(text_or_in("en", "common.cancel", "ignored"), "Cancel");
}
