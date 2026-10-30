use porthmos_core::VfsPath;

use super::History;

fn path(text: &str) -> VfsPath {
    VfsPath::new(text)
}

#[test]
fn back_returns_the_previous_folder_and_enables_forward() {
    let mut history = History::new();
    history.visit(path("/a"));
    assert!(history.can_back());
    assert!(!history.can_forward());

    assert_eq!(history.back(path("/b")), Some(path("/a")));

    assert!(!history.can_back());
    assert!(history.can_forward());
    assert_eq!(history.forward(path("/a")), Some(path("/b")));
    assert!(history.can_back());
}

#[test]
fn visiting_clears_forward() {
    let mut history = History::new();
    history.visit(path("/a"));
    history.back(path("/b"));
    assert!(history.can_forward());

    history.visit(path("/b"));

    assert!(!history.can_forward());
}

#[test]
fn back_on_an_empty_history_is_none() {
    let mut history = History::new();

    assert_eq!(history.back(path("/a")), None);
    assert_eq!(history.forward(path("/a")), None);
}

#[test]
fn history_is_capped_at_100_entries() {
    let mut history = History::new();
    for n in 0..150 {
        history.visit(path(&format!("/d{n}")));
    }

    let mut steps = 0;
    let mut current = path("/now");
    while let Some(previous) = history.back(current.clone()) {
        current = previous;
        steps += 1;
    }

    assert_eq!(steps, 100);
    assert_eq!(current, path("/d50"));
}
