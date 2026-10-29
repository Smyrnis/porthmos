use super::super::App;

const NO_KEYRING_HINT: &str = "Passwords are not saved: no system keyring. They are kept until Porthmos quits.";

impl App {
    pub fn keyring_hint(&self) -> Option<&'static str> {
        (!self.keyring_available).then_some(NO_KEYRING_HINT)
    }

    pub fn keyring_waiting(&self) -> bool {
        self.keyring_waiting
    }
}

#[cfg(test)]
mod tests;
