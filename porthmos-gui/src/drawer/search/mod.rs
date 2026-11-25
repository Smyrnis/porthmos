use porthmos_core::i18n::label as tr;
use porthmos_term::{GridPoint, Match, SearchDirection};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SearchState {
    pub open: bool,
    pub query: String,
    pub missed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub columns: u16,
    pub rows: u16,
    pub visible_top: i32,
    pub first_line: i32,
}

impl Viewport {
    fn last_line(self) -> i32 {
        i32::from(self.rows.saturating_sub(1))
    }

    fn last_column(self) -> u16 {
        self.columns.saturating_sub(1)
    }
}

pub fn resume_point(last: Option<Match>, direction: SearchDirection, view: Viewport) -> GridPoint {
    let last_column = view.last_column();
    match (last, direction) {
        (None, SearchDirection::Forward) => GridPoint { line: view.visible_top, column: 0 },
        (None, SearchDirection::Backward) => {
            GridPoint { line: view.visible_top + view.last_line(), column: last_column }
        }
        (Some(found), SearchDirection::Forward) => {
            if found.end.column < last_column {
                GridPoint { line: found.end.line, column: found.end.column + 1 }
            } else if found.end.line >= view.last_line() {
                GridPoint { line: view.first_line, column: 0 }
            } else {
                GridPoint { line: found.end.line + 1, column: 0 }
            }
        }
        (Some(found), SearchDirection::Backward) => {
            if found.start.column > 0 {
                GridPoint { line: found.start.line, column: found.start.column - 1 }
            } else if found.start.line <= view.first_line {
                GridPoint { line: view.last_line(), column: last_column }
            } else {
                GridPoint { line: found.start.line - 1, column: last_column }
            }
        }
    }
}

pub fn search_status(missed: bool, query: &str) -> Option<String> {
    (missed && !query.is_empty()).then(|| tr("No match"))
}

#[cfg(test)]
mod tests;
