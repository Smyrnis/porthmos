use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use porthmos_vfs::IncomingConnection;
use tokio::sync::mpsc;

type Entries = HashMap<(String, u32), mpsc::Sender<IncomingConnection>>;

#[derive(Clone, Default)]
pub(crate) struct ForwardRegistry(Arc<Mutex<Entries>>);

impl ForwardRegistry {
    fn entries(&self) -> MutexGuard<'_, Entries> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn register(&self, address: &str, port: u32, sender: mpsc::Sender<IncomingConnection>) {
        self.entries().insert((address.to_string(), port), sender);
    }

    pub(crate) fn unregister(&self, address: &str, port: u32) {
        self.entries().remove(&(address.to_string(), port));
    }

    pub(crate) fn route(&self, address: &str, port: u32) -> Option<mpsc::Sender<IncomingConnection>> {
        self.entries().get(&(address.to_string(), port)).cloned()
    }

    pub(crate) fn clear(&self) {
        self.entries().clear();
    }
}

#[cfg(test)]
mod tests;
