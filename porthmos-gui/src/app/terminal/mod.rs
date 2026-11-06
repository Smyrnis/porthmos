use porthmos_core::edit::EditorExit;
use porthmos_core::{Command, Severity, ShellInvocation};

use super::editing::JobRole;
use super::{App, PanelSide};
use crate::editing::CommandSpec;
use crate::processes::ProcessJob;

impl App {
    pub fn terminal_possible(&self) -> bool {
        match self.active_side() {
            PanelSide::Local => true,
            PanelSide::Remote => self.sessions.active().is_some_and(|session| session.shell_available),
        }
    }

    pub(in crate::app) fn request_terminal(&mut self) {
        if self.active_side() == PanelSide::Local {
            let directory = self.local.panel.path().clone();
            self.core.send(Command::PrepareLocalShell { directory });
            return;
        }
        let Some(session) = self.sessions.active() else {
            return self.push_notice(Severity::Warning, "Connect to a server first".to_string());
        };
        if !session.shell_available {
            return self
                .push_notice(Severity::Warning, "This connection doesn't support a terminal session".to_string());
        }
        let directory = Some(session.pane.panel.path().clone());
        self.core.send(Command::PrepareShell { session: session.id, directory });
    }

    pub(in crate::app) fn shell_ready(&mut self, invocation: ShellInvocation) {
        let ShellInvocation { program, args, env } = invocation;
        let inner = CommandSpec { program, args };
        match self.launcher.terminal(inner, env, false, self.gui_settings.terminal.as_deref()) {
            Ok(launch) => {
                let token = self.next_process_token;
                self.next_process_token += 1;
                self.jobs.insert(token, JobRole::Terminal);
                self.launcher.spawn(ProcessJob { token, spec: launch.spec, env: launch.env, cleanup: launch.cleanup });
            }
            Err(error) => self.push_notice(Severity::Error, format!("Couldn't open a terminal: {error}")),
        }
    }

    pub(in crate::app) fn terminal_finished(&mut self, exit: EditorExit) {
        if let EditorExit::LaunchFailed(message) = exit {
            self.push_notice(Severity::Error, format!("Couldn't open a terminal: {message}"));
        }
    }
}

#[cfg(test)]
mod tests;
