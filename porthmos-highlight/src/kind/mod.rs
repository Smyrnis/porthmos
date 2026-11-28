use std::sync::OnceLock;

use syntect::parsing::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Plain,
    Comment,
    String,
    Number,
    Keyword,
    Type,
    Function,
    Constant,
    Operator,
    Punctuation,
    Error,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Highlighted {
    pub lines: Vec<Vec<Span>>,
}

const SCOPE_KINDS: [(&str, TokenKind); 22] = [
    ("comment", TokenKind::Comment),
    ("string", TokenKind::String),
    ("constant.numeric", TokenKind::Number),
    ("constant", TokenKind::Constant),
    ("keyword.operator", TokenKind::Operator),
    ("entity.name.function", TokenKind::Function),
    ("support.function", TokenKind::Function),
    ("variable.function", TokenKind::Function),
    ("entity.name.type", TokenKind::Type),
    ("entity.name.class", TokenKind::Type),
    ("support.type", TokenKind::Type),
    ("support.class", TokenKind::Type),
    ("entity.name.struct", TokenKind::Type),
    ("entity.name.enum", TokenKind::Type),
    ("entity.name.trait", TokenKind::Type),
    ("entity.name.interface", TokenKind::Type),
    ("entity.name.union", TokenKind::Type),
    ("keyword", TokenKind::Keyword),
    ("storage", TokenKind::Keyword),
    ("invalid", TokenKind::Error),
    ("markup.deleted", TokenKind::Error),
    ("markup.changed", TokenKind::Warn),
];

struct Table {
    rows: Vec<(Scope, TokenKind)>,
    punctuation: Scope,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| Table {
        rows: SCOPE_KINDS.iter().filter_map(|(name, kind)| Scope::new(name).ok().map(|scope| (scope, *kind))).collect(),
        punctuation: Scope::new("punctuation").expect("a valid scope name"),
    })
}

pub(crate) fn kind_of_scopes(scopes: &[Scope]) -> TokenKind {
    let table = table();
    for scope in scopes.iter().rev() {
        if let Some((_, kind)) = table.rows.iter().find(|(prefix, _)| prefix.is_prefix_of(*scope)) {
            return *kind;
        }
    }
    if scopes.iter().any(|scope| table.punctuation.is_prefix_of(*scope)) {
        return TokenKind::Punctuation;
    }
    TokenKind::Plain
}

#[cfg(test)]
mod tests;
