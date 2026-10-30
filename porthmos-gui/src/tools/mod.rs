use crate::bookmarks::BookmarksState;
use crate::history_view::HistoryState;
use crate::palette::PaletteState;
use crate::preview_view::PreviewState;
use crate::search::SearchState;
use crate::settings_view::SettingsState;

#[derive(Debug)]
pub enum Tool {
    Search(SearchState),
    Bookmarks(BookmarksState),
    History(HistoryState),
    Preview(PreviewState),
    Palette(PaletteState),
    Settings(SettingsState),
    Sync,
}
