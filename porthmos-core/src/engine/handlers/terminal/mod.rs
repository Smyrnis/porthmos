use porthmos_vfs::ShellSize;

use crate::{
    Handover, Message,
    engine::{Engine, Event, SessionId, TerminalId},
    error, i18n,
    tasks::Scope,
};

const TERMINAL_TYPE: &str = "xterm-256color";

impl Engine {
    pub(crate) fn open_terminal(&mut self, id: TerminalId, session: SessionId, size: ShellSize) {
        let Some(live) = self.book.sessions.get(&session) else {
            let message = Message::from(i18n::text("common.session_disconnected"));
            self.emit(Event::TerminalOpenFailed { id, message });
            return;
        };
        let fs = live.fs.clone();
        let events = self.events.clone();
        self.tasks.spawn("terminal", Scope::Background, move |_| async move {
            let event = match fs.open_shell(TERMINAL_TYPE, size, &[]).await {
                Ok(stream) => Event::TerminalOpened { id, session, stream: Handover::new(stream) },
                Err(err) => Event::TerminalOpenFailed {
                    id,
                    message: error::failure_report(i18n::text("connection.terminal_failed"), &err),
                },
            };
            let _ = events.send(event);
        });
    }
}

#[cfg(test)]
mod tests;
