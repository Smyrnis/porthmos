use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWriteExt, DuplexStream, ReadBuf};

use crate::{CommandOutput, CommandRunner, CommandStream, ErrorKind, ProtocolError, StreamEnd};

pub struct ScriptedStream {
    pub chunks: Vec<Vec<u8>>,
    pub delay: Option<Duration>,
    pub end: StreamEnd,
}

struct WatchedReader {
    inner: DuplexStream,
    dropped: Arc<AtomicBool>,
}

impl AsyncRead for WatchedReader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl Drop for WatchedReader {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[derive(Clone)]
enum Answer {
    Output(CommandOutput),
    Failure(ErrorKind, String),
}

type QueuedStream = Result<(ScriptedStream, Arc<AtomicBool>), (ErrorKind, String)>;

type Responder = Box<dyn Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync>;

#[derive(Default)]
struct State {
    responder: Option<Responder>,
    answers: VecDeque<Answer>,
    commands: Vec<String>,
    shells: Vec<String>,
    hang_next: bool,
    streams: VecDeque<QueuedStream>,
}

#[derive(Default)]
pub struct FakeRunner {
    state: Mutex<State>,
}

impl FakeRunner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn push(&self, result: Result<CommandOutput, ProtocolError>) {
        let answer = match result {
            Ok(output) => Answer::Output(output),
            Err(error) => Answer::Failure(error.kind(), error.to_string()),
        };
        self.state().answers.push_back(answer);
    }

    pub fn respond_with(
        &self, responder: impl Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync + 'static,
    ) {
        self.state().responder = Some(Box::new(responder));
    }

    pub fn commands(&self) -> Vec<String> {
        self.state().commands.clone()
    }

    pub fn shells(&self) -> Vec<String> {
        self.state().shells.clone()
    }

    pub fn calls(&self) -> usize {
        self.state().commands.len()
    }

    pub fn push_stream(&self, stream: ScriptedStream) -> Arc<AtomicBool> {
        let dropped = Arc::new(AtomicBool::new(false));
        self.state().streams.push_back(Ok((stream, dropped.clone())));
        dropped
    }

    pub fn push_stream_failure(&self, error: ProtocolError) {
        self.state().streams.push_back(Err((error.kind(), error.to_string())));
    }

    pub fn hang(&self) {
        self.state().hang_next = true;
    }
}

#[async_trait]
impl CommandRunner for FakeRunner {
    async fn run_with_input(&self, command: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        let text = if input.is_empty() { command.to_string() } else { String::from_utf8_lossy(input).into_owned() };
        let (hang, answer) = {
            let mut state = self.state();
            state.commands.push(text.clone());
            state.shells.push(command.to_string());
            let hang = std::mem::take(&mut state.hang_next);
            if let Some(responder) = &state.responder {
                (hang, Err(responder(&text)))
            } else {
                let answer =
                    if state.answers.len() > 1 { state.answers.pop_front() } else { state.answers.front().cloned() };
                (hang, Ok(answer))
            }
        };
        if hang {
            std::future::pending::<()>().await;
        }
        let answer = match answer {
            Err(responded) => return responded,
            Ok(answer) => answer,
        };
        match answer {
            None => Ok(CommandOutput::default()),
            Some(Answer::Output(output)) => Ok(output),
            Some(Answer::Failure(kind, text)) => Err(ProtocolError::new(kind, anyhow!("{text}"))),
        }
    }

    async fn open_stream(&self, command: &str, input: &[u8]) -> Result<CommandStream, ProtocolError> {
        let (script, dropped) = {
            let mut state = self.state();
            let text = if input.is_empty() { command.to_string() } else { String::from_utf8_lossy(input).into_owned() };
            state.commands.push(text);
            state.shells.push(command.to_string());
            match state.streams.pop_front() {
                Some(Ok(scripted)) => scripted,
                Some(Err((kind, text))) => return Err(ProtocolError::new(kind, anyhow!("{text}"))),
                None => (
                    ScriptedStream {
                        chunks: Vec::new(),
                        delay: None,
                        end: StreamEnd { status: Some(0), stderr: Vec::new() },
                    },
                    Arc::new(AtomicBool::new(false)),
                ),
            }
        };
        let (mut sink, source) = tokio::io::duplex(64 * 1024);
        let (ended_tx, ended) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            for (index, chunk) in script.chunks.iter().enumerate() {
                if let (Some(delay), true) = (script.delay, index > 0) {
                    tokio::time::sleep(delay).await;
                }
                if sink.write_all(chunk).await.is_err() {
                    return;
                }
            }
            drop(sink);
            let _ = ended_tx.send(script.end);
        });
        Ok(CommandStream { output: Box::new(WatchedReader { inner: source, dropped }), ended })
    }
}
