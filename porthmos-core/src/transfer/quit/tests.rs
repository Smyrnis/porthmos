use super::QUIT_WITH_TRANSFERS_TEXT;

#[test]
fn the_text_says_the_transfers_stop_now() {
    let text = QUIT_WITH_TRANSFERS_TEXT.to_lowercase();

    assert!(text.contains("stop now"), "{text}");
}

#[test]
fn the_text_promises_that_the_unfinished_transfers_come_back_next_time() {
    assert!(QUIT_WITH_TRANSFERS_TEXT.contains("come back next time"), "{QUIT_WITH_TRANSFERS_TEXT}");
}

#[test]
fn the_text_tells_how_to_continue_them() {
    assert!(QUIT_WITH_TRANSFERS_TEXT.contains("retry"), "{QUIT_WITH_TRANSFERS_TEXT}");
    assert!(QUIT_WITH_TRANSFERS_TEXT.contains("connect"), "{QUIT_WITH_TRANSFERS_TEXT}");
}

#[test]
fn the_text_no_longer_says_they_are_lost() {
    let text = QUIT_WITH_TRANSFERS_TEXT.to_lowercase();

    assert!(!text.contains("not restored"), "{text}");
    assert!(!text.contains("copy the same file again"), "{text}");
}
