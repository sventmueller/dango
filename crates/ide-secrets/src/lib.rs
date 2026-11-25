//! Where the app keeps what must not be in its settings — CI tokens, provider keys, dictation keys: the login
//! keychain under the app's service, one account per secret, or memory for tests and the self-test.
//!
//! ```
//! use ide_secrets::{Memory, SecretStore};
//!
//! let store = Memory::default();
//! store.set("ci:github.com", "token")?;
//! assert_eq!(store.get("ci:github.com")?.as_deref(), Some("token"));
//! store.delete("ci:github.com")?;
//! assert_eq!(store.get("ci:github.com")?, None);
//! # Ok::<(), ide_secrets::SecretError>(())
//! ```

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use ide_core::LockIgnoringPoison;

/// errSecItemNotFound.
const NOT_FOUND: i32 = -25300;

/// Why the keychain refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretError {
    /// The keychain's status code.
    pub(crate) status: i32,
    /// What the system says about it.
    pub(crate) message: String,
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the keychain refused ({}): {}", self.status, self.message)
    }
}

impl ide_core::explain::Explain for SecretError {
    fn explain(&self) -> String {
        splat_i18n::tf("Der Schlüsselbund lehnt ab: {}", &[&self.message])
    }
}

impl std::error::Error for SecretError {}

impl From<security_framework::base::Error> for SecretError {
    fn from(error: security_framework::base::Error) -> SecretError {
        SecretError {
            status: error.code(),
            message: error.to_string(),
        }
    }
}

/// Where secrets are kept.
pub trait SecretStore: Send + Sync {
    /// The secret saved for `account`.
    fn get(&self, account: &str) -> Result<Option<String>, SecretError>;
    /// Saves `secret` for `account`, in place of what was there.
    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError>;
    /// Forgets the secret of `account`; nothing to forget is no error.
    fn delete(&self, account: &str) -> Result<(), SecretError>;
}

/// The login keychain under one service name.
pub struct Keychain {
    service: String,
}

impl Keychain {
    /// The keychain items of `service`, which is the app's bundle identifier.
    pub fn new(service: &str) -> Keychain {
        Keychain {
            service: service.to_string(),
        }
    }

    /// The keychain items of this build of the app, named like its data folders.
    pub fn of_app() -> Keychain {
        Keychain::new(ide_core::paths::identity())
    }
}

impl SecretStore for Keychain {
    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        match security_framework::passwords::get_generic_password(&self.service, account) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        Ok(security_framework::passwords::set_generic_password(
            &self.service,
            account,
            secret.as_bytes(),
        )?)
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        match security_framework::passwords::delete_generic_password(&self.service, account) {
            Err(e) if e.code() != NOT_FOUND => Err(e.into()),
            _ => Ok(()),
        }
    }
}

/// A store that refuses everything, for code that reads secrets before the app handed its store over: a key then
/// fails to save in the user's sight instead of vanishing with the process.
pub struct NotReady;

impl NotReady {
    fn refused() -> SecretError {
        SecretError {
            status: 0,
            message: "the app's store of secrets is not ready".to_string(),
        }
    }
}

impl SecretStore for NotReady {
    fn get(&self, _account: &str) -> Result<Option<String>, SecretError> {
        Err(NotReady::refused())
    }

    fn set(&self, _account: &str, _secret: &str) -> Result<(), SecretError> {
        Err(NotReady::refused())
    }

    fn delete(&self, _account: &str) -> Result<(), SecretError> {
        Err(NotReady::refused())
    }
}

/// Secrets kept in memory, for tests and the self-test, which leave the keychain alone.
#[derive(Default)]
pub struct Memory {
    secrets: Mutex<HashMap<String, String>>,
}

impl SecretStore for Memory {
    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        Ok(self.secrets.lock_ignoring_poison().get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        self.secrets
            .lock_ignoring_poison()
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        self.secrets.lock_ignoring_poison().remove(account);
        Ok(())
    }
}

/// Another store with what it answered and was given remembered, so each secret is read from it once: the keychain
/// answers through another process, too slow for a menu that asks for the keys of every endpoint as it opens. A change
/// made outside the app, as in Keychain Access, shows after the next launch.
pub struct Remembered<S> {
    store: S,
    known: Mutex<HashMap<String, Option<String>>>,
}

impl<S: SecretStore> Remembered<S> {
    /// `store`, remembering.
    pub fn new(store: S) -> Remembered<S> {
        Remembered {
            store,
            known: Mutex::new(HashMap::new()),
        }
    }
}

impl<S: SecretStore> SecretStore for Remembered<S> {
    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        if let Some(known) = self.known.lock_ignoring_poison().get(account) {
            return Ok(known.clone());
        }
        let read = self.store.get(account)?;
        self.known
            .lock_ignoring_poison()
            .insert(account.to_string(), read.clone());
        Ok(read)
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        let result = self.store.set(account, secret);
        let mut known = self.known.lock_ignoring_poison();
        match &result {
            Ok(()) => known.insert(account.to_string(), Some(secret.to_string())),
            Err(_) => known.remove(account),
        };
        result
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        let result = self.store.delete(account);
        let mut known = self.known.lock_ignoring_poison();
        match &result {
            Ok(()) => known.insert(account.to_string(), None),
            Err(_) => known.remove(account),
        };
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Memory that counts how often it was read.
    #[derive(Default)]
    struct Counted {
        memory: Memory,
        reads: std::sync::atomic::AtomicUsize,
    }

    impl SecretStore for Counted {
        fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.memory.get(account)
        }

        fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
            self.memory.set(account, secret)
        }

        fn delete(&self, account: &str) -> Result<(), SecretError> {
            self.memory.delete(account)
        }
    }

    #[test]
    fn a_remembered_store_reads_each_secret_once_and_knows_what_it_was_given() {
        let store = Remembered::new(Counted::default());
        assert_eq!(store.get("a").unwrap(), None);
        assert_eq!(store.get("a").unwrap(), None);
        assert_eq!(store.store.reads.load(std::sync::atomic::Ordering::Relaxed), 1);
        store.set("a", "1").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("1"));
        store.delete("a").unwrap();
        assert_eq!(store.get("a").unwrap(), None);
        assert_eq!(store.store.reads.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(
            store.store.memory.get("a").unwrap(),
            None,
            "the change reached the store"
        );
    }

    #[test]
    fn a_remembered_store_that_refuses_is_asked_again() {
        let store = Remembered::new(NotReady);
        assert!(store.get("a").is_err());
        assert!(store.set("a", "1").is_err());
        assert!(store.get("a").is_err(), "nothing was remembered");
    }

    #[test]
    fn a_refusal_names_its_status() {
        let error = SecretError {
            status: -25293,
            message: "denied".into(),
        };
        assert_eq!(error.to_string(), "the keychain refused (-25293): denied");
    }

    #[test]
    fn memory_replaces_and_forgets() {
        let store = Memory::default();
        store.set("a", "1").unwrap();
        store.set("a", "2").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("2"));
        store.delete("a").unwrap();
        store.delete("a").unwrap();
        assert_eq!(store.get("a").unwrap(), None);
    }

    #[test]
    fn the_user_reads_the_systems_words_without_the_status_code() {
        use ide_core::explain::Explain;
        let error = SecretError {
            status: -25293,
            message: "denied".into(),
        };
        let words = error.explain();
        assert!(words.contains("denied") && !words.contains("-25293"), "{words}");
    }

    #[test]
    fn a_keychain_error_keeps_its_status_and_the_systems_message() {
        let error = SecretError::from(security_framework::base::Error::from_code(NOT_FOUND));
        assert_eq!(error.status, NOT_FOUND);
        assert!(!error.message.is_empty());
        assert!(error.to_string().starts_with("the keychain refused (-25300): "));
    }

    #[test]
    fn a_store_that_is_not_ready_refuses_every_call_in_words() {
        let store = NotReady;
        let refused = NotReady::refused();
        assert_eq!(store.get("a"), Err(refused.clone()));
        assert_eq!(store.set("a", "1"), Err(refused.clone()));
        assert_eq!(store.delete("a"), Err(refused.clone()));
        assert!(refused.to_string().contains("not ready"));
    }

    #[test]
    fn the_apps_keychain_is_named_like_its_data_folders() {
        assert_eq!(Keychain::new("com.example.ide").service, "com.example.ide");
        assert_eq!(Keychain::of_app().service, ide_core::paths::identity());
    }
}
