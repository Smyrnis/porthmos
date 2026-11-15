use iced::Size;

use super::*;
use crate::connections::form::{FieldKind, FieldView};

fn field(label: &str, required: bool) -> FieldView {
    FieldView { key: "k".to_string(), label: label.to_string(), required, kind: FieldKind::Text(String::new()) }
}

#[test]
fn required_fields_get_a_star() {
    assert_eq!(field_label(&field("Host", true)), "Host *");
    assert_eq!(field_label(&field("Port", false)), "Port");
}

#[test]
fn the_dialog_keeps_its_size_in_a_large_window_and_shrinks_in_a_small_one() {
    assert_eq!(dialog_size(Size::new(1100.0, 700.0), FORM_WIDTH, FORM_HEIGHT), Size::new(FORM_WIDTH, FORM_HEIGHT));
    assert_eq!(dialog_size(Size::new(480.0, 320.0), FORM_WIDTH, FORM_HEIGHT), Size::new(448.0, 288.0));
    assert_eq!(dialog_size(Size::new(10.0, 10.0), FORM_WIDTH, FORM_HEIGHT), Size::new(0.0, 0.0));
}

#[test]
fn the_missing_host_title_says_why() {
    assert_eq!(missing_title("web", false), "web is no longer in ~/.ssh/config");
    assert_eq!(missing_title("web", true), "web is hidden by a saved connection");
}

#[test]
fn the_secret_placeholder_follows_the_state() {
    assert_eq!(secret_placeholder(true, false), "Saved. Type to replace");
    assert_eq!(secret_placeholder(true, true), "Will be removed when you save");
    assert_eq!(secret_placeholder(false, false), "");
}
