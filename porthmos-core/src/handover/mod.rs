use std::{
    fmt,
    sync::{Arc, Mutex},
};

pub struct Handover<T>(Arc<Mutex<Option<T>>>);

impl<T> Handover<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(Mutex::new(Some(value))))
    }

    pub fn take(&self) -> Option<T> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
    }
}

impl<T> Clone for Handover<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> fmt::Debug for Handover<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Handover(..)")
    }
}

#[cfg(test)]
mod tests;
