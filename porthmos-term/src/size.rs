#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub columns: u16,
    pub rows: u16,
}

impl TermSize {
    pub const MAX_COLUMNS: u16 = 512;
    pub const MAX_ROWS: u16 = 256;

    pub fn clamped(columns: u16, rows: u16) -> Self {
        Self { columns: columns.clamp(1, Self::MAX_COLUMNS), rows: rows.clamp(1, Self::MAX_ROWS) }
    }
}
