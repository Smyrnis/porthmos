use super::super::{Engine, Internal};
use crate::tasks::Scope;

impl Engine {
    pub(crate) fn start_keyring_probe(&self) {
        let (secrets, internal) = (self.secrets.clone(), self.internal.clone());
        self.tasks.spawn("keyring-probe", Scope::Background, move |_| async move {
            let available = secrets.probe().await;
            let _ = internal.send(Internal::KeyringProbed { available });
        });
    }
}

#[cfg(test)]
mod tests;
