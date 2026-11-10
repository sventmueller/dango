//! API keys, kept in the login keychain under the app's service and one account per endpoint; never in settings.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::provider::Endpoint;

/// errSecItemNotFound.
const NOT_FOUND: i32 = -25300;

/// Where the keys are kept.
pub trait SecretStore: Send + Sync {
    /// The secret saved for `account`.
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    /// Saves `secret` for `account`, in place of what was there.
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    /// Forgets the secret of `account`.
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// The keychain account of `endpoint`'s key.
pub fn account(endpoint: &Endpoint) -> String {
    format!("agent.{}", endpoint.id)
}

/// The login keychain under one service name.
pub struct Keychain {
    service: String,
}

impl Keychain {
    /// The keychain items of `service`.
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
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        match security_framework::passwords::get_generic_password(&self.service, account) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        security_framework::passwords::set_generic_password(&self.service, account, secret.as_bytes())
            .map_err(|e| e.to_string())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        match security_framework::passwords::delete_generic_password(&self.service, account) {
            Err(e) if e.code() != NOT_FOUND => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

/// Secrets in memory, for tests and the self-test, which leave the keychain alone.
#[derive(Default)]
pub struct Memory {
    secrets: Mutex<HashMap<String, String>>,
}

impl SecretStore for Memory {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self
            .secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(account)
            .cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.secrets.lock().unwrap_or_else(|e| e.into_inner()).remove(account);
        Ok(())
    }
}

/// A key as it is passed around: its text never shows in debug output or logs.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// The key `text`, unless it is blank.
    pub fn new(text: &str) -> Option<ApiKey> {
        let text = text.trim();
        (!text.is_empty()).then(|| ApiKey(text.to_string()))
    }

    /// The key's text, for the request header only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey(…)")
    }
}

/// The key `store` keeps for `endpoint`.
pub fn key_for(store: &dyn SecretStore, endpoint: &Endpoint) -> Result<Option<ApiKey>, String> {
    Ok(store.get(&account(endpoint))?.as_deref().and_then(ApiKey::new))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;

    #[test]
    fn a_key_is_kept_per_endpoint_and_never_printed() {
        let store = Memory::default();
        let anthropic = Endpoint::preset(ProviderKind::Anthropic).unwrap();
        let openai = Endpoint::preset(ProviderKind::OpenAi).unwrap();
        store.set(&account(&anthropic), " sk-ant-secret \n").unwrap();
        let key = key_for(&store, &anthropic).unwrap().unwrap();
        assert_eq!(key.expose(), "sk-ant-secret");
        assert!(!format!("{key:?}").contains("secret"));
        assert_eq!(key_for(&store, &openai).unwrap(), None);
        store.delete(&account(&anthropic)).unwrap();
        assert_eq!(key_for(&store, &anthropic).unwrap(), None);
    }
}
