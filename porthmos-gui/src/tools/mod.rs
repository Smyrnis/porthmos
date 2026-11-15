use crate::bookmarks::BookmarksState;
use crate::history_view::HistoryState;
use crate::palette::PaletteState;
use crate::preview_view::PreviewState;

#[derive(Debug)]
pub enum Tool {
    Bookmarks(BookmarksState),
    History(HistoryState),
    Preview(PreviewState),
    Palette(PaletteState),
    Sync,
}
