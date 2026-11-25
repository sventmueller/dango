//! API keys, kept in the login keychain under the app's service and one account per endpoint; never in settings.

pub use ide_secrets::{Keychain, Memory, NotReady, Remembered, SecretError, SecretStore};

use crate::provider::Endpoint;

/// The keychain account of `endpoint`'s key.
pub fn account(endpoint: &Endpoint) -> String {
    format!("agent.{}", endpoint.id)
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
pub fn key_for(store: &dyn SecretStore, endpoint: &Endpoint) -> Result<Option<ApiKey>, SecretError> {
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
