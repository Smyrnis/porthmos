use std::{ops::BitOr, sync::Arc};

use crate::TermSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style(u8);

impl Style {
    pub const BOLD: Style = Style(1);
    pub const DIM: Style = Style(2);
    pub const ITALIC: Style = Style(4);
    pub const UNDERLINE: Style = Style(8);
    pub const INVERSE: Style = Style(16);
    pub const STRIKEOUT: Style = Style(32);
    pub const HIDDEN: Style = Style(64);

    pub fn contains(self, other: Style) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for Style {
    type Output = Style;

    fn bitor(self, other: Style) -> Style {
        Style(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellWidth {
    Single,
    Wide,
    Spacer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub character: char,
    pub combining: Option<Box<[char]>>,
    pub width: CellWidth,
    pub foreground: Color,
    pub background: Color,
    pub style: Style,
    pub hyperlink: Option<Arc<str>>,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: u16,
    pub column: u16,
    pub shape: CursorShape,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MouseMode {
    #[default]
    Off,
    Click,
    Drag,
    Motion,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modes {
    pub application_cursor: bool,
    pub application_keypad: bool,
    pub bracketed_paste: bool,
    pub mouse: MouseMode,
    pub mouse_sgr: bool,
    pub alternate_screen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub size: TermSize,
    pub cells: Vec<Cell>,
    pub cursor: Cursor,
    pub display_offset: usize,
    pub history: usize,
    pub modes: Modes,
}

impl Snapshot {
    pub fn row_text(&self, row: u16) -> String {
        let columns = usize::from(self.size.columns);
        let start = usize::from(row) * columns;
        let mut text = String::new();
        for cell in &self.cells[start..start + columns] {
            if cell.width == CellWidth::Spacer {
                continue;
            }
            text.push(cell.character);
            if let Some(marks) = &cell.combining {
                text.extend(marks.iter());
            }
        }
        text.truncate(text.trim_end_matches(' ').len());
        text
    }
}
