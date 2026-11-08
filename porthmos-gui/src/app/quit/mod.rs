use iced::{Task, window};

use super::{App, Message};
use crate::modal::Modal;
use crate::runtime;

impl App {
    pub(in crate::app) fn request_close(&mut self) -> Task<Message> {
        if self.closing {
            return Task::none();
        }
        let transfers = self.transfers.has_unfinished();
        let edits = self.edits_busy;
        if !transfers && !edits {
            return self.begin_close();
        }
        if self.modals.any(|modal| matches!(modal, Modal::ConfirmQuit { .. })) {
            return Task::none();
        }
        self.modals.push(Modal::ConfirmQuit { transfers, edits });
        window::gain_focus(self.windows.main())
    }

    pub(in crate::app) fn begin_close(&mut self) -> Task<Message> {
        if self.closing {
            return Task::none();
        }
        self.closing = true;
        self.save_window_memory();
        self.cancel_search_work();
        if let Some(job) = self.staging.take() {
            job.staging.remove();
        }
        let core = self.core.clone();
        let shutdown = Task::perform(runtime::shut_down(core, runtime::SHUTDOWN_BOUND), |()| Message::ShutdownFinished);
        let closing = self.windows.secondary().into_iter().map(window::close);
        Task::batch(closing.chain(std::iter::once(shutdown)))
    }
}

#[cfg(test)]
mod tests;
