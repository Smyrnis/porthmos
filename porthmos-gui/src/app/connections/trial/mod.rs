use iced::Task;
use iced::widget::Id;
use porthmos_core::{Answer, Command, Question, RequestId, TestOutcome};

use super::super::{App, Message};
use crate::connections::Screen;
use crate::connections::trial::{TestPrompt, TestState};

impl App {
    pub fn test_password_id() -> Id {
        Id::new("test-password")
    }

    pub fn form_prompt_open(&self) -> bool {
        matches!(&self.connections_screen, Some(Screen::Form(state)) if state.prompt.is_some())
    }

    pub(in crate::app::connections) fn cancel_running_test(&mut self) {
        if self.running_test.take().is_some() {
            self.core.send(Command::CancelTest);
        }
        if let Some(state) = self.form_mut() {
            state.test = None;
            state.prompt = None;
        }
    }

    pub(in crate::app::connections) fn cancel_prompt(&mut self) {
        let Some(prompt) = self.form_mut().and_then(|state| state.prompt.take()) else {
            return;
        };
        self.core.send(Command::Answer { request_id: prompt.request_id, answer: None, save: false });
    }

    fn start_test(&mut self) {
        if self.running_test.is_some() {
            return;
        }
        let Some(state) = self.form_mut() else {
            return;
        };
        if let Err(message) = state.check() {
            state.error = Some(message);
            return;
        }
        state.error = None;
        state.test = Some(TestState::Running);
        let original = state.original_name().map(str::to_string);
        let draft = state.draft();
        let test_id = self.next_test_id;
        self.next_test_id += 1;
        self.running_test = Some(test_id);
        self.core.send(Command::TestConnection { test_id, original, draft: Box::new(draft) });
    }

    fn submit_prompt(&mut self) {
        let Some(prompt) = self.form_mut().and_then(|state| state.prompt.take()) else {
            return;
        };
        let password = prompt.text.into_string();
        self.core.send(Command::Answer {
            request_id: prompt.request_id,
            answer: Some(Answer::Password(password)),
            save: false,
        });
    }

    pub(in crate::app) fn test_finished(&mut self, test_id: u64, outcome: TestOutcome) {
        if self.running_test != Some(test_id) {
            return;
        }
        self.running_test = None;
        if let Some(state) = self.form_mut() {
            state.prompt = None;
            state.test = (outcome != TestOutcome::Cancelled).then_some(TestState::Finished(outcome));
        }
    }

    pub(in crate::app) fn test_question(&mut self, test_id: u64, request_id: RequestId, question: Question) {
        if self.running_test != Some(test_id) {
            return;
        }
        match question {
            Question::Password { username, name } => {
                if let Some(state) = self.form_mut() {
                    state.prompt = Some(TestPrompt::new(request_id, username, name));
                    self.prompt_version += 1;
                } else {
                    self.core.send(Command::Answer { request_id, answer: None, save: false });
                }
            }
            Question::TrustHostKey { .. } | Question::TrustCertificate { .. } => {
                self.core.send(Command::Answer { request_id, answer: None, save: false });
            }
        }
    }

    pub(in crate::app) fn trial_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FormTestPressed => self.start_test(),
            Message::FormTestCancelled => self.cancel_running_test(),
            Message::TestPasswordTyped(typed) => {
                if let Some(prompt) = self.form_mut().and_then(|state| state.prompt.as_mut()) {
                    prompt.text = typed;
                }
            }
            Message::TestPasswordSubmitted => self.submit_prompt(),
            Message::TestPasswordCancelled => self.cancel_prompt(),
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests;
