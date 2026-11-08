mod hits;
mod lines;

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::{Duration, Instant},
};

use futures_util::{Stream, stream};
use porthmos_ssh::{Session, shell_quote};
pub use porthmos_vfs::SearchEvent;
use porthmos_vfs::{Entry, Glob, SearchQuery, VfsPath, join_remote};
use russh::{Channel, ChannelMsg, client::Msg};
use russh_sftp::client::SftpSession;
use tokio::sync::mpsc;

use self::{
    hits::{Outcome, stream_hits},
    lines::LineSplitter,
};
use crate::{fs::metadata_from_sftp, identities::Identities};

const LOOKUP_CONCURRENCY: usize = 16;
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const EXIT_GRACE: Duration = Duration::from_secs(1);
const EXIT_UNKNOWN: i64 = -1;
const FIND_PARTIAL_FAILURE: i64 = 1;

struct OwnedChannel {
    channel: Option<Channel<Msg>>,
}

impl Drop for OwnedChannel {
    fn drop(&mut self) {
        if let Some(channel) = self.channel.take() {
            tokio::spawn(async move {
                let _ = channel.close().await;
            });
        }
    }
}

struct FindOutput {
    channel: OwnedChannel,
    splitter: LineSplitter,
    queued: VecDeque<String>,
    ended: bool,
    quiet_since: Option<Instant>,
    cancel: Arc<AtomicBool>,
    exit: Arc<AtomicI64>,
}

pub async fn search_remote(
    session: &Session, sftp: &SftpSession, identities: &Identities, query: SearchQuery,
    tx: mpsc::UnboundedSender<SearchEvent>, cancel: Arc<AtomicBool>,
) {
    let root = query.root.to_string_lossy().into_owned();
    let SearchQuery { pattern, max_depth, max_results, .. } = &query;
    let (max_depth, max_results) = (*max_depth, *max_results);
    let exit = Arc::new(AtomicI64::new(EXIT_UNKNOWN));

    if let Some(channel) = start_find(session, &root, pattern, max_depth).await {
        let paths = Box::pin(find_paths(channel, cancel.clone(), exit.clone()));
        let outcome =
            stream_hits(paths, |path| describe(sftp, identities, path), LOOKUP_CONCURRENCY, max_results, &tx, &cancel)
                .await;
        match outcome {
            Outcome::Cancelled | Outcome::ReceiverGone => return,
            Outcome::Finished { found, truncated } => {
                if found > 0 || find_completed(&exit) {
                    let _ = tx.send(SearchEvent::Done { truncated });
                    return;
                }
            }
        }
    }

    search_remote_walk(sftp, identities, query, tx, cancel).await;
}

fn find_completed(exit: &AtomicI64) -> bool {
    matches!(exit.load(Ordering::Relaxed), 0..=FIND_PARTIAL_FAILURE)
}

async fn describe(sftp: &SftpSession, identities: &Identities, path: String) -> Option<Entry> {
    let metadata = sftp.metadata(&path).await.ok()?;
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    let (owner, group) = identities.ownership(&metadata);
    Some(Entry {
        name,
        path: VfsPath::new(&path),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        permissions: metadata.permissions,
        modified: metadata_from_sftp(&metadata).modified,
        owner,
        group,
        ..Default::default()
    })
}

async fn start_find(session: &Session, root: &str, pattern: &str, max_depth: usize) -> Option<OwnedChannel> {
    let channel = session.handle().channel_open_session().await.ok()?;
    let command = format!("find {} -maxdepth {max_depth} -iname {}", shell_quote(root), shell_quote(pattern));
    channel.exec(true, command.into_bytes()).await.ok()?;
    Some(OwnedChannel { channel: Some(channel) })
}

fn find_paths(channel: OwnedChannel, cancel: Arc<AtomicBool>, exit: Arc<AtomicI64>) -> impl Stream<Item = String> {
    let state = FindOutput {
        channel,
        splitter: LineSplitter::default(),
        queued: VecDeque::new(),
        ended: false,
        quiet_since: None,
        cancel,
        exit,
    };
    stream::unfold(state, |mut state| async move {
        loop {
            if let Some(path) = state.queued.pop_front() {
                return Some((path, state));
            }
            if state.ended || state.cancel.load(Ordering::Relaxed) {
                return None;
            }
            if state.quiet_since.is_some_and(|since| since.elapsed() >= EXIT_GRACE) {
                state.queued.extend(state.splitter.finish());
                state.ended = true;
                continue;
            }
            let channel = state.channel.channel.as_mut()?;
            let message = tokio::select! {
                message = channel.wait() => message,
                () = tokio::time::sleep(POLL_INTERVAL) => continue,
            };
            match message {
                Some(ChannelMsg::Data { data }) => state.queued.extend(state.splitter.push(&data)),
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    state.exit.store(i64::from(exit_status), Ordering::Relaxed)
                }
                Some(ChannelMsg::Eof) => state.quiet_since = Some(Instant::now()),
                Some(ChannelMsg::Close) | None => {
                    state.queued.extend(state.splitter.finish());
                    state.ended = true;
                }
                Some(_) => {}
            }
        }
    })
}

async fn search_remote_walk(
    sftp: &SftpSession, identities: &Identities, query: SearchQuery, tx: mpsc::UnboundedSender<SearchEvent>,
    cancel: Arc<AtomicBool>,
) {
    let root = query.root.to_string_lossy().into_owned();
    let SearchQuery { pattern, max_depth, max_results, .. } = query;
    let glob = Glob::new(&pattern);
    let mut found = 0usize;
    let mut truncated = false;
    let mut stack: Vec<(String, usize)> = vec![(root, 0)];

    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if depth > max_depth {
            continue;
        }

        let entries = match sftp.read_dir(&dir).await {
            Ok(read_dir) => read_dir.collect::<Vec<_>>(),
            Err(_) => continue,
        };

        for dir_entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            if found >= max_results {
                truncated = true;
                break;
            }

            let name = dir_entry.file_name();
            let metadata = dir_entry.metadata();
            let path = join_remote(&dir, &name);

            if glob.matches(&name) {
                let (owner, group) = identities.ownership(&metadata);
                let _ = tx.send(SearchEvent::Found(Entry {
                    name: name.clone(),
                    path: VfsPath::new(&path),
                    is_dir: metadata.is_dir(),
                    size: metadata.len(),
                    permissions: metadata.permissions,
                    modified: metadata_from_sftp(&metadata).modified,
                    owner,
                    group,
                    ..Default::default()
                }));
                found += 1;
            }

            if metadata.is_dir() {
                stack.push((path, depth + 1));
            }
        }

        if found >= max_results {
            truncated = true;
            break;
        }
    }

    let _ = tx.send(SearchEvent::Done { truncated });
}
