use crate::i18n;
use porthmos_vfs::VfsPath;

use super::super::{Engine, Event, Location};
use crate::{
    Severity,
    config::bookmarks::{self, Bookmark},
};

impl Engine {
    fn save_bookmarks(&self) {
        if let Err(err) = bookmarks::save_to(&self.paths.bookmarks_file(), &self.bookmarks) {
            self.report(Severity::Error, format!("{err:#}"));
        }
    }

    fn refuse_protected_bookmarks(&self) -> bool {
        let protected = self.bookmarks.is_protected();
        if protected {
            self.report(Severity::Error, i18n::text("bookmarks.protected"));
        }
        protected
    }

    pub(crate) fn publish_bookmarks(&self) {
        self.emit(Event::Bookmarks(self.bookmarks.iter().cloned().collect()));
    }

    pub(crate) fn add_bookmark(&mut self, label: String, location: Location, path: VfsPath) {
        let host = match location {
            Location::Local => None,
            Location::Session(id) => match self.book.sessions.get(&id) {
                Some(session) => Some(session.name.clone()),
                None => {
                    self.report(Severity::Warning, i18n::text("bookmarks.connect_first"));
                    return;
                }
            },
        };

        if self.refuse_protected_bookmarks() {
            return;
        }
        let bookmark = Bookmark { label, path, host };
        if self.bookmarks.iter().any(|existing| *existing == bookmark) {
            self.info(i18n::text_with("bookmarks.already", &[("label", &bookmark.label)]));
            return;
        }
        self.bookmarks.add(bookmark);
        self.save_bookmarks();
        self.publish_bookmarks();
    }

    pub(crate) fn remove_bookmark(&mut self, index: usize) {
        if self.refuse_protected_bookmarks() {
            return;
        }
        if let Some(removed) = self.bookmarks.remove(index) {
            self.save_bookmarks();
            self.info(i18n::text_with("bookmarks.removed", &[("label", &removed.label)]));
            self.publish_bookmarks();
        }
    }
}

#[cfg(test)]
mod tests;
