use crate::i18n;
use std::{sync::Arc, time::Duration};

use porthmos_vfs::{FileSystem, VfsPath};

use super::super::{Engine, Event, Internal, Location, RequestId};
use crate::{
    Message, failure_report,
    preview::{PREVIEW_LIMIT, Preview, PreviewSlot, read_preview},
    tasks::Scope,
};

pub(crate) const PREVIEW_REST: Duration = Duration::from_millis(150);

fn name_of(path: &VfsPath) -> String {
    path.file_name().map_or_else(|| path.to_string(), |name| name.to_string_lossy().into_owned())
}

#[derive(Debug, Default)]
pub(crate) struct PreviewLane {
    pub(crate) request: Option<u64>,
    pub(crate) busy: bool,
    pub(crate) hint: Option<RequestId>,
    pub(crate) next: Option<(u64, Location, VfsPath)>,
}

#[derive(Debug, Default)]
pub(crate) struct PreviewSlots {
    pub(crate) tool: PreviewLane,
    pub(crate) pane: PreviewLane,
}

impl PreviewSlots {
    fn lane(&mut self, slot: PreviewSlot) -> &mut PreviewLane {
        match slot {
            PreviewSlot::Tool => &mut self.tool,
            PreviewSlot::Pane => &mut self.pane,
        }
    }
}

fn task_name(slot: PreviewSlot) -> &'static str {
    match slot {
        PreviewSlot::Tool => "preview",
        PreviewSlot::Pane => "pane-preview",
    }
}

impl Engine {
    fn preview_connection_closed(&self, request_id: u64, path: &VfsPath) {
        let name = name_of(path);
        self.emit(Event::PreviewReady {
            request_id,
            result: Err(Message::new(i18n::text_with("preview.closed", &[("name", &name)]))),
            name,
        });
    }

    fn spawn_preview(&mut self, slot: PreviewSlot, request_id: u64, fs: Arc<dyn FileSystem>, path: VfsPath) {
        self.previews.lane(slot).busy = true;
        let internal = self.internal.clone();
        self.tasks.spawn(task_name(slot), Scope::Preview(slot), move |cancel| async move {
            let name = name_of(&path);
            let result = match read_preview(fs.as_ref(), &path, PREVIEW_LIMIT, &cancel).await {
                Ok(preview) => Ok(preview),
                Err(err) => {
                    tracing::debug!("{err:?}");
                    Err(failure_report(i18n::text_with("preview.failed", &[("name", &name)]), &err))
                }
            };
            let _ = internal.send(Internal::Preview { slot, request_id, name, result });
        });
    }

    fn stop_running_read(&mut self, slot: PreviewSlot) {
        if self.previews.lane(slot).busy {
            self.tasks.cancel(Scope::Preview(slot));
            self.tasks.forget(Scope::Preview(slot));
        }
    }

    pub(crate) fn preview_now(&mut self, request_id: u64, location: Location, path: VfsPath) {
        self.start_preview(PreviewSlot::Tool, request_id, location, path);
    }

    pub(crate) fn start_preview(&mut self, slot: PreviewSlot, request_id: u64, location: Location, path: VfsPath) {
        self.stop_running_read(slot);
        let Some(fs) = self.fs_for(location) else {
            let lane = self.previews.lane(slot);
            lane.request = None;
            lane.next = None;
            self.preview_connection_closed(request_id, &path);
            return;
        };
        let lane = self.previews.lane(slot);
        lane.request = Some(request_id);
        if lane.busy {
            lane.next = Some((request_id, location, path));
        } else {
            self.spawn_preview(slot, request_id, fs, path);
        }
    }

    pub(crate) fn finish_preview(
        &mut self, slot: PreviewSlot, request_id: u64, name: String, result: Result<Preview, Message>,
    ) {
        let lane = self.previews.lane(slot);
        lane.busy = false;
        if let Some((next_id, location, path)) = lane.next.take() {
            match self.fs_for(location) {
                Some(fs) => self.spawn_preview(slot, next_id, fs, path),
                None => {
                    self.previews.lane(slot).request = None;
                    self.preview_connection_closed(next_id, &path);
                }
            }
        }
        let lane = self.previews.lane(slot);
        if lane.request != Some(request_id) {
            return;
        }
        lane.request = None;
        self.emit(Event::PreviewReady { request_id, name, result: result.map(Arc::new) });
    }

    pub(crate) fn hint_preview(&mut self, request_id: u64, location: Location, path: VfsPath) {
        self.tasks.cancel(Scope::PreviewHint);
        let lane = &mut self.previews.pane;
        lane.hint = Some(request_id);
        lane.request = None;
        lane.next = None;
        self.stop_running_read(PreviewSlot::Pane);
        let internal = self.internal.clone();
        self.tasks.spawn("preview-hint", Scope::PreviewHint, move |cancel| async move {
            tokio::time::sleep(PREVIEW_REST).await;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let _ = internal.send(Internal::PreviewRested { request_id, location, path });
        });
    }

    pub(crate) fn cancel_preview(&mut self, slot: PreviewSlot) {
        if slot == PreviewSlot::Pane {
            self.tasks.cancel(Scope::PreviewHint);
        }
        let lane = self.previews.lane(slot);
        lane.hint = None;
        lane.request = None;
        lane.next = None;
        self.stop_running_read(slot);
    }
}

#[cfg(test)]
mod tests;
