use syntect::parsing::Scope;

use super::{TokenKind, kind_of_scopes};

fn kind(stack: &[&str]) -> TokenKind {
    let scopes: Vec<Scope> = stack.iter().map(|name| Scope::new(name).unwrap()).collect();
    kind_of_scopes(&scopes)
}

#[test]
fn every_scope_family_maps_to_its_kind() {
    let cases = [
        ("comment.line.double-slash.rust", TokenKind::Comment),
        ("string.quoted.double.rust", TokenKind::String),
        ("constant.numeric.integer.decimal", TokenKind::Number),
        ("constant.language.boolean", TokenKind::Constant),
        ("keyword.operator.arithmetic", TokenKind::Operator),
        ("keyword.control.rust", TokenKind::Keyword),
        ("storage.modifier.rust", TokenKind::Keyword),
        ("entity.name.function.rust", TokenKind::Function),
        ("support.function.builtin", TokenKind::Function),
        ("variable.function.shell", TokenKind::Function),
        ("entity.name.type.struct", TokenKind::Type),
        ("entity.name.class.python", TokenKind::Type),
        ("support.type.primitive", TokenKind::Type),
        ("support.class.builtin", TokenKind::Type),
        ("entity.name.struct.rust", TokenKind::Type),
        ("entity.name.enum.rust", TokenKind::Type),
        ("entity.name.trait.rust", TokenKind::Type),
        ("entity.name.interface.java", TokenKind::Type),
        ("entity.name.union.c", TokenKind::Type),
        ("punctuation.separator.comma", TokenKind::Punctuation),
        ("invalid.illegal.bad", TokenKind::Error),
        ("markup.deleted.diff", TokenKind::Error),
        ("markup.changed.diff", TokenKind::Warn),
    ];
    for (scope, expected) in cases {
        assert_eq!(kind(&[scope]), expected, "{scope}");
    }
}

#[test]
fn an_empty_scope_stack_is_plain() {
    assert_eq!(kind(&[]), TokenKind::Plain);
}

#[test]
fn an_unknown_scope_is_plain() {
    assert_eq!(kind(&["source.rust", "meta.block"]), TokenKind::Plain);
}

#[test]
fn an_inner_string_wins_over_an_outer_keyword() {
    assert_eq!(kind(&["keyword.control", "string.quoted"]), TokenKind::String);
}

#[test]
fn storage_type_is_a_keyword_because_grammars_use_it_for_fn_let_and_struct() {
    for scope in ["storage.type.function.rust", "storage.type.rust", "storage.type.struct.rust"] {
        assert_eq!(kind(&[scope]), TokenKind::Keyword, "{scope}");
    }
}

#[test]
fn a_keyword_inside_storage_type_wins() {
    assert_eq!(kind(&["storage.type.function.python", "keyword.declaration.function.python"]), TokenKind::Keyword);
}

#[test]
fn a_quote_mark_inside_a_string_is_still_a_string() {
    assert_eq!(kind(&["string.quoted.double", "punctuation.definition.string.begin"]), TokenKind::String);
}

#[test]
fn a_comment_marker_inside_a_comment_is_still_a_comment() {
    assert_eq!(kind(&["comment.line", "punctuation.definition.comment"]), TokenKind::Comment);
}
