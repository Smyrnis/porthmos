use super::*;

#[test]
fn ids_are_handed_out_in_order_starting_at_zero() {
    let mut book = SessionBook::default();

    assert_eq!((book.allocate_id(), book.allocate_id(), book.allocate_id()), (0, 1, 2));
}

#[test]
fn a_new_book_has_nothing_in_it() {
    let book = SessionBook::default();

    assert!(book.sessions.is_empty());
    assert!(book.lost_sessions.is_empty());
    assert!(book.connecting.is_empty());
}
