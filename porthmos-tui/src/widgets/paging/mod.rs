use std::cell::Cell;

use crate::widgets::filter_line::FilterLine;

const DEFAULT_PAGE_ROWS: usize = 10;

#[derive(Debug)]
pub struct PageRows(Cell<usize>);

impl Default for PageRows {
    fn default() -> Self {
        Self(Cell::new(DEFAULT_PAGE_ROWS))
    }
}

impl PageRows {
    pub fn get(&self) -> usize {
        self.0.get()
    }

    pub fn remember(&self, rows: usize) {
        self.0.set(rows.max(1));
    }
}

pub trait Paged: FilterLine {
    fn page_rows(&self) -> usize;
    fn row_count(&self) -> usize;

    fn page_down(&mut self) {
        self.move_cursor(self.page_rows() as isize);
    }

    fn page_up(&mut self) {
        self.move_cursor(-(self.page_rows() as isize));
    }

    fn cursor_to_first(&mut self) {
        self.move_cursor(-(self.row_count() as isize));
    }

    fn cursor_to_last(&mut self) {
        self.move_cursor(self.row_count() as isize);
    }
}

#[cfg(test)]
mod tests;
