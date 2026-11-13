//! Settings in two layers — the user's, and a project's where a setting allows it — and the app's own state, such as
//! the recently opened folders.

use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::store::{Migration, Store};

pub use crate::store::Locked;

/// The migrations of the settings file, in order; see `store::migrate`.
const SETTINGS_MIGRATIONS: &[Migration] = &[];

/// The migrations of the state file.
const STATE_MIGRATIONS: &[Migration] = &[];

/// Where a setting may be set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Only in the user's settings.
    User,
    /// In the user's settings, and in a project's, which wins there.
    Project,
}

/// A named setting of type `T` with its default.
pub struct Key<T> {
    /// The name in the JSON file.
    pub name: &'static str,
    /// Where it may be set.
    pub scope: Scope,
    default: fn() -> T,
    kind: PhantomData<fn() -> T>,
}

impl<T> Key<T> {
    /// A setting only the user sets.
    pub const fn user(name: &'static str, default: fn() -> T) -> Key<T> {
        Key {
            name,
            scope: Scope::User,
            default,
            kind: PhantomData,
        }
    }

    /// A setting a project may set for itself.
    pub const fn project(name: &'static str, default: fn() -> T) -> Key<T> {
        Key {
            name,
            scope: Scope::Project,
            default,
            kind: PhantomData,
        }
    }

    /// The value when nobody set one.
    pub fn default_value(&self) -> T {
        (self.default)()
    }
}

/// The settings of the app.
pub mod keys {
    use super::{Key, SearchEngine};

    /// Opens the folders that were open when the app quit.
    pub const REOPEN_PROJECTS: Key<bool> = Key::user("reopenProjects", || true);

    /// The size of text in editors and terminals, in points.
    pub const FONT_SIZE: Key<f64> = Key::project("fontSize", || 12.0);

    /// Formats a file with its language server when ⌘S saves it.
    pub const FORMAT_ON_SAVE: Key<bool> = Key::project("formatOnSave", || false);

    /// Shows spaces, tabs and whitespace at the end of lines in editors.
    pub const SHOW_WHITESPACE: Key<bool> = Key::user("showWhitespace", || false);
    /// Whether editors show the minimap beside the text (E107).
    pub const MINIMAP: Key<bool> = Key::user("minimap", || false);
    /// How many header lines Sticky Scroll holds at the top of an editor, 0 for none (E108).
    pub const STICKY_SCROLL_LINES: Key<usize> = Key::user("stickyScrollLines", || 5);

    /// Shows the language server's inlay hints, such as inferred types and parameter names, in editors (E83).
    pub const INLAY_HINTS: Key<bool> = Key::user("inlayHints", || true);

    /// The languages, by name, whose editors leave inlay hints out although they are on.
    pub const INLAY_HINTS_OFF: Key<Vec<String>> = Key::user("inlayHintsOffFor", Vec::new);

    /// Shows who changed the cursor's line last at its end, after the cursor rests there (E86).
    pub const BLAME_AT_LINE_END: Key<bool> = Key::user("blameAtLineEnd", || false);

    /// Whether a single click opens a file in the window's preview tab, which the next one replaces; off, every file
    /// gets a tab of its own (E62).
    pub const PREVIEW_TABS: Key<bool> = Key::user("previewTabs", || false);

    /// Whether a Markdown file opens with its preview beside it, which closes with its editor.
    pub const MARKDOWN_PREVIEW: Key<bool> = Key::user("markdownPreviewBeside", || true);

    /// Whether the file tree also lists what Git ignores and what the system hides, marked as such.
    pub const SHOW_IGNORED_FILES: Key<bool> = Key::user("showIgnoredFiles", || false);

    /// Where the browser's address field sends words that are no address.
    pub const SEARCH_ENGINE: Key<SearchEngine> = Key::user("searchEngine", || SearchEngine::DuckDuckGo);

    /// The panels whose symbol the activity bar leaves out, by the app's name for each panel.
    pub const HIDDEN_PANELS: Key<Vec<String>> = Key::user("hiddenSidebarPanels", Vec::new);

    /// ⌥ in the terminal is Meta for every key, instead of typing the characters the keyboard layout puts on it.
    pub const OPTION_AS_META: Key<bool> = Key::user("optionAsMeta", || false);
}

/// A search engine the browser can send words to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchEngine {
    /// duckduckgo.com, the default.
    DuckDuckGo,
    /// google.com.
    Google,
    /// bing.com.
    Bing,
    /// startpage.com.
    Startpage,
}

impl SearchEngine {
    /// Every engine, in the order the settings list them.
    pub const ALL: [SearchEngine; 4] = [
        SearchEngine::DuckDuckGo,
        SearchEngine::Google,
        SearchEngine::Bing,
        SearchEngine::Startpage,
    ];

    /// Its name, which no language translates.
    pub fn name(self) -> &'static str {
        match self {
            SearchEngine::DuckDuckGo => "DuckDuckGo",
            SearchEngine::Google => "Google",
            SearchEngine::Bing => "Bing",
            SearchEngine::Startpage => "Startpage",
        }
    }
}

/// What the state file holds.
pub mod remembered {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{AgentChoice, Key};

    /// The folders that were open when the app last quit.
    pub const OPEN_PROJECTS: Key<Vec<PathBuf>> = Key::user("openProjects", Vec::new);

    /// Recently opened folders, the latest first.
    pub const RECENT_PROJECTS: Key<Vec<PathBuf>> = Key::user("recentProjects", Vec::new);

    /// What the user picked last for each agent, by the agent's id, which its next sessions start with.
    pub const AGENT_CHOICES: Key<BTreeMap<String, AgentChoice>> = Key::user("agentChoices", BTreeMap::new);
}

/// The model, reasoning effort and permission mode the user picked for an agent, each as the agent names it; unset
/// leaves the agent's own default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentChoice {
    /// The model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// How much the model thinks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// What runs without asking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

/// How many folders the recent list keeps.
pub const RECENT_LIMIT: usize = 12;

/// The user's settings and, inside a project, the project's.
#[derive(Debug, Clone)]
pub struct Settings {
    user: Store,
    project: Option<Store>,
}

impl Settings {
    /// The user's settings from `path`.
    pub fn load(path: PathBuf) -> Settings {
        Settings {
            user: Store::load(path, SETTINGS_MIGRATIONS),
            project: None,
        }
    }

    /// The user's settings from the app's data folder.
    pub fn load_user() -> Settings {
        Settings::load(paths::data_dir().join("settings.json"))
    }

    /// These settings inside the project at `root`.
    #[must_use]
    pub fn in_project(mut self, root: &Path) -> Settings {
        self.project = Some(Store::load(
            paths::project_dir(root).join("settings.json"),
            SETTINGS_MIGRATIONS,
        ));
        self
    }

    /// The value of `key`: the project's where allowed, else the user's, else the default. A value of the wrong type
    /// counts as unset.
    pub fn get<T: DeserializeOwned>(&self, key: &Key<T>) -> T {
        let project = match key.scope {
            Scope::Project => self.project.as_ref().and_then(|p| p.get(key.name)),
            Scope::User => None,
        };
        project
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .or_else(|| {
                self.user
                    .get(key.name)
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
            })
            .unwrap_or_else(|| key.default_value())
    }

    /// Sets `key` in the user's settings and writes them.
    pub fn set<T: Serialize>(&mut self, key: &Key<T>, value: &T) -> io::Result<()> {
        let value = serde_json::to_value(value).map_err(io::Error::other)?;
        self.user.set(key.name, value, SETTINGS_MIGRATIONS)
    }

    /// Sets `key` in the project's settings and writes them.
    pub fn set_for_project<T: Serialize>(&mut self, key: &Key<T>, value: &T) -> io::Result<()> {
        if key.scope != Scope::Project {
            return Err(io::Error::other(format!("{} is set by the user only", key.name)));
        }
        let project = self
            .project
            .as_mut()
            .ok_or_else(|| io::Error::other("no project is open"))?;
        let value = serde_json::to_value(value).map_err(io::Error::other)?;
        project.set(key.name, value, SETTINGS_MIGRATIONS)
    }

    /// Settings files that are not written, and why: one the user should look at.
    pub fn problems(&self) -> Vec<(PathBuf, Locked)> {
        [Some(&self.user), self.project.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|store| store.locked().map(|locked| (store.path().to_path_buf(), locked)))
            .collect()
    }
}

/// What the app remembers between runs that the user does not set: open and recent folders.
#[derive(Debug, Clone)]
pub struct State {
    store: Store,
}

impl State {
    /// The state from `path`.
    pub fn load(path: PathBuf) -> State {
        State {
            store: Store::load(path, STATE_MIGRATIONS),
        }
    }

    /// The state from the app's data folder.
    pub fn load_user() -> State {
        State::load(paths::data_dir().join("state.json"))
    }

    /// The value of `key`, or its default.
    pub fn get<T: DeserializeOwned>(&self, key: &Key<T>) -> T {
        self.store
            .get(key.name)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| key.default_value())
    }

    /// Sets `key` and writes the state.
    pub fn set<T: Serialize>(&mut self, key: &Key<T>, value: &T) -> io::Result<()> {
        let value = serde_json::to_value(value).map_err(io::Error::other)?;
        self.store.set(key.name, value, STATE_MIGRATIONS)
    }

    /// Puts `folder` first in the recent folders.
    pub fn note_recent(&mut self, folder: &Path) -> io::Result<()> {
        let mut recent = self.get(&remembered::RECENT_PROJECTS);
        recent.retain(|p| p != folder);
        recent.insert(0, folder.to_path_buf());
        recent.truncate(RECENT_LIMIT);
        self.set(&remembered::RECENT_PROJECTS, &recent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::scratch;

    #[test]
    fn a_project_wins_only_where_the_setting_allows_it() {
        let dir = scratch("layers");
        let root = dir.join("project");
        let project_settings = root.join(ide_product::PROJECT_DIR);
        std::fs::create_dir_all(&project_settings).unwrap();
        std::fs::write(
            project_settings.join("settings.json"),
            r#"{"fontSize": 15, "reopenProjects": false}"#,
        )
        .unwrap();
        let mut settings = Settings::load(dir.join("user.json")).in_project(&root);
        assert_eq!(settings.get(&keys::FONT_SIZE), 15.0);
        assert!(
            settings.get(&keys::REOPEN_PROJECTS),
            "a user-only setting ignores the project"
        );
        settings.set(&keys::FONT_SIZE, &11.0).unwrap();
        assert_eq!(settings.get(&keys::FONT_SIZE), 15.0, "the project still wins");
        assert!(settings.set_for_project(&keys::REOPEN_PROJECTS, &false).is_err());
    }

    #[test]
    fn a_value_of_the_wrong_type_counts_as_unset() {
        let dir = scratch("wrong-type");
        std::fs::write(dir.join("user.json"), r#"{"fontSize": "large"}"#).unwrap();
        let settings = Settings::load(dir.join("user.json"));
        assert_eq!(settings.get(&keys::FONT_SIZE), keys::FONT_SIZE.default_value());
    }

    #[test]
    fn a_search_engine_is_stored_by_a_lasting_name() {
        let dir = scratch("search-engine");
        let mut settings = Settings::load(dir.join("user.json"));
        assert_eq!(settings.get(&keys::SEARCH_ENGINE), SearchEngine::DuckDuckGo);
        settings.set(&keys::SEARCH_ENGINE, &SearchEngine::Startpage).unwrap();
        let text = std::fs::read_to_string(dir.join("user.json")).unwrap();
        assert!(text.contains(r#""searchEngine": "startpage""#), "{text}");
        std::fs::write(dir.join("user.json"), r#"{"searchEngine": "altavista"}"#).unwrap();
        let settings = Settings::load(dir.join("user.json"));
        assert_eq!(settings.get(&keys::SEARCH_ENGINE), SearchEngine::DuckDuckGo);
    }

    #[test]
    fn a_damaged_settings_file_is_reported() {
        let dir = scratch("problems");
        std::fs::write(dir.join("user.json"), "[1, 2").unwrap();
        let settings = Settings::load(dir.join("user.json"));
        assert_eq!(settings.problems(), vec![(dir.join("user.json"), Locked::Damaged)]);
    }

    #[test]
    fn an_agents_choice_comes_back_and_a_missing_part_stays_unset() {
        let dir = scratch("agent-choices");
        let mut state = State::load(dir.join("state.json"));
        let mut choices = state.get(&remembered::AGENT_CHOICES);
        assert!(choices.is_empty());
        choices.insert(
            "claude".into(),
            AgentChoice {
                model: Some("opus".into()),
                effort: None,
                mode: Some("bypassPermissions".into()),
            },
        );
        state.set(&remembered::AGENT_CHOICES, &choices).unwrap();
        let text = std::fs::read_to_string(dir.join("state.json")).unwrap();
        assert!(!text.contains("effort"), "an unset part is left out: {text}");
        let again = State::load(dir.join("state.json")).get(&remembered::AGENT_CHOICES);
        assert_eq!(again, choices);
    }

    #[test]
    fn recent_folders_are_unique_latest_first_and_limited() {
        let dir = scratch("recent");
        let mut state = State::load(dir.join("state.json"));
        for n in 0..=RECENT_LIMIT {
            state.note_recent(Path::new(&format!("/p/{n}"))).unwrap();
        }
        state.note_recent(Path::new("/p/3")).unwrap();
        let recent = State::load(dir.join("state.json")).get(&remembered::RECENT_PROJECTS);
        assert_eq!(recent.len(), RECENT_LIMIT);
        assert_eq!(recent[0], Path::new("/p/3"));
        assert_eq!(recent.iter().filter(|p| *p == Path::new("/p/3")).count(), 1);
    }
}
