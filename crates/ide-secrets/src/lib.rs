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
#![warn(missing_docs, unreachable_pub)]

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
    pub status: i32,
    /// What the system says about it.
    pub message: String,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
