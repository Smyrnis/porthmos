use unicode_width::UnicodeWidthStr;

use crate::widgets::text_fit::truncate_start_with_ellipsis;

pub trait FilterLine {
    fn filter(&self) -> Option<&str>;
    fn editing_filter(&self) -> bool;
    fn start_filter(&mut self);
    fn finish_filter(&mut self);
    fn type_filter(&mut self, character: char);
    fn erase_filter(&mut self);
    fn clear_filter(&mut self);
    fn move_cursor(&mut self, delta: isize);
}

pub fn status(editing: bool, text: Option<&str>, matched: usize, total: usize, width: usize) -> Option<String> {
    let (prefix, text, suffix) = match (editing, text) {
        (true, text) => ("/", text.unwrap_or_default(), format!("\u{2588} ({matched} of {total})")),
        (false, Some(text)) => ("filter: ", text, format!(" ({matched} of {total})")),
        (false, None) => return None,
    };
    let room = width.saturating_sub(prefix.width() + suffix.width());
    let shown = truncate_start_with_ellipsis(text, room);
    Some(format!("{prefix}{shown}{suffix}"))
}

#[cfg(test)]
mod tests;
