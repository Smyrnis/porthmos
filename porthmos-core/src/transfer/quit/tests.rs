use super::QUIT_WITH_TRANSFERS_TEXT;

#[test]
fn the_text_says_the_transfers_stop_and_are_not_brought_back() {
    let text = QUIT_WITH_TRANSFERS_TEXT.to_lowercase();

    assert!(text.contains("stop"), "{text}");
    assert!(text.contains("not restored"), "{text}");
}

#[test]
fn the_text_tells_how_to_continue_a_stopped_transfer() {
    assert!(QUIT_WITH_TRANSFERS_TEXT.contains("copy the same file again"), "{QUIT_WITH_TRANSFERS_TEXT}");
    assert!(QUIT_WITH_TRANSFERS_TEXT.contains("Resume"), "{QUIT_WITH_TRANSFERS_TEXT}");
}

#[test]
fn the_text_no_longer_promises_that_the_queue_comes_back() {
    assert!(!QUIT_WITH_TRANSFERS_TEXT.contains("resumed next time"), "{QUIT_WITH_TRANSFERS_TEXT}");
}
