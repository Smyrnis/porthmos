use crate::bookmarks::BookmarksState;
use crate::history_view::HistoryState;
use crate::preview_view::PreviewState;
use crate::search::SearchState;

#[derive(Debug)]
pub enum Tool {
    Search(SearchState),
    Bookmarks(BookmarksState),
    History(HistoryState),
    Preview(PreviewState),
}
