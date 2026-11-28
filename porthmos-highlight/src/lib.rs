mod engine;
mod kind;
mod lines;

pub use engine::{HIGHLIGHT_BUDGET, MAX_HIGHLIGHT_BYTES, MAX_HIGHLIGHT_LINE_BYTES, SyntaxId, highlight, syntax_for};
pub use kind::{Highlighted, Span, TokenKind};
pub use lines::lines_of;
