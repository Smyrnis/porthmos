use std::{any::Any, collections::HashMap, sync::mpsc, time::Duration};

use keyring_core::api::{CredentialApi, CredentialStoreApi};

use super::*;

struct WatchedStore {
    dropped: mpsc::Sender<bool>,
}

impl CredentialStoreApi for WatchedStore {
    fn vendor(&self) -> String {
        "test".to_string()
    }

    fn id(&self) -> String {
        "watched".to_string()
    }

    fn build(&self, _: &str, _: &str, _: Option<&HashMap<&str, &str>>) -> keyring_core::Result<Entry> {
        Err(Error::NoEntry)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Drop for WatchedStore {
    fn drop(&mut self) {
        let _ = self.dropped.send(tokio::runtime::Handle::try_current().is_ok());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_opened_store_is_never_dropped_on_a_runtime_thread() {
    let (sender, receiver) = mpsc::channel();
    let backend = KeyringBackend::with_store(Arc::new(WatchedStore { dropped: sender }));

    tokio::spawn(async move { drop(backend) }).await.unwrap();

    let dropped_inside_a_runtime = receiver.recv_timeout(Duration::from_secs(2)).expect("the store was dropped");
    assert!(!dropped_inside_a_runtime);
}

#[test]
fn the_opened_store_is_dropped_exactly_once_even_outside_a_runtime() {
    let (sender, receiver) = mpsc::channel();
    let backend = KeyringBackend::with_store(Arc::new(WatchedStore { dropped: sender }));

    drop(backend);

    assert!(!receiver.recv_timeout(Duration::from_secs(2)).expect("the store was dropped"));
    assert!(receiver.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn a_backend_whose_store_was_never_opened_drops_quietly() {
    drop(KeyringBackend::lazy());
}

struct TooLongStore {
    attribute: &'static str,
    limit: u32,
}

#[derive(Debug)]
struct TooLongCredential {
    attribute: &'static str,
    limit: u32,
}

impl CredentialStoreApi for TooLongStore {
    fn vendor(&self) -> String {
        "test".to_string()
    }

    fn id(&self) -> String {
        "too-long".to_string()
    }

    fn build(&self, _: &str, _: &str, _: Option<&HashMap<&str, &str>>) -> keyring_core::Result<Entry> {
        Ok(Entry::new_with_credential(Arc::new(TooLongCredential { attribute: self.attribute, limit: self.limit })))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CredentialApi for TooLongCredential {
    fn set_secret(&self, _: &[u8]) -> keyring_core::Result<()> {
        Err(Error::TooLong(self.attribute.to_string(), self.limit))
    }

    fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
        Err(Error::NoEntry)
    }

    fn delete_credential(&self) -> keyring_core::Result<()> {
        Err(Error::NoEntry)
    }

    fn get_credential(&self) -> keyring_core::Result<Option<Arc<keyring_core::api::Credential>>> {
        Ok(None)
    }

    fn get_specifiers(&self) -> Option<(String, String)> {
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn backend_refusing(attribute: &'static str, limit: u32) -> KeyringBackend {
    KeyringBackend::with_store(Arc::new(TooLongStore { attribute, limit }))
}

#[test]
fn a_password_the_store_finds_too_long_names_the_limit_in_characters() {
    let error = backend_refusing("password encoded as UTF-16", 2560).set("profile:x", "secret").unwrap_err();

    assert_eq!(error, SecretError::Backend("too long; the keyring accepts at most 1280 characters".to_string()));
}

#[test]
fn any_other_too_long_attribute_keeps_the_stores_numbers() {
    let error = backend_refusing("user", 256).set("profile:x", "secret").unwrap_err();

    assert_eq!(error, SecretError::Backend("user is too long (limit 256)".to_string()));
}

#[cfg(windows)]
struct TestAccount(String);

#[cfg(windows)]
impl TestAccount {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(format!("porthmos:test:{}-{nanos}", std::process::id()))
    }
}

#[cfg(windows)]
impl Drop for TestAccount {
    fn drop(&mut self) {
        let _ = KeyringBackend::lazy().delete(&self.0);
    }
}

#[cfg(windows)]
#[test]
fn a_secret_written_by_one_backend_is_read_by_a_new_one() {
    let account = TestAccount::new();
    let writer = KeyringBackend::lazy();
    writer.set(&account.0, "s3cret \u{fc}").unwrap();
    drop(writer);

    let reader = KeyringBackend::lazy();
    assert_eq!(reader.get(&account.0).unwrap().as_deref(), Some("s3cret \u{fc}"));
    reader.delete(&account.0).unwrap();
    assert_eq!(reader.get(&account.0).unwrap(), None);
}

#[cfg(windows)]
#[test]
fn deleting_a_missing_secret_is_not_an_error() {
    let account = TestAccount::new();
    assert!(KeyringBackend::lazy().delete(&account.0).is_ok());
}

#[cfg(windows)]
#[test]
fn a_secret_of_1280_characters_fits_and_1281_is_refused() {
    let account = TestAccount::new();
    let backend = KeyringBackend::lazy();

    assert!(backend.set(&account.0, &"a".repeat(1280)).is_ok());
    assert!(matches!(backend.set(&account.0, &"a".repeat(1281)), Err(SecretError::Backend(_))));
}
