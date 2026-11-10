use super::*;

#[derive(Default)]
struct Rows {
    cursor: usize,
    count: usize,
    page: PageRows,
}

impl FilterLine for Rows {
    fn filter(&self) -> Option<&str> {
        None
    }

    fn editing_filter(&self) -> bool {
        false
    }

    fn start_filter(&mut self) {}

    fn finish_filter(&mut self) {}

    fn type_filter(&mut self, _character: char) {}

    fn erase_filter(&mut self) {}

    fn clear_filter(&mut self) {}

    fn move_cursor(&mut self, delta: isize) {
        if self.count == 0 {
            return;
        }
        self.cursor = (self.cursor as isize + delta).clamp(0, self.count as isize - 1) as usize;
    }
}

impl Paged for Rows {
    fn page_rows(&self) -> usize {
        self.page.get()
    }

    fn row_count(&self) -> usize {
        self.count
    }
}

fn rows(count: usize) -> Rows {
    Rows { count, ..Rows::default() }
}

#[test]
fn a_page_is_ten_rows_until_a_drawing_says_otherwise() {
    assert_eq!(PageRows::default().get(), 10);
}

#[test]
fn a_remembered_page_is_never_smaller_than_one_row() {
    let page = PageRows::default();

    page.remember(0);

    assert_eq!(page.get(), 1);
    page.remember(37);
    assert_eq!(page.get(), 37);
}

#[test]
fn paging_moves_by_the_remembered_rows_and_stops_at_the_ends() {
    let mut list = rows(25);
    list.page.remember(8);

    list.page_down();
    assert_eq!(list.cursor, 8);
    list.page_down();
    list.page_down();
    list.page_down();
    assert_eq!(list.cursor, 24);
    list.page_up();
    assert_eq!(list.cursor, 16);
    for _ in 0..5 {
        list.page_up();
    }
    assert_eq!(list.cursor, 0);
}

#[test]
fn first_and_last_jump_to_the_ends_from_anywhere() {
    let mut list = rows(300);
    list.cursor = 123;

    list.cursor_to_last();
    assert_eq!(list.cursor, 299);
    list.cursor_to_first();
    assert_eq!(list.cursor, 0);
}

#[test]
fn an_empty_list_stays_put() {
    let mut list = rows(0);

    list.page_down();
    list.page_up();
    list.cursor_to_last();
    list.cursor_to_first();

    assert_eq!(list.cursor, 0);
}

#[test]
fn a_single_row_list_stays_on_its_row() {
    let mut list = rows(1);

    list.cursor_to_last();
    list.page_down();

    assert_eq!(list.cursor, 0);
}
