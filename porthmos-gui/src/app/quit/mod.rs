use iced::Task;

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
        if !self.modals.any(|modal| matches!(modal, Modal::ConfirmQuit { .. })) {
            self.modals.push(Modal::ConfirmQuit { transfers, edits });
        }
        Task::none()
    }

    pub(in crate::app) fn begin_close(&mut self) -> Task<Message> {
        if self.closing {
            return Task::none();
        }
        self.closing = true;
        if let Some(job) = self.staging.take() {
            job.staging.remove();
        }
        let core = self.core.clone();
        Task::perform(runtime::shut_down(core, runtime::SHUTDOWN_BOUND), |()| Message::ShutdownFinished)
    }
}

#[cfg(test)]
mod tests;
