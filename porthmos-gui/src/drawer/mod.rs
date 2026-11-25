use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::Arc,
};

use iced::{
    Subscription,
    futures::{StreamExt, stream},
};
use porthmos_core::{Handover, SessionId, ShellSize, ShellStream, TerminalId, VfsPath};
use porthmos_term::{Snapshot, TermSize, TerminalEvent, TerminalSession, Transport};
use tokio::sync::{mpsc, watch};

use crate::app::Message;

pub mod keys;

const OPENING_COLUMNS: u16 = 80;
const OPENING_ROWS: u16 = 24;

pub fn opening_size() -> TermSize {
    TermSize::clamped(OPENING_COLUMNS, OPENING_ROWS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSource {
    Remote(SessionId),
    Local,
}

#[derive(Clone)]
pub struct SharedSession(Arc<TerminalSession>);

impl SharedSession {
    pub fn new(session: TerminalSession) -> Self {
        Self(Arc::new(session))
    }
}

impl Deref for SharedSession {
    type Target = TerminalSession;

    fn deref(&self) -> &TerminalSession {
        &self.0
    }
}

impl fmt::Debug for SharedSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SharedSession(..)")
    }
}

#[derive(Debug, Clone)]
pub struct Wanted {
    pub id: TerminalId,
    pub source: TerminalSource,
    pub directory: VfsPath,
}

pub struct TerminalTab {
    pub id: TerminalId,
    pub source: TerminalSource,
    pub title: String,
    pub session: Option<SharedSession>,
    pub frozen: Option<Snapshot>,
    pub events: Handover<mpsc::Receiver<TerminalEvent>>,
    pub exited: Option<Option<u32>>,
    pub size: TermSize,
}

#[derive(Default)]
pub struct Drawer {
    pub open: bool,
    pub focused: bool,
    pub tab: Option<TerminalTab>,
    pub wanted: Option<Wanted>,
    last_id: TerminalId,
}

impl Drawer {
    pub fn shows_focus(&self) -> bool {
        self.open
            && self.focused
            && (self.wanted.is_some() || self.tab.as_ref().is_some_and(|tab| tab.exited.is_none()))
    }

    pub fn allocate_id(&mut self) -> TerminalId {
        self.last_id += 1;
        self.last_id
    }
}

pub fn into_transport(stream: ShellStream) -> Transport {
    let ShellStream { reader, writer, resize, exit } = stream;
    Transport {
        reader,
        writer,
        resize: Box::new(move |size| resize(ShellSize { columns: size.columns, rows: size.rows })),
        exit,
    }
}

pub fn tab_stream(
    id: TerminalId, redraws: watch::Receiver<u64>, events: mpsc::Receiver<TerminalEvent>,
) -> impl stream::Stream<Item = Message> {
    stream::unfold((Some(redraws), events), move |(mut redraws, mut events)| async move {
        loop {
            let message = tokio::select! {
                Some(changed) = async {
                    match redraws.as_mut() {
                        Some(redraws) => Some(redraws.changed().await),
                        None => None,
                    }
                } => match changed {
                    Ok(()) => Message::TerminalRedrawn,
                    Err(_) => {
                        redraws = None;
                        continue;
                    }
                },
                event = events.recv() => match event {
                    Some(event) => Message::TerminalEventReceived(id, event),
                    None => return None,
                },
            };
            return Some((message, (redraws, events)));
        }
    })
}

struct TabFeed {
    id: TerminalId,
    redraws: watch::Receiver<u64>,
    events: Handover<mpsc::Receiver<TerminalEvent>>,
}

impl Hash for TabFeed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl TerminalTab {
    pub fn snapshot(&self) -> Option<Snapshot> {
        match &self.session {
            Some(session) => Some(session.snapshot()),
            None => self.frozen.clone(),
        }
    }

    pub fn end(&mut self) {
        if let Some(session) = self.session.take() {
            self.frozen = Some(session.snapshot());
        }
        self.exited.get_or_insert(None);
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let Some(session) = &self.session else {
            return Subscription::none();
        };
        let feed = TabFeed { id: self.id, redraws: session.redraws(), events: self.events.clone() };
        Subscription::run_with(feed, |feed| match feed.events.take() {
            Some(events) => tab_stream(feed.id, feed.redraws.clone(), events).boxed(),
            None => stream::empty().boxed(),
        })
    }
}

#[cfg(test)]
mod tests;
