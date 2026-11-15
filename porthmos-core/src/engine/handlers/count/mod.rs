use std::sync::atomic::Ordering;

use porthmos_vfs::VfsPath;

use super::super::{Engine, Event, Location, RequestId};
use crate::{
    delete_prompt::count::{COUNT_LIMIT, Contents, count_contents},
    tasks::Scope,
};

impl Engine {
    pub(crate) fn count_contents(&mut self, request_id: RequestId, location: Location, folders: Vec<VfsPath>) {
        self.cancel_count();
        let Some(fs) = self.fs_for(location) else {
            self.emit(Event::ContentsCounted { request_id, contents: Contents::Unavailable });
            return;
        };
        let events = self.events.clone();
        self.tasks.spawn("count-contents", Scope::Count, move |cancel| async move {
            let contents = match count_contents(fs.as_ref(), &folders, COUNT_LIMIT, &cancel).await {
                Ok(Some(count)) => Contents::Counted(count),
                Ok(None) => return,
                Err(err) => {
                    tracing::debug!("{err:?}");
                    Contents::Unavailable
                }
            };
            if !cancel.load(Ordering::Relaxed) {
                let _ = events.send(Event::ContentsCounted { request_id, contents });
            }
        });
    }

    pub(crate) fn cancel_count(&mut self) {
        self.tasks.cancel(Scope::Count);
        self.tasks.forget(Scope::Count);
    }
}

#[cfg(test)]
mod tests;
