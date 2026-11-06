use std::{sync::Arc, time::Duration};

use porthmos_vfs::{FileSystem, ProtocolError, VfsPath};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    super::{Engine, Event, Internal, Location, Reporter, SessionId},
    failure_message,
};
use crate::{Message, Severity, failure_report, tasks::Scope};

#[track_caller]
fn send_failure(reporter: &Reporter, message: Message, cause: &dyn std::fmt::Debug) {
    reporter.report_cause(Severity::Error, message, cause);
}

fn quoted_name(path: &VfsPath) -> String {
    format!("\"{}\"", path.file_name().unwrap_or_default().to_string_lossy())
}

fn differs_only_in_case(from: &VfsPath, to: &VfsPath) -> bool {
    from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase()
}

const FREE_SPACE_TIMEOUT: Duration = Duration::from_secs(10);

async fn announce_free_space(fs: &dyn FileSystem, location: Location, path: VfsPath, events: &UnboundedSender<Event>) {
    let bytes = match tokio::time::timeout(FREE_SPACE_TIMEOUT, fs.free_space(&path)).await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(err)) => {
            tracing::debug!("free space of {} unavailable: {err:?}", path);
            None
        }
        Err(_) => {
            tracing::debug!("free space of {} timed out", path);
            None
        }
    };
    let _ = events.send(Event::FreeSpace { location, path, bytes });
}

fn announce_folder(internal: &UnboundedSender<Internal>, location: Location, path: &VfsPath) {
    if let Location::Session(session) = location {
        let _ = internal.send(Internal::FolderShown { session, path: path.clone() });
    }
}

async fn list_into(
    fs: Arc<dyn FileSystem>, location: Location, path: Option<VfsPath>, reveal: Option<String>,
    events: UnboundedSender<Event>, internal: UnboundedSender<Internal>, reporter: Reporter,
) {
    let path = match path {
        Some(path) => path,
        None => match fs.home().await {
            Ok(home) => home,
            Err(err) => {
                send_failure(&reporter, failure_message(location, "Unable to list home directory", &err), &err);
                return;
            }
        },
    };
    match fs.list(&path).await {
        Ok(entries) => {
            let revealed = reveal.filter(|name| entries.iter().any(|entry| &entry.name == name));
            announce_folder(&internal, location, &path);
            let _ = events.send(Event::Listed { location, path: path.clone(), entries });
            if let Some(name) = revealed {
                let _ = events.send(Event::Revealed { location, folder: path.clone(), name });
            }
            announce_free_space(fs.as_ref(), location, path, &events).await;
        }
        Err(err) => {
            send_failure(&reporter, failure_message(location, format!("Unable to list {}", path), &err), &err);
        }
    }
}

fn start_directory(home: &VfsPath, requested: &str) -> VfsPath {
    match requested.strip_prefix('~') {
        Some("") => home.clone(),
        Some(under_home) if under_home.starts_with('/') => home.join(under_home.trim_start_matches('/')),
        _ => home.join(requested),
    }
}

async fn resolve_start_directory(fs: &dyn FileSystem, requested: &str) -> Result<VfsPath, ProtocolError> {
    if VfsPath::new(requested).is_absolute() {
        return Ok(VfsPath::new(requested));
    }
    Ok(start_directory(&fs.home().await?, requested))
}

struct StartFolder {
    name: String,
    requested: Option<String>,
    resume: Option<VfsPath>,
}

async fn list_start_into(
    fs: Arc<dyn FileSystem>, session: SessionId, start: StartFolder, events: UnboundedSender<Event>,
    internal: UnboundedSender<Internal>, reporter: Reporter,
) {
    let StartFolder { name, requested, resume } = start;
    let location = Location::Session(session);
    if let Some(folder) = resume
        && let Ok(entries) = fs.list(&folder).await
    {
        announce_folder(&internal, location, &folder);
        let _ = events.send(Event::Listed { location, path: folder.clone(), entries });
        announce_free_space(fs.as_ref(), location, folder, &events).await;
        return;
    }
    let Some(requested) = requested else {
        list_into(fs, location, None, None, events, internal, reporter).await;
        return;
    };
    let path = match resolve_start_directory(fs.as_ref(), &requested).await {
        Ok(path) => path,
        Err(err) => {
            send_failure(&reporter, failure_message(location, "Unable to list home directory", &err), &err);
            return;
        }
    };
    match fs.list(&path).await {
        Ok(entries) => {
            announce_folder(&internal, location, &path);
            let _ = events.send(Event::Listed { location, path: path.clone(), entries });
            announce_free_space(fs.as_ref(), location, path, &events).await;
        }
        Err(err) => {
            let context = format!("Unable to open {} on {name}, showing the home directory instead", path);
            reporter.report_cause(Severity::Warning, failure_report(context, &err), &err);
            list_into(fs, location, None, None, events, internal, reporter).await;
        }
    }
}

fn scope_for(location: Location) -> Scope {
    match location {
        Location::Session(id) => Scope::Session(id),
        Location::Local => Scope::Background,
    }
}

impl Engine {
    pub(crate) fn open_start_directory(&mut self, session: SessionId) {
        let Some(live) = self.sessions.get(&session) else {
            return;
        };
        let start = StartFolder {
            name: live.name.clone(),
            requested: live.entry.start_path().map(str::to_string),
            resume: self.resume_folders.remove(&live.name),
        };
        let work = list_start_into(
            live.fs.clone(),
            session,
            start,
            self.events.clone(),
            self.internal.clone(),
            self.reporter.clone(),
        );
        self.tasks.spawn("list-start", Scope::Session(session), move |_| work);
    }

    pub(crate) fn remember_folder(&mut self, session: SessionId, path: VfsPath) {
        if self.sessions.contains_key(&session) {
            self.session_folders.insert(session, path);
        }
    }

    pub(crate) fn keep_folder_for_reconnect(&mut self, session: SessionId, name: &str, was_lost: bool) {
        let folder = self.session_folders.remove(&session);
        match folder.filter(|_| was_lost) {
            Some(folder) => self.resume_folders.insert(name.to_string(), folder),
            None => self.resume_folders.remove(name),
        };
    }

    pub(crate) fn navigate(&mut self, location: Location, from: &VfsPath, to: VfsPath) {
        match from.ancestors().find(|candidate| candidate.parent().as_ref() == Some(&to)) {
            Some(left) => self.reveal(location, left),
            None => self.list(location, Some(to)),
        }
    }

    fn filesystem_or_drop(&self, location: Location) -> Option<Arc<dyn FileSystem>> {
        let fs = self.fs_for(location);
        if fs.is_none() {
            tracing::debug!("dropping a file operation for a session that is gone: {location:?}");
        }
        fs
    }

    pub(crate) fn list(&mut self, location: Location, path: Option<VfsPath>) {
        let Some(fs) = self.filesystem_or_drop(location) else {
            return;
        };
        let work =
            list_into(fs, location, path, None, self.events.clone(), self.internal.clone(), self.reporter.clone());
        self.tasks.spawn("list", scope_for(location), move |_| work);
    }

    pub(crate) fn reveal(&mut self, location: Location, path: VfsPath) {
        let Some(fs) = self.filesystem_or_drop(location) else {
            return;
        };
        let (folder, name) = match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => (parent, Some(name.to_string_lossy().to_string())),
            _ => (path, None),
        };
        let work = list_into(
            fs,
            location,
            Some(folder),
            name,
            self.events.clone(),
            self.internal.clone(),
            self.reporter.clone(),
        );
        self.tasks.spawn("list", scope_for(location), move |_| work);
    }

    pub(crate) fn create_dir(&mut self, location: Location, path: VfsPath) {
        let Some(fs) = self.filesystem_or_drop(location) else {
            return;
        };
        let events = self.events.clone();
        let reporter = self.reporter.clone();
        self.tasks.spawn("create-dir", scope_for(location), move |_| async move {
            match fs.create_dir(&path).await {
                Ok(()) => {
                    let _ = events.send(Event::LocationChanged { location });
                }
                Err(err) if fs.stat(&path).await.is_ok() => {
                    let shown = anyhow::anyhow!("{} already exists", quoted_name(&path));
                    send_failure(&reporter, failure_message(location, "Unable to create directory", &shown), &err);
                }
                Err(err) => {
                    send_failure(&reporter, failure_message(location, "Unable to create directory", &err), &err);
                }
            }
        });
    }

    pub(crate) fn rename(&mut self, location: Location, from: VfsPath, to: VfsPath) {
        let Some(fs) = self.filesystem_or_drop(location) else {
            return;
        };
        let events = self.events.clone();
        let reporter = self.reporter.clone();
        self.tasks.spawn("rename", scope_for(location), move |_| async move {
            if from != to && !differs_only_in_case(&from, &to) && fs.stat(&to).await.is_ok() {
                let err = anyhow::anyhow!("{} already exists", quoted_name(&to));
                send_failure(&reporter, failure_message(location, "Unable to rename", &err), &err);
                return;
            }
            match fs.rename(&from, &to).await {
                Ok(()) => {
                    let _ = events.send(Event::LocationChanged { location });
                }
                Err(err) => {
                    send_failure(&reporter, failure_message(location, "Unable to rename", &err), &err);
                }
            }
        });
    }

    pub(crate) fn delete(&mut self, location: Location, paths: Vec<VfsPath>) {
        let Some(fs) = self.filesystem_or_drop(location) else {
            return;
        };
        let events = self.events.clone();
        let reporter = self.reporter.clone();
        self.tasks.spawn("delete", scope_for(location), move |_| async move {
            for path in paths {
                if let Err(err) = fs.delete(&path).await {
                    send_failure(&reporter, failure_message(location, "Unable to delete", &err), &err);
                    return;
                }
            }
            let _ = events.send(Event::LocationChanged { location });
        });
    }
}

#[cfg(test)]
mod tests;
