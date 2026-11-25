use super::*;

#[test]
fn the_first_take_gets_the_value_and_later_takes_get_none() {
    let handover = Handover::new(7);

    assert_eq!(handover.take(), Some(7));
    assert_eq!(handover.take(), None);
}

#[test]
fn a_clone_shares_the_value() {
    let handover = Handover::new(String::from("once"));
    let copy = handover.clone();

    assert_eq!(copy.take(), Some(String::from("once")));
    assert_eq!(handover.take(), None);
}

#[test]
fn debug_does_not_print_the_content() {
    let handover = Handover::new(String::from("secret-content"));

    assert!(!format!("{handover:?}").contains("secret-content"));
}
