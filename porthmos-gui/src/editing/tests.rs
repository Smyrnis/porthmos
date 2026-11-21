use super::waiting_notice;

#[test]
fn waiting_notice_makes_names_printable() {
    assert_eq!(waiting_notice("a.txt"), "Editing a.txt. Save the file in your editor, then press Done.");
    let hostile = waiting_notice("evil\u{1b}[31m\nname");
    assert!(!hostile.contains('\u{1b}'));
    assert!(!hostile.contains('\n'));
}
