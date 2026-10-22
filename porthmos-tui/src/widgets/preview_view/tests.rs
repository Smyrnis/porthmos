use std::sync::Arc;

use porthmos_core::preview::{Preview, PreviewContent};
use ratatui::{Terminal, backend::TestBackend};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn text_preview(content: &str) -> Arc<Preview> {
    Arc::new(Preview {
        size: Some(content.len() as u64),
        shown: content.len(),
        truncated: false,
        content: PreviewContent::Text(content.to_string()),
    })
}

fn binary_preview(bytes: &[u8]) -> Arc<Preview> {
    Arc::new(Preview {
        size: Some(bytes.len() as u64),
        shown: bytes.len(),
        truncated: false,
        content: PreviewContent::Binary(bytes.to_vec()),
    })
}

fn ready(name: &str, preview: Arc<Preview>) -> PreviewView {
    let mut view = PreviewView::loading(name.to_string());
    view.finish(Ok(preview));
    view
}

fn draw(view: &PreviewView, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_preview(frame, frame.area(), view, &theme)).unwrap();
    terminal
}

fn lines(n: usize) -> String {
    (1..=n).map(|index| format!("line {index}\n")).collect()
}

#[test]
fn while_loading_the_title_names_the_file_and_the_body_says_loading() {
    let view = PreviewView::loading("notes.txt".to_string());

    let rows = screen_rows(&draw(&view, 80, 12));

    assert!(rows[0].starts_with("preview \u{b7} notes.txt"), "{:?}", rows[0]);
    assert!(rows.iter().any(|row| row.contains("loading")), "{rows:?}");
    assert!(rows[11].contains("esc close"), "{:?}", rows[11]);
}

#[test]
fn a_failure_is_shown_in_the_body_in_the_error_colour_line_by_line() {
    let theme = indexed();
    let mut view = PreviewView::loading("a.txt".to_string());
    view.finish(Err("Couldn't preview a.txt:\nPermission denied".to_string()));

    let terminal = draw(&view, 80, 12);

    let (x, y) = find_text(&terminal, "Permission denied").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.err);
    assert!(find_text(&terminal, "Couldn't preview a.txt:").is_some());
}

#[test]
fn text_is_drawn_with_a_right_aligned_dim_line_number_gutter() {
    let theme = indexed();
    let view = ready("a.txt", text_preview(&lines(12)));

    let terminal = draw(&view, 80, 20);

    let rows = screen_rows(&terminal);
    assert!(rows[2].starts_with(" 1  line 1"), "{:?}", rows[2]);
    assert!(rows[11].starts_with("10  line 10"), "{:?}", rows[11]);
    let (x, y) = find_text(&terminal, "line 3").unwrap();
    assert_eq!(cell_fg(&terminal, 0, y), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
}

#[test]
fn the_title_row_tells_what_kind_of_file_and_how_big() {
    let view = ready("a.txt", text_preview("hello"));

    let rows = screen_rows(&draw(&view, 80, 12));

    assert!(rows[0].trim_end().ends_with("text \u{b7} 5 B"), "{:?}", rows[0]);
}

#[test]
fn a_cut_file_says_how_much_of_it_is_shown() {
    let preview = Arc::new(Preview {
        size: Some(3_600_000),
        shown: 262_144,
        truncated: true,
        content: PreviewContent::Text("x".to_string()),
    });
    let view = ready("big.log", preview);

    let rows = screen_rows(&draw(&view, 80, 12));

    assert!(rows[0].trim_end().ends_with("first 256.0K of 3.4M"), "{:?}", rows[0]);
}

#[test]
fn an_empty_file_says_so() {
    let view = ready("empty", text_preview(""));

    let rows = screen_rows(&draw(&view, 80, 12));

    assert!(rows.iter().any(|row| row.contains("(empty file)")), "{rows:?}");
}

#[test]
fn tabs_expand_to_the_next_multiple_of_four_columns() {
    let view = ready("a", text_preview("a\tb\n\tc\nabcd\te\n"));

    let rows = screen_rows(&draw(&view, 80, 12));

    assert!(rows[2].contains("1  a   b"), "{:?}", rows[2]);
    assert!(rows[3].contains("2      c"), "{:?}", rows[3]);
    assert!(rows[4].contains("3  abcd    e"), "{:?}", rows[4]);
}

#[test]
fn carriage_returns_at_the_end_of_a_line_are_dropped_and_the_final_newline_makes_no_extra_line() {
    let view = ready("a", text_preview("one\r\ntwo\r\n"));

    let rows = screen_rows(&draw(&view, 80, 12));

    assert!(rows[2].contains("1  one") && !rows[2].contains('\r'), "{:?}", rows[2]);
    assert!(rows[3].contains("2  two"), "{:?}", rows[3]);
    assert!(rows[4].trim().is_empty(), "{:?}", rows[4]);
}

#[test]
fn escape_sequences_and_bidi_controls_in_the_text_never_reach_the_screen() {
    let view = ready("a", text_preview("\u{1b}[2J\u{1b}]0;pwned\u{7}red\u{202e}gnp\nbell\u{7}\u{8}\u{0}\n"));

    let terminal = draw(&view, 80, 12);

    assert!(!has_unprintable_cell(&terminal));
    assert!(find_text(&terminal, "red").is_some());
}

#[test]
fn a_hostile_file_name_is_neutralised_in_the_title() {
    let view = PreviewView::loading("we\u{1b}[31mird\u{202e}".to_string());

    assert!(!has_unprintable_cell(&draw(&view, 80, 12)));
}

#[test]
fn a_long_line_is_cut_with_an_ellipsis_when_not_wrapping() {
    let view = ready("a", text_preview(&format!("{}\n", "x".repeat(200))));

    let rows = screen_rows(&draw(&view, 40, 12));

    assert!(rows[2].trim_end().ends_with('\u{2026}'), "{:?}", rows[2]);
    assert_eq!(rows[2].trim_end().width(), 40);
}

#[test]
fn wrapping_continues_a_long_line_on_the_next_rows_with_a_blank_gutter() {
    let mut view = ready("a", text_preview(&format!("{}\nshort\n", "x".repeat(100))));
    view.toggle_wrap();

    let rows = screen_rows(&draw(&view, 43, 12));

    assert!(rows[2].starts_with("1  xxxx"), "{:?}", rows[2]);
    assert!(rows[3].starts_with("   xxxx"), "{:?}", rows[3]);
    assert!(rows[4].starts_with("   xxxx"), "{:?}", rows[4]);
    assert!(rows[5].starts_with("2  short"), "{:?}", rows[5]);
    let shown: usize = rows[2..5].iter().map(|row| row.matches('x').count()).sum();
    assert_eq!(shown, 100);
}

#[test]
fn wrapping_counts_wide_characters_by_their_display_width() {
    let mut view = ready("a", text_preview(&"\u{4e2d}".repeat(30)));
    view.toggle_wrap();

    let terminal = draw(&view, 23, 12);

    for row in screen_rows(&terminal) {
        assert!(row.matches('\u{4e2d}').count() <= 10, "{row:?}");
    }
    let total: usize = screen_rows(&terminal).iter().map(|row| row.matches('\u{4e2d}').count()).sum();
    assert_eq!(total, 30);
}

#[test]
fn the_wrap_toggle_flips_and_goes_back() {
    let mut view = ready("a", text_preview("x"));

    assert!(!view.wrap());
    view.toggle_wrap();
    assert!(view.wrap());
    view.toggle_wrap();
    assert!(!view.wrap());
}

#[test]
fn scrolling_moves_the_window_and_stops_at_both_ends() {
    let mut view = ready("a", text_preview(&lines(100)));
    draw(&view, 80, 12);

    view.scroll(3);
    assert_eq!(view.top(), 3);
    view.scroll(-10);
    assert_eq!(view.top(), 0);
    view.scroll(1000);
    assert_eq!(view.top(), 91);
    view.scroll(1);
    assert_eq!(view.top(), 91);
}

#[test]
fn a_page_is_the_body_height_less_one_row_of_overlap() {
    let mut view = ready("a", text_preview(&lines(100)));
    draw(&view, 80, 12);

    view.page(1);
    assert_eq!(view.top(), 8);
    view.page(1);
    assert_eq!(view.top(), 16);
    view.page(-1);
    assert_eq!(view.top(), 8);
}

#[test]
fn top_and_bottom_jump_to_the_ends_and_the_last_line_is_visible_at_the_bottom() {
    let mut view = ready("a", text_preview(&lines(100)));
    draw(&view, 80, 12);

    view.go_bottom();
    let rows = screen_rows(&draw(&view, 80, 12));
    assert!(rows[10].contains("line 100"), "{:?}", rows[10]);
    view.go_top();
    assert_eq!(view.top(), 0);
}

#[test]
fn a_file_shorter_than_the_screen_cannot_scroll() {
    let mut view = ready("a", text_preview(&lines(3)));
    draw(&view, 80, 20);

    view.scroll(5);
    view.go_bottom();

    assert_eq!(view.top(), 0);
}

#[test]
fn the_footer_shows_how_far_down_the_file_you_are() {
    let mut view = ready("a", text_preview(&lines(100)));
    let top = screen_rows(&draw(&view, 80, 12));
    view.go_bottom();
    let bottom = screen_rows(&draw(&view, 80, 12));

    assert!(top[11].trim_end().ends_with("9%"), "{:?}", top[11]);
    assert!(bottom[11].trim_end().ends_with("100%"), "{:?}", bottom[11]);
}

#[test]
fn the_hex_row_matches_the_classic_dump_layout() {
    let bytes = b"Hello world\n\x00\x01\x02\xff";

    assert_eq!(hex_row(0, bytes), "00000000  48 65 6c 6c 6f 20 77 6f  72 6c 64 0a 00 01 02 ff  |Hello world.....|");
}

#[test]
fn a_short_last_row_is_padded_so_the_ascii_column_lines_up() {
    let full = hex_row(0x10, b"0123456789abcdef");
    let short = hex_row(0x20, b"xyz");

    assert_eq!(short, "00000020  78 79 7a                                          |xyz|");
    assert_eq!(full.find('|'), short.find('|'));
}

#[test]
fn a_binary_file_is_drawn_as_a_hex_dump_with_offsets() {
    let view = ready("a.bin", binary_preview(&(0..=255u8).collect::<Vec<u8>>()));

    let rows = screen_rows(&draw(&view, 90, 12));

    assert!(rows[0].trim_end().ends_with("binary \u{b7} 256 B"), "{:?}", rows[0]);
    assert!(rows[2].starts_with("00000000  00 01 02 03"), "{:?}", rows[2]);
    assert!(rows[3].starts_with("00000010  10 11 12 13"), "{:?}", rows[3]);
}

#[test]
fn the_hex_dump_scrolls_by_rows_and_the_wrap_key_does_nothing_for_it() {
    let mut view = ready("a.bin", binary_preview(&vec![7u8; 16 * 100]));
    draw(&view, 90, 12);

    view.scroll(5);
    view.toggle_wrap();
    let rows = screen_rows(&draw(&view, 90, 12));

    assert!(rows[2].starts_with("00000050"), "{:?}", rows[2]);
}

#[test]
fn a_hex_row_cut_by_a_narrow_terminal_never_overflows() {
    let view = ready("a.bin", binary_preview(&[7u8; 64]));

    for row in screen_rows(&draw(&view, 40, 12)) {
        assert!(row.width() <= 40, "{row:?}");
    }
}

#[test]
fn no_size_of_terminal_panics() {
    let text = ready("a\tb", text_preview(&format!("{}\n\u{4e2d}\u{6587}\t{}\n", "y".repeat(500), lines(50))));
    let binary = ready("b", binary_preview(&vec![1u8; 1000]));
    let loading = PreviewView::loading("c".to_string());
    let mut failed = PreviewView::loading("d".to_string());
    failed.finish(Err("boom\nline two".to_string()));
    let mut wrapped = ready("e", text_preview(&lines(30)));
    wrapped.toggle_wrap();

    for view in [&text, &binary, &loading, &failed, &wrapped] {
        for width in 0..=70 {
            for height in 0..=14 {
                let _ = draw(view, width, height);
            }
        }
    }
}

fn long_lines(count: usize) -> String {
    (1..=count).map(|index| format!("L{index} {}\n", "w".repeat(200))).collect()
}

fn first_body_line(terminal: &Terminal<TestBackend>) -> String {
    screen_rows(terminal)[2].clone()
}

#[test]
fn nothing_of_the_screen_underneath_shows_through_the_viewer() {
    let theme = indexed();
    let view = ready("a.txt", text_preview("short\n"));
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();

    terminal
        .draw(|frame| {
            let junk = ["JUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNKJUNK"; 12];
            frame.render_widget(ratatui::widgets::Paragraph::new(junk.join("\n")), frame.area());
            render_preview(frame, frame.area(), &view, &theme);
        })
        .unwrap();

    for row in screen_rows(&terminal) {
        assert!(!row.contains("JUNK") && !row.contains('J'), "{row:?}");
    }
}

#[test]
fn while_loading_or_failed_the_footer_row_is_clean_too() {
    let theme = indexed();
    let loading = PreviewView::loading("a".to_string());
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();

    terminal
        .draw(|frame| {
            let junk = ["XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"; 8];
            frame.render_widget(ratatui::widgets::Paragraph::new(junk.join("\n")), frame.area());
            render_preview(frame, frame.area(), &loading, &theme);
        })
        .unwrap();

    assert!(screen_rows(&terminal).iter().all(|row| !row.contains('X')));
}

#[test]
fn the_text_is_processed_once_and_the_layout_only_again_when_the_width_or_wrap_changes() {
    let mut view = ready("a", text_preview(&lines(500)));

    for _ in 0..5 {
        draw(&view, 80, 12);
    }
    assert_eq!(view.layouts(), 1);

    draw(&view, 90, 12);
    assert_eq!(view.layouts(), 2);
    view.scroll(3);
    draw(&view, 90, 12);
    assert_eq!(view.layouts(), 2);
    view.toggle_wrap();
    draw(&view, 90, 12);
    assert_eq!(view.layouts(), 3);
}

#[test]
fn wrapping_keeps_the_line_you_were_reading_in_view() {
    let mut view = ready("a", text_preview(&long_lines(1000)));
    draw(&view, 80, 12);
    view.scroll(899);
    assert!(first_body_line(&draw(&view, 80, 12)).contains("L900 "));

    view.toggle_wrap();
    let wrapped = draw(&view, 80, 12);

    assert!(first_body_line(&wrapped).contains("L900 "), "{:?}", first_body_line(&wrapped));
    view.toggle_wrap();
    let back = draw(&view, 80, 12);
    assert!(first_body_line(&back).contains("L900 "), "{:?}", first_body_line(&back));
}

#[test]
fn wrapping_while_at_the_bottom_stays_at_the_bottom() {
    let mut view = ready("a", text_preview(&long_lines(300)));
    draw(&view, 80, 12);
    view.go_bottom();
    draw(&view, 80, 12);

    view.toggle_wrap();
    let wrapped = screen_rows(&draw(&view, 80, 12));

    assert!(wrapped[10].contains('w'), "{:?}", wrapped[10]);
    assert!(wrapped.iter().any(|row| row.contains("100%")), "{wrapped:?}");
}

#[test]
fn a_resize_keeps_the_line_you_were_reading_in_view() {
    let mut view = ready("a", text_preview(&long_lines(1000)));
    view.toggle_wrap();
    draw(&view, 80, 12);
    view.scroll(2000);
    let before = first_body_line(&draw(&view, 80, 12));
    let line: String = before.split_whitespace().next().unwrap().to_string();

    let after = first_body_line(&draw(&view, 120, 12));

    assert!(after.contains(&line) || after.trim_start().starts_with('w'), "{before:?} {after:?}");
}

#[test]
fn text_lines_replace_every_control_format_and_separator_character() {
    let processed = text_lines(
        "\u{1b}[2J\u{1b}]0;x\u{7}a\u{202e}b\u{2028}c\u{2029}d\u{9b}e\u{0}f\u{200b}g\u{2060}h\u{feff}i\u{7f}j",
    );

    assert_eq!(processed, ["?[2J?]0;x?a?b?c?d?e?f?g?h?i?j"]);
    for line in &processed {
        assert!(line.chars().all(|c| !c.is_control()));
    }
}

#[test]
fn zero_width_joiners_and_variation_selectors_are_kept_so_emoji_still_work() {
    let processed = text_lines("\u{2764}\u{fe0f} \u{1f469}\u{200d}\u{1f469}");

    assert_eq!(processed, ["\u{2764}\u{fe0f} \u{1f469}\u{200d}\u{1f469}"]);
}

#[test]
fn emoji_with_a_variation_selector_count_as_two_cells_when_wrapping() {
    let mut view = ready("a", text_preview(&"\u{2764}\u{fe0f}".repeat(100)));
    view.toggle_wrap();

    let terminal = draw(&view, 43, 14);

    let hearts: Vec<usize> = screen_rows(&terminal).iter().map(|row| row.matches('\u{2764}').count()).collect();
    assert!(hearts.iter().all(|count| *count <= 20), "{hearts:?}");
    assert_eq!(hearts.iter().sum::<usize>(), 100);
}

#[test]
fn a_cut_line_of_emoji_still_ends_with_the_ellipsis_inside_the_width() {
    let view = ready("a", text_preview(&"\u{2764}\u{fe0f}".repeat(100)));

    let rows = screen_rows(&draw(&view, 30, 12));

    assert!(rows[2].trim_end().ends_with('\u{2026}'), "{:?}", rows[2]);
}

#[test]
fn the_footer_of_a_binary_file_leaves_out_the_wrap_hint_and_a_text_file_has_it() {
    let binary = screen_rows(&draw(&ready("a.bin", binary_preview(&[1u8; 40])), 90, 12));
    let text = screen_rows(&draw(&ready("a.txt", text_preview("x")), 90, 12));

    assert!(!binary[11].contains("wrap"), "{:?}", binary[11]);
    assert!(text[11].contains("w wrap"), "{:?}", text[11]);
}

#[test]
fn a_size_the_server_did_not_tell_is_left_out_of_the_header() {
    let preview = Arc::new(Preview {
        size: None,
        shown: 262_144,
        truncated: true,
        content: PreviewContent::Text("x".to_string()),
    });
    let unknown_whole = Arc::new(Preview {
        size: None,
        shown: 5,
        truncated: false,
        content: PreviewContent::Text("hello".to_string()),
    });

    let cut = screen_rows(&draw(&ready("a", preview), 80, 12));
    let whole = screen_rows(&draw(&ready("a", unknown_whole), 80, 12));

    assert!(cut[0].trim_end().ends_with("first 256.0K"), "{:?}", cut[0]);
    assert!(whole[0].trim_end().ends_with("text \u{b7} 5 B"), "{:?}", whole[0]);
}
