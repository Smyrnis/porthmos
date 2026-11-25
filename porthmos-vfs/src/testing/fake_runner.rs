use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::anyhow;
use async_trait::async_trait;

use crate::{CommandOutput, CommandRunner, ErrorKind, ProtocolError};

#[derive(Clone)]
enum Answer {
    Output(CommandOutput),
    Failure(ErrorKind, String),
}

type Responder = Box<dyn Fn(&str) -> Result<CommandOutput, ProtocolError> + Send + Sync>;

#[derive(Default)]
struct State {
    responder: Option<Responder>,
    answers: VecDeque<Answer>,
    commands: Vec<String>,
    shells: Vec<String>,
    hang_next: bool,
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
}
