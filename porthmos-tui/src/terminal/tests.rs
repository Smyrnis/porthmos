use std::io;

use crossterm::event::KeyboardEnhancementFlags;

use super::*;

#[test]
fn the_flags_ask_only_for_disambiguation() {
    assert_eq!(KEYBOARD_FLAGS, KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES);
}

#[test]
fn enhancement_is_wanted_only_when_the_terminal_says_it_is_supported() {
    assert!(enhancement_wanted(Ok(true)));
    assert!(!enhancement_wanted(Ok(false)));
    assert!(!enhancement_wanted(Err(io::Error::other("no answer"))));
}

#[test]
fn the_support_question_is_put_to_the_terminal_only_once() {
    let cell = std::sync::OnceLock::new();
    let mut asked = 0;

    let first = cached_support(&cell, || {
        asked += 1;
        Ok(true)
    });
    let second = cached_support(&cell, || {
        asked += 1;
        Ok(false)
    });

    assert!(first && second);
    assert_eq!(asked, 1);
}

#[test]
fn an_unanswered_support_question_is_remembered_as_not_supported() {
    let cell = std::sync::OnceLock::new();

    assert!(!cached_support(&cell, || Err(io::Error::other("no answer"))));
    assert!(!cached_support(&cell, || Ok(true)));
}
