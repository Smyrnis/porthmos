use super::*;

#[test]
fn hidden_and_system_attributes_both_hide() {
    assert!(is_hidden_attribute(0x2));
    assert!(is_hidden_attribute(0x4));
    assert!(is_hidden_attribute(0x6));
    assert!(!is_hidden_attribute(0x20));
    assert!(!is_hidden_attribute(0));
}

#[cfg(unix)]
#[test]
fn local_entries_are_never_flagged_hidden_on_unix() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".dotfile"), b"x").unwrap();

    let entries = crate::list(dir.path()).unwrap();

    assert!(!entries[0].hidden);
    assert!(entries[0].is_hidden());
}

#[cfg(windows)]
#[test]
fn a_file_marked_hidden_is_listed_as_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("secret.txt");
    std::fs::write(&secret, b"x").unwrap();
    std::fs::write(dir.path().join("plain.txt"), b"x").unwrap();
    assert!(std::process::Command::new("attrib").arg("+H").arg(&secret).status().unwrap().success());

    let entries = crate::list(dir.path()).unwrap();

    let flagged: Vec<(&str, bool)> = {
        let mut found: Vec<_> = entries.iter().map(|entry| (entry.name.as_str(), entry.hidden)).collect();
        found.sort();
        found
    };
    assert_eq!(flagged, [("plain.txt", false), ("secret.txt", true)]);
}
