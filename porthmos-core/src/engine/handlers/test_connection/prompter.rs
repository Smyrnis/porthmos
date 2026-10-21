use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use porthmos_vfs::{Answer, Prompter, Question, async_trait};
use tokio::{sync::mpsc::UnboundedSender, time::Instant};

use crate::{
    NotTrusted,
    engine::{Event, RequestId, prompter::PendingQuestions},
};

#[derive(Default)]
pub(super) struct Shared {
    waited: Mutex<Duration>,
    waiting_since: Mutex<Option<Instant>>,
    login: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Shared {
    fn begin_wait(&self) {
        *lock(&self.waiting_since) = Some(Instant::now());
    }

    fn end_wait(&self) {
        if let Some(since) = lock(&self.waiting_since).take() {
            *lock(&self.waited) += since.elapsed();
        }
    }

    pub(super) fn waiting_total(&self) -> Duration {
        let open = lock(&self.waiting_since).map_or(Duration::ZERO, |since| since.elapsed());
        *lock(&self.waited) + open
    }

    pub(super) fn login_started(&self) -> bool {
        self.login.load(Ordering::Relaxed)
    }
}

struct Forget {
    questions: PendingQuestions,
    request_id: RequestId,
}

impl Drop for Forget {
    fn drop(&mut self) {
        self.questions.forget(self.request_id);
    }
}

pub(super) struct TestPrompter {
    pub(super) questions: PendingQuestions,
    pub(super) events: UnboundedSender<Event>,
    pub(super) test_id: u64,
    pub(super) shared: Arc<Shared>,
    pub(super) untrusted: Option<NotTrusted>,
}

#[async_trait]
impl Prompter for TestPrompter {
    async fn ask(&mut self, question: Question) -> Option<Answer> {
        match question {
            Question::Password { .. } => {
                let (request_id, receiver) = self.questions.register();
                let _forget = Forget { questions: self.questions.clone(), request_id };
                self.shared.begin_wait();
                let asked = self.events.send(Event::TestQuestion { test_id: self.test_id, request_id, question });
                let answer = if asked.is_ok() { receiver.await.ok().flatten() } else { None };
                self.shared.end_wait();
                match answer {
                    Some((Answer::Password(secret), _)) => {
                        self.shared.login.store(true, Ordering::Relaxed);
                        Some(Answer::Password(secret))
                    }
                    _ => None,
                }
            }
            Question::TrustHostKey { key_type, fingerprint, .. } => {
                self.untrusted = Some(NotTrusted::HostKey { key_type, fingerprint });
                None
            }
            Question::TrustCertificate { fingerprint, subject, .. } => {
                self.untrusted = Some(NotTrusted::Certificate { fingerprint, subject });
                None
            }
        }
    }
}
