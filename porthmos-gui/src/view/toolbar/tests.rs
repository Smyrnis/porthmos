use super::show_titles;

#[test]
fn labels_collapse_below_900() {
    assert!(!show_titles(899.0));
    assert!(show_titles(900.0));
}
