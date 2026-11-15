//! What a project keeps of debugging between launches (E94, Review C18): its breakpoints, watch expressions and the
//! state of the debug panel, in the project's layout file beside its window tabs. No session is kept; a debug console
//! comes back as a tab of the layout (`layout::Tab::Debug`) holding its finished history.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::layout::{MIGRATIONS, file_of};
use crate::store::Store;

/// The field of the layout file that holds it.
const FIELD: &str = "debugging";

/// A place where a session stops, or logs, in a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakpoint {
    /// The file.
    pub path: PathBuf,
    /// The line, from 0.
    pub line: usize,
    /// Whether sessions get it; a disabled one stays in place, drawn outlined.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// An expression the session stops only when it is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// How many hits it lets pass, in the adapter's words, such as `5` or `>= 5`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    /// A message the session logs instead of stopping, which makes it a log point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
}

impl Breakpoint {
    /// An enabled breakpoint without conditions at `line` of `path`.
    pub fn new(path: impl Into<PathBuf>, line: usize) -> Breakpoint {
        Breakpoint {
            path: path.into(),
            line,
            enabled: true,
            condition: None,
            hit_condition: None,
            log_message: None,
        }
    }

    /// Whether it logs instead of stopping.
    pub fn is_log_point(&self) -> bool {
        self.log_message.as_deref().is_some_and(|m| !m.is_empty())
    }
}

fn yes() -> bool {
    true
}

/// An exception filter an adapter offered, such as "Rust panics", and whether the user wants sessions to stop there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionFilter {
    /// The adapter's id for it.
    pub id: String,
    /// The adapter's name for it.
    pub label: String,
    /// Whether sessions stop there.
    pub enabled: bool,
}

/// What a project keeps of debugging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Debugging {
    /// The breakpoints of every file of the project.
    #[serde(default)]
    pub breakpoints: Vec<Breakpoint>,
    /// Whether breakpoints are on at all, which ⌘Y switches without losing them.
    #[serde(default = "yes")]
    pub active: bool,
    /// The watch expressions, in the order the user added them.
    #[serde(default)]
    pub watches: Vec<String>,
    /// The exception filters the last adapter offered, with the user's choices.
    #[serde(default)]
    pub exception_filters: Vec<ExceptionFilter>,
    /// The name of the start configuration chosen last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<String>,
    /// The sections of the debug panel the user folded, by name.
    #[serde(default)]
    pub folded: Vec<String>,
}

impl Default for Debugging {
    fn default() -> Debugging {
        Debugging {
            breakpoints: Vec::new(),
            active: true,
            watches: Vec::new(),
            exception_filters: Vec::new(),
            configuration: None,
            folded: Vec::new(),
        }
    }
}

/// What kind of line a debug console shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleKind {
    /// An expression the user evaluated.
    Input,
    /// The answer to it.
    Result,
    /// What the program or the adapter wrote.
    Output,
    /// An error, of the program or of an evaluation.
    Error,
    /// What the app says about the session, such as that it ended.
    Note,
}

/// A line of a debug console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleLine {
    /// What kind of line it is.
    pub kind: ConsoleKind,
    /// Its text, which may hold several lines.
    pub text: String,
}

/// What the project at `root` keeps of debugging; the default when it kept nothing or its file is unreadable.
pub fn load(root: &Path) -> Debugging {
    load_from(file_of(root))
}

/// Keeps `debugging` for the project at `root`, beside its window tabs.
pub fn save(root: &Path, debugging: &Debugging) -> io::Result<()> {
    save_to(file_of(root), debugging)
}

fn load_from(file: PathBuf) -> Debugging {
    let store = Store::load(file, MIGRATIONS);
    store
        .get(FIELD)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

fn save_to(file: PathBuf, debugging: &Debugging) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let value = serde_json::to_value(debugging).map_err(io::Error::other)?;
    Store::load(file, MIGRATIONS).set(FIELD, value, MIGRATIONS)
}

/// Version 2 knew no debugging; version 3 has the field, and a debug console among the tab kinds, which version 2
/// would not read, so it must leave such a file alone.
pub(crate) fn add_debugging(values: &mut Map<String, Value>) {
    if !values.contains_key(FIELD)
        && let Ok(value) = serde_json::to_value(Debugging::default())
    {
        values.insert(FIELD.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Layout, Tab, WindowTab};

    fn sample() -> Debugging {
        let mut conditional = Breakpoint::new("/p/src/main.rs", 11);
        conditional.condition = Some("n > 3".into());
        conditional.enabled = false;
        let mut log = Breakpoint::new("/p/src/lib.rs", 2);
        log.log_message = Some("n = {n}".into());
        Debugging {
            breakpoints: vec![Breakpoint::new("/p/src/main.rs", 4), conditional, log],
            active: false,
            watches: vec!["n".into(), "items.len()".into()],
            exception_filters: vec![ExceptionFilter {
                id: "rust_panic".into(),
                label: "Rust: on panic".into(),
                enabled: true,
            }],
            configuration: Some("Debug shop".into()),
            folded: vec!["breakpoints".into()],
        }
    }

    #[test]
    fn debugging_comes_back_as_it_was_kept_beside_the_tabs() {
        let scratch_dir = crate::store::tests::scratch("debugging");
        let file = scratch_dir.join("layouts/shop.json");
        assert_eq!(load_from(file.clone()), Debugging::default());
        let layout = Layout {
            tabs: vec![WindowTab::single(Tab::Debug {
                title: "shop".into(),
                history: vec![ConsoleLine {
                    kind: ConsoleKind::Output,
                    text: "hello\n".into(),
                }],
            })],
            selected: Some(0),
        };
        crate::layout::tests::save_layout(file.clone(), &layout);
        save_to(file.clone(), &sample()).unwrap();
        assert_eq!(load_from(file.clone()), sample());
        assert_eq!(crate::layout::tests::load_layout(file.clone()), Some(layout));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains(r#""version": 4"#), "{text}");
        assert!(text.contains(r#""kind": "debug""#), "{text}");
        assert!(text.contains(r#""log_message": "n = {n}""#), "{text}");
    }

    #[test]
    fn a_layout_of_version_two_gets_the_field_and_keeps_its_tabs() {
        let scratch_dir = crate::store::tests::scratch("debugging-migration");
        let file = scratch_dir.join("layouts/shop.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            r#"{"version": 2, "tabs": {"tabs": [{"documents": [{"kind": "welcome"}], "share": 0.5}], "selected": 0}}"#,
        )
        .unwrap();
        assert_eq!(load_from(file.clone()), Debugging::default());
        let store = Store::load(file.clone(), MIGRATIONS);
        assert!(store.get(FIELD).is_some(), "the migration adds the field");
        assert_eq!(
            crate::layout::tests::load_layout(file),
            Some(Layout {
                tabs: vec![WindowTab::single(Tab::Welcome)],
                selected: Some(0),
            })
        );
    }

    #[test]
    fn a_newer_layout_is_never_overwritten_by_this_version() {
        let scratch_dir = crate::store::tests::scratch("debugging-newer");
        let file = scratch_dir.join("layouts/shop.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let newer = r#"{"version": 99, "debugging": {"breakpoints": []}}"#;
        std::fs::write(&file, newer).unwrap();
        assert!(save_to(file.clone(), &sample()).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), newer);
    }

    #[test]
    fn a_breakpoint_written_by_hand_needs_only_its_place() {
        let read: Breakpoint = serde_json::from_str(r#"{"path": "/p/a.rs", "line": 3}"#).unwrap();
        assert_eq!(read, Breakpoint::new("/p/a.rs", 3));
        assert!(!read.is_log_point());
        let written = serde_json::to_string(&read).unwrap();
        assert_eq!(written, r#"{"path":"/p/a.rs","line":3,"enabled":true}"#);
    }
}
