use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use porthmos_monitor::{LineSplitter, LogLine, LogSource, LogSourceError, SHELL_COMMAND, log_script};
use porthmos_vfs::{CommandRunner, CommandStream, ErrorKind, StreamEnd};
use tokio::{
    io::AsyncReadExt,
    sync::{mpsc::UnboundedSender, oneshot::error::RecvError},
    time::{MissedTickBehavior, interval},
};

use super::{
    super::{Engine, Event, Internal, LogId, LogState, SessionId},
    first_line,
};
use crate::{Message, i18n, tasks::Scope};

pub(crate) const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
pub(crate) const MAX_BATCH_LINES: usize = 2000;
pub(crate) const MAX_PENDING_LINES: usize = 20_000;
pub(crate) const MAX_OPEN_LOGS: usize = 6;

const READ_CHUNK: usize = 16 * 1024;
const COMMAND_NOT_FOUND: u32 = 127;

struct LogRun {
    session: SessionId,
    run: u64,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct LogsRun {
    runs: HashMap<LogId, LogRun>,
    next_run: u64,
}

fn unavailable(key: &str) -> LogState {
    LogState::Unavailable(Message::new(i18n::text(key)))
}

fn source_problem(error: LogSourceError) -> LogState {
    unavailable(match error {
        LogSourceError::EmptyPath => "logs.empty_path",
        LogSourceError::PathTooLong => "logs.path_too_long",
        LogSourceError::BadCharacter => "logs.bad_character",
    })
}

fn judge_end(source: &LogSource, ended: Result<StreamEnd, RecvError>) -> LogState {
    let end = match ended {
        Ok(end) => end,
        Err(_) => return LogState::Ended(Message::new(i18n::text("logs.stopped"))),
    };
    match end.status {
        Some(0) => LogState::Ended(Message::new(i18n::text("logs.ended"))),
        Some(COMMAND_NOT_FOUND) => unavailable(match source {
            LogSource::File(_) => "logs.no_tail",
            LogSource::Journal(_) => "logs.no_journal",
        }),
        _ => LogState::Ended(Message::new(first_line(&end.stderr).unwrap_or_else(|| i18n::text("logs.stopped")))),
    }
}

struct Outbox {
    id: LogId,
    run: u64,
    internal: UnboundedSender<Internal>,
}

impl Outbox {
    fn send(&self, state: LogState) {
        let _ = self.internal.send(Internal::Log { id: self.id, run: self.run, state });
    }

    fn end(&self, state: LogState) {
        let _ = self.internal.send(Internal::LogEnded { id: self.id, run: self.run, state });
    }
}

#[derive(Default)]
struct Pending {
    lines: VecDeque<LogLine>,
    skipped: u64,
}

impl Pending {
    fn push(&mut self, fresh: &mut Vec<LogLine>) {
        self.lines.extend(fresh.drain(..));
        let excess = self.lines.len().saturating_sub(MAX_PENDING_LINES);
        self.lines.drain(..excess);
        self.skipped += excess as u64;
    }

    fn flush(&mut self, outbox: &Outbox) {
        if self.lines.is_empty() && self.skipped == 0 {
            return;
        }
        let take = self.lines.len().min(MAX_BATCH_LINES);
        let lines = self.lines.drain(..take).collect();
        outbox.send(LogState::Lines { lines, skipped: std::mem::take(&mut self.skipped) });
    }
}

async fn read_until_end(stream: CommandStream, source: &LogSource, outbox: &Outbox) -> LogState {
    let CommandStream { mut output, mut ended } = stream;
    let mut splitter = LineSplitter::new();
    let mut pending = Pending::default();
    let mut fresh = Vec::new();
    let mut buffer = vec![0u8; READ_CHUNK];
    let mut tick = interval(FLUSH_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut reading = true;
    let mut end = None;
    loop {
        tokio::select! {
            biased;
            _ = tick.tick() => {
                pending.flush(outbox);
                if pending.lines.is_empty()
                    && let Some(state) = end.take()
                {
                    return state;
                }
            }
            read = output.read(&mut buffer), if reading => match read {
                Ok(0) => {
                    splitter.finish(&mut fresh);
                    pending.push(&mut fresh);
                    reading = false;
                }
                Ok(count) => {
                    splitter.feed(&buffer[..count], &mut fresh);
                    pending.push(&mut fresh);
                }
                Err(error) => {
                    reading = false;
                    end = Some(LogState::Ended(Message::new(error.to_string())));
                }
            },
            ended = &mut ended, if !reading && end.is_none() => end = Some(judge_end(source, ended)),
        }
    }
}

async fn follow(runner: Arc<dyn CommandRunner>, script: String, source: LogSource, outbox: Outbox) {
    let state = match runner.open_stream(SHELL_COMMAND, script.as_bytes()).await {
        Err(error) if error.kind() == ErrorKind::Unsupported => unavailable("logs.unavailable_no_runner"),
        Err(error) => LogState::Ended(Message::new(error.to_string())),
        Ok(stream) => {
            outbox.send(LogState::Started);
            read_until_end(stream, &source, &outbox).await
        }
    };
    outbox.end(state);
}

async fn cancelled(cancel: Arc<AtomicBool>) {
    let mut tick = interval(FLUSH_INTERVAL);
    while !cancel.load(Ordering::Relaxed) {
        tick.tick().await;
    }
}

impl Engine {
    fn refuse_log(&self, id: LogId, state: LogState) {
        self.emit(Event::Log { id, state });
    }

    pub(crate) fn start_log(&mut self, id: LogId, session: SessionId, source: &LogSource, initial_lines: u32) {
        self.stop_log(id);
        let Some(live) = self.book.sessions.get(&session) else {
            return self.refuse_log(id, unavailable("logs.unavailable_no_session"));
        };
        let Some(runner) = live.fs.runner() else {
            return self.refuse_log(id, unavailable("logs.unavailable_no_runner"));
        };
        let script = match log_script(source, initial_lines) {
            Ok(script) => script,
            Err(error) => return self.refuse_log(id, source_problem(error)),
        };
        if self.logs_run.runs.len() >= MAX_OPEN_LOGS {
            let count = MAX_OPEN_LOGS.to_string();
            let message = Message::new(i18n::text_with("logs.too_many", &[("count", &count)]));
            return self.refuse_log(id, LogState::Unavailable(message));
        }
        let run = self.logs_run.next_run;
        self.logs_run.next_run += 1;
        let outbox = Outbox { id, run, internal: self.internal.clone() };
        let source = source.clone();
        let cancel = self.tasks.spawn("log-follow", Scope::Background, move |cancel| async move {
            tokio::select! {
                biased;
                () = cancelled(cancel) => {}
                () = follow(runner, script, source, outbox) => {}
            }
        });
        self.logs_run.runs.insert(id, LogRun { session, run, cancel });
    }

    pub(crate) fn stop_log(&mut self, id: LogId) {
        if let Some(log) = self.logs_run.runs.remove(&id) {
            log.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn lose_logs_of(&mut self, session: SessionId) {
        let mut lost: Vec<LogId> =
            self.logs_run.runs.iter().filter(|(_, log)| log.session == session).map(|(id, _)| *id).collect();
        lost.sort_unstable();
        for id in lost {
            self.stop_log(id);
            self.emit(Event::Log { id, state: LogState::Ended(Message::new(i18n::text("logs.connection_lost"))) });
        }
    }

    fn current_log(&self, id: LogId, run: u64) -> bool {
        self.logs_run.runs.get(&id).is_some_and(|log| log.run == run)
    }

    pub(crate) fn log_progressed(&mut self, id: LogId, run: u64, state: LogState) {
        if self.current_log(id, run) {
            self.emit(Event::Log { id, state });
        }
    }

    pub(crate) fn log_ended(&mut self, id: LogId, run: u64, state: LogState) {
        if self.current_log(id, run) {
            self.logs_run.runs.remove(&id);
            self.emit(Event::Log { id, state });
        }
    }
}

#[cfg(test)]
mod tests;
