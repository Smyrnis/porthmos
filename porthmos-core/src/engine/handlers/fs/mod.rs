use std::{sync::Arc, time::Duration};

use porthmos_vfs::{FileSystem, ProtocolError, VfsPath};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    super::{Engine, Event, Internal, Location, Reporter, SessionId},
    failure_message,
};
use crate::{
    Message, Severity, failure_report,
    path_completion::{complete, split_directory},
    path_text::{needs_home, resolve_path_text},
    tasks::Scope,
};

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

const PERMISSION_BITS: u32 = 0o7777;
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

fn unchanged_completion(location: Location, text: String) -> Event {
    let completion = complete(&text, location, &[]);
    Event::PathCompleted { location, text, completion }
}

fn announce_folder(internal: &UnboundedSender<Internal>, location: Location, path: &VfsPath) {
    let _ = internal.send(Internal::FolderShown { location, path: path.clone() });
}

async fn list_into(
    fs: Arc<dyn FileSystem>, location: Location, path: Option<VfsPath>, reveal: Option<String>,
    events: UnboundedSender<Event>, internal: UnboundedSender<Internal>, reporter: Reporter,
) -> bool {
    let path = match path {
        Some(path) => path,
        None => match fs.home().await {
            Ok(home) => home,
            Err(err) => {
                send_failure(&reporter, failure_message(location, "Unable to list home directory", &err), &err);
                return false;
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
            true
        }
        Err(err) => {
            send_failure(&reporter, failure_message(location, format!("Unable to list {}", path), &err), &err);
            false
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
    remembered: Option<VfsPath>,
}

async fn list_start_into(
    fs: Arc<dyn FileSystem>, session: SessionId, start: StartFolder, events: UnboundedSender<Event>,
    internal: UnboundedSender<Internal>, reporter: Reporter,
) {
    let StartFolder { name, requested, resume, remembered } = start;
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
        if let Some(folder) = remembered
            && let Ok(entries) = fs.list(&folder).await
        {
            announce_folder(&internal, location, &folder);
            let _ = events.send(Event::Listed { location, path: folder.clone(), entries });
            announce_free_space(fs.as_ref(), location, folder, &events).await;
            return;
        }
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
            remembered: self.last_used.store.folder(&live.name),
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

    pub(crate) fn remember_folder(&mut self, location: Location, path: VfsPath) {
        match location {
            Location::Local => self.remember_local_folder(&path),
            Location::Session(session) => self.remember_session_folder(session, path),
        }
    }

    fn remember_session_folder(&mut self, session: SessionId, path: VfsPath) {
        let Some(live) = self.sessions.get(&session) else {
            return;
        };
        let name = live.name.clone();
        self.record_last_folder(&name, &path);
        self.session_folders.insert(session, path);
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

    pub(crate) fn go_to_path(&mut self, location: Location, from: VfsPath, text: String) {
        let fs = if text.trim().is_empty() { None } else { self.filesystem_or_drop(location) };
        let Some(fs) = fs else {
            self.emit(Event::PathRejected { location });
            return;
        };
        let events = self.events.clone();
        let internal = self.internal.clone();
        let reporter = self.reporter.clone();
        self.tasks.spawn("go-to-path", scope_for(location), move |_| async move {
            let home = if needs_home(&text) {
                match fs.home().await {
                    Ok(home) => home,
                    Err(err) => {
                        send_failure(
                            &reporter,
                            failure_message(location, "Unable to find the home directory", &err),
                            &err,
                        );
                        let _ = events.send(Event::PathRejected { location });
                        return;
                    }
                }
            } else {
                from.clone()
            };
            let target = resolve_path_text(location, &text, &from, &home);
            let opened = list_into(fs, location, Some(target), None, events.clone(), internal, reporter).await;
            let _ = events.send(if opened { Event::PathOpened { location } } else { Event::PathRejected { location } });
        });
    }

    pub(crate) fn complete_path(&mut self, location: Location, from: VfsPath, text: String) {
        let fs = if text == "~" { None } else { self.filesystem_or_drop(location) };
        let Some(fs) = fs else {
            self.emit(unchanged_completion(location, text));
            return;
        };
        let events = self.events.clone();
        self.tasks.spawn("complete-path", scope_for(location), move |_| async move {
            let (prefix, _) = split_directory(&text, location);
            let home = if needs_home(prefix) {
                match fs.home().await {
                    Ok(home) => home,
                    Err(_) => {
                        let _ = events.send(unchanged_completion(location, text));
                        return;
                    }
                }
            } else {
                from.clone()
            };
            let folder = resolve_path_text(location, prefix, &from, &home);
            let entries = fs.list(&folder).await.unwrap_or_default();
            let completion = complete(&text, location, &entries);
            let _ = events.send(Event::PathCompleted { location, text, completion });
        });
    }

    pub(crate) fn list(&mut self, location: Location, path: Option<VfsPath>) {
        let Some(fs) = self.filesystem_or_drop(location) else {
            return;
        };
        let work =
            list_into(fs, location, path, None, self.events.clone(), self.internal.clone(), self.reporter.clone());
        self.tasks.spawn("list", scope_for(location), move |_| async move {
            work.await;
        });
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
        self.tasks.spawn("list", scope_for(location), move |_| async move {
            work.await;
        });
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

    pub(crate) fn set_permissions(&mut self, location: Location, paths: Vec<VfsPath>, mode: u32) {
        let Some(fs) = self.filesystem_or_drop(location) else {
            return;
        };
        if !fs.can_set_permissions() {
            let subject = match location {
                Location::Local => crate::delete_prompt::THIS_COMPUTER.to_string(),
                Location::Session(session) => self.session_name(session),
            };
            self.report(Severity::Warning, format!("{subject} can't change permissions"));
            return;
        }
        let events = self.events.clone();
        let reporter = self.reporter.clone();
        self.tasks.spawn("set-permissions", scope_for(location), move |_| async move {
            let mut changed = false;
            let mut refused: Option<(VfsPath, ProtocolError)> = None;
            for path in paths {
                match fs.set_permissions(&path, mode & PERMISSION_BITS).await {
                    Ok(()) => changed = true,
                    Err(err) if refused.is_none() => refused = Some((path, err)),
                    Err(_) => {}
                }
            }
            if let Some((path, err)) = refused {
                let context = format!("Unable to change permissions of {}", quoted_name(&path));
                send_failure(&reporter, failure_message(location, context, &err), &err);
            }
            if changed {
                let _ = events.send(Event::LocationChanged { location });
            }
        });
    }
}

#[cfg(test)]
mod tests;
