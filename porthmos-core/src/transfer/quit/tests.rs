use super::quit_with_transfers_text;

#[test]
fn the_text_says_the_transfers_stop_now() {
    let text = quit_with_transfers_text().to_lowercase();

    assert!(text.contains("stop now"), "{text}");
}

#[test]
fn the_text_promises_that_the_unfinished_transfers_come_back_next_time() {
    let text = quit_with_transfers_text();
    assert!(text.contains("come back next time"), "{text}");
}

#[test]
fn the_text_tells_how_to_continue_them() {
    let text = quit_with_transfers_text();
    assert!(text.contains("retry"), "{text}");
    assert!(text.contains("connect"), "{text}");
}

#[test]
fn the_text_no_longer_says_they_are_lost() {
    let text = quit_with_transfers_text().to_lowercase();

    assert!(!text.contains("not restored"), "{text}");
    assert!(!text.contains("copy the same file again"), "{text}");
}
