use std::sync::{Arc, Mutex, MutexGuard};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot, watch},
    task::{AbortHandle, JoinHandle},
};

use crate::{ResizeFn, Snapshot, TermSize, Terminal, TerminalConfig, TerminalEvent, Transport};

const READ_CHUNK: usize = 64 * 1024;
const EVENT_QUEUE: usize = 64;
const REPLY_QUEUE: usize = 16;

fn locked(terminal: &Mutex<Terminal>) -> MutexGuard<'_, Terminal> {
    terminal.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct TerminalSession {
    terminal: Arc<Mutex<Terminal>>,
    input: mpsc::UnboundedSender<Vec<u8>>,
    resize: ResizeFn,
    redraws: watch::Receiver<u64>,
    pump: AbortHandle,
    supervisor: JoinHandle<()>,
    writer: JoinHandle<()>,
}

async fn pump_output(
    mut reader: Box<dyn AsyncRead + Send + Unpin>, terminal: Arc<Mutex<Terminal>>, replies: mpsc::Sender<Vec<u8>>,
    events: mpsc::Sender<TerminalEvent>, redraw: watch::Sender<u64>, exit: oneshot::Receiver<Option<u32>>,
) {
    let mut buffer = vec![0u8; READ_CHUNK];
    while let Ok(count) = reader.read(&mut buffer).await {
        if count == 0 {
            break;
        }
        let feed = locked(&terminal).feed(&buffer[..count]);
        if !feed.replies.is_empty() {
            let _ = replies.try_send(feed.replies);
        }
        for event in feed.events {
            let _ = events.try_send(event);
        }
        redraw.send_modify(|count| *count += 1);
    }
    let status = exit.await.unwrap_or(None);
    let _ = events.send(TerminalEvent::Exited(status)).await;
}

impl TerminalSession {
    pub fn spawn(
        transport: Transport, size: TermSize, config: TerminalConfig,
    ) -> (TerminalSession, mpsc::Receiver<TerminalEvent>) {
        let Transport { reader, mut writer, resize, exit } = transport;
        let terminal = Arc::new(Mutex::new(Terminal::new(size, config)));
        let (input, mut pending) = mpsc::unbounded_channel::<Vec<u8>>();
        let (replies, mut queued_replies) = mpsc::channel::<Vec<u8>>(REPLY_QUEUE);
        let (events, receiver) = mpsc::channel(EVENT_QUEUE);
        let (redraw, redraws) = watch::channel(0u64);

        let writing = tokio::spawn(async move {
            loop {
                let bytes = tokio::select! {
                    Some(bytes) = pending.recv() => bytes,
                    Some(bytes) = queued_replies.recv() => bytes,
                    else => break,
                };
                if writer.write_all(&bytes).await.is_err() || writer.flush().await.is_err() {
                    break;
                }
            }
        });

        let supervised_events = events.clone();
        let pumping = tokio::spawn(pump_output(reader, terminal.clone(), replies, events, redraw, exit));
        let pump = pumping.abort_handle();
        let supervisor = tokio::spawn(async move {
            if pumping.await.is_err() {
                let _ = supervised_events.send(TerminalEvent::Exited(None)).await;
            }
        });

        let session = TerminalSession { terminal, input, resize, redraws, pump, supervisor, writer: writing };
        (session, receiver)
    }

    pub fn write(&self, bytes: Vec<u8>) {
        let _ = self.input.send(bytes);
    }

    pub fn resize(&self, size: TermSize) {
        let size = TermSize::clamped(size.columns, size.rows);
        locked(&self.terminal).resize(size);
        (self.resize)(size);
    }

    pub fn snapshot(&self) -> Snapshot {
        locked(&self.terminal).snapshot()
    }

    pub fn with_terminal<R>(&self, action: impl FnOnce(&mut Terminal) -> R) -> R {
        action(&mut locked(&self.terminal))
    }

    pub fn redraws(&self) -> watch::Receiver<u64> {
        self.redraws.clone()
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.supervisor.abort();
        self.pump.abort();
        self.writer.abort();
    }
}

#[cfg(test)]
mod tests;
