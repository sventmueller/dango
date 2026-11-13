//! Where the app keeps its data on this Mac.

use std::path::PathBuf;
use std::sync::OnceLock;

static IDENTITY: OnceLock<String> = OnceLock::new();
static HOME: OnceLock<PathBuf> = OnceLock::new();

/// Names the data folders after `bundle_id`, so a debug build keeps apart from the release; only the first call counts.
pub fn set_identity(bundle_id: &str) {
    let _ = IDENTITY.set(bundle_id.to_string());
}

/// The bundle identifier the data folders are named after.
pub fn identity() -> &'static str {
    IDENTITY.get().map_or(ide_product::BUNDLE_ID, String::as_str)
}

/// Puts the data folders under `home` instead of the user's home folder, for self-tests; only the first call counts.
pub fn set_home(home: PathBuf) {
    let _ = HOME.set(home);
}

/// The user's home folder, or the one `set_home` chose.
pub fn home() -> PathBuf {
    if let Some(home) = HOME.get() {
        return home.clone();
    }
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// Settings, sessions and anything else that must survive: `~/Library/Application Support/<identity>`.
pub fn data_dir() -> PathBuf {
    home().join("Library/Application Support").join(identity())
}

/// Where the app installs language servers and debug adapters: `tools` in the data folder.
pub fn tools_dir() -> PathBuf {
    data_dir().join("tools")
}

/// What can be rebuilt, such as pasted images and downloaded tools' archives: `~/Library/Caches/<identity>`.
pub fn cache_dir() -> PathBuf {
    home().join("Library/Caches").join(identity())
}

/// Logs that stay on this Mac: `~/Library/Logs/<identity>`.
pub fn logs_dir() -> PathBuf {
    home().join("Library/Logs").join(identity())
}

/// The folder of a project's own settings, at its current name or, if only that exists, an earlier one.
pub fn project_dir(root: &std::path::Path) -> PathBuf {
    let current = root.join(ide_product::PROJECT_DIR);
    if current.exists() {
        return current;
    }
    ide_product::PREVIOUS_PROJECT_DIRS
        .iter()
        .map(|name| root.join(name))
        .find(|dir| dir.exists())
        .unwrap_or(current)
}

/// A name for the project at `root` in the app's folders: its folder's name, for a person looking, and a hash of the
/// whole path, which keeps two folders of the same name apart.
pub fn project_key(root: &std::path::Path) -> String {
    let name: String = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    format!("{name}-{:016x}", fnv1a(root.as_os_str().as_encoded_bytes()))
}

/// FNV-1a, whose value stays the same across Rust versions, unlike the standard library's hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_projects_key_names_its_folder_and_keeps_same_named_ones_apart() {
        let key = project_key(std::path::Path::new("/work/my shop"));
        assert!(key.starts_with("my_shop-"), "{key}");
        assert_ne!(key, project_key(std::path::Path::new("/other/my shop")));
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn data_lives_under_the_identity_in_the_library() {
        let dir = data_dir();
        assert!(dir.ends_with(identity()));
        assert!(dir.parent().unwrap().ends_with("Library/Application Support"));
        assert_ne!(data_dir(), cache_dir());
    }

    #[test]
    fn a_project_without_its_folder_gets_the_current_name() {
        let root = std::env::temp_dir().join("ide-core-no-project-folder");
        assert_eq!(project_dir(&root), root.join(ide_product::PROJECT_DIR));
    }
}
