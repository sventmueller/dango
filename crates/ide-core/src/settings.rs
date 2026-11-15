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
pub(crate) enum Scope {
    /// Only in the user's settings.
    User,
    /// In the user's settings, and in a project's, which wins there.
    Project,
}

/// Whether a setting of the user follows them to their other Macs (E113).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sharing {
    /// The same on every Mac of the user.
    Shared,
    /// Kept on this Mac: paths, local addresses, consents and reminders.
    PerMac,
}

/// A setting as the registry of every setting knows it, whatever its type: the app gathers those of every crate, and
/// the sync takes along the ones it marks shared (E113).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declared {
    /// The name in the JSON file.
    pub name: &'static str,
    /// Where it may be set.
    pub(crate) scope: Scope,
    /// Whether it follows the user to their other Macs.
    pub sync: Sharing,
}

/// A named setting of type `T` with its default.
pub struct Key<T> {
    /// The name in the JSON file.
    pub(crate) name: &'static str,
    /// Where it may be set.
    pub(crate) scope: Scope,
    /// Whether it follows the user to their other Macs.
    pub(crate) sync: Sharing,
    default: fn() -> T,
    kind: PhantomData<fn() -> T>,
}

impl<T> Key<T> {
    /// A setting only the user sets.
    pub const fn user(name: &'static str, sync: Sharing, default: fn() -> T) -> Key<T> {
        Key {
            name,
            scope: Scope::User,
            sync,
            default,
            kind: PhantomData,
        }
    }

    /// A setting a project may set for itself.
    pub(crate) const fn project(name: &'static str, sync: Sharing, default: fn() -> T) -> Key<T> {
        Key {
            name,
            scope: Scope::Project,
            sync,
            default,
            kind: PhantomData,
        }
    }

    /// The setting as the registry of every setting knows it.
    pub const fn declared(&self) -> Declared {
        Declared {
            name: self.name,
            scope: self.scope,
            sync: self.sync,
        }
    }

    /// The value when nobody set one.
    pub fn default_value(&self) -> T {
        (self.default)()
    }
}

/// The settings of the app.
pub mod keys {
    use super::{Declared, FormatterChoice, Key, SearchEngine, Sharing};

    /// The settings this crate declares, for the app's registry of every setting.
    pub const SETTINGS: &[Declared] = &[
        REOPEN_PROJECTS.declared(),
        FONT_SIZE.declared(),
        FORMAT_ON_SAVE.declared(),
        SHOW_WHITESPACE.declared(),
        MINIMAP.declared(),
        FORMATTER.declared(),
        EMMET.declared(),
        VIM_MODE.declared(),
        VIM_CLIPBOARD.declared(),
        ESLINT_FIX_ON_SAVE.declared(),
        DIAGNOSTICS_AT_LINE_END.declared(),
        STICKY_SCROLL_LINES.declared(),
        INLAY_HINTS.declared(),
        INLAY_HINTS_OFF.declared(),
        BLAME_AT_LINE_END.declared(),
        PREVIEW_TABS.declared(),
        MARKDOWN_PREVIEW.declared(),
        SHOW_IGNORED_FILES.declared(),
        SEARCH_ENGINE.declared(),
        HIDDEN_PANELS.declared(),
        OPTION_AS_META.declared(),
    ];

    /// Opens the folders that were open when the app quit.
    pub const REOPEN_PROJECTS: Key<bool> = Key::user("reopenProjects", Sharing::Shared, || true);

    /// The size of text in editors and terminals, in points.
    pub const FONT_SIZE: Key<f64> = Key::project("fontSize", Sharing::Shared, || 12.0);

    /// Formats a file with its language server when ⌘S saves it.
    pub const FORMAT_ON_SAVE: Key<bool> = Key::project("formatOnSave", Sharing::Shared, || false);

    /// Shows spaces, tabs and whitespace at the end of lines in editors.
    pub const SHOW_WHITESPACE: Key<bool> = Key::user("showWhitespace", Sharing::Shared, || false);
    /// Whether editors show the minimap beside the text (E107).
    pub const MINIMAP: Key<bool> = Key::user("minimap", Sharing::Shared, || false);
    /// Who formats each language, by the language's name; a language not listed follows the project's Prettier
    /// configuration (E133).
    pub const FORMATTER: Key<std::collections::BTreeMap<String, FormatterChoice>> =
        Key::project("formatter", Sharing::Shared, std::collections::BTreeMap::new);
    /// Whether Emmet's abbreviations show in completion for HTML, CSS and JSX (E109).
    pub const EMMET: Key<bool> = Key::user("emmet", Sharing::Shared, || true);
    /// Whether editors use Vim's modes and keys (E106).
    pub const VIM_MODE: Key<bool> = Key::user("vimMode", Sharing::Shared, || false);
    /// Whether Vim's unnamed register is the clipboard too (E106).
    pub const VIM_CLIPBOARD: Key<bool> = Key::user("vimClipboard", Sharing::Shared, || false);
    /// Whether ⌘S runs ESLint's fixes before formatting (E132).
    pub const ESLINT_FIX_ON_SAVE: Key<bool> = Key::user("eslintFixOnSave", Sharing::Shared, || false);
    /// Whether a line's most serious diagnostic shows at its end (E130).
    pub const DIAGNOSTICS_AT_LINE_END: Key<bool> = Key::user("diagnosticsAtLineEnd", Sharing::Shared, || false);
    /// How many header lines Sticky Scroll holds at the top of an editor, 0 for none (E108).
    pub const STICKY_SCROLL_LINES: Key<usize> = Key::user("stickyScrollLines", Sharing::Shared, || 5);

    /// Shows the language server's inlay hints, such as inferred types and parameter names, in editors (E83).
    pub const INLAY_HINTS: Key<bool> = Key::user("inlayHints", Sharing::Shared, || true);

    /// The languages, by name, whose editors leave inlay hints out although they are on.
    pub const INLAY_HINTS_OFF: Key<Vec<String>> = Key::user("inlayHintsOffFor", Sharing::Shared, Vec::new);

    /// Shows who changed the cursor's line last at its end, after the cursor rests there (E86).
    pub const BLAME_AT_LINE_END: Key<bool> = Key::user("blameAtLineEnd", Sharing::Shared, || false);

    /// Whether a single click opens a file in the window's preview tab, which the next one replaces; off, every file
    /// gets a tab of its own (E62).
    pub const PREVIEW_TABS: Key<bool> = Key::user("previewTabs", Sharing::Shared, || false);

    /// Whether a Markdown file opens with its preview beside it, which closes with its editor.
    pub const MARKDOWN_PREVIEW: Key<bool> = Key::user("markdownPreviewBeside", Sharing::Shared, || true);

    /// Whether the file tree also lists what Git ignores and what the system hides, marked as such.
    pub const SHOW_IGNORED_FILES: Key<bool> = Key::user("showIgnoredFiles", Sharing::Shared, || false);

    /// Where the browser's address field sends words that are no address.
    pub const SEARCH_ENGINE: Key<SearchEngine> =
        Key::user("searchEngine", Sharing::Shared, || SearchEngine::DuckDuckGo);

    /// The panels whose symbol the activity bar leaves out, by the app's name for each panel.
    pub const HIDDEN_PANELS: Key<Vec<String>> = Key::user("hiddenSidebarPanels", Sharing::Shared, Vec::new);

    /// ⌥ in the terminal is Meta for every key, instead of typing the characters the keyboard layout puts on it.
    pub const OPTION_AS_META: Key<bool> = Key::user("optionAsMeta", Sharing::Shared, || false);
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

/// Who formats a language, as the setting `formatter` names it (E133).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatterChoice {
    /// The language server.
    Server,
    /// Prettier.
    Prettier,
    /// A word the app does not know, kept as written; the language then follows the project's configuration.
    #[serde(untagged)]
    Other(String),
}

/// Why a setting could not be set for the project.
#[cfg(test)]
#[derive(Debug)]
pub(crate) enum SetError {
    /// The setting, by name, is the user's only.
    UserOnly(&'static str),
    /// No project is open.
    NoProject,
    /// The value has no JSON form.
    Encode(serde_json::Error),
    /// The project's settings file could not be written.
    Write(io::Error),
}

#[cfg(test)]
impl std::fmt::Display for SetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetError::UserOnly(name) => write!(f, "{name} is set by the user only"),
            SetError::NoProject => f.write_str("no project is open"),
            SetError::Encode(error) => write!(f, "the value has no JSON form: {error}"),
            SetError::Write(error) => error.fmt(f),
        }
    }
}

#[cfg(test)]
impl std::error::Error for SetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SetError::Encode(error) => Some(error),
            SetError::Write(error) => Some(error),
            SetError::UserOnly(_) | SetError::NoProject => None,
        }
    }
}

/// What the state file holds.
pub mod remembered {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{AgentChoice, StateKey};

    /// The folders that were open when the app last quit.
    pub const OPEN_PROJECTS: StateKey<Vec<PathBuf>> = StateKey::new("openProjects", Vec::new);

    /// Recently opened folders, the latest first.
    pub const RECENT_PROJECTS: StateKey<Vec<PathBuf>> = StateKey::new("recentProjects", Vec::new);

    /// What the user picked last for each agent, by the agent's id, which its next sessions start with.
    pub const AGENT_CHOICES: StateKey<BTreeMap<String, AgentChoice>> = StateKey::new("agentChoices", BTreeMap::new);
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

type Listener = std::sync::Arc<dyn Fn(&Path, &[String]) + Send + Sync>;

static LISTENERS: std::sync::Mutex<Vec<Listener>> = std::sync::Mutex::new(Vec::new());

/// Calls `listener` with the settings file and the names of the settings after every write of a setting, on the
/// thread that wrote it.
pub fn on_change(listener: impl Fn(&Path, &[String]) + Send + Sync + 'static) {
    use crate::sync::LockIgnoringPoison;
    LISTENERS.lock_ignoring_poison().push(std::sync::Arc::new(listener));
}

fn changed(path: &Path, names: &[String]) {
    use crate::sync::LockIgnoringPoison;
    let listeners: Vec<Listener> = LISTENERS.lock_ignoring_poison().clone();
    for listener in listeners {
        listener(path, names);
    }
}

/// How many folders the recent list keeps.
pub(crate) const RECENT_LIMIT: usize = 12;

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
        self.set_value(key.name, value)
    }

    /// Sets the user's setting `name` to `value` as it is stored, and writes them.
    pub fn set_value(&mut self, name: &str, value: serde_json::Value) -> io::Result<()> {
        self.user.set(name, value, SETTINGS_MIGRATIONS)?;
        changed(self.user.path(), &[name.to_string()]);
        Ok(())
    }

    /// Takes the user's setting `name` out, so that it falls back to its default, and writes them.
    pub fn remove(&mut self, name: &str) -> io::Result<()> {
        self.user.remove(name, SETTINGS_MIGRATIONS)?;
        changed(self.user.path(), &[name.to_string()]);
        Ok(())
    }

    /// The user's settings as they are stored, by name, without the file's version.
    pub fn user_values(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut values = self.user.values().clone();
        values.remove("version");
        values
    }

    /// The version the settings file has after its migrations, which a synced copy carries (E114).
    pub fn schema_version() -> u64 {
        1 + SETTINGS_MIGRATIONS.len() as u64
    }

    /// Brings `values` of an older schema `version` to the current one with the settings' migrations.
    pub fn migrate_values(values: &mut serde_json::Map<String, serde_json::Value>, version: u64) {
        crate::store::migrate_from(values, version, SETTINGS_MIGRATIONS);
    }

    /// Sets `key` in the project's settings and writes them.
    #[cfg(test)]
    pub(crate) fn set_for_project<T: Serialize>(&mut self, key: &Key<T>, value: &T) -> Result<(), SetError> {
        if key.scope != Scope::Project {
            return Err(SetError::UserOnly(key.name));
        }
        let project = self.project.as_mut().ok_or(SetError::NoProject)?;
        let value = serde_json::to_value(value).map_err(SetError::Encode)?;
        project
            .set(key.name, value, SETTINGS_MIGRATIONS)
            .map_err(SetError::Write)
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

/// A named entry of the app's state file with its default; never synced to other Macs.
pub struct StateKey<T> {
    /// The name in the JSON file.
    pub(crate) name: &'static str,
    default: fn() -> T,
    kind: PhantomData<fn() -> T>,
}

impl<T> StateKey<T> {
    /// An entry of the state file.
    pub const fn new(name: &'static str, default: fn() -> T) -> StateKey<T> {
        StateKey {
            name,
            default,
            kind: PhantomData,
        }
    }
}

/// What the app remembers between runs that the user does not set: open and recent folders.
#[derive(Debug, Clone)]
pub struct State {
    store: Store,
}

impl State {
    /// The state from `path`.
    pub(crate) fn load(path: PathBuf) -> State {
        State {
            store: Store::load(path, STATE_MIGRATIONS),
        }
    }

    /// The state from the app's data folder.
    pub fn load_user() -> State {
        State::load(paths::data_dir().join("state.json"))
    }

    /// The value of `key`, or its default.
    pub fn get<T: DeserializeOwned>(&self, key: &StateKey<T>) -> T {
        self.store
            .get(key.name)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| (key.default)())
    }

    /// Sets `key` and writes the state.
    pub fn set<T: Serialize>(&mut self, key: &StateKey<T>, value: &T) -> io::Result<()> {
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
        assert!(matches!(
            settings.set_for_project(&keys::REOPEN_PROJECTS, &false),
            Err(SetError::UserOnly("reopenProjects"))
        ));
        assert!(matches!(
            Settings::load(dir.join("user.json")).set_for_project(&keys::FONT_SIZE, &13.0),
            Err(SetError::NoProject)
        ));
    }

    #[test]
    fn the_formatter_setting_reads_and_writes_as_stored() {
        let stored = serde_json::json!({"typescript": "prettier", "rust": "server", "css": "biome"});
        let read: std::collections::BTreeMap<String, FormatterChoice> = serde_json::from_value(stored.clone()).unwrap();
        assert_eq!(read["typescript"], FormatterChoice::Prettier);
        assert_eq!(read["rust"], FormatterChoice::Server);
        assert_eq!(read["css"], FormatterChoice::Other("biome".into()));
        assert_eq!(serde_json::to_value(&read).unwrap(), stored);
        assert!(serde_json::from_value::<FormatterChoice>(serde_json::json!(2)).is_err());
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
