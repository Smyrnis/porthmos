use super::super::{Engine, Event};
use crate::{Severity, last_used::LastUsed};

pub(crate) struct LastUsedLog {
    pub(crate) store: LastUsed,
    write_warned: bool,
}

impl LastUsedLog {
    pub(crate) fn new(store: LastUsed) -> Self {
        Self { store, write_warned: false }
    }
}

impl Engine {
    pub(crate) fn publish_last_used(&self) {
        self.emit(Event::LastUsed(self.last_used.store.times()));
    }

    pub(crate) fn record_use(&mut self, name: &str) {
        let now = (self.clock)();
        self.last_used.store.record(name, now);
        self.last_used_changed();
    }

    pub(crate) fn carry_last_used(&mut self, from: &str, to: &str) {
        if self.last_used.store.rename(from, to) {
            self.last_used_changed();
        }
    }

    pub(crate) fn merge_last_used(&mut self, from: &str, to: &str) {
        if self.last_used.store.merge(from, to) {
            self.last_used_changed();
        }
    }

    pub(crate) fn forget_last_used(&mut self, name: &str) {
        if self.last_used.store.forget(name) {
            self.last_used_changed();
        }
    }

    fn last_used_changed(&mut self) {
        self.persist_last_used();
        self.publish_last_used();
    }

    fn persist_last_used(&mut self) {
        if !self.last_used.store.is_writable() {
            return self.last_used_write_failed(
                "the existing file could not be read or set aside, so it was left alone".to_string(),
            );
        }
        let path = self.last_used.store.path().to_path_buf();
        let outcome = self
            .last_used
            .store
            .render()
            .map_err(|err| format!("{err:#}"))
            .and_then(|text| self.writer.write(path, text.into_bytes(), 0o600));
        if let Err(message) = outcome {
            self.last_used_write_failed(message);
        }
    }

    pub(crate) fn last_used_write_failed(&mut self, message: String) {
        if self.last_used.write_warned {
            tracing::debug!("{message}");
            return;
        }
        self.last_used.write_warned = true;
        self.report(Severity::Warning, format!("Couldn't save last-used times: {message}"));
    }
}

#[cfg(test)]
mod tests;
