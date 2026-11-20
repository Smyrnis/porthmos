use super::*;

#[test]
fn a_plain_message_has_no_cause_and_displays_as_itself() {
    let message = Message::new("Disconnected from srv");

    assert_eq!(message.summary(), "Disconnected from srv");
    assert_eq!(message.cause(), None);
    assert_eq!(message.to_string(), "Disconnected from srv");
}

#[test]
fn a_message_with_a_cause_displays_the_summary_then_the_cause_on_the_next_line() {
    let message = Message::with_cause("Unable to connect to web", &"Connection refused");

    assert_eq!(message.summary(), "Unable to connect to web");
    assert_eq!(message.cause(), Some("Connection refused"));
    assert_eq!(message.to_string(), "Unable to connect to web:\nConnection refused");
}

#[test]
fn an_empty_cause_is_no_cause() {
    let message = Message::with_cause("Upload failed", &"");

    assert_eq!(message.cause(), None);
    assert_eq!(message.to_string(), "Upload failed");
}

#[test]
fn parsing_the_displayed_form_gives_back_the_summary_and_the_cause() {
    let original = Message::with_cause("Unable to list /srv", &"Permission denied");

    assert_eq!(Message::parse(&original.to_string()), original);
}

#[test]
fn parsing_text_without_a_line_break_is_a_plain_message() {
    assert_eq!(Message::parse("Authentication failed for web"), Message::new("Authentication failed for web"));
}

#[test]
fn parsing_splits_only_at_the_first_line_break() {
    let message = Message::parse("Sync failed:\nfirst\nsecond");

    assert_eq!(message.summary(), "Sync failed");
    assert_eq!(message.cause(), Some("first\nsecond"));
}

#[test]
fn the_one_line_form_joins_summary_and_cause_with_a_colon() {
    let message = Message::with_cause("Unable to connect to web", &"Connection refused");

    assert_eq!(message.one_line(), "Unable to connect to web: Connection refused");
    assert_eq!(Message::new("Done").one_line(), "Done");
}

#[test]
fn the_one_line_form_flattens_a_multi_line_cause() {
    let message = Message::parse("Sync failed:\nfirst\n  second");

    assert_eq!(message.one_line(), "Sync failed: first second");
}

#[test]
fn a_message_equals_its_displayed_text() {
    let message = Message::with_cause("Upload failed", &"disk full");

    assert_eq!(message, "Upload failed:\ndisk full");
    assert_eq!(message, "Upload failed:\ndisk full".to_string());
    assert!(message != "Upload failed");
}

#[test]
fn text_converts_into_a_plain_message() {
    let from_str: Message = "hello".into();
    let from_string: Message = "hello".to_string().into();

    assert_eq!(from_str, from_string);
    assert_eq!(from_str.cause(), None);
}

#[test]
fn text_in_the_displayed_form_is_split_when_converted() {
    let message: Message = "Unable to list /srv:\nPermission denied".to_string().into();

    assert_eq!(message.summary(), "Unable to list /srv");
    assert_eq!(message.cause(), Some("Permission denied"));
}

#[test]
fn a_saved_form_that_parse_would_rewrite_is_kept_as_written() {
    let saved = "550-no such file\n550 try later";

    assert_eq!(Message::from_display(saved).to_string(), saved);
}

#[test]
fn a_saved_form_that_parse_reads_back_exactly_is_split_into_summary_and_cause() {
    assert_eq!(
        Message::from_display("Could not copy a.txt:\ndisk full"),
        Message::with_cause("Could not copy a.txt", &"disk full")
    );
    assert_eq!(Message::from_display("Disconnected"), Message::new("Disconnected"));
}

#[test]
fn a_summary_with_a_line_break_comes_back_as_the_same_text() {
    let original = Message::with_cause("Transfer failed: a\nb", &"err");

    assert_eq!(Message::from_display(&original.to_string()).to_string(), original.to_string());
}
