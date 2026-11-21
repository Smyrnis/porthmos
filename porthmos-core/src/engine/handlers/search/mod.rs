use std::{sync::atomic::Ordering, time::Duration};

use porthmos_vfs::{SearchEvent, SearchQuery, VfsPath};

use super::super::{Engine, Event, Location, Reporter, RequestId};
use crate::{Message, Severity, tasks::Scope};

pub(crate) const SEARCH_DEBOUNCE: Duration = Duration::from_millis(120);

fn event_for(request_id: RequestId, search_event: SearchEvent) -> Event {
    match search_event {
        SearchEvent::Found(entry) => Event::SearchFound { request_id, entry },
        SearchEvent::Done { truncated, unreadable } => Event::SearchDone { request_id, truncated, unreadable },
        SearchEvent::Failed(message) => Event::SearchFailed { request_id, message: Message::parse(&message) },
    }
}

impl Engine {
    pub(crate) fn search(&mut self, request_id: RequestId, location: Location, root: VfsPath, pattern: String) {
        self.tasks.cancel(Scope::Search);
        let generation = self.search.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let current_generation = self.search.generation.clone();

        let Some(fs) = self.fs_for(location) else {
            return;
        };
        let events = self.events.clone();
        let (max_depth, max_results) = (self.search.max_depth, self.search.max_results);
        let reporter = Reporter::new(events.clone());
        self.tasks.spawn("search", Scope::Search, move |cancel| async move {
            tokio::time::sleep(SEARCH_DEBOUNCE).await;
            if cancel.load(Ordering::Relaxed) || current_generation.load(Ordering::Relaxed) != generation {
                return;
            }
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let forward = async {
                while let Some(search_event) = rx.recv().await {
                    if let SearchEvent::Failed(message) = &search_event {
                        reporter.log(Severity::Error, message);
                    }
                    let _ = events.send(event_for(request_id, search_event));
                }
            };
            let query = SearchQuery { max_depth, max_results, ..SearchQuery::new(root, pattern) };
            tokio::join!(fs.search(query, tx, cancel), forward);
        });
    }

    pub(crate) fn cancel_search(&mut self) {
        self.tasks.cancel(Scope::Search);
    }
}

#[cfg(test)]
mod tests;
