use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use porthmos_vfs::{FileSystem, VfsPath};
use tokio::sync::mpsc::UnboundedSender;

use super::super::{Engine, Event, Location, RequestId, SessionId};
use crate::{
    Message, failure_report,
    properties::{Digest, digest::sha256, measure::measure},
    tasks::Scope,
};

const MEASURE_FAILED: &str = "Could not measure the selection";
const CHECKSUM_FAILED: &str = "Could not read the file";
const NOT_A_FILE: &str = "not a file";
const SESSION_GONE: &str = "session disconnected";

async fn checksum_one(
    fs: &dyn FileSystem, request_id: RequestId, path: &VfsPath, events: &UnboundedSender<Event>, cancel: &AtomicBool,
) -> Option<Result<Digest, Message>> {
    let metadata = match fs.stat(path).await {
        Ok(metadata) => metadata,
        Err(err) => return Some(Err(failure_report(CHECKSUM_FAILED, &err))),
    };
    if metadata.is_dir() {
        return Some(Err(Message::new(NOT_A_FILE)));
    }
    let bytes_total = metadata.size;
    let mut progress = |bytes_done: u64| {
        if !cancel.load(Ordering::Relaxed) {
            let _ = events.send(Event::ChecksumProgress { request_id, path: path.clone(), bytes_done, bytes_total });
        }
    };
    match sha256(fs, path, cancel, &mut progress).await {
        Ok(Some(digest)) => Some(Ok(digest)),
        Ok(None) => None,
        Err(err) => Some(Err(failure_report(CHECKSUM_FAILED, &err))),
    }
}

impl Engine {
    fn claim_properties(&mut self, request_id: RequestId, location: Location) {
        if self.properties_request != Some(request_id) {
            self.stop_properties();
            self.properties_request = Some(request_id);
        }
        self.properties_location = Some(location);
    }

    pub(crate) fn stop_properties_of(&mut self, session: SessionId) {
        if self.properties_location == Some(Location::Session(session)) {
            self.stop_properties();
            self.properties_request = None;
            self.properties_location = None;
        }
    }

    fn stop_properties(&mut self) {
        for scope in [Scope::Measure, Scope::Checksum] {
            self.tasks.cancel(scope);
            self.tasks.forget(scope);
        }
    }

    pub(crate) fn measure(&mut self, request_id: RequestId, location: Location, paths: Vec<VfsPath>) {
        self.claim_properties(request_id, location);
        self.tasks.cancel(Scope::Measure);
        self.tasks.forget(Scope::Measure);
        let Some(fs) = self.fs_for(location) else {
            let result = Err(failure_report(MEASURE_FAILED, &SESSION_GONE));
            self.emit(Event::Measured { request_id, result });
            return;
        };
        let events = self.events.clone();
        self.tasks.spawn("measure", Scope::Measure, move |cancel| async move {
            let mut progress = |measured: &_| {
                if !cancel.load(Ordering::Relaxed) {
                    let _ = events.send(Event::MeasureProgress { request_id, measured: *measured });
                }
            };
            let result = match measure(fs.as_ref(), &paths, &cancel, &mut progress).await {
                Ok(Some(measured)) => Ok(measured),
                Ok(None) => return,
                Err(err) => Err(failure_report(MEASURE_FAILED, &err)),
            };
            if !cancel.load(Ordering::Relaxed) {
                let _ = events.send(Event::Measured { request_id, result });
            }
        });
    }

    pub(crate) fn checksum(&mut self, request_id: RequestId, location: Location, paths: Vec<VfsPath>) {
        self.claim_properties(request_id, location);
        self.tasks.cancel(Scope::Checksum);
        self.tasks.forget(Scope::Checksum);
        let Some(fs) = self.fs_for(location) else {
            for path in paths {
                let result = Err(failure_report(CHECKSUM_FAILED, &SESSION_GONE));
                self.emit(Event::Checksummed { request_id, path, result });
            }
            return;
        };
        let events = self.events.clone();
        self.tasks.spawn("checksum", Scope::Checksum, move |cancel| async move {
            let fs: Arc<dyn FileSystem> = fs;
            for path in paths {
                let Some(result) = checksum_one(fs.as_ref(), request_id, &path, &events, &cancel).await else {
                    return;
                };
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let _ = events.send(Event::Checksummed { request_id, path, result });
            }
        });
    }

    pub(crate) fn cancel_properties(&mut self, request_id: RequestId) {
        if self.properties_request == Some(request_id) {
            self.stop_properties();
            self.properties_request = None;
            self.properties_location = None;
        }
    }
}

#[cfg(test)]
mod tests;
