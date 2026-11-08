use std::{sync::Arc, time::Duration};

use porthmos_vfs::{FileSystem, VfsPath};

use super::super::{Engine, Event, Internal, Location};
use crate::{
    preview::{PREVIEW_LIMIT, Preview, read_preview},
    tasks::Scope,
    user_message,
};

pub(crate) const PREVIEW_REST: Duration = Duration::from_millis(150);

fn name_of(path: &VfsPath) -> String {
    path.file_name().map_or_else(|| path.to_string(), |name| name.to_string_lossy().into_owned())
}

impl Engine {
    fn preview_connection_closed(&self, request_id: u64, path: &VfsPath) {
        let name = name_of(path);
        self.emit(Event::PreviewReady {
            request_id,
            result: Err(format!("Couldn't preview {name}: the connection was closed")),
            name,
        });
    }

    fn spawn_preview(&mut self, request_id: u64, fs: Arc<dyn FileSystem>, path: VfsPath) {
        self.preview_busy = true;
        let internal = self.internal.clone();
        self.tasks.spawn("preview", Scope::Preview, move |cancel| async move {
            let name = name_of(&path);
            let result = match read_preview(fs.as_ref(), &path, PREVIEW_LIMIT, &cancel).await {
                Ok(preview) => Ok(preview),
                Err(err) => {
                    tracing::debug!("{err:?}");
                    Err(user_message(format!("Couldn't preview {name}"), &err))
                }
            };
            let _ = internal.send(Internal::Preview { request_id, name, result });
        });
    }

    pub(crate) fn start_preview(&mut self, request_id: u64, location: Location, path: VfsPath) {
        if self.preview_busy {
            self.tasks.cancel(Scope::Preview);
            self.tasks.forget(Scope::Preview);
        }
        let Some(fs) = self.fs_for(location) else {
            self.preview_request = None;
            self.preview_next = None;
            self.preview_connection_closed(request_id, &path);
            return;
        };
        self.preview_request = Some(request_id);
        if self.preview_busy {
            self.preview_next = Some((request_id, location, path));
        } else {
            self.spawn_preview(request_id, fs, path);
        }
    }

    pub(crate) fn finish_preview(&mut self, request_id: u64, name: String, result: Result<Preview, String>) {
        self.preview_busy = false;
        if let Some((next_id, location, path)) = self.preview_next.take() {
            match self.fs_for(location) {
                Some(fs) => self.spawn_preview(next_id, fs, path),
                None => {
                    self.preview_request = None;
                    self.preview_connection_closed(next_id, &path);
                }
            }
        }
        if self.preview_request != Some(request_id) {
            return;
        }
        self.preview_request = None;
        self.emit(Event::PreviewReady { request_id, name, result: result.map(Arc::new) });
    }

    pub(crate) fn hint_preview(&mut self, request_id: u64, location: Location, path: VfsPath) {
        self.tasks.cancel(Scope::PreviewHint);
        self.preview_hint = Some(request_id);
        self.preview_request = None;
        self.preview_next = None;
        if self.preview_busy {
            self.tasks.cancel(Scope::Preview);
            self.tasks.forget(Scope::Preview);
        }
        let internal = self.internal.clone();
        self.tasks.spawn("preview-hint", Scope::PreviewHint, move |cancel| async move {
            tokio::time::sleep(PREVIEW_REST).await;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let _ = internal.send(Internal::PreviewRested { request_id, location, path });
        });
    }

    pub(crate) fn cancel_preview(&mut self) {
        self.tasks.cancel(Scope::PreviewHint);
        self.preview_hint = None;
        self.preview_request = None;
        self.preview_next = None;
        if self.preview_busy {
            self.tasks.cancel(Scope::Preview);
            self.tasks.forget(Scope::Preview);
        }
    }
}

#[cfg(test)]
mod tests;
