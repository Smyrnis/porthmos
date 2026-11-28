use iced::Size;
use porthmos_core::TokenKind;

use crate::geometry::ROW_HEIGHT as LIST_ROW_HEIGHT;
use crate::preview_view::Piece;
use crate::preview_view::PreviewState;
use crate::testing::Harness;
use crate::theme::Palette;
use crate::tools::Tool;
use crate::view::preview::{kind_color, single_plain};
use crate::view::tools::{PREVIEW_CHROME, PREVIEW_HEIGHT, PREVIEW_WIDTH, frame_size, list_height, overlay};

fn loading_tool() -> Tool {
    Tool::Preview(PreviewState::loading(1, "a.txt".to_string()))
}

#[test]
fn the_preview_frame_is_large_and_shrinks_with_the_window() {
    assert_eq!(frame_size(&loading_tool(), Size::new(1400.0, 900.0)), Size::new(PREVIEW_WIDTH, PREVIEW_HEIGHT));
    assert_eq!(frame_size(&loading_tool(), Size::new(480.0, 320.0)), Size::new(448.0, 288.0));
}

#[test]
fn the_body_leaves_room_for_the_header_and_buttons() {
    assert_eq!(list_height(&loading_tool(), Size::new(1400.0, 900.0)), PREVIEW_HEIGHT - PREVIEW_CHROME);
    assert_eq!(list_height(&loading_tool(), Size::new(10.0, 10.0)), LIST_ROW_HEIGHT);
}

#[test]
fn the_overlay_is_present_while_a_preview_loads() {
    let mut harness = Harness::new();
    let folder = harness.start_folder();
    let entry = porthmos_core::Entry { name: "a.txt".to_string(), path: folder.join("a.txt"), ..Default::default() };
    harness.feed(porthmos_core::Event::Listed {
        location: porthmos_core::Location::Local,
        path: folder,
        entries: vec![entry],
    });
    harness.press(crate::app::Message::PanelClicked(crate::app::PanelSide::Local, 1));
    harness.press(crate::app::Message::MoreItemChosen(crate::menus::MoreItem::Preview));

    assert!(overlay(&harness.app).is_some());
}

#[test]
fn a_preview_failure_keeps_its_lines_but_loses_control_characters() {
    let message = porthmos_core::Message::with_cause("Could not preview", &"\u{1b}[2Jgone");

    let text = super::failure_text(&message);

    assert_eq!(text, "Could not preview:\n?[2Jgone");
}

const ALL_KINDS: [TokenKind; 12] = [
    TokenKind::Plain,
    TokenKind::Comment,
    TokenKind::String,
    TokenKind::Number,
    TokenKind::Keyword,
    TokenKind::Type,
    TokenKind::Function,
    TokenKind::Constant,
    TokenKind::Operator,
    TokenKind::Punctuation,
    TokenKind::Error,
    TokenKind::Warn,
];

fn dark_palette() -> Palette {
    Palette::new(&crate::theme::ThemeSettings::default())
}

#[test]
fn every_token_kind_has_the_colour_of_its_role() {
    let palette = dark_palette();
    let expected = [
        (TokenKind::Plain, palette.fg),
        (TokenKind::Operator, palette.fg),
        (TokenKind::Punctuation, palette.fg),
        (TokenKind::Comment, palette.fg_dim),
        (TokenKind::String, palette.syntax_string),
        (TokenKind::Number, palette.syntax_number),
        (TokenKind::Constant, palette.syntax_number),
        (TokenKind::Keyword, palette.syntax_keyword),
        (TokenKind::Type, palette.syntax_type),
        (TokenKind::Function, palette.syntax_function),
        (TokenKind::Error, palette.err),
        (TokenKind::Warn, palette.warn),
    ];

    assert_eq!(expected.len(), ALL_KINDS.len());
    for (kind, colour) in expected {
        assert_eq!(kind_color(kind, &palette), colour, "{kind:?}");
    }
}

#[test]
fn the_coloured_kinds_differ_from_plain_text() {
    let palette = dark_palette();

    for kind in [TokenKind::Comment, TokenKind::String, TokenKind::Keyword, TokenKind::Error, TokenKind::Warn] {
        assert_ne!(kind_color(kind, &palette), kind_color(TokenKind::Plain, &palette), "{kind:?}");
    }
}

fn piece(text: &str, kind: TokenKind) -> Piece {
    Piece { text: text.to_string(), kind }
}

#[test]
fn a_row_of_one_plain_piece_takes_the_plain_text_path() {
    assert_eq!(single_plain(&[piece("hello", TokenKind::Plain)]), Some("hello"));
}

#[test]
fn a_coloured_or_split_row_takes_the_rich_text_path() {
    assert_eq!(single_plain(&[piece("fn", TokenKind::Keyword)]), None);
    assert_eq!(single_plain(&[piece("a", TokenKind::Plain), piece("b", TokenKind::Plain)]), None);
    assert_eq!(single_plain(&[]), None);
}
