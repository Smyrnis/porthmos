use super::*;

#[test]
fn permission_problems_point_at_permissions_and_the_admin() {
    assert_eq!(
        advice_for("permission denied \u{2014} user can't read /var/log/x"),
        Some("check the file's permissions or ask the server admin for access")
    );
}

#[test]
fn full_destinations_ask_for_space() {
    let expected = Some("free some space on the destination and retry");

    assert_eq!(advice_for("No space left on device"), expected);
    assert_eq!(advice_for("disk quota exceeded"), expected);
}

#[test]
fn missing_files_suggest_refreshing() {
    let expected = Some("the file was moved or deleted; refresh the listing");

    assert_eq!(advice_for("No such file or directory"), expected);
    assert_eq!(advice_for("path not found"), expected);
}

#[test]
fn connection_problems_suggest_checking_the_connection() {
    let expected = Some("check the connection, then retry");

    for message in ["operation timed out", "Connection reset by peer", "broken pipe", "connection refused"] {
        assert_eq!(advice_for(message), expected, "{message}");
    }
}

#[test]
fn matching_is_case_insensitive() {
    assert!(advice_for("PERMISSION DENIED").is_some());
}

#[test]
fn unknown_and_empty_messages_have_no_advice() {
    assert_eq!(advice_for("something odd happened"), None);
    assert_eq!(advice_for(""), None);
}

#[test]
fn a_disconnected_session_suggests_checking_the_connection() {
    assert_eq!(advice_for("session disconnected"), Some("check the connection, then retry"));
}
