//! A thread's conversation on disk, so it can be resumed: the messages as the providers see them, the title, the
//! model and the tokens spent. The cards are rebuilt from the messages.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::conversation::Message;

/// The format's version; a file of another one is not read.
const VERSION: u32 = 1;

/// What a stored thread holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub(crate) version: u32,
    pub(crate) title: String,
    /// The endpoint it talked to, by id.
    pub endpoint: String,
    /// The model it used last.
    pub model: String,
    pub(crate) messages: Vec<Message>,
    /// The tokens and cost so far.
    pub(crate) totals: Totals,
    /// The same, by model.
    #[serde(default)]
    pub(crate) by_model: BTreeMap<String, Totals>,
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
pub(crate) struct Totals {
    /// Input tokens neither read from nor written to the cache.
    pub(crate) input: u64,
    /// Output tokens.
    pub(crate) output: u64,
    /// Input tokens read from the cache.
    pub(crate) cache_read: u64,
    /// Input tokens written to the cache.
    pub(crate) cache_write: u64,
    /// What it cost, as far as the prices are known.
    pub(crate) cost_usd: Option<f64>,
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

/// Why a thread was not written.
#[derive(Debug)]
pub(crate) enum SaveError {
    /// It could not be written as JSON.
    Encode(serde_json::Error),
    /// Writing `path` failed.
    Io {
        /// The file or folder.
        path: PathBuf,
        /// The system's reason.
        error: std::io::Error,
    },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Encode(error) => write!(f, "the thread could not be written: {error}"),
            SaveError::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SaveError::Encode(error) => Some(error),
            SaveError::Io { error, .. } => Some(error),
        }
    }
}

/// Writes `stored` to `path` whole, with `ide_core::atomic_write`.
pub(crate) fn save(path: &Path, stored: &Stored) -> Result<(), SaveError> {
    let mut stored = stored.clone();
    stored.version = VERSION;
    let text = serde_json::to_vec(&stored).map_err(SaveError::Encode)?;
    ide_core::atomic_write::write(path, &text).map_err(|error| SaveError::Io {
        path: path.to_path_buf(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_is_read_back_as_it_was_written_and_a_foreign_version_is_refused() {
        let dir = ide_core::testing::scratch("agent-store");
        let path = dir.join("t.json");
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
    }
}
