use unicode_width::UnicodeWidthStr;

use super::{fit, full_text_hint};

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

#[test]
fn a_cut_name_offers_its_whole_text_and_a_name_that_fits_offers_nothing() {
    assert_eq!(full_text_hint("abcdefghij", 6), Some("abcdefghij".to_string()));
    assert_eq!(full_text_hint("abcdef", 6), None);
    assert_eq!(full_text_hint("short", 20), None);
}

#[test]
fn the_offered_text_has_hostile_characters_replaced_like_the_shown_text() {
    assert_eq!(full_text_hint("fdp.\u{202e}txt-and-a-long-tail", 6), Some("fdp.·txt-and-a-long-tail".to_string()));
}

fn quadratic_fit(name: &str, max_cells: usize) -> String {
    if max_cells == 0 {
        return String::new();
    }
    let cleaned: String = name.chars().map(super::visible).collect();
    if cleaned.width() <= max_cells {
        return cleaned;
    }
    let mut shown = String::new();
    for character in cleaned.chars().take(max_cells.saturating_mul(2).saturating_add(2)) {
        shown.push(character);
        if shown.width() + 1 > max_cells {
            shown.pop();
            break;
        }
    }
    shown.push('…');
    shown
}

#[test]
fn the_cut_matches_the_cell_by_cell_cut_for_every_kind_of_text() {
    let names = [
        "abcdefghijklmnopqrstuvwxyz".to_string(),
        "καλημέρα κόσμε, καλημέρα".to_string(),
        "文件文件文件文件文件文件".to_string(),
        "🙂🙂🙂🙂🙂🙂🙂🙂".to_string(),
        "❤️❤️❤️❤️❤️❤️".to_string(),
        "👨‍👩‍👧 family 👨‍👩‍👧 photo 👨‍👩‍👧".to_string(),
        "שלום עולם שלום עולם".to_string(),
        "لا لا لا لا لا لا لا".to_string(),
        "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}".to_string(),
        "\u{301}".repeat(40),
        "ｆｕｌｌｗｉｄｔｈ　ｔｅｘｔ".to_string(),
        "a\tb\u{202e}c\nd".repeat(5),
        "x".repeat(1000),
    ];
    for name in &names {
        for budget in 0..=30 {
            assert_eq!(fit(name, budget), quadratic_fit(name, budget), "{name:?} at {budget}");
        }
    }
}

#[test]
fn a_long_wide_line_is_cut_in_linear_time() {
    let line = "καλημέρα κόσμε 文件 ".repeat(120);
    let started = std::time::Instant::now();

    for _ in 0..300 {
        assert!(fit(&line, 400).width() <= 400);
    }

    assert!(started.elapsed() < std::time::Duration::from_millis(700), "{:?}", started.elapsed());
}
