use std::path::{Path, PathBuf};

use porthmos_core::i18n;

fn sources(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "tests") {
                sources(&path, found);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && path.file_name().is_some_and(|name| name != "tests.rs")
        {
            found.push(path);
        }
    }
}

fn unescape(source: &str) -> String {
    let mut text = String::new();
    let mut characters = source.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            text.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => text.push('\n'),
            Some('u') => {
                let digits: String = characters.by_ref().skip(1).take_while(|digit| *digit != '}').collect();
                text.push(char::from_u32(u32::from_str_radix(&digits, 16).unwrap()).unwrap());
            }
            Some(other) => text.push(other),
            None => {}
        }
    }
    text
}

fn literals_passed_to_tr(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("tr(\"") {
        let before = rest[..start].chars().next_back();
        let literal_start = start + 4;
        let mut end = literal_start;
        let bytes = rest.as_bytes();
        while end < bytes.len() && bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        if before.is_none_or(|character| !(character.is_alphanumeric() || character == '_')) {
            found.push(unescape(&rest[literal_start..end]));
        }
        rest = &rest[end.min(rest.len())..];
    }
    found
}

#[test]
fn every_text_the_window_app_translates_is_in_the_language_file() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut missing = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        for text in literals_passed_to_tr(&source) {
            let key = i18n::label_key(&text);
            if i18n::text(&key) == key {
                missing.push(format!("{}: \"{text}\" needs {key}", file.display()));
            }
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}

#[test]
fn the_scan_finds_the_texts_passed_to_tr() {
    let found = literals_passed_to_tr("button(tr(\"Open\"), x); let a = str(\"no\"); tr(\"A \\\"b\\\"\")");

    assert_eq!(found, vec!["Open".to_string(), "A \"b\"".to_string()]);
}
