use super::*;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| name.to_string()).collect()
}

#[test]
fn one_to_three_names_are_listed_in_full() {
    assert_eq!(offer_summary(&names(&["prod"])), "prod");
    assert_eq!(offer_summary(&names(&["prod", "nas"])), "prod, nas");
    assert_eq!(offer_summary(&names(&["prod", "nas", "db"])), "prod, nas, db");
}

#[test]
fn more_than_three_names_are_cut_and_counted() {
    assert_eq!(offer_summary(&names(&["a", "b", "c", "d"])), "a, b, c and 1 more");
    assert_eq!(offer_summary(&names(&["a", "b", "c", "d", "e"])), "a, b, c and 2 more");
}

#[test]
fn no_names_give_an_empty_summary() {
    assert_eq!(offer_summary(&[]), "");
}
