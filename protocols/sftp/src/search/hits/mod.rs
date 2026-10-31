use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
};

use futures_util::{Stream, StreamExt};
use porthmos_vfs::{Entry, SearchEvent, SearchSender};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Finished { found: usize, truncated: bool },
    Cancelled,
    ReceiverGone,
}

pub(crate) async fn stream_hits<Paths, Lookup, Looked>(
    paths: Paths, lookup: Lookup, concurrency: usize, max_results: usize, tx: &SearchSender, cancel: &AtomicBool,
) -> Outcome
where
    Paths: Stream<Item = String> + Unpin,
    Lookup: FnMut(String) -> Looked,
    Looked: Future<Output = Option<Entry>>,
{
    let mut hits = paths.map(lookup).buffer_unordered(concurrency);
    let mut found = 0usize;

    while let Some(hit) = hits.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Outcome::Cancelled;
        }
        let Some(entry) = hit else {
            continue;
        };
        if tx.send(SearchEvent::Found(entry)).is_err() {
            return Outcome::ReceiverGone;
        }
        found += 1;
        if found >= max_results {
            return Outcome::Finished { found, truncated: true };
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Outcome::Cancelled;
    }
    Outcome::Finished { found, truncated: false }
}

#[cfg(test)]
mod tests;
