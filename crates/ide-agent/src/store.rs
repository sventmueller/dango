//! A thread's conversation on disk, so it can be resumed: the messages as the providers see them, the title, the
//! model and the tokens spent. The cards are rebuilt from the messages.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::conversation::Message;

/// The format's version; a file of another one is not read.
const VERSION: u32 = 1;

/// What a stored thread holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    /// The format's version.
    pub version: u32,
    /// The thread's title.
    pub title: String,
    /// The endpoint it talked to, by id.
    pub endpoint: String,
    /// The model it used last.
    pub model: String,
    /// The conversation.
    pub messages: Vec<Message>,
    /// The tokens and cost so far.
    pub totals: Totals,
    /// The same, by model.
    #[serde(default)]
    pub by_model: BTreeMap<String, Totals>,
}

/// Why a stored thread could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The session was started without a store.
    NoStore,
    /// The file could not be read or is not a stored thread; the system's or the parser's words.
    Unreadable(String),
    /// The file was written in another version of the format.
    Version(u32),
}

/// Tokens and cost of a thread so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Totals {
    /// Input tokens neither read from nor written to the cache.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Input tokens read from the cache.
    pub cache_read: u64,
    /// Input tokens written to the cache.
    pub cache_write: u64,
    /// What it cost, as far as the prices are known.
    pub cost_usd: Option<f64>,
}

/// Reads the thread stored at `path`.
pub fn load(path: &Path) -> Result<Stored, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|e| LoadError::Unreadable(e.to_string()))?;
    let stored: Stored = serde_json::from_str(&text).map_err(|e| LoadError::Unreadable(e.to_string()))?;
    if stored.version != VERSION {
        return Err(LoadError::Version(stored.version));
    }
    Ok(stored)
}

/// Writes `stored` to `path` whole: to a temporary file beside it, then renamed over it.
pub fn save(path: &Path, stored: &Stored) -> Result<(), String> {
    let mut stored = stored.clone();
    stored.version = VERSION;
    let text = serde_json::to_vec(&stored).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let temporary = path.with_extension("json.part");
    std::fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_is_read_back_as_it_was_written_and_a_foreign_version_is_refused() {
        let path = std::env::temp_dir().join(format!("ide-agent-store-{}/t.json", std::process::id()));
        let stored = Stored {
            title: "Fix the build".into(),
            endpoint: "anthropic".into(),
            model: "claude-opus-5-5".into(),
            messages: vec![Message::user("hi")],
            totals: Totals {
                input: 3,
                cost_usd: Some(0.5),
                ..Totals::default()
            },
            ..Stored::default()
        };
        save(&path, &stored).unwrap();
        let read = load(&path).unwrap();
        assert_eq!(
            (read.title.as_str(), read.messages.len(), read.totals.cost_usd),
            ("Fix the build", 1, Some(0.5))
        );
        std::fs::write(
            &path,
            r#"{"version": 9, "title": "", "endpoint": "", "model": "", "messages": [], "totals": {}}"#,
        )
        .unwrap();
        assert_eq!(load(&path), Err(LoadError::Version(9)));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
