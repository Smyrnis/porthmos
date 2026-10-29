use unicode_width::UnicodeWidthStr;

use super::fit;

#[test]
fn a_short_name_is_unchanged() {
    assert_eq!(fit("notes.txt", 20), "notes.txt");
}

#[test]
fn a_name_with_a_newline_is_shown_on_one_line() {
    assert_eq!(fit("first\nsecond", 20), "first·second");
    assert_eq!(fit("a\r\nb", 20), "a··b");
}

#[test]
fn control_characters_are_replaced() {
    assert_eq!(fit("\t\u{7}\u{1b}x\u{7f}", 20), "···x·");
}

#[test]
fn direction_overrides_cannot_reorder_the_rest_of_the_row() {
    assert_eq!(fit("fdp.\u{202e}txt", 20), "fdp.·txt");
    assert_eq!(fit("a\u{2066}b\u{2069}c\u{200f}", 20), "a·b·c·");
}

#[test]
fn a_long_name_is_cut_with_an_ellipsis_at_the_end() {
    assert_eq!(fit("abcdefghij", 6), "abcde…");
    assert_eq!(fit("abcdef", 6), "abcdef");
}

#[test]
fn a_budget_of_zero_or_one_leaves_nothing_or_an_ellipsis() {
    assert_eq!(fit("abc", 0), "");
    assert_eq!(fit("abc", 1), "…");
    assert_eq!(fit("a", 1), "a");
}

#[test]
fn double_width_names_are_cut_by_cells_not_characters() {
    assert_eq!(fit("文件文件文件", 5), "文件…");
    assert_eq!(fit("文件", 4), "文件");
}

#[test]
fn wide_and_emoji_names_never_exceed_the_budget() {
    let names = [
        "ünï-文件 with spaces.txt",
        "🙂🙂🙂🙂🙂🙂🙂🙂",
        "❤️❤️❤️❤️❤️❤️",
        "👨‍👩‍👧 family photo",
        "שלום עולם שלום עולם",
        "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}",
        "ｆｕｌｌｗｉｄｔｈ　ｔｅｘｔ",
    ];
    for name in names {
        for budget in 0..=24 {
            let shown = fit(name, budget);
            assert!(shown.width() <= budget, "{name:?} at {budget} gave {shown:?} ({})", shown.width());
            assert!(!shown.contains('\n'));
        }
    }
}

#[test]
fn a_very_long_name_is_cut_quickly() {
    let name = "x".repeat(100_000);
    let started = std::time::Instant::now();

    let shown = fit(&name, 30);

    assert_eq!(shown.width(), 30);
    assert!(started.elapsed() < std::time::Duration::from_millis(200));
}
